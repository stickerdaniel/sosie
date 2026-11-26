mod common;

use common::{all_fonts, decode_png, decode_webp, hex, json, pixmap, reference_chart, SANS};
use sha2::{Digest, Sha256};
use sosie_core::{
    chart, encode, layout, rasterize, Direction, DisplayList, FamilyList, Fonts, Format, Inline,
    OverflowWrap, Paragraph, Pixmap, TextStyle, Theme, WhiteSpace,
};
use std::collections::BTreeMap;

fn render(fonts: &Fonts, text: &str, direction: Direction) -> Pixmap {
    let style = TextStyle {
        family: FamilyList::parse(SANS),
        size: 14.0,
        line_height: 21.0,
        weight: 400,
        italic: false,
        color: hex("#1f2328"),
        white_space: WhiteSpace::Normal,
        tab_size: 8,
        background: None,
    };
    let para = Paragraph {
        width: 784.0,
        strut: style.clone(),
        overflow_wrap: OverflowWrap::Anywhere,
        direction,
        dpr: 2.0,
    };
    let bold = TextStyle {
        weight: 600,
        ..style.clone()
    };
    let inlines = [
        Inline::Text {
            text: text.to_string(),
            style,
        },
        Inline::Text {
            text: " done.".to_string(),
            style: bold,
        },
    ];
    let t = layout(fonts, &para, &inlines).unwrap_or_else(|e| panic!("{e}"));
    let mut list = DisplayList::new(800.0, (t.height + 16.0).ceil(), hex("#ffffff"));
    t.paint(&mut list, 8.0, 8.0);
    rasterize(fonts, &list, 2.0, &Default::default()).expect("render")
}

const CASES: [(&str, &str, Direction); 4] = [
    ("ascii", "The renderer draws each comment with its own font size and weight before the agent posts it, so nobody needs a browser for that step.", Direction::Ltr),
    ("emoji", "Shipped 🚀🎉 with 👩‍💻 and 🇩🇪 flags, keycaps 1️⃣ #️⃣ and skin tones ✌🏽👋🏿 in one line of review text.", Direction::Ltr),
    ("cjk", "レンダラーはエージェントが投稿する前に、各コメントを独自のフォントサイズで描画します。渲染器在代理发布之前绘制每条评论。", Direction::Ltr),
    ("rtl", "המעבד מצייר כל תגובה לפני שהסוכן מפרסם אותו and يرسم العارض كل تعليق", Direction::Auto),
];

fn render_chart(fonts: &Fonts, theme: Theme) -> Pixmap {
    let list =
        chart::render(fonts, &reference_chart(), theme, 814).unwrap_or_else(|e| panic!("{e}"));
    rasterize(fonts, &list, 2.0, &Default::default()).expect("render")
}

fn cases(fonts: &Fonts) -> Vec<(String, Pixmap)> {
    let text = CASES
        .iter()
        .map(|(name, text, dir)| (name.to_string(), render(fonts, text, *dir)));
    let charts = [("chart-light", Theme::Light), ("chart-dark", Theme::Dark)]
        .into_iter()
        .map(|(name, theme)| (name.to_string(), render_chart(fonts, theme)));
    let synthetic = synthetic()
        .into_iter()
        .map(|(name, pixmap)| (name.to_string(), pixmap));
    text.chain(charts).chain(synthetic).collect()
}

fn synthetic() -> [(&'static str, Pixmap); 3] {
    [
        (
            "gradient-333x77",
            pixmap(333, 77, |x, y| {
                [
                    (x * 255 / 332) as u8,
                    (y * 255 / 76) as u8,
                    ((x + y) % 256) as u8,
                    255,
                ]
            }),
        ),
        (
            "transparent-64x48",
            pixmap(64, 48, |x, y| {
                let a = if x < 32 { 0 } else { (x * 8 - 256) as u8 };
                [(x * 4) as u8, (y * 5) as u8, 200, a]
            }),
        ),
        (
            "flat-400x200",
            pixmap(400, 200, |x, y| {
                if y < 4 {
                    [x as u8, (x >> 8) as u8 * 50 + y as u8, 7, 255]
                } else {
                    [9, 105, 218, 255]
                }
            }),
        ),
    ]
}

fn encoded(fonts: &Fonts) -> Vec<(String, Format, Pixmap, Vec<u8>)> {
    cases(fonts)
        .into_iter()
        .flat_map(|(name, pixmap)| {
            [Format::Png, Format::Webp].map(|format| {
                let bytes = encode(&pixmap, format).unwrap_or_else(|e| panic!("{name}: {e}"));
                let key = format!("{name}.{}", format.extension());
                (key, format, pixmap.clone(), bytes)
            })
        })
        .collect()
}

#[test]
fn encoded_bytes_are_identical_on_every_platform() {
    let fonts = all_fonts();
    let expected: BTreeMap<String, String> = json("crates/sosie-core/tests/determinism.json");
    let actual: BTreeMap<String, String> = encoded(&fonts)
        .into_iter()
        .map(|(key, _, _, bytes)| (key, format!("{:x}", Sha256::digest(bytes))))
        .collect();
    assert_eq!(
        actual,
        expected,
        "SHA-256 per case and format on {}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}

#[test]
fn encoded_images_decode_to_the_pixmap() {
    let fonts = all_fonts();
    for (key, format, pixmap, bytes) in encoded(&fonts) {
        let (w, h, rgba) = match format {
            Format::Png => decode_png(&bytes),
            Format::Webp => decode_webp(&bytes),
        };
        assert_eq!((w, h), (pixmap.width, pixmap.height), "{key}: size");
        assert!(rgba == pixmap.rgba, "{key}: decoded pixels differ");
    }
}
