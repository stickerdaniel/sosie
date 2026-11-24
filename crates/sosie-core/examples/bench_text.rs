use sosie_core::{
    encode_png, layout, rasterize, Direction, DisplayList, FamilyList, Fonts, Inline,
    InlineBackground, Limits, OverflowWrap, Paragraph, Rgba, TextLayout, TextStyle, WhiteSpace,
};
use std::time::{Duration, Instant};

const SANS: &str = r#"-apple-system, BlinkMacSystemFont, "Segoe UI", "Noto Sans", Helvetica, Arial, sans-serif, "Apple Color Emoji", "Segoe UI Emoji""#;
const MONO: &str =
    r#"ui-monospace, SFMono-Regular, "SF Mono", Menlo, Consolas, "Liberation Mono", monospace"#;
const WIDTH: f32 = 800.0;
const DPR: f32 = 2.0;
const RUNS: usize = 200;

fn comment() -> (Paragraph, Vec<Inline>) {
    let fg = Rgba::hex(0x1f2328);
    let sans = TextStyle {
        family: FamilyList::parse(SANS),
        size: 14.0,
        line_height: 21.0,
        weight: 400,
        italic: false,
        color: fg,
        white_space: WhiteSpace::Normal,
        tab_size: 8,
        background: None,
    };
    let code = TextStyle {
        family: FamilyList::parse(MONO),
        size: 11.9,
        white_space: WhiteSpace::BreakSpaces,
        tab_size: 4,
        background: Some(InlineBackground {
            padding_x: 0.4 * 11.9,
            padding_y: 0.2 * 11.9,
            radius: 6.0,
            color: Rgba::parse_hex("#818b981f").unwrap_or(fg),
        }),
        ..sans.clone()
    };
    let bold = TextStyle {
        weight: 600,
        ..sans.clone()
    };
    let prose = "The renderer draws each review comment with its own font size and weight before the agent posts it, so nobody needs a browser for that step. Layout follows the CSS rules exactly as Chromium does when it wraps text at the edge of a column. ";
    let mut inlines = Vec::new();
    for i in 0..4 {
        inlines.push(Inline::Text {
            text: prose.to_string(),
            style: sans.clone(),
        });
        inlines.push(Inline::Text {
            text: format!("start_line_{i}"),
            style: code.clone(),
        });
        inlines.push(Inline::Text {
            text: " and a strong claim. ".to_string(),
            style: bold.clone(),
        });
    }
    let para = Paragraph {
        width: WIDTH - 32.0,
        strut: sans,
        overflow_wrap: OverflowWrap::Anywhere,
        direction: Direction::Ltr,
    };
    (para, inlines)
}

fn list(t: &TextLayout) -> DisplayList {
    let mut l = DisplayList::new(WIDTH, (t.height + 32.0).ceil(), Rgba::WHITE);
    t.paint(&mut l, 16.0, 16.0);
    l
}

fn stats(mut v: Vec<Duration>) -> String {
    v.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    format!(
        "p50 {:.3} ms, p95 {:.3} ms",
        ms(v[v.len() / 2]),
        ms(v[v.len() * 95 / 100])
    )
}

fn main() {
    let (para, inlines) = comment();
    let limits = Limits::default();
    let t0 = Instant::now();
    let fonts = Fonts::new();
    let fonts_new = t0.elapsed();
    let t1 = Instant::now();
    let t = layout(&fonts, &para, &inlines);
    let png = rasterize(&fonts, &list(&t), DPR, &limits)
        .map(|p| encode_png(&p))
        .unwrap_or_default();
    let cold = t1.elapsed();
    let (mut lay, mut ras, mut enc, mut all) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for _ in 0..RUNS {
        let a = Instant::now();
        let t = layout(&fonts, &para, &inlines);
        let b = Instant::now();
        let pm = rasterize(&fonts, &list(&t), DPR, &limits).unwrap_or_else(|e| panic!("{e}"));
        let c = Instant::now();
        let bytes = encode_png(&pm);
        let d = Instant::now();
        assert_eq!(bytes.len(), png.len());
        lay.push(b - a);
        ras.push(c - b);
        enc.push(d - c);
        all.push(d - a);
    }
    println!(
        "text comment: {} lines at {WIDTH} px, dpr {DPR}, {} by {} px PNG of {} bytes",
        t.lines.len(),
        (WIDTH * DPR) as u32,
        ((t.height + 32.0).ceil() * DPR) as u32,
        png.len()
    );
    println!("Fonts::new: {:.3} ms", fonts_new.as_secs_f64() * 1000.0);
    println!(
        "cold render (layout, raster, PNG): {:.3} ms",
        cold.as_secs_f64() * 1000.0
    );
    println!("warm layout: {}", stats(lay));
    println!("warm raster: {}", stats(ras));
    println!("warm PNG encode: {}", stats(enc));
    println!("warm total: {}", stats(all));
    if let Some(path) = std::env::args().nth(1) {
        std::fs::write(&path, &png).unwrap_or_else(|e| panic!("{path}: {e}"));
    }
}
