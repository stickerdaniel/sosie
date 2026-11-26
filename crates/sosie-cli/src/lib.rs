#![forbid(unsafe_code)]

mod input;
pub mod output;

use output::{OutputError, Targets};
use sosie_core::chart::NAME_WIDTH;
use sosie_core::{
    chart, encode, rasterize, Chart, ChartError, EncodeError, Fonts, Format, Pixmap, Theme,
    WrappedName,
};
use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const USAGE: &str = r#"Usage: sosie chart <metrics.json> <out-prefix> [--width 978] [--format webp|png]

Renders metric bars in light and dark at dpr 2, writes
<out-prefix>-light.<ext> and <out-prefix>-dark.<ext> and prints both paths.

metrics.json: { "title"?: string, "metrics": [{ "name": string, "unit": string, "before": number, "after": number, "lowerIsBetter": boolean }] }

  --width   CSS px from 646 to 4096, default 978
  --format  webp (lossless, default) or png

A name wider than the 250 px name column wraps and makes its row taller.
Each such name prints a warning to stderr; the files are still written.

These receivers need --format png: Slack image blocks, Outlook for Windows,
LinkedIn and Facebook og:image, Gmail (converts WebP to JPG), desk albums
with labels.

Exit status: 2 for usage, JSON and chart errors, 1 for I/O and encode errors."#;

const DEFAULT_WIDTH: u32 = 978;
const DPR: f32 = 2.0;

#[derive(Debug)]
pub enum Error {
    Usage(String),
    Input(String),
    Read { path: PathBuf, source: io::Error },
    Encode { path: PathBuf, source: EncodeError },
    Output(OutputError),
    Stdout(io::Error),
}

impl Error {
    pub fn exit_code(&self) -> u8 {
        match self {
            Error::Usage(_) | Error::Input(_) => 2,
            Error::Read { .. } | Error::Encode { .. } | Error::Output(_) | Error::Stdout(_) => 1,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Usage(why) => write!(f, "sosie: {why}\n\n{USAGE}"),
            Error::Input(why) => f.write_str(why),
            Error::Read { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Encode {
                path,
                source: source @ EncodeError::TooLarge { .. },
            } => write!(f, "{}: {source}; PNG has no such limit", path.display()),
            Error::Encode { path, source } => write!(f, "{}: {source}", path.display()),
            Error::Output(e) => e.fmt(f),
            Error::Stdout(e) => write!(f, "stdout: {e}"),
        }
    }
}

impl std::error::Error for Error {}

struct ChartArgs {
    input: PathBuf,
    prefix: OsString,
    width: u32,
    format: Format,
}

enum Command {
    Help,
    Chart(ChartArgs),
}

fn usage(why: impl Into<String>) -> Error {
    Error::Usage(why.into())
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, Error> {
    let mut args = args.into_iter();
    match args.next() {
        Some(cmd) if cmd == "chart" => {}
        Some(flag) if flag == "--help" || flag == "-h" => return Ok(Command::Help),
        Some(cmd) => return Err(usage(format!("unknown command {}", cmd.display()))),
        None => return Err(usage("missing command")),
    }
    let mut positional = Vec::new();
    let mut width = DEFAULT_WIDTH;
    let mut format = Format::Webp;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("--help" | "-h") => return Ok(Command::Help),
            Some("--width") => {
                width = args
                    .next()
                    .and_then(|v| v.to_str()?.parse().ok())
                    .ok_or_else(|| usage("--width needs a whole number of CSS px"))?;
            }
            Some("--format") => {
                format = match args.next().as_ref().and_then(|v| v.to_str()) {
                    Some("webp") => Format::Webp,
                    Some("png") => Format::Png,
                    _ => return Err(usage("--format needs webp or png")),
                };
            }
            Some(flag) if flag.starts_with("--") => {
                return Err(usage(format!("unknown option {flag}")))
            }
            _ => positional.push(arg),
        }
    }
    let [input, prefix] = <[OsString; 2]>::try_from(positional)
        .map_err(|_| usage("chart needs <metrics.json> and <out-prefix>"))?;
    Ok(Command::Chart(ChartArgs {
        input: input.into(),
        prefix,
        width,
        format,
    }))
}

pub fn run(args: impl IntoIterator<Item = OsString>, stdout: &mut impl Write) -> Result<(), Error> {
    match parse(args)? {
        Command::Help => writeln!(stdout, "{USAGE}").map_err(Error::Stdout),
        Command::Chart(args) => chart_command(&args, stdout),
    }
}

fn output_paths(prefix: &OsString, format: Format) -> [PathBuf; 2] {
    ["light", "dark"].map(|theme| {
        let mut path = prefix.clone();
        path.push(format!("-{theme}.{}", format.extension()));
        PathBuf::from(path)
    })
}

fn render(
    fonts: &Fonts,
    chart: &Chart,
    theme: Theme,
    width: u32,
) -> Result<(Pixmap, Vec<WrappedName>), Error> {
    let rendered = chart::render(fonts, chart, theme, width).map_err(|e| match e {
        ChartError::Width { .. } => Error::Input(format!("--width: {e}")),
        e => Error::Input(format!("metrics.json: {e}")),
    })?;
    let pixmap = rasterize(fonts, &rendered.list, DPR, &Default::default())
        .map_err(|e| Error::Input(format!("metrics.json: {e}")))?;
    Ok((pixmap, rendered.wrapped))
}

fn chart_command(args: &ChartArgs, stdout: &mut impl Write) -> Result<(), Error> {
    let json = std::fs::read(&args.input).map_err(|source| Error::Read {
        path: args.input.clone(),
        source,
    })?;
    let input = input::parse(&json).map_err(Error::Input)?;
    let chart = Chart::new(input.title, input.metrics)
        .map_err(|e| Error::Input(format!("metrics.json: {e}")))?;
    let fonts = Fonts::new();
    let [light, dark] = [Theme::Light, Theme::Dark].map(|t| render(&fonts, &chart, t, args.width));
    let ((light, wrapped), (dark, _)) = (light?, dark?);
    let pixmaps = [light, dark];
    let targets = Targets::check(output_paths(&args.prefix, args.format)).map_err(Error::Output)?;
    let paths = targets.paths();
    let [light, dark] = std::array::from_fn(|i| {
        encode(&pixmaps[i], args.format).map_err(|source| Error::Encode {
            path: paths[i].clone(),
            source,
        })
    });
    let bytes = [light?, dark?];
    let stale = targets
        .write(
            bytes.each_ref().map(Vec::as_slice),
            |from: &Path, to: &Path| std::fs::rename(from, to),
        )
        .map_err(Error::Output)?;
    for backup in stale {
        eprintln!("sosie: warning: {backup}");
    }
    for WrappedName { index, lines } in wrapped {
        let name = &chart.metrics()[index].name;
        eprintln!("sosie: warning: metrics[{index}] name wraps onto {lines} lines in the {NAME_WIDTH} px name column: {name:?}; a shorter name keeps the row on one line");
    }
    for path in paths {
        writeln!(stdout, "{}", path.display()).map_err(Error::Stdout)?;
    }
    Ok(())
}
