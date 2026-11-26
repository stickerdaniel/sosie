use crate::color::Rgba;
use crate::display::{DisplayList, Item, Radii, Rect, Shape};
use crate::fonts::{FamilyList, Fonts};
use crate::limits::{LimitError, Limits};
use crate::text::{
    layout, Direction, Inline, OverflowWrap, Paragraph, TextLayout, TextStyle, WhiteSpace,
};
use std::fmt;

pub const PRIMER: &str = "@primer/primitives@11.10.0";
pub const FONT_STACK: &str = r#""Mona Sans VF", -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans Backtick Fix", "Noto Sans", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji""#;

pub const NAME_WIDTH: f32 = 250.0;
const MAX_WIDTH: u32 = 4096;
const CHECKED_DPR: f32 = 2.0;
const MAX_CHANGE: f64 = 1e21;
const MINUS: char = '\u{2212}';
const ARROW: char = '\u{2192}';
const BORDER: f32 = 1.0;
const RADIUS: f32 = 6.0;
const PADDING_X: f32 = 16.0;
const HEADER_PADDING_Y: f32 = 12.0;
const ROW_PADDING_Y: f32 = 10.0;
const GAP: f32 = 16.0;
const TRACK: f32 = 10.0;
const MIN_TRACK: f32 = 100.0;
const VALUE_WIDTH: f32 = 150.0;
const MIN_CHANGE_WIDTH: f32 = 64.0;
const FIXED_COLUMNS: f32 = 2.0 * (BORDER + PADDING_X) + NAME_WIDTH + VALUE_WIDTH + 3.0 * GAP;
const MIN_WIDTH: u32 = (FIXED_COLUMNS + MIN_TRACK + MIN_CHANGE_WIDTH) as u32;
const MIN_FILL_PERCENT: f64 = 0.5;
const STRIPE: f32 = 3.0;
const STRIPE_PERIOD: f32 = 6.0;
const STRIPE_ALPHA_PERCENT: u32 = 45;
const OUTLINE_ALPHA_PERCENT: u32 = 55;
const FIT_TOLERANCE: f32 = 1.0 / 64.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Metric {
    pub name: String,
    pub unit: String,
    pub before: f64,
    pub after: f64,
    pub lower_is_better: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Chart {
    title: Option<String>,
    metrics: Vec<Metric>,
}

impl Chart {
    pub fn new(title: Option<String>, metrics: Vec<Metric>) -> Result<Chart, ChartError> {
        if metrics.is_empty() {
            return Err(ChartError::NoMetrics);
        }
        for (index, metric) in metrics.iter().enumerate() {
            validate(metric).map_err(|error| ChartError::Metric { index, error })?;
        }
        Ok(Chart {
            title: title.filter(|t| !t.is_empty()),
            metrics,
        })
    }

    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    pub fn metrics(&self) -> &[Metric] {
        &self.metrics
    }
}

fn validate(metric: &Metric) -> Result<(), MetricError> {
    if metric.name.is_empty() {
        Err(MetricError::EmptyName)
    } else if !metric.before.is_finite() || !metric.after.is_finite() {
        Err(MetricError::NonFiniteValue)
    } else {
        percent_change(metric.before, metric.after).map(drop)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricError {
    EmptyName,
    NonFiniteValue,
    BeforeZero,
    Change,
    NameTooWide,
    ValueTooWide,
    ChangeTooWide,
}

impl fmt::Display for MetricError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MetricError::EmptyName => write!(f, "the name is empty"),
            MetricError::NonFiniteValue => write!(f, "before and after must be finite numbers"),
            MetricError::BeforeZero => write!(f, "before is 0, so the change has no percentage"),
            MetricError::Change => write!(
                f,
                "the change is not a finite percentage below {MAX_CHANGE:e}"
            ),
            MetricError::NameTooWide => write!(
                f,
                "a word of the name is wider than the {NAME_WIDTH} px name column"
            ),
            MetricError::ValueTooWide => write!(
                f,
                "before, arrow and after are wider than the {VALUE_WIDTH} px value column"
            ),
            MetricError::ChangeTooWide => write!(
                f,
                "the change leaves the track narrower than {MIN_TRACK} px at this width"
            ),
        }
    }
}

