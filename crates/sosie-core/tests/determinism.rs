mod common;

use common::{all_fonts, decode_png, hex, json, reference_chart, SANS};
use sha2::{Digest, Sha256};
use sosie_core::{
    chart, encode_png, layout, rasterize, Direction, DisplayList, FamilyList, Fonts, Inline,
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
    text.chain(charts).collect()
}

#[test]
fn png_bytes_are_identical_on_every_platform() {
    let fonts = all_fonts();
    let expected: BTreeMap<String, String> = json("crates/sosie-core/tests/determinism.json");
    let actual: BTreeMap<String, String> = cases(&fonts)
        .into_iter()
        .map(|(name, pixmap)| (name, format!("{:x}", Sha256::digest(encode_png(&pixmap)))))
        .collect();
    assert_eq!(
        actual,
        expected,
        "PNG SHA-256 per case on {}-{}",
        std::env::consts::OS,
        std::env::consts::ARCH
    );
}

#[test]
fn png_decodes_to_the_rasterized_pixels() {
    let fonts = all_fonts();
    for (name, pixmap) in cases(&fonts) {
        let (w, h, rgba) = decode_png(&encode_png(&pixmap));
        assert_eq!((w, h), (pixmap.width, pixmap.height), "{name}: size");
        assert!(rgba == pixmap.rgba, "{name}: decoded pixels differ");
    }
}
