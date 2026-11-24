mod common;

use common::{all_fonts, hex, MONO, SANS};
use sosie_core::{
    layout, rasterize, Direction, DisplayList, FamilyList, Fonts, Inline, InlineBox, Item,
    LayoutError, Limits, OverflowWrap, Paragraph, TextLayout, TextStyle, Warning, WhiteSpace,
    MAX_FONT_SIZE,
};

fn style(family: &str, white_space: WhiteSpace) -> TextStyle {
    TextStyle {
        family: FamilyList::parse(family),
        size: 14.0,
        line_height: 21.0,
        weight: 400,
        italic: false,
        color: hex("#1f2328"),
        white_space,
        tab_size: 8,
        background: None,
    }
}

fn para(
    width: f32,
    white_space: WhiteSpace,
    overflow_wrap: OverflowWrap,
    direction: Direction,
) -> Paragraph {
    Paragraph {
        width,
        strut: style(SANS, white_space),
        overflow_wrap,
        direction,
    }
}

fn text(t: &str, white_space: WhiteSpace) -> Inline {
    Inline::Text {
        text: t.to_string(),
        style: style(SANS, white_space),
    }
}

fn lines(t: &TextLayout) -> Vec<&str> {
    t.lines.iter().map(|l| t.line_text(l).trim_end()).collect()
}

