mod common;

use common::{all_fonts, hex, read, LAZY_FONTS, MONO, SANS};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sosie_core::{
    layout, Direction, DisplayList, FamilyList, FontError, Fonts, Inline, Item, OverflowWrap, Pack,
    Paragraph, TextStyle, WhiteSpace,
};

#[derive(Deserialize)]
struct LockEntry {
    file: String,
    sha256: String,
    bytes: u64,
    pack: String,
}

#[derive(Deserialize)]
struct Lock {
    fonts: Vec<LockEntry>,
}

fn style(family: &str, weight: u16, italic: bool) -> TextStyle {
    TextStyle {
        family: FamilyList::parse(family),
        size: 14.0,
        line_height: 21.0,
        weight,
        italic,
        color: hex("#1f2328"),
        white_space: WhiteSpace::Normal,
        tab_size: 8,
        background: None,
    }
}

fn faces(fonts: &Fonts, text: &str, style: TextStyle) -> Vec<&'static str> {
    let para = Paragraph {
        width: 400.0,
        strut: style.clone(),
        overflow_wrap: OverflowWrap::Anywhere,
        direction: Direction::Auto,
        dpr: 1.0,
    };
    let t = layout(
        fonts,
        &para,
        &[Inline::Text {
            text: text.to_string(),
            style,
        }],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut list = DisplayList::new(400.0, 100.0, hex("#ffffff"));
    t.paint(&mut list, 0.0, 0.0);
    let mut out: Vec<&'static str> = list
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs { face, .. } => fonts.face(*face).map(|f| f.file()),
            _ => None,
        })
        .collect();
    out.dedup();
    out
}

#[test]
fn lock_file_pins_every_shipped_face() {
    let lock: Lock = serde_json::from_slice(&read("assets/fonts.lock.json")).expect("lock json");
    let mut fonts = Fonts::new();
    for e in &lock.fonts {
        let data = read(&format!("assets/fonts/{}", e.file));
        assert_eq!(data.len() as u64, e.bytes, "{}", e.file);
        assert_eq!(
            format!("{:x}", Sha256::digest(&data)),
            e.sha256,
            "{}",
            e.file
        );
        let pack = fonts
            .register(data)
            .unwrap_or_else(|err| panic!("{}: {err}", e.file));
        assert_eq!(format!("{pack:?}").to_lowercase(), e.pack, "{}", e.file);
    }
    for pack in [
        Pack::Core,
        Pack::Emoji,
        Pack::Cjk,
        Pack::Arabic,
        Pack::Hebrew,
    ] {
        assert!(fonts.loaded(pack), "{pack:?}");
    }
    assert_eq!(lock.fonts.len(), 10);
}

#[test]
fn a_file_outside_the_lock_is_refused() {
    let mut fonts = Fonts::new();
    let mut data = read("assets/fonts/NotoSansHebrew-Regular.ttf");
    data[100] ^= 1;
    assert!(matches!(
        fonts.register(data),
        Err(FontError::Unknown { .. })
    ));
    assert!(!fonts.loaded(Pack::Hebrew));
}

#[test]
fn lazy_packs_start_unloaded() {
    let fonts = Fonts::new();
    assert!(fonts.loaded(Pack::Core));
    for pack in [Pack::Emoji, Pack::Cjk, Pack::Arabic, Pack::Hebrew] {
        assert!(!fonts.loaded(pack), "{pack:?}");
    }
    assert_eq!(LAZY_FONTS.len(), 4);
}

#[test]
fn weight_and_style_select_the_css_matching_face() {
    let fonts = Fonts::new();
    assert_eq!(
        faces(&fonts, "Weight", style(SANS, 400, false)),
        ["NotoSans-Regular.ttf"]
    );
    assert_eq!(
        faces(&fonts, "Weight", style(SANS, 500, false)),
        ["NotoSans-Regular.ttf"]
    );
    assert_eq!(
        faces(&fonts, "Weight", style(SANS, 600, false)),
        ["NotoSans-Bold.ttf"]
    );
    assert_eq!(
        faces(&fonts, "Weight", style(SANS, 600, true)),
        ["NotoSans-BoldItalic.ttf"]
    );
    assert_eq!(
        faces(&fonts, "Weight", style(SANS, 300, true)),
        ["NotoSans-Italic.ttf"]
    );
    assert_eq!(
        faces(&fonts, "fn main()", style(MONO, 700, false)),
        ["LiberationMono-Bold.ttf"]
    );
    assert_eq!(
        faces(&fonts, "fn main()", style(MONO, 400, true)),
        ["LiberationMono-Regular.ttf"]
    );
}

#[test]
fn characters_fall_back_per_cluster_in_chromium_order() {
    let fonts = all_fonts();
    assert_eq!(
        faces(&fonts, "ok 🚀", style(SANS, 400, false)),
        ["NotoSans-Regular.ttf", "NotoColorEmoji.ttf"]
    );
    assert_eq!(
        faces(&fonts, "日本", style(MONO, 400, false)),
        ["NotoSansCJK-Regular.ttc"]
    );
    assert_eq!(
        faces(&fonts, "שלום", style(SANS, 400, false)),
        ["NotoSansHebrew-Regular.ttf"]
    );
    assert_eq!(
        faces(&fonts, "مرحبا", style(SANS, 400, false)),
        ["NotoSansArabic-Regular.ttf"]
    );
    assert_eq!(
        faces(&fonts, "Привет", style(MONO, 400, false)),
        ["LiberationMono-Regular.ttf"]
    );
}

#[test]
fn an_unloaded_pack_leaves_its_characters_on_the_primary_face() {
    let fonts = Fonts::new();
    assert_eq!(
        faces(&fonts, "日本", style(SANS, 400, false)),
        ["NotoSans-Regular.ttf"]
    );
}
