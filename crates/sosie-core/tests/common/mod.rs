#![allow(dead_code)]

use serde::Deserialize;
use sosie_core::{
    layout, Direction, DisplayList, FamilyList, Fonts, Inline, InlineBackground, Item,
    OverflowWrap, Paragraph, Radii, Rect, Rgba, Shape, TextLayout, TextStyle, WhiteSpace,
};
use std::path::{Path, PathBuf};

pub const SANS: &str = r#""Mona Sans VF", -apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji""#;
pub const MONO: &str =
    r#"ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace"#;
pub const LAZY_FONTS: [&str; 4] = [
    "NotoColorEmoji.ttf",
    "NotoSansCJK-Regular.ttc",
    "NotoSansArabic-Regular.ttf",
    "NotoSansHebrew-Regular.ttf",
];

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

pub fn read(rel: &str) -> Vec<u8> {
    let p = root().join(rel);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

pub fn json<T: for<'de> Deserialize<'de>>(rel: &str) -> T {
    serde_json::from_slice(&read(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

pub fn all_fonts() -> Fonts {
    let mut fonts = Fonts::new();
    for f in LAZY_FONTS {
        fonts
            .register(read(&format!("assets/fonts/{f}")))
            .unwrap_or_else(|e| panic!("{f}: {e}"));
    }
    fonts
}

#[derive(Deserialize, Clone)]
pub struct SpanIn {
    pub t: String,
    pub k: String,
    pub w: u16,
    pub i: bool,
}

#[derive(Deserialize, Clone)]
pub struct LayoutIn {
    pub id: String,
    pub stratum: String,
    pub width: f32,
    pub block: String,
    pub spans: Vec<SpanIn>,
}

#[derive(Deserialize)]
pub struct ChromeLine {
    pub text: String,
    pub w: f32,
}

#[derive(Deserialize)]
pub struct ChromeLayout {
    pub id: String,
    pub height: f32,
    pub lines: Vec<ChromeLine>,
}

pub struct Theme {
    pub bg: Rgba,
    pub fg: Rgba,
    pub code: Rgba,
    pub pre: Rgba,
}

pub fn hex(s: &str) -> Rgba {
    Rgba::parse_hex(s).unwrap_or_else(|| panic!("bad color {s}"))
}

pub fn theme(name: &str) -> Theme {
    let v = if name == "light" {
        ["#ffffff", "#1f2328", "#818b981f", "#f6f8fa"]
    } else {
        ["#0d1117", "#f0f6fc", "#656c7633", "#151b23"]
    };
    Theme {
        bg: hex(v[0]),
        fg: hex(v[1]),
        code: hex(v[2]),
        pre: hex(v[3]),
    }
}

fn floor64(v: f32) -> f32 {
    (v * 64.0).floor() / 64.0
}

fn style(
    family: &str,
    size: f32,
    line_height: f32,
    white_space: WhiteSpace,
    tab_size: u32,
    color: Rgba,
) -> TextStyle {
    TextStyle {
        family: FamilyList::parse(family),
        size,
        line_height,
        weight: 400,
        italic: false,
        color,
        white_space,
        tab_size,
        background: None,
    }
}

pub fn block_style(block: &str, color: Rgba) -> (TextStyle, OverflowWrap) {
    match block {
        "pre" => (
            style(MONO, 11.9, floor64(11.9 * 1.45), WhiteSpace::Pre, 4, color),
            OverflowWrap::Normal,
        ),
        "cell" => (
            style(MONO, 12.0, 18.0, WhiteSpace::PreWrap, 8, color),
            OverflowWrap::Anywhere,
        ),
        _ => (
            style(SANS, 14.0, 21.0, WhiteSpace::Normal, 8, color),
            OverflowWrap::Anywhere,
        ),
    }
}

pub fn inlines(l: &LayoutIn, fg: Rgba, code: Rgba) -> (Paragraph, Vec<Inline>) {
    let (strut, overflow_wrap) = block_style(&l.block, fg);
    let spans = l
        .spans
        .iter()
        .map(|s| {
            let style = match (l.block.as_str(), s.k.as_str()) {
                ("p", "code") => TextStyle {
                    background: Some(InlineBackground {
                        padding_x: 0.4 * 11.9,
                        padding_y: 0.2 * 11.9,
                        radius: 6.0,
                        color: code,
                    }),
                    ..style(MONO, 11.9, 21.0, WhiteSpace::BreakSpaces, 4, fg)
                },
                ("p", _) => TextStyle {
                    weight: s.w,
                    italic: s.i,
                    ..strut.clone()
                },
                _ => strut.clone(),
            };
            Inline::Text {
                text: s.t.clone(),
                style,
            }
        })
        .collect();
    let rtl = l
        .spans
        .iter()
        .any(|s| s.t.chars().any(|c| ('\u{590}'..='\u{8ff}').contains(&c)));
    let para = Paragraph {
        width: l.width,
        strut,
        overflow_wrap,
        direction: if rtl { Direction::Auto } else { Direction::Ltr },
    };
    (para, spans)
}

pub fn lay_out(fonts: &Fonts, l: &LayoutIn) -> TextLayout {
    let (para, spans) = inlines(l, hex("#000000"), hex("#eeeeee"));
    layout(fonts, &para, &spans).unwrap_or_else(|e| panic!("{e}"))
}

#[derive(Deserialize, Clone)]
pub struct BoxIn {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub r: f32,
    pub bw: f32,
    pub fill: String,
    pub border: String,
}

#[derive(Deserialize, Clone)]
pub struct PaintCase {
    pub id: String,
    pub stratum: String,
    pub theme: String,
    pub kind: String,
    #[serde(default)]
    pub block: String,
    pub width: f32,
    #[serde(default)]
    pub spans: Vec<SpanIn>,
    #[serde(default)]
    pub boxes: Vec<BoxIn>,
}

#[derive(Deserialize)]
pub struct CaseBox {
    pub id: String,
    pub w: f32,
    pub h: f32,
}

pub const CASE_PADDING: f32 = 8.0;
pub const PRE_PADDING: f32 = 16.0;

pub fn case_height(fonts: &Fonts, c: &PaintCase) -> f32 {
    match c.kind.as_str() {
        "shapes" => 160.0 + 2.0 * CASE_PADDING,
        _ => {
            let t = theme(&c.theme);
            let (para, spans) = inlines(&layout_in(c), t.fg, t.code);
            let text = layout(fonts, &para, &spans)
                .unwrap_or_else(|e| panic!("{e}"))
                .height;
            let pad = if c.block == "pre" { PRE_PADDING } else { 0.0 };
            (text + 2.0 * pad + 2.0 * CASE_PADDING).ceil()
        }
    }
}

pub fn layout_in(c: &PaintCase) -> LayoutIn {
    LayoutIn {
        id: c.id.clone(),
        stratum: c.stratum.clone(),
        width: c.width,
        block: c.block.clone(),
        spans: c.spans.clone(),
    }
}

pub fn paint_case(fonts: &Fonts, c: &PaintCase, width: f32, height: f32) -> DisplayList {
    let t = theme(&c.theme);
    let mut list = DisplayList::new(width, height, t.bg);
    let o = CASE_PADDING;
    if c.kind == "shapes" {
        for b in &c.boxes {
            let shape = Shape {
                rect: Rect {
                    x: o + b.x,
                    y: o + b.y,
                    w: b.w,
                    h: b.h,
                },
                radii: Radii::all(b.r),
            };
            list.push(Item::Fill {
                shape,
                color: hex(&b.fill),
            });
            if b.bw > 0.0 {
                list.push(Item::Border {
                    shape,
                    width: b.bw,
                    color: hex(&b.border),
                });
            }
        }
        return list;
    }
    let (para, spans) = inlines(&layout_in(c), t.fg, t.code);
    let text = layout(fonts, &para, &spans).unwrap_or_else(|e| panic!("{e}"));
    let pre = c.block == "pre";
    if pre {
        list.push(Item::Fill {
            shape: Shape {
                rect: Rect {
                    x: o,
                    y: o,
                    w: c.width + 2.0 * PRE_PADDING,
                    h: height - 2.0 * o,
                },
                radii: Radii::all(6.0),
            },
            color: t.pre,
        });
    }
    let inset = if pre { o + PRE_PADDING } else { o };
    text.paint(&mut list, inset, inset);
    list
}

pub fn decode_png(bytes: &[u8]) -> (u32, u32, Vec<u8>) {
    let dec = png::Decoder::new(std::io::Cursor::new(bytes));
    let mut rd = dec.read_info().expect("png header");
    let mut buf = vec![0; rd.output_buffer_size().expect("png size")];
    let info = rd.next_frame(&mut buf).expect("png frame");
    let px = match info.color_type {
        png::ColorType::Rgba => buf[..info.buffer_size()].to_vec(),
        png::ColorType::Rgb => buf[..info.buffer_size()]
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        t => panic!("unsupported png color type {t:?}"),
    };
    (info.width, info.height, px)
}

pub fn diff_ratio(a: &[u8], e: &[u8], w: u32, h: u32, tol: u8, slack: i32) -> (f64, u64) {
    let mut diff = 0u64;
    for y in 0..h {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let close = |j: usize| (0..4).all(|k| a[i + k].abs_diff(e[j + k]) <= tol);
            let mut ok = false;
            'n: for dy in -slack..=slack {
                for dx in -slack..=slack {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    if nx < 0 || ny < 0 || nx as u32 >= w || ny as u32 >= h {
                        continue;
                    }
                    if close(((ny as u32 * w + nx as u32) * 4) as usize) {
                        ok = true;
                        break 'n;
                    }
                }
            }
            if !ok {
                diff += 1;
            }
        }
    }
    let counted = w as u64 * h as u64;
    (diff as f64 / counted.max(1) as f64, counted)
}

pub fn write_report(name: &str, value: &serde_json::Value) {
    let tmp = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let dir = tmp.parent().unwrap_or(tmp).join("parity");
    std::fs::create_dir_all(&dir).expect("report dir");
    std::fs::write(
        dir.join(name),
        serde_json::to_string_pretty(value).expect("report json"),
    )
    .expect("report write");
}