fn glyph_xs(t: &TextLayout) -> Vec<f32> {
    let mut list = DisplayList::new(400.0, 100.0, hex("#ffffff"));
    t.paint(&mut list, 0.0, 0.0);
    list.items
        .iter()
        .flat_map(|i| match i {
            Item::Glyphs { glyphs, .. } => glyphs.iter().map(|g| g.x).collect(),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn normal_white_space_collapses_runs_and_drops_leading_space() {
    let fonts = Fonts::new();
    let t = layout(
        &fonts,
        &para(
            400.0,
            WhiteSpace::Normal,
            OverflowWrap::Anywhere,
            Direction::Ltr,
        ),
        &[
            text("  one   two\n\tthree ", WhiteSpace::Normal),
            text(" four", WhiteSpace::Normal),
        ],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(t.text, "one two three four");
    assert_eq!(lines(&t), ["one two three four"]);
}

#[test]
fn pre_keeps_spaces_and_breaks_only_at_newlines() {
    let fonts = Fonts::new();
    let mut p = para(60.0, WhiteSpace::Pre, OverflowWrap::Normal, Direction::Ltr);
    p.strut.family = FamilyList::parse(MONO);
    let pre = Inline::Text {
        text: "let  x = 1; // a long line that overflows\n\nend".to_string(),
        style: p.strut.clone(),
    };
    let t = layout(&fonts, &p, &[pre]).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        lines(&t),
        ["let  x = 1; // a long line that overflows", "", "end"]
    );
    assert_eq!(t.height, 63.0);
}

#[test]
fn normal_text_wraps_at_spaces_within_the_width() {
    let fonts = Fonts::new();
    let t = layout(
        &fonts,
        &para(
            120.0,
            WhiteSpace::Normal,
            OverflowWrap::Anywhere,
            Direction::Ltr,
        ),
        &[text(
            "the quick brown fox jumps over the lazy dog",
            WhiteSpace::Normal,
        )],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(t.lines.len() > 1);
    assert!(
        t.lines.iter().all(|l| l.advance <= 120.0),
        "{:?}",
        t.lines.iter().map(|l| l.advance).collect::<Vec<_>>()
    );
    assert_eq!(
        lines(&t).join(" "),
        "the quick brown fox jumps over the lazy dog"
    );
}

#[test]
fn overflow_wrap_decides_whether_a_long_token_breaks() {
    let fonts = Fonts::new();
    let token = [text(
        "SupercalifragilisticexpialidociousWithoutBreaks",
        WhiteSpace::Normal,
    )];
    let anywhere = layout(
        &fonts,
        &para(
            100.0,
            WhiteSpace::Normal,
            OverflowWrap::Anywhere,
            Direction::Ltr,
        ),
        &token,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let normal = layout(
        &fonts,
        &para(
            100.0,
            WhiteSpace::Normal,
            OverflowWrap::Normal,
            Direction::Ltr,
        ),
        &token,
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(anywhere.lines.len() > 1);
    assert!(anywhere.lines.iter().all(|l| l.advance <= 100.0));
    assert_eq!(normal.lines.len(), 1);
}

#[test]
fn an_inline_box_sits_on_the_baseline_and_raises_the_line() {
    let fonts = Fonts::new();
    let t = layout(
        &fonts,
        &para(
            400.0,
            WhiteSpace::Normal,
            OverflowWrap::Anywhere,
            Direction::Ltr,
        ),
        &[
            text("avatar ", WhiteSpace::Normal),
            Inline::Box(InlineBox {
                width: 20.0,
                height: 40.0,
            }),
            text(" name", WhiteSpace::Normal),
        ],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(t.lines.len(), 1);
    let line = &t.lines[0];
    let b = line.boxes[0];
    assert_eq!(b.inline, 1);
    assert_eq!(b.y + b.height, line.baseline);
    assert!(line.height > 40.0);
    assert!(b.x > 0.0 && b.x + b.width < line.advance);
}

#[test]
fn an_rtl_paragraph_aligns_to_the_right_edge() {
    let fonts = all_fonts();
    let t = layout(
        &fonts,
        &para(
            400.0,
            WhiteSpace::Normal,
            OverflowWrap::Anywhere,
            Direction::Auto,
        ),
        &[text("שלום עולם", WhiteSpace::Normal)],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let xs = glyph_xs(&t);
    let min = xs.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(min > 300.0, "leftmost glyph at {min}");
    let ltr = layout(
        &fonts,
        &para(
            400.0,
            WhiteSpace::Normal,
            OverflowWrap::Anywhere,
            Direction::Ltr,
        ),
        &[text("hello world", WhiteSpace::Normal)],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(
        glyph_xs(&ltr).iter().copied().fold(f32::INFINITY, f32::min),
        0.0
    );
}

#[test]
fn an_empty_paragraph_has_no_lines() {
    let fonts = Fonts::new();
    let t = layout(
        &fonts,
        &para(
            400.0,
            WhiteSpace::Normal,
            OverflowWrap::Anywhere,
            Direction::Ltr,
        ),
        &[],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert!(t.lines.is_empty());
    assert_eq!(t.height, 0.0);
}

#[test]
fn a_line_may_end_before_a_small_kana() {
    let fonts = all_fonts();
    let t = layout(
        &fonts,
        &para(
            30.0,
            WhiteSpace::Normal,
            OverflowWrap::Normal,
            Direction::Ltr,
        ),
        &[text("ああっ", WhiteSpace::Normal)],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(lines(&t), ["ああ", "っ"]);
}

fn bold(t: &str) -> Inline {
    Inline::Text {
        text: t.to_string(),
        style: TextStyle {
            weight: 600,
            ..style(SANS, WhiteSpace::Normal)
        },
    }
}

fn faces(fonts: &Fonts, t: &TextLayout) -> Vec<&'static str> {
    let mut list = DisplayList::new(400.0, 100.0, hex("#ffffff"));
    t.paint(&mut list, 0.0, 0.0);
    list.items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs { face, .. } => fonts.face(*face).map(|f| f.file()),
            _ => None,
        })
        .collect()
}

#[test]
fn bold_without_a_bold_face_renders_regular_and_warns() {
    let fonts = all_fonts();
    let p = para(
        400.0,
        WhiteSpace::Normal,
        OverflowWrap::Normal,
        Direction::Auto,
    );
    for (sample, file) in [
        ("漢字", "NotoSansCJK-Regular.ttc"),
        ("مرحبا", "NotoSansArabic-Regular.ttf"),
        ("שלום", "NotoSansHebrew-Regular.ttf"),
    ] {
        let t = layout(&fonts, &p, &[bold(sample)]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(faces(&fonts, &t), [file], "{sample}");
        assert_eq!(t.warnings, [Warning::SyntheticBoldUnsupported], "{sample}");
        assert_eq!(t.warnings[0].code(), "synthetic-bold-unsupported");
    }
}

#[test]
fn bold_latin_and_regular_cjk_raise_no_warning() {
    let fonts = all_fonts();
    let p = para(
        400.0,
        WhiteSpace::Normal,
        OverflowWrap::Normal,
        Direction::Ltr,
    );
    let latin = layout(&fonts, &p, &[bold("strong claim")]).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(faces(&fonts, &latin), ["NotoSans-Bold.ttf"]);
    assert!(latin.warnings.is_empty());
    let cjk =
        layout(&fonts, &p, &[text("漢字", WhiteSpace::Normal)]).unwrap_or_else(|e| panic!("{e}"));
    assert!(cjk.warnings.is_empty());
}

fn glyphs(t: &TextLayout) -> Vec<(u32, f32)> {
    let mut list = DisplayList::new(400.0, 100.0, hex("#ffffff"));
    t.paint(&mut list, 0.0, 0.0);
    list.items
        .iter()
        .flat_map(|i| match i {
            Item::Glyphs { glyphs, .. } => glyphs.iter().map(|g| (g.id, g.x)).collect(),
            _ => Vec::new(),
        })
        .collect()
}

fn pieces(parts: &[&str], white_space: WhiteSpace) -> Vec<Inline> {
    parts.iter().map(|p| text(p, white_space)).collect()
}

#[test]
fn equal_adjacent_spans_shape_as_one_run() {
    let fonts = all_fonts();
    let p = para(
        400.0,
        WhiteSpace::Normal,
        OverflowWrap::Normal,
        Direction::Auto,
    );
    for (whole, split) in [("AV", ["A", "V"]), ("مرحبا", ["م", "رحبا"])] {
        let one = layout(&fonts, &p, &pieces(&[whole], WhiteSpace::Normal))
            .unwrap_or_else(|e| panic!("{e}"));
        let two = layout(&fonts, &p, &pieces(&split, WhiteSpace::Normal))
            .unwrap_or_else(|e| panic!("{e}"));
        let (a, b) = (glyphs(&one), glyphs(&two));
        let ids = |g: &[(u32, f32)]| g.iter().map(|x| x.0).collect::<Vec<_>>();
        assert_eq!(ids(&a), ids(&b), "{whole}: glyph ids");
        for ((_, xa), (_, xb)) in a.iter().zip(&b) {
            assert!(
                (xa - xb).abs() <= 1.0 / 64.0,
                "{whole}: glyph x {xa} vs {xb}"
            );
        }
        let (wa, wb) = (one.lines[0].advance, two.lines[0].advance);
        assert!(
            (wa - wb).abs() <= 1.0 / 64.0,
            "{whole}: advance {wa} vs {wb}"
        );
    }
}

#[test]
fn a_trailing_preserved_newline_adds_no_line() {
    let fonts = Fonts::new();
    let p = para(400.0, WhiteSpace::Pre, OverflowWrap::Normal, Direction::Ltr);
    for (input, count) in [("a", 1), ("a\n", 1), ("\n", 1), ("a\n\n", 2), ("\n\n", 2)] {
        let t =
            layout(&fonts, &p, &[text(input, WhiteSpace::Pre)]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(t.lines.len(), count, "{input:?}: lines");
        assert_eq!(t.height, 21.0 * count as f32, "{input:?}: height");
    }
}

#[test]
fn crlf_breaks_a_preserved_line_like_lf() {
    let fonts = Fonts::new();
    let p = para(400.0, WhiteSpace::Pre, OverflowWrap::Normal, Direction::Ltr);
    for input in ["a\nb", "a\r\nb"] {
        let t =
            layout(&fonts, &p, &[text(input, WhiteSpace::Pre)]).unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(lines(&t), ["a", "b"], "{input:?}");
        assert_eq!(t.height, 42.0, "{input:?}");
        assert!(
            glyphs(&t).iter().all(|g| g.0 != 0),
            "{input:?}: missing glyph"
        );
    }
}

#[test]
fn a_zero_tab_size_gives_the_tab_no_advance() {
    let fonts = Fonts::new();
    let p = para(400.0, WhiteSpace::Pre, OverflowWrap::Normal, Direction::Ltr);
    let zero = TextStyle {
        tab_size: 0,
        ..style(SANS, WhiteSpace::Pre)
    };
    let run = |t: &str| {
        layout(
            &fonts,
            &p,
            &[Inline::Text {
                text: t.to_string(),
                style: zero.clone(),
            }],
        )
        .unwrap_or_else(|e| panic!("{e}"))
    };
    let tab = run("a\tb");
    let plain = run("ab");
    assert!(tab.lines[0].advance.is_finite());
    let (xs, plain_xs) = (glyph_xs(&tab), glyph_xs(&plain));
    assert_eq!(xs.len(), 2);
    assert!(
        (xs[1] - plain_xs[1]).abs() <= 1.0 / 64.0,
        "{xs:?} vs {plain_xs:?}"
    );
}

#[test]
fn break_spaces_may_break_between_preserved_spaces() {
    let fonts = all_fonts();
    let mono = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        ..style(MONO, WhiteSpace::BreakSpaces)
    };
    let p = Paragraph {
        width: 13.0,
        strut: mono.clone(),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
    };
    let t = layout(
        &fonts,
        &p,
        &[Inline::Text {
            text: "a    b".to_string(),
            style: mono,
        }],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let texts: Vec<&str> = t.lines.iter().map(|l| t.line_text(l)).collect();
    assert_eq!(texts, ["a ", "  ", " b"]);
    assert!(t.lines.iter().all(|l| l.advance <= 13.0));
}

#[test]
fn a_font_size_outside_the_supported_range_is_refused() {
    let fonts = Fonts::new();
    let p = para(
        200.0,
        WhiteSpace::Normal,
        OverflowWrap::Normal,
        Direction::Ltr,
    );
    for size in [1.0e20, f32::INFINITY, f32::NAN, -1.0, MAX_FONT_SIZE + 1.0] {
        let big = Inline::Text {
            text: "AV".to_string(),
            style: TextStyle {
                size,
                ..style(SANS, WhiteSpace::Normal)
            },
        };
        assert!(
            matches!(
                layout(&fonts, &p, &[big]),
                Err(LayoutError::FontSize { .. })
            ),
            "{size}"
        );
    }
    let largest = Inline::Text {
        text: "AV".to_string(),
        style: TextStyle {
            size: MAX_FONT_SIZE,
            ..style(SANS, WhiteSpace::Normal)
        },
    };
    assert!(layout(&fonts, &p, &[largest]).is_ok());
}

fn arabic(size: f32, line_height: f32, white_space: WhiteSpace) -> TextStyle {
    TextStyle {
        family: FamilyList::parse("Noto Sans Arabic"),
        size,
        line_height,
        color: hex("#000000"),
        ..style(SANS, white_space)
    }
}

#[test]
fn an_rtl_ligature_paints_where_chromium_puts_it() {
    let fonts = all_fonts();
    let st = arabic(60.0, 80.0, WhiteSpace::Pre);
    let p = Paragraph {
        width: 40.0,
        strut: st.clone(),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Rtl,
    };
    for (word, left, ink_x, ink_y) in [
        ("لا", 25.078125, (27, 55), (16, 58)),
        ("بلا", 7.90625, (10, 56), (16, 70)),
        ("لالا", -9.84375, (0, 55), (16, 58)),
    ] {
        let t = layout(
            &fonts,
            &p,
            &[Inline::Text {
                text: word.to_string(),
                style: st.clone(),
            }],
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let mut list = DisplayList::new(120.0, 100.0, hex("#ffffff"));
        t.paint(&mut list, 20.0, 0.0);
        let x = list
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Glyphs { x, glyphs, .. } => glyphs.first().map(|g| x + g.x),
                _ => None,
            })
            .fold(f32::INFINITY, f32::min);
        assert!(
            (x - left).abs() <= 1.0 / 64.0,
            "{word}: run starts at {x}, Chromium at {left}"
        );
        let pm =
            rasterize(&fonts, &list, 1.0, &Limits::default()).unwrap_or_else(|e| panic!("{e}"));
        let ink: Vec<(u32, u32)> = (0..pm.height)
            .flat_map(|y| (0..pm.width).map(move |x| (x, y)))
            .filter(|&(x, y)| pm.rgba[((y * pm.width + x) * 4) as usize] != 255)
            .collect();
        let span = |v: Vec<u32>| (*v.iter().min().unwrap_or(&0), *v.iter().max().unwrap_or(&0));
        let (xs, ys) = (
            span(ink.iter().map(|p| p.0).collect()),
            span(ink.iter().map(|p| p.1).collect()),
        );
        let near = |a: (u32, u32), b: (u32, u32)| a.0.abs_diff(b.0) <= 1 && a.1.abs_diff(b.1) <= 1;
        assert!(near(xs, ink_x), "{word}: ink x {xs:?}, Chromium {ink_x:?}");
        assert!(near(ys, ink_y), "{word}: ink y {ys:?}, Chromium {ink_y:?}");
    }
}

#[test]
fn break_spaces_may_break_after_each_preserved_tab() {
    let fonts = all_fonts();
    let mono = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 80.0,
        tab_size: 1,
        ..style(MONO, WhiteSpace::BreakSpaces)
    };
    let p = Paragraph {
        width: 13.0,
        strut: mono.clone(),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
    };
    let t = layout(
        &fonts,
        &p,
        &[Inline::Text {
            text: "a\t\tb".to_string(),
            style: mono,
        }],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let texts: Vec<&str> = t.lines.iter().map(|l| t.line_text(l)).collect();
    assert_eq!(texts, ["a\t", "\tb"]);
    assert_eq!(t.height, 160.0);
    assert!(t.lines.iter().all(|l| l.advance <= 13.0));
}

#[test]
fn trimmed_cjk_brackets_paint_inside_the_measured_box() {
    let fonts = all_fonts();
    let st = TextStyle {
        line_height: 80.0,
        color: hex("#000000"),
        ..style("Noto Sans", WhiteSpace::Pre)
    };
    let short = [0.0, 14.0, 21.0, 35.0];
    let long = [0.0, 14.0, 21.0, 35.0, 49.0, 56.0, 70.0];
    let cases: [(&[&str], f32, &[f32], u32); 4] = [
        (&["中「（文"], 49.0, &short, 48),
        (&["中「（", "文"], 49.0, &short, 48),
        (&["中「（文。）A"], 78.953125, &long, 78),
        (&["中「（文。", "）A"], 78.953125, &long, 78),
    ];
    for (parts, advance, chromium, ink_right) in cases {
        let p = Paragraph {
            width: advance,
            strut: st.clone(),
            overflow_wrap: OverflowWrap::Normal,
            direction: Direction::Ltr,
        };
        let inlines: Vec<Inline> = parts
            .iter()
            .map(|t| Inline::Text {
                text: t.to_string(),
                style: st.clone(),
            })
            .collect();
        let t = layout(&fonts, &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
        assert!(
            (t.lines[0].advance - advance).abs() <= 1.0 / 64.0,
            "{parts:?}: advance {}, Chromium {advance}",
            t.lines[0].advance
        );
        let xs = glyph_xs(&t);
        assert!(
            xs.len() == chromium.len()
                && xs
                    .iter()
                    .zip(chromium)
                    .all(|(x, c)| (x - c).abs() <= 1.0 / 64.0),
            "{parts:?}: glyphs at {xs:?}, Chromium {chromium:?}"
        );
        let mut list = DisplayList::new(200.0, 100.0, hex("#ffffff"));
        t.paint(&mut list, 0.0, 0.0);
        let pm =
            rasterize(&fonts, &list, 1.0, &Limits::default()).unwrap_or_else(|e| panic!("{e}"));
        let ink: Vec<u32> = (0..pm.height)
            .flat_map(|y| (0..pm.width).map(move |x| (x, y)))
            .filter(|&(x, y)| pm.rgba[((y * pm.width + x) * 4) as usize] != 255)
            .map(|(x, _)| x)
            .collect();
        let (left, right) = (
            ink.iter().copied().min().unwrap_or(0),
            ink.iter().copied().max().unwrap_or(0),
        );
        assert!(
            left.abs_diff(1) <= 1 && right.abs_diff(ink_right) <= 1 && (right as f32) < advance,
            "{parts:?}: ink x {left}..{right}, Chromium 1..{ink_right}"
        );
    }
}

#[test]
fn pre_wrap_whitespace_may_break_before_break_spaces_whitespace() {
    let fonts = all_fonts();
    let mono = |white_space| TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 80.0,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    let p = Paragraph {
        width: 13.0,
        strut: mono(WhiteSpace::BreakSpaces),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
    };
    for (first, second, b_left) in [("a\t", "\tb", 6.015625), ("a ", " b", 6.0)] {
        let t = layout(
            &fonts,
            &p,
            &[
                Inline::Text {
                    text: first.to_string(),
                    style: mono(WhiteSpace::PreWrap),
                },
                Inline::Text {
                    text: second.to_string(),
                    style: mono(WhiteSpace::BreakSpaces),
                },
            ],
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let texts: Vec<&str> = t.lines.iter().map(|l| t.line_text(l)).collect();
        assert_eq!(texts, [first, second]);
        assert_eq!(t.height, 160.0);
        let b = glyph_xs(&t).last().copied().unwrap_or(f32::NAN);
        assert!(
            (b - b_left).abs() <= 1.0 / 64.0,
            "{first:?}{second:?}: b at {b}, Chromium {b_left}"
        );
    }
}

fn cjk(size: f32, color: &str) -> TextStyle {
    TextStyle {
        size,
        line_height: 80.0,
        color: hex(color),
        ..style("Noto Sans", WhiteSpace::Pre)
    }
}

fn spans(parts: &[(&str, f32, &str)]) -> Vec<Inline> {
    parts
        .iter()
        .map(|&(t, size, color)| Inline::Text {
            text: t.to_string(),
            style: cjk(size, color),
        })
        .collect()
}

fn painted(fonts: &Fonts, t: &TextLayout, color: &str) -> (f32, (u32, u32)) {
    let mut list = DisplayList::new(320.0, 120.0, hex("#ffffff"));
    t.paint(&mut list, 20.0, 0.0);
    let want = hex(color);
    let x = list
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs {
                x, glyphs, color, ..
            } if *color == want => glyphs.first().map(|g| x + g.x),
            _ => None,
        })
        .fold(f32::INFINITY, f32::min);
    let pm = rasterize(fonts, &list, 1.0, &Limits::default()).unwrap_or_else(|e| panic!("{e}"));
    let xs: Vec<u32> = (0..pm.height)
        .flat_map(|y| (0..pm.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let i = ((y * pm.width + x) * 4) as usize;
            let (r, g, b) = (pm.rgba[i], pm.rgba[i + 1], pm.rgba[i + 2]);
            if want.r > want.b {
                r > g && r > b
            } else {
                b > g && b > r
            }
        })
        .map(|(x, _)| x)
        .collect();
    let ink = (
        xs.iter().copied().min().unwrap_or(0),
        xs.iter().copied().max().unwrap_or(0),
    );
    (x, ink)
}

#[test]
fn an_rtl_trimmed_bracket_paints_where_chromium_puts_it() {
    let fonts = all_fonts();
    for (direction, left, ink) in [
        (Direction::Ltr, 34.0, (34, 38)),
        (Direction::Rtl, 85.0, (85, 89)),
    ] {
        let p = Paragraph {
            width: 100.0,
            strut: cjk(14.0, "#000000"),
            overflow_wrap: OverflowWrap::Normal,
            direction,
        };
        let t = layout(
            &fonts,
            &p,
            &spans(&[
                ("\u{202e}中「", 14.0, "#000000"),
                ("（", 14.0, "#ff0000"),
                ("文\u{202c}", 14.0, "#000000"),
            ]),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let (x, xs) = painted(&fonts, &t, "#ff0000");
        assert!(
            (x - left).abs() <= 1.0 / 64.0,
            "{direction:?}: bracket at {x}, Chromium {left}"
        );
        assert!(
            xs.0.abs_diff(ink.0) <= 1 && xs.1.abs_diff(ink.1) <= 1,
            "{direction:?}: bracket ink x {xs:?}, Chromium {ink:?}"
        );
    }
}

#[test]
fn a_cjk_trim_uses_the_trimmed_characters_own_size() {
    let fonts = all_fonts();
    let p = Paragraph {
        width: 300.0,
        strut: cjk(14.0, "#000000"),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
    };
    for (first, second, advance, left, ink) in [
        (14.0, 60.0, 141.0, 41.0, (43, 158)),
        (60.0, 14.0, 118.0, 110.0, (110, 137)),
        (0.0, 60.0, 120.0, 20.0, (22, 137)),
    ] {
        let t = layout(
            &fonts,
            &p,
            &spans(&[("中。", first, "#ff0000"), ("。文", second, "#0000ff")]),
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let got = t.lines[0].advance;
        assert!(
            (got - advance).abs() <= 1.0 / 64.0,
            "{first} then {second} px: advance {got}, Chromium {advance}"
        );
        let (x, xs) = painted(&fonts, &t, "#0000ff");
        assert!(
            (x - left).abs() <= 1.0 / 64.0,
            "{first} then {second} px: second span at {x}, Chromium {left}"
        );
        assert!(
            xs.0.abs_diff(ink.0) <= 1 && xs.1.abs_diff(ink.1) <= 1,
            "{first} then {second} px: second span ink x {xs:?}, Chromium {ink:?}"
        );
    }
}
