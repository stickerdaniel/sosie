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

const MIN_WIDTH: u32 = 310;
const MAX_WIDTH: u32 = 4096;
const CHECKED_DPR: f32 = 2.0;
const MAX_CHANGE: f64 = 1e21;
const MINUS: char = '\u{2212}';
const BORDER: f32 = 1.0;
const RADIUS: f32 = 6.0;
const PADDING_X: f32 = 16.0;
const HEADER_PADDING_Y: f32 = 12.0;
const ROW_PADDING_TOP: f32 = 12.0;
const ROW_PADDING_BOTTOM: f32 = 14.0;
const HEAD_MARGIN: f32 = 6.0;
const GAP: f32 = 16.0;
const BAR: f32 = 22.0;
const TRACK: f32 = 10.0;
const VALUE_WIDTH: f32 = 144.0;
const MIN_FILL_PERCENT: f64 = 0.5;
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
            MetricError::NameTooWide => {
                write!(
                    f,
                    "a line of the name is wider than the space beside the change"
                )
            }
            MetricError::ValueTooWide => write!(
                f,
                "the key and the value are wider than the {VALUE_WIDTH} px value column"
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
                "width {width} px is outside {MIN_WIDTH} to {MAX_WIDTH} px"
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

    fn after_fill(&self, tone: Tone) -> Rgba {
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
    fn set(
        &self,
        text: &str,
        font: Font,
        color: Rgba,
        white_space: WhiteSpace,
        width: f32,
    ) -> TextLayout {
        let style = TextStyle {
            family: self.family.clone(),
            size: font.size,
            line_height: font.line_height,
            weight: font.weight,
            italic: false,
            color,
            white_space,
            tab_size: 8,
            background: None,
        };
        let para = Paragraph {
            width,
            strut: style.clone(),
            overflow_wrap: OverflowWrap::Normal,
            direction: Direction::Ltr,
        };
        let inlines = [Inline::Text {
            text: text.to_string(),
            style,
        }];
        layout(self.fonts, &para, &inlines).expect("chart font sizes lie within the layout limits")
    }
}

fn track_width(inner: f32) -> f32 {
    inner - GAP - VALUE_WIDTH
}

fn advance(text: &TextLayout) -> f32 {
    text.lines.iter().map(|l| l.advance).fold(0.0, f32::max)
}

fn overflows(text: &TextLayout, room: f32) -> bool {
    text.lines.iter().any(|l| l.advance > room + FIT_TOLERANCE)
}

struct Bar {
    width: f32,
    color: Rgba,
    key: TextLayout,
    value: TextLayout,
}

struct Row {
    name: TextLayout,
    change: TextLayout,
    bars: [Bar; 2],
}

impl Row {
    fn new(
        set: &Typesetter,
        palette: &Palette,
        metric: &Metric,
        inner: f32,
    ) -> Result<Row, MetricError> {
        let change = percent_change(metric.before, metric.after)?;
        let tone = tone(change, metric.lower_is_better);
        let change = set.set(
            &format_change(change),
            BODY_BOLD,
            palette.change_color(tone),
            WhiteSpace::Pre,
            inner,
        );
        let room = inner - GAP - advance(&change);
        if room <= 0.0 {
            return Err(MetricError::NameTooWide);
        }
        let name = set.set(
            &metric.name,
            BODY_BOLD,
            palette.fg_default,
            WhiteSpace::Normal,
            room,
        );
        if overflows(&name, room) {
            return Err(MetricError::NameTooWide);
        }
        let track = track_width(inner);
        let max = metric.before.abs().max(metric.after.abs());
        let bar = |value: f64, color: Rgba, label: &str, font: Font, text_color: Rgba| {
            let key = set.set(label, SMALL, palette.fg_muted, WhiteSpace::Pre, VALUE_WIDTH);
            let text = collapse_white_space(&format_value(value, &metric.unit));
            let value_text = set.set(&text, font, text_color, WhiteSpace::Pre, VALUE_WIDTH);
            if advance(&key) + advance(&value_text) > VALUE_WIDTH {
                return Err(MetricError::ValueTooWide);
            }
            let percent = (value.abs() / max * 100.0).max(MIN_FILL_PERCENT);
            Ok(Bar {
                width: (track as f64 * percent / 100.0) as f32,
                color,
                key,
                value: value_text,
            })
        };
        let bars = [
            bar(
                metric.before,
                palette.border_emphasis,
                "Before",
                SMALL,
                palette.fg_muted,
            )?,
            bar(
                metric.after,
                palette.after_fill(tone),
                "After",
                SMALL_BOLD,
                palette.fg_default,
            )?,
        ];
        Ok(Row { name, change, bars })
    }

    fn head_height(&self) -> f32 {
        self.name.height.max(self.change.height)
    }

    fn height(&self) -> f32 {
        ROW_PADDING_TOP + self.head_height() + HEAD_MARGIN + 2.0 * BAR + ROW_PADDING_BOTTOM
    }

    fn paint(&self, list: &mut DisplayList, palette: &Palette, left: f32, right: f32, top: f32) {
        let head = top + ROW_PADDING_TOP;
        self.name.paint(list, left, head);
        self.change.paint(list, right - advance(&self.change), head);
        let mut bar_top = head + self.head_height() + HEAD_MARGIN;
        for bar in &self.bars {
            let track = Shape {
                rect: Rect {
                    x: left,
                    y: bar_top + (BAR - TRACK) / 2.0,
                    w: track_width(right - left),
                    h: TRACK,
                },
                radii: Radii::all(RADIUS),
            };
            list.push(Item::Fill {
                shape: track,
                color: palette.bg_neutral_muted,
            });
            list.push(Item::ClippedFill {
                shape: Shape {
                    rect: Rect {
                        w: bar.width,
                        ..track.rect
                    },
                    ..track
                },
                clip: track,
                color: bar.color,
            });
            let text_top = bar_top + (BAR - SMALL.line_height) / 2.0;
            bar.key.paint(list, right - VALUE_WIDTH, text_top);
            bar.value.paint(list, right - advance(&bar.value), text_top);
            bar_top += BAR;
        }
    }
}

pub fn render(
    fonts: &Fonts,
    chart: &Chart,
    theme: Theme,
    width: u32,
) -> Result<DisplayList, ChartError> {
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
    let rows = chart
        .metrics()
        .iter()
        .enumerate()
        .map(|(index, metric)| {
            Row::new(&set, palette, metric, inner)
                .map_err(|error| ChartError::Metric { index, error })
        })
        .collect::<Result<Vec<_>, _>>()?;
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
        row.paint(&mut list, palette, left, right, y);
        y += row.height();
    }
    Ok(list)
}
