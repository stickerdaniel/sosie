#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match sosie_cli::run(std::env::args_os().skip(1), &mut std::io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(e.exit_code())
        }
    }
}