impl std::error::Error for MetricError {}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ChartError {
    NoMetrics,
    Width { width: u32 },
    Size(LimitError),
    TitleTooWide,
    Metric { index: usize, error: MetricError },
}

impl fmt::Display for ChartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChartError::NoMetrics => write!(f, "the chart needs at least one metric"),
            ChartError::Width { width } => write!(
                f,
                "width {width} px is outside {MIN_WIDTH} to {MAX_WIDTH} px; {MIN_WIDTH} px leaves a {MIN_TRACK} px track beside the {NAME_WIDTH} px name, {VALUE_WIDTH} px value and {MIN_CHANGE_WIDTH} px change columns"
            ),
            ChartError::Size(e) => write!(f, "the chart at dpr {CHECKED_DPR} is too large: {e}"),
            ChartError::TitleTooWide => write!(f, "a line of the title is wider than the header"),
            ChartError::Metric { index, error } => write!(f, "metrics[{index}]: {error}"),
        }
    }
}

impl std::error::Error for ChartError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ChartError::Metric { error, .. } => Some(error),
            ChartError::Size(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    pub const fn palette(self) -> &'static Palette {
        match self {
            Theme::Light => &Palette::LIGHT,
            Theme::Dark => &Palette::DARK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub bg_default: Rgba,
    pub bg_muted: Rgba,
    pub bg_neutral_muted: Rgba,
    pub bg_success_emphasis: Rgba,
    pub bg_danger_emphasis: Rgba,
    pub border_default: Rgba,
    pub border_muted: Rgba,
    pub border_emphasis: Rgba,
    pub fg_default: Rgba,
    pub fg_muted: Rgba,
    pub fg_success: Rgba,
    pub fg_danger: Rgba,
}

impl Palette {
    pub const LIGHT: Palette = Palette {
        bg_default: Rgba::hex(0xffffff),
        bg_muted: Rgba::hex(0xf6f8fa),
        bg_neutral_muted: Rgba {
            a: 0x1f,
            ..Rgba::hex(0x818b98)
        },
        bg_success_emphasis: Rgba::hex(0x1f883d),
        bg_danger_emphasis: Rgba::hex(0xcf222e),
        border_default: Rgba::hex(0xd1d9e0),
        border_muted: Rgba {
            a: 0xb3,
            ..Rgba::hex(0xd1d9e0)
        },
        border_emphasis: Rgba::hex(0x818b98),
        fg_default: Rgba::hex(0x1f2328),
        fg_muted: Rgba::hex(0x59636e),
        fg_success: Rgba::hex(0x1a7f37),
        fg_danger: Rgba::hex(0xd1242f),
    };

    pub const DARK: Palette = Palette {
        bg_default: Rgba::hex(0x0d1117),
        bg_muted: Rgba::hex(0x151b23),
        bg_neutral_muted: Rgba {
            a: 0x33,
            ..Rgba::hex(0x656c76)
        },
        bg_success_emphasis: Rgba::hex(0x238636),
        bg_danger_emphasis: Rgba::hex(0xda3633),
        border_default: Rgba::hex(0x3d444d),
        border_muted: Rgba {
            a: 0xb3,
            ..Rgba::hex(0x3d444d)
        },
        border_emphasis: Rgba::hex(0x656c76),
        fg_default: Rgba::hex(0xf0f6fc),
        fg_muted: Rgba::hex(0x9198a1),
        fg_success: Rgba::hex(0x3fb950),
        fg_danger: Rgba::hex(0xf85149),
    };

    fn change_color(&self, tone: Tone) -> Rgba {
        match tone {
            Tone::Success => self.fg_success,
            Tone::Danger => self.fg_danger,
            Tone::Neutral => self.fg_muted,
        }
    }

    fn tone_fill(&self, tone: Tone) -> Rgba {
        match tone {
            Tone::Success => self.bg_success_emphasis,
            Tone::Danger => self.bg_danger_emphasis,
            Tone::Neutral => self.border_emphasis,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Success,
    Danger,
    Neutral,
}

pub fn tone(change: f64, lower_is_better: bool) -> Tone {
    if change == 0.0 {
        Tone::Neutral
    } else if (change < 0.0) == lower_is_better {
        Tone::Success
    } else {
        Tone::Danger
    }
}

pub fn format_value(value: f64, unit: &str) -> String {
    let rounded = round_half_up(&js_decimal(value.abs()), 2);
    let (int, frac) = rounded.split_once('.').unwrap_or((&rounded, ""));
    let mut out = String::new();
    if value < 0.0 {
        out.push(MINUS);
    }
    for (i, digit) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(digit);
    }
    if !frac.is_empty() {
        out.push('.');
        out.push_str(frac);
    }
    if !unit.is_empty() {
        out.push(' ');
        out.push_str(unit);
    }
    out
}

pub fn percent_change(before: f64, after: f64) -> Result<f64, MetricError> {
    if before == 0.0 {
        return Err(MetricError::BeforeZero);
    }
    let raw = (after - before) / before.abs() * 100.0;
    if !raw.is_finite() || raw.abs() >= MAX_CHANGE {
        return Err(MetricError::Change);
    }
    let places = if raw.abs() >= 10.0 { 0 } else { 1 };
    let magnitude: f64 = round_half_up(&format!("{:.60}", raw.abs()), places)
        .parse()
        .expect("a rounded decimal parses as f64");
    let change = if raw < 0.0 { -magnitude } else { magnitude };
    Ok(if change == 0.0 { 0.0 } else { change })
}

pub fn format_change(change: f64) -> String {
    if change == 0.0 {
        return "0 %".to_string();
    }
    let sign = if change < 0.0 { MINUS } else { '+' };
    format!("{sign}{} %", change.abs())
}

fn js_decimal(x: f64) -> String {
    let shortest = format!("{x:e}");
    let (mantissa, _) = shortest.split_once('e').expect("{:e} prints an exponent");
    let places = mantissa.len().saturating_sub(2);
    let nearest = format!("{x:.places$e}");
    let (mantissa, exponent) = nearest.split_once('e').expect("{:e} prints an exponent");
    let digits: String = mantissa.chars().filter(|&c| c != '.').collect();
    let exponent: i32 = exponent.parse().expect("{:e} prints an integer exponent");
    let point = exponent + 1;
    match usize::try_from(point) {
        Ok(point) if point >= digits.len() => {
            format!("{digits}{}", "0".repeat(point - digits.len()))
        }
        Ok(point) if point > 0 => format!("{}.{}", &digits[..point], &digits[point..]),
        _ => format!("0.{}{digits}", "0".repeat(point.unsigned_abs() as usize)),
    }
}

fn collapse_white_space(text: &str) -> String {
    text.split([' ', '\t', '\n', '\r'])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn round_half_up(decimal: &str, places: usize) -> String {
    let (int, frac) = decimal.split_once('.').unwrap_or((decimal, ""));
    let mut digits: Vec<u8> = int
        .bytes()
        .chain(frac.bytes().chain(std::iter::repeat(b'0')).take(places))
        .collect();
    if frac.as_bytes().get(places).is_some_and(|&d| d >= b'5') {
        match digits.iter().rposition(|&d| d != b'9') {
            Some(i) => {
                digits[i] += 1;
                digits[i + 1..].fill(b'0');
            }
            None => {
                digits.fill(b'0');
                digits.insert(0, b'1');
            }
        }
    }
    let (int, frac) = digits.split_at(digits.len() - places);
    let kept = frac.iter().rposition(|&d| d != b'0').map_or(0, |i| i + 1);
    let mut out: String = int.iter().map(|&d| char::from(d)).collect();
    if kept > 0 {
        out.push('.');
        out.extend(frac[..kept].iter().map(|&d| char::from(d)));
    }
    out
}

#[derive(Clone, Copy)]
struct Font {
    size: f32,
    line_height: f32,
    weight: u16,
}

const BODY_BOLD: Font = Font {
    size: 14.0,
    line_height: 21.0,
    weight: 600,
};
const SMALL: Font = Font {
    size: 12.0,
    line_height: 18.0,
    weight: 400,
};
const SMALL_BOLD: Font = Font {
    weight: 600,
    ..SMALL
};

struct Typesetter<'a> {
    fonts: &'a Fonts,
    family: FamilyList,
}

impl Typesetter<'_> {
    fn style(&self, font: Font, color: Rgba, white_space: WhiteSpace) -> TextStyle {
        TextStyle {
            family: self.family.clone(),
            size: font.size,
            line_height: font.line_height,
            weight: font.weight,
            italic: false,
            color,
            white_space,
            tab_size: 8,
            background: None,
        }
    }

    fn set_runs(
        &self,
        runs: &[(&str, Font, Rgba)],
        white_space: WhiteSpace,
        width: f32,
    ) -> TextLayout {
        let inlines: Vec<Inline> = runs
            .iter()
            .map(|&(text, font, color)| Inline::Text {
                text: text.to_string(),
                style: self.style(font, color, white_space),
            })
            .collect();
        let (_, font, color) = runs[0];
        let para = Paragraph {
            width,
            strut: self.style(font, color, white_space),
            overflow_wrap: OverflowWrap::Normal,
            direction: Direction::Ltr,
            dpr: CHECKED_DPR,
        };
        layout(self.fonts, &para, &inlines).expect("chart font sizes lie within the layout limits")
    }

    fn set(
        &self,
        text: &str,
        font: Font,
        color: Rgba,
        white_space: WhiteSpace,
        width: f32,
    ) -> TextLayout {
        self.set_runs(&[(text, font, color)], white_space, width)
    }
}

fn advance(text: &TextLayout) -> f32 {
    text.lines.iter().map(|l| l.advance).fold(0.0, f32::max)
}

fn overflows(text: &TextLayout, room: f32) -> bool {
    text.lines.iter().any(|l| l.advance > room + FIT_TOLERANCE)
}

fn with_alpha(color: Rgba, percent: u32) -> Rgba {
    Rgba {
        a: ((color.a as u32 * percent + 50) / 100) as u8,
        ..color
    }
}

#[derive(Clone, Copy)]
enum Ink {
    Solid,
    Hatched,
}

#[derive(Clone, Copy)]
struct Segment {
    from: f64,
    to: f64,
    color: Rgba,
    ink: Ink,
}

fn segments(palette: &Palette, metric: &Metric, tone: Tone, scale: f64) -> Vec<Segment> {
    let percent = |v: f64| v.abs() / scale * 100.0;
    let (before, after) = (percent(metric.before), percent(metric.after));
    let fill = palette.tone_fill(tone);
    let solid = |from, to, color| Segment {
        from,
        to,
        color,
        ink: Ink::Solid,
    };
    if after < before {
        vec![
            solid(0.0, after.max(MIN_FILL_PERCENT), fill),
            Segment {
                from: after,
                to: before,
                color: fill,
                ink: Ink::Hatched,
            },
        ]
    } else if after > before {
        vec![
            solid(0.0, before, palette.border_emphasis),
            solid(before, after, fill),
        ]
    } else {
        vec![solid(0.0, before.max(MIN_FILL_PERCENT), fill)]
    }
}

struct Row {
    name: TextLayout,
    values: TextLayout,
    change: TextLayout,
    segments: Vec<Segment>,
}

impl Row {
    fn new(
        set: &Typesetter,
        palette: &Palette,
        metric: &Metric,
        scale: f64,
    ) -> Result<Row, MetricError> {
        let change = percent_change(metric.before, metric.after)?;
        let tone = tone(change, metric.lower_is_better);
        let name = set.set(
            &metric.name,
            BODY_BOLD,
            palette.fg_default,
            WhiteSpace::Normal,
            NAME_WIDTH,
        );
        if overflows(&name, NAME_WIDTH) {
            return Err(MetricError::NameTooWide);
        }
        let before = format!("{} {ARROW} ", format_value(metric.before, ""));
        let after = collapse_white_space(&format_value(metric.after, &metric.unit));
        let values = set.set_runs(
            &[
                (&before, SMALL, palette.fg_muted),
                (&after, SMALL_BOLD, palette.fg_default),
            ],
            WhiteSpace::Pre,
            VALUE_WIDTH,
        );
        if advance(&values) > VALUE_WIDTH {
            return Err(MetricError::ValueTooWide);
        }
        let change = set.set(
            &format_change(change),
            BODY_BOLD,
            palette.change_color(tone),
            WhiteSpace::Pre,
            f32::MAX,
        );
        Ok(Row {
            name,
            values,
            change,
            segments: segments(palette, metric, tone, scale),
        })
    }

    fn content_height(&self) -> f32 {
        [&self.name, &self.values, &self.change]
            .iter()
            .map(|t| t.height)
            .fold(TRACK, f32::max)
    }

    fn height(&self) -> f32 {
        2.0 * ROW_PADDING_Y + self.content_height()
    }

    fn paint(&self, list: &mut DisplayList, palette: &Palette, columns: &Columns, top: f32) {
        let content = self.content_height();
        let centered = |h: f32| top + ROW_PADDING_Y + (content - h) / 2.0;
        self.name
            .paint(list, columns.name, centered(self.name.height));
        let track = Shape {
            rect: Rect {
                x: columns.track,
                y: centered(TRACK),
                w: columns.track_width,
                h: TRACK,
            },
            radii: Radii::all(RADIUS),
        };
        list.push(Item::Fill {
            shape: track,
            color: palette.bg_neutral_muted,
        });
        let width = f64::from(columns.track_width);
        for segment in &self.segments {
            let x = (f64::from(columns.track) + width * segment.from / 100.0) as f32;
            let w = (width * (segment.to - segment.from) / 100.0) as f32;
            let rect = Rect { x, w, ..track.rect };
            match segment.ink {
                Ink::Solid => list.push(Item::ClippedFill {
                    shape: Shape {
                        rect,
                        radii: Radii::default(),
                    },
                    clip: track,
                    color: segment.color,
                }),
                Ink::Hatched => {
                    let shape = Shape {
                        rect,
                        radii: Radii {
                            tl: 0.0,
                            tr: RADIUS,
                            br: RADIUS,
                            bl: 0.0,
                        },
                    };
                    list.push(Item::Hatch {
                        shape,
                        clip: track,
                        color: with_alpha(segment.color, STRIPE_ALPHA_PERCENT),
                        stripe: STRIPE,
                        period: STRIPE_PERIOD,
                    });
                    list.push(Item::ClippedBorder {
                        shape,
                        clip: track,
                        width: BORDER,
                        color: with_alpha(segment.color, OUTLINE_ALPHA_PERCENT),
                    });
                }
            }
        }
        self.values.paint(
            list,
            columns.values_end - advance(&self.values),
            centered(self.values.height),
        );
        self.change.paint(
            list,
            columns.right - advance(&self.change),
            centered(self.change.height),
        );
    }
}

struct Columns {
    name: f32,
    track: f32,
    track_width: f32,
    values_end: f32,
    right: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrappedName {
    pub index: usize,
    pub lines: usize,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Rendered {
    pub list: DisplayList,
    pub wrapped: Vec<WrappedName>,
}

pub fn render(
    fonts: &Fonts,
    chart: &Chart,
    theme: Theme,
    width: u32,
) -> Result<Rendered, ChartError> {
    if !(MIN_WIDTH..=MAX_WIDTH).contains(&width) {
        return Err(ChartError::Width { width });
    }
    let width = width as f32;
    let palette = theme.palette();
    let set = Typesetter {
        fonts,
        family: FamilyList::parse(FONT_STACK),
    };
    let left = BORDER + PADDING_X;
    let right = width - left;
    let inner = right - left;
    let title = match chart.title() {
        Some(title) => {
            let text = set.set(
                title,
                BODY_BOLD,
                palette.fg_default,
                WhiteSpace::Normal,
                inner,
            );
            if overflows(&text, inner) {
                return Err(ChartError::TitleTooWide);
            }
            Some(text)
        }
        None => None,
    };
    let scale = chart
        .metrics()
        .iter()
        .map(|m| m.before.abs().max(m.after.abs()))
        .fold(0.0, f64::max);
    let rows = chart
        .metrics()
        .iter()
        .enumerate()
        .map(|(index, metric)| {
            Row::new(&set, palette, metric, scale)
                .map_err(|error| ChartError::Metric { index, error })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let widest_change = width - FIXED_COLUMNS - MIN_TRACK;
    if let Some(index) = rows.iter().position(|r| advance(&r.change) > widest_change) {
        return Err(ChartError::Metric {
            index,
            error: MetricError::ChangeTooWide,
        });
    }
    let change_width = rows
        .iter()
        .map(|r| advance(&r.change))
        .fold(MIN_CHANGE_WIDTH, f32::max);
    let columns = Columns {
        name: left,
        track: left + NAME_WIDTH + GAP,
        track_width: width - FIXED_COLUMNS - change_width,
        values_end: right - change_width - GAP,
        right,
    };
    let header = title.map(|text| {
        let height = 2.0 * HEADER_PADDING_Y + text.height;
        (text, height)
    });
    let height = BORDER
        + header.as_ref().map_or(0.0, |(_, h)| h + BORDER)
        + rows.iter().map(Row::height).sum::<f32>()
        + (rows.len() - 1) as f32 * BORDER
        + BORDER;
    Limits::default()
        .output(width, height, CHECKED_DPR)
        .map_err(ChartError::Size)?;
    let mut list = DisplayList::new(width, height, palette.bg_default);
    list.push(Item::Border {
        shape: Shape {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                w: width,
                h: height,
            },
            radii: Radii::all(RADIUS),
        },
        width: BORDER,
        color: palette.border_default,
    });
    let band = |y: f32, h: f32| Rect {
        x: BORDER,
        y,
        w: width - 2.0 * BORDER,
        h,
    };
    let mut y = BORDER;
    if let Some((text, h)) = &header {
        let inner_radius = RADIUS - BORDER;
        list.push(Item::Fill {
            shape: Shape {
                rect: band(y, h + BORDER),
                radii: Radii {
                    tl: inner_radius,
                    tr: inner_radius,
                    br: 0.0,
                    bl: 0.0,
                },
            },
            color: palette.bg_muted,
        });
        text.paint(&mut list, left, y + HEADER_PADDING_Y);
        list.push(Item::Fill {
            shape: Shape {
                rect: band(y + h, BORDER),
                radii: Radii::default(),
            },
            color: palette.border_default,
        });
        y += h + BORDER;
    }
    for (i, row) in rows.iter().enumerate() {
        if i > 0 {
            list.push(Item::Fill {
                shape: Shape {
                    rect: band(y, BORDER),
                    radii: Radii::default(),
                },
                color: palette.border_muted,
            });
            y += BORDER;
        }
        row.paint(&mut list, palette, &columns, y);
        y += row.height();
    }
    let wrapped = rows
        .iter()
        .enumerate()
        .filter(|(_, row)| row.name.lines.len() > 1)
        .map(|(index, row)| WrappedName {
            index,
            lines: row.name.lines.len(),
        })
        .collect();
    Ok(Rendered { list, wrapped })
}
