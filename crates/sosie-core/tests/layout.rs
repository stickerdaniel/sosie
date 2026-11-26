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

#[test]
fn whitespace_after_a_break_hangs_like_chromium() {
    let fonts = all_fonts();
    let mono = |white_space| TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 80.0,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    use WhiteSpace::{BreakSpaces as B, Normal as N, PreWrap as W};
    let cases = [
        (
            &[(W, "a "), (N, " "), (B, " b")][..],
            19.0,
            OverflowWrap::Anywhere,
            160.0,
            6.0,
            122.0,
        ),
        (
            &[(B, "a "), (W, " "), (B, " b")],
            7.0,
            OverflowWrap::Normal,
            240.0,
            0.0,
            202.0,
        ),
        (
            &[(B, "a "), (W, " b")],
            7.0,
            OverflowWrap::Normal,
            160.0,
            0.0,
            122.0,
        ),
        (
            &[(B, "a\t"), (W, "\t"), (B, "\tb")],
            7.0,
            OverflowWrap::Normal,
            240.0,
            0.0,
            202.0,
        ),
        (
            &[(B, "a\t"), (W, " b")],
            7.0,
            OverflowWrap::Normal,
            240.0,
            0.0,
            202.0,
        ),
    ];
    for (parts, width, overflow_wrap, height, b_x, b_baseline) in cases {
        let p = Paragraph {
            width,
            strut: mono(B),
            overflow_wrap,
            direction: Direction::Ltr,
        };
        let inlines: Vec<Inline> = parts
            .iter()
            .map(|&(ws, t)| Inline::Text {
                text: t.to_string(),
                style: mono(ws),
            })
            .collect();
        let t = layout(&fonts, &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
        let mut list = DisplayList::new(100.0, 400.0, hex("#ffffff"));
        t.paint(&mut list, 0.0, 0.0);
        let b = list
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Glyphs {
                    x,
                    baseline,
                    glyphs,
                    ..
                } => glyphs.last().map(|g| (x + g.x, baseline + g.y)),
                _ => None,
            })
            .next_back();
        assert_eq!(t.height, height, "{parts:?}: height");
        assert!(
            b.is_some_and(|(x, y)| (x - b_x).abs() <= 1.0 / 64.0 && y == b_baseline),
            "{parts:?}: b at {b:?}, Chromium ({b_x}, {b_baseline})"
        );
    }
}

#[test]
fn unmirrored_punctuation_keeps_its_trim_side_in_rtl() {
    let fonts = all_fonts();
    for (mark, next, ltr_ink, rtl_ink) in [
        ("。", "）文\u{202c}", (48, 52), (179, 183)),
        ("、", "）文\u{202c}", (48, 52), (179, 183)),
        ("，", "）文\u{202c}", (49, 52), (180, 183)),
        ("．", "）文\u{202c}", (50, 52), (181, 183)),
        ("。", "。文\u{202c}", (48, 52), (179, 183)),
    ] {
        for (direction, left, ink) in [
            (Direction::Ltr, 48.0, ltr_ink),
            (Direction::Rtl, 179.0, rtl_ink),
        ] {
            let p = Paragraph {
                width: 180.0,
                strut: cjk(14.0, "#000000"),
                overflow_wrap: OverflowWrap::Normal,
                direction,
            };
            let t = layout(
                &fonts,
                &p,
                &spans(&[
                    ("\u{202e}中", 14.0, "#000000"),
                    (mark, 14.0, "#ff0000"),
                    (next, 14.0, "#000000"),
                ]),
            )
            .unwrap_or_else(|e| panic!("{e}"));
            let (x, xs) = painted(&fonts, &t, "#ff0000");
            assert!(
                (x - left).abs() <= 1.0 / 64.0,
                "{mark}{next} {direction:?}: mark at {x}, Chromium {left}"
            );
            assert!(
                xs.0.abs_diff(ink.0) <= 1 && xs.1.abs_diff(ink.1) <= 1,
                "{mark}{next} {direction:?}: mark ink x {xs:?}, Chromium {ink:?}"
            );
        }
    }
}

fn mono_layout(
    fonts: &Fonts,
    parts: &[(WhiteSpace, &str)],
    width: f32,
) -> (TextLayout, Option<(f32, f32)>) {
    mono_wrapped(fonts, parts, width, OverflowWrap::Normal)
}

fn mono_wrapped(
    fonts: &Fonts,
    parts: &[(WhiteSpace, &str)],
    width: f32,
    overflow_wrap: OverflowWrap,
) -> (TextLayout, Option<(f32, f32)>) {
    let mono = |white_space| TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 80.0,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    let p = Paragraph {
        width,
        strut: mono(WhiteSpace::BreakSpaces),
        overflow_wrap,
        direction: Direction::Ltr,
    };
    let inlines: Vec<Inline> = parts
        .iter()
        .map(|&(ws, t)| Inline::Text {
            text: t.to_string(),
            style: mono(ws),
        })
        .collect();
    let t = layout(fonts, &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
    let mut list = DisplayList::new(100.0, 400.0, hex("#ffffff"));
    t.paint(&mut list, 0.0, 0.0);
    let last = list
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs {
                x,
                baseline,
                glyphs,
                ..
            } => glyphs.last().map(|g| (x + g.x, baseline + g.y)),
            _ => None,
        })
        .next_back();
    (t, last)
}

type MonoCase<'a> = (&'a [(WhiteSpace, &'a str)], f32, f32, Option<(f32, f32)>);

fn assert_mono(cases: &[MonoCase]) {
    let fonts = all_fonts();
    for &(parts, width, height, last) in cases {
        let (t, got) = mono_layout(&fonts, parts, width);
        assert_eq!(t.height, height, "{parts:?}: height");
        assert!(
            match (got, last) {
                (Some(g), Some(c)) => (g.0 - c.0).abs() <= 1.0 / 64.0 && g.1 == c.1,
                (None, None) => true,
                _ => false,
            },
            "{parts:?}: last glyph at {got:?}, Chromium {last:?}"
        );
    }
}

#[test]
fn hanging_spans_keep_the_tab_before_the_break() {
    use WhiteSpace::{BreakSpaces as B, PreWrap as W};
    assert_mono(&[
        (&[(B, "a\t"), (W, "  b")], 7.0, 240.0, Some((0.0, 202.0))),
        (
            &[(B, "a\t"), (W, " "), (W, " b")],
            7.0,
            240.0,
            Some((0.0, 202.0)),
        ),
        (
            &[(B, "a\t"), (W, " "), (W, "\tb")],
            7.0,
            240.0,
            Some((0.0, 202.0)),
        ),
    ]);
}

#[test]
fn a_preserved_newline_does_not_end_a_span_for_hanging() {
    use WhiteSpace::{BreakSpaces as B, PreWrap as W};
    assert_mono(&[
        (&[(B, "a\t"), (W, " \n")], 7.0, 160.0, Some((0.0, 42.0))),
        (&[(B, "a\t"), (W, " \nb")], 7.0, 240.0, Some((0.0, 202.0))),
        (&[(B, "a\t"), (W, " ")], 7.0, 80.0, Some((0.0, 42.0))),
    ]);
}

#[test]
fn a_normal_space_at_a_line_start_collapses() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, PreWrap as W};
    assert_mono(&[
        (&[(B, "a\t"), (N, " b")], 7.0, 160.0, Some((0.0, 122.0))),
        (&[(W, "a\n"), (N, " b")], 13.0, 160.0, Some((0.0, 122.0))),
        (&[(W, "a\n"), (N, " b")], 7.0, 160.0, Some((0.0, 122.0))),
    ]);
}

#[test]
fn a_normal_space_may_break_before_break_spaces_whitespace() {
    use WhiteSpace::{BreakSpaces as B, Normal as N};
    assert_mono(&[(&[(N, "a "), (B, " b")], 13.0, 160.0, Some((6.0, 122.0)))]);
    let (t, _) = mono_layout(&all_fonts(), &[(N, "a "), (B, " b")], 13.0);
    assert!(
        t.lines.iter().all(|l| l.advance <= 13.0),
        "advances {:?}",
        t.lines.iter().map(|l| l.advance).collect::<Vec<_>>()
    );
}

#[test]
fn the_last_line_may_end_a_sixty_fourth_past_the_width() {
    use WhiteSpace::{Normal as N, PreWrap as W};
    assert_mono(&[
        (&[(N, "aa bb cc dd")], 30.0, 160.0, Some((24.0, 122.0))),
        (&[(W, "aa bb cc dd")], 30.0, 160.0, Some((24.0, 122.0))),
    ]);
}

#[test]
fn a_spans_own_white_space_governs_wrapping_inside_it() {
    let fonts = all_fonts();
    let mono = |white_space| TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 80.0,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P};
    for (strut, inline, text, width, height, last) in [
        (B, P, "aa bb cc dd", 30.0, 80.0, (60.0, 42.0)),
        (N, P, "aa bb cc dd", 30.0, 80.0, (60.0, 42.0)),
        (P, N, "aa bb cc dd", 30.0, 160.0, (24.0, 122.0)),
        (N, P, "aa-bb", 25.0, 80.0, (24.0, 42.0)),
    ] {
        let p = Paragraph {
            width,
            strut: mono(strut),
            overflow_wrap: OverflowWrap::Normal,
            direction: Direction::Ltr,
        };
        let t = layout(
            &fonts,
            &p,
            &[Inline::Text {
                text: text.to_string(),
                style: mono(inline),
            }],
        )
        .unwrap_or_else(|e| panic!("{e}"));
        let mut list = DisplayList::new(100.0, 400.0, hex("#ffffff"));
        t.paint(&mut list, 0.0, 0.0);
        let got = list
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Glyphs {
                    x,
                    baseline,
                    glyphs,
                    ..
                } => glyphs.last().map(|g| (x + g.x, baseline + g.y)),
                _ => None,
            })
            .next_back();
        assert_eq!(t.height, height, "{inline:?} in {strut:?}: height");
        assert!(
            got.is_some_and(|g| (g.0 - last.0).abs() <= 1.0 / 64.0 && g.1 == last.1),
            "{inline:?} in {strut:?}: last glyph at {got:?}, Chromium {last:?}"
        );
    }
}

#[test]
fn a_break_between_spans_follows_the_span_before_it_or_the_strut() {
    use WhiteSpace::{Normal as N, Pre as P};
    let fonts = all_fonts();
    let mono = |white_space| TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 80.0,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    for (strut, first, second, height) in [
        (P, P, P, 80.0),
        (N, P, P, 160.0),
        (P, P, N, 80.0),
        (P, N, P, 160.0),
        (P, N, N, 160.0),
    ] {
        let p = Paragraph {
            width: 25.0,
            strut: mono(strut),
            overflow_wrap: OverflowWrap::Normal,
            direction: Direction::Ltr,
        };
        let t = layout(
            &fonts,
            &p,
            &[
                Inline::Text {
                    text: "aa-".to_string(),
                    style: mono(first),
                },
                Inline::Text {
                    text: "bb".to_string(),
                    style: mono(second),
                },
            ],
        )
        .unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(
            t.height, height,
            "{first:?} then {second:?} in {strut:?}: height"
        );
    }
}

#[test]
fn overflow_wrap_anywhere_does_not_break_inside_pre() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P};
    let fonts = all_fonts();
    let cases: [MonoCase; 5] = [
        (
            &[(P, "a\t"), (P, "\tb")],
            7.0,
            80.0,
            Some((18.015625, 42.0)),
        ),
        (
            &[(B, "a\t"), (P, "\tb")],
            7.0,
            240.0,
            Some((6.015625, 202.0)),
        ),
        (&[(P, "abcdef")], 13.0, 80.0, Some((30.0, 42.0))),
        (&[(P, "abc"), (N, "def")], 13.0, 240.0, Some((0.0, 202.0))),
        (&[(N, "abc"), (P, "def")], 13.0, 240.0, Some((12.0, 202.0))),
    ];
    for (parts, width, height, last) in cases {
        let (t, got) = mono_wrapped(&fonts, parts, width, OverflowWrap::Anywhere);
        assert_eq!(t.height, height, "{parts:?}: height");
        assert!(
            matches!((got, last), (Some(g), Some(c)) if (g.0 - c.0).abs() <= 1.0 / 64.0 && g.1 == c.1),
            "{parts:?}: last glyph at {got:?}, Chromium {last:?}"
        );
    }
}

type Placed = Vec<(u32, f32, f32)>;
type Parts = &'static [(WhiteSpace, &'static str)];

fn shared_fonts() -> &'static Fonts {
    static FONTS: std::sync::OnceLock<Fonts> = std::sync::OnceLock::new();
    FONTS.get_or_init(all_fonts)
}

fn mono_run(
    strut: WhiteSpace,
    overflow_wrap: OverflowWrap,
    direction: Direction,
    size: f32,
    parts: &[(WhiteSpace, &str)],
    width: f32,
) -> (f32, Placed) {
    let parts: Vec<_> = parts.iter().map(|&(ws, t)| (ws, t, 80.0)).collect();
    mono_spans((strut, 80.0), overflow_wrap, direction, size, &parts, width)
}

fn mono_spans(
    strut: (WhiteSpace, f32),
    overflow_wrap: OverflowWrap,
    direction: Direction,
    size: f32,
    parts: &[(WhiteSpace, &str, f32)],
    width: f32,
) -> (f32, Placed) {
    let parts: Vec<_> = parts.iter().map(|&(ws, t, lh)| (ws, t, lh, None)).collect();
    mono_padded(strut, overflow_wrap, direction, size, &parts, width)
}

fn mono_padded(
    strut: (WhiteSpace, f32),
    overflow_wrap: OverflowWrap,
    direction: Direction,
    size: f32,
    parts: &[(WhiteSpace, &str, f32, Option<f32>)],
    width: f32,
) -> (f32, Placed) {
    let mono = |white_space, line_height| TextStyle {
        family: FamilyList::parse(MONO),
        size,
        line_height,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    let p = Paragraph {
        width,
        strut: mono(strut.0, strut.1),
        overflow_wrap,
        direction,
    };
    let inlines: Vec<Inline> = parts
        .iter()
        .map(|&(ws, t, line_height, padding_x)| Inline::Text {
            text: t.to_string(),
            style: TextStyle {
                background: padding_x.map(|padding_x| sosie_core::InlineBackground {
                    padding_x,
                    padding_y: 0.0,
                    radius: 0.0,
                    color: hex("#000000"),
                }),
                ..mono(ws, line_height)
            },
        })
        .collect();
    let t = layout(shared_fonts(), &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
    let mut list = DisplayList::new(100.0, 400.0, hex("#ffffff"));
    t.paint(&mut list, 0.0, 0.0);
    let placed = list
        .items
        .iter()
        .flat_map(|i| match i {
            Item::Glyphs {
                x,
                baseline,
                glyphs,
                ..
            } => glyphs
                .iter()
                .map(|g| (g.id, x + g.x, baseline + g.y))
                .collect(),
            _ => Vec::new(),
        })
        .collect();
    (t.height, placed)
}

fn mono_at(placed: &Placed, c: &str) -> (f32, f32) {
    let (_, glyphs) = mono_run(
        WhiteSpace::Pre,
        OverflowWrap::Normal,
        Direction::Ltr,
        10.0,
        &[(WhiteSpace::Pre, c)],
        100.0,
    );
    let id = glyphs[0].0;
    let found: Vec<_> = placed.iter().filter(|g| g.0 == id).collect();
    assert_eq!(
        found.len(),
        1,
        "{c} painted {} times in {placed:?}",
        found.len()
    );
    (found[0].1, found[0].2)
}

fn assert_at(what: &str, got: (f32, f32), chromium: (f32, f32)) {
    assert!(
        (got.0 - chromium.0).abs() <= 1.0 / 64.0 && got.1 == chromium.1,
        "{what}: at {got:?}, Chromium {chromium:?}"
    );
}

#[test]
fn a_pre_spans_leading_whitespace_may_start_a_line() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    for (strut, lead, height, b) in [
        (N, " b", 160.0, (6.0, 122.0)),
        (P, " b", 160.0, (6.0, 122.0)),
        (N, "\tb", 160.0, (6.015625, 122.0)),
        (W, " b", 80.0, (12.015625, 42.0)),
        (B, " b", 80.0, (12.015625, 42.0)),
    ] {
        let parts = [(N, "a"), (P, lead)];
        let (h, placed) = mono_run(
            strut,
            OverflowWrap::Normal,
            Direction::Ltr,
            10.0,
            &parts,
            13.0,
        );
        let what = format!("{parts:?} in {strut:?}");
        assert_eq!(h, height, "{what}: height");
        assert_at(&what, mono_at(&placed, "b"), b);
    }
}

#[test]
fn whitespace_after_an_overflowing_pre_line_starts_the_next_line() {
    use WhiteSpace::{Normal as N, Pre as P, PreWrap as W};
    for (strut, overflow_wrap, parts, width, height, last) in [
        (
            N,
            OverflowWrap::Anywhere,
            [(P, "ab"), (W, " c")],
            7.0,
            240.0,
            ("c", (0.0, 202.0)),
        ),
        (
            N,
            OverflowWrap::Anywhere,
            [(P, "abcd"), (W, " e")],
            13.0,
            160.0,
            ("e", (6.0, 122.0)),
        ),
        (
            P,
            OverflowWrap::Anywhere,
            [(P, "ab"), (W, " c")],
            7.0,
            160.0,
            ("c", (0.0, 122.0)),
        ),
        (
            N,
            OverflowWrap::Normal,
            [(P, "abcd"), (W, " e")],
            13.0,
            160.0,
            ("e", (0.0, 122.0)),
        ),
    ] {
        let (h, placed) = mono_run(strut, overflow_wrap, Direction::Ltr, 10.0, &parts, width);
        let what = format!("{parts:?} in {strut:?}, {overflow_wrap:?}");
        assert_eq!(h, height, "{what}: height");
        assert_at(&what, mono_at(&placed, last.0), last.1);
    }
}

#[test]
fn overflow_wrap_anywhere_fits_a_sixty_fourth_past_the_width() {
    use WhiteSpace::Normal as N;
    for (width, height) in [(19.203125, 240.0), (19.1875, 240.0), (19.171875, 480.0)] {
        let (h, _) = mono_run(
            N,
            OverflowWrap::Anywhere,
            Direction::Ltr,
            16.0,
            &[(N, "abcdef")],
            width,
        );
        assert_eq!(h, height, "width {width}: height");
    }
}

#[test]
fn a_tab_takes_the_paragraph_direction_in_rtl() {
    use WhiteSpace::{Normal as N, Pre as P, PreWrap as W};
    for (parts, width, a, b) in [
        (
            &[(P, "a\tb")][..],
            100.0,
            (93.984375, 42.0),
            (81.96875, 42.0),
        ),
        (
            &[(W, "a\tb")][..],
            100.0,
            (93.984375, 42.0),
            (81.96875, 42.0),
        ),
        (
            &[(N, "a"), (P, "\tb")][..],
            100.0,
            (93.984375, 42.0),
            (81.96875, 42.0),
        ),
        (
            &[(P, "a\t"), (N, " b")][..],
            13.0,
            (6.984375, 42.0),
            (6.984375, 122.0),
        ),
    ] {
        let (_, placed) = mono_run(N, OverflowWrap::Normal, Direction::Rtl, 10.0, parts, width);
        let what = format!("{parts:?}");
        assert_at(&what, mono_at(&placed, "a"), a);
        assert_at(&what, mono_at(&placed, "b"), b);
    }
}

#[test]
fn a_lone_carriage_return_paints_nothing_and_takes_no_width() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    for (overflow_wrap, parts, width, height, b) in [
        (
            OverflowWrap::Normal,
            &[(W, "a\r"), (N, "\nb")][..],
            100.0,
            80.0,
            (12.015625, 42.0),
        ),
        (
            OverflowWrap::Anywhere,
            &[(W, "a\r"), (N, "\nb")][..],
            7.0,
            160.0,
            (0.0, 122.0),
        ),
        (
            OverflowWrap::Normal,
            &[(P, "a\rb")][..],
            100.0,
            80.0,
            (6.015625, 42.0),
        ),
    ] {
        let (h, placed) = mono_run(B, overflow_wrap, Direction::Ltr, 10.0, parts, width);
        let what = format!("{parts:?}, {overflow_wrap:?}");
        assert_eq!(h, height, "{what}: height");
        assert_at(&what, mono_at(&placed, "b"), b);
        let blank = mono_run(
            P,
            OverflowWrap::Normal,
            Direction::Ltr,
            10.0,
            &[(P, " ")],
            100.0,
        )
        .1[0]
            .0;
        let inked = placed.iter().filter(|g| g.0 != blank).count();
        assert_eq!(inked, 2, "{what}: painted glyphs {placed:?}");
    }
}

fn trimmed_line_end(strut: WhiteSpace, direction: Direction) -> (Vec<(f32, f32)>, f32) {
    let fonts = all_fonts();
    let cjk = |white_space| TextStyle {
        family: FamilyList::parse(SANS),
        size: 14.0,
        line_height: 80.0,
        color: hex("#ff0000"),
        background: Some(sosie_core::InlineBackground {
            padding_x: 0.0,
            padding_y: 0.0,
            radius: 0.0,
            color: hex("#0000ff"),
        }),
        ..style("Noto Sans", white_space)
    };
    let p = Paragraph {
        width: 21.0,
        strut: cjk(strut),
        overflow_wrap: OverflowWrap::Normal,
        direction,
    };
    let t = layout(
        &fonts,
        &p,
        &[Inline::Text {
            text: "中）文".to_string(),
            style: cjk(WhiteSpace::Normal),
        }],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let mut list = DisplayList::new(320.0, 200.0, hex("#ffffff"));
    t.paint(&mut list, 20.0, 0.0);
    let backgrounds = list
        .items
        .iter()
        .filter_map(|i| match i {
            Item::TextBackground { x0, x1, .. } => Some((*x0, *x1)),
            _ => None,
        })
        .collect();
    let first = list
        .items
        .iter()
        .find_map(|i| match i {
            Item::Glyphs { x, glyphs, .. } => glyphs.first().map(|g| x + g.x),
            _ => None,
        })
        .unwrap_or(f32::NAN);
    (backgrounds, first)
}

#[test]
fn a_wrapped_line_trims_its_end_bracket_under_a_pre_strut() {
    for strut in [WhiteSpace::Pre, WhiteSpace::Normal] {
        let (backgrounds, first) = trimmed_line_end(strut, Direction::Ltr);
        assert_eq!(
            backgrounds.first(),
            Some(&(20.0, 41.0)),
            "{strut:?}: background"
        );
        assert_eq!(first, 20.0, "{strut:?}: first glyph");
    }
}

#[test]
fn an_rtl_line_aligns_by_its_trimmed_width() {
    for strut in [WhiteSpace::Normal, WhiteSpace::Pre] {
        let (backgrounds, first) = trimmed_line_end(strut, Direction::Rtl);
        assert_eq!(
            backgrounds.first(),
            Some(&(20.0, 41.0)),
            "{strut:?}: background"
        );
        assert_eq!(first, 20.0, "{strut:?}: first glyph");
    }
}

#[test]
fn an_overridden_line_end_bracket_paints_inside_the_line() {
    let fonts = all_fonts();
    let red = TextStyle {
        size: 14.0,
        line_height: 80.0,
        color: hex("#ff0000"),
        ..style("Noto Sans", WhiteSpace::Normal)
    };
    let p = Paragraph {
        width: 21.0,
        strut: TextStyle {
            color: hex("#000000"),
            ..red.clone()
        },
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
    };
    let t = layout(
        &fonts,
        &p,
        &[Inline::Text {
            text: "\u{202e}中）文\u{202c}".to_string(),
            style: red,
        }],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let (_, ink) = painted(&fonts, &t, "#ff0000");
    assert!(
        ink.0.abs_diff(20) <= 1 && ink.1.abs_diff(39) <= 1,
        "ink x {ink:?}, Chromium (20, 39)"
    );
}

#[test]
fn terminal_whitespace_after_an_overflowing_pre_line_hangs() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    let tails: [(&[(WhiteSpace, &str)], f32); 10] = [
        (&[(W, " ")], 80.0),
        (&[(W, "  ")], 80.0),
        (&[(W, "\t")], 80.0),
        (&[(W, " \n")], 160.0),
        (&[(W, " "), (W, " ")], 80.0),
        (&[(W, " "), (N, " ")], 80.0),
        (&[(W, "\t"), (W, " ")], 80.0),
        (&[(W, " "), (W, "\t")], 80.0),
        (&[(W, " "), (N, " "), (W, "\t")], 80.0),
        (&[(W, " "), (W, " \n")], 160.0),
    ];
    for strut in [N, W, B] {
        for (tail, height) in tails {
            let parts: Vec<_> = [(P, "ab")].iter().chain(tail).copied().collect();
            let (h, _) = mono_run(
                strut,
                OverflowWrap::Anywhere,
                Direction::Ltr,
                10.0,
                &parts,
                7.0,
            );
            assert_eq!(h, height, "{parts:?} in {strut:?}: height");
        }
        for tail in [(P, " "), (W, "\u{200b}"), (W, "\u{2060}"), (W, "\u{202c}")] {
            for width in [1.0, 7.0] {
                let parts = [(P, "ab"), (W, " "), tail];
                let (h, _) = mono_run(
                    strut,
                    OverflowWrap::Anywhere,
                    Direction::Ltr,
                    10.0,
                    &parts,
                    width,
                );
                assert_eq!(h, 160.0, "{parts:?} in {strut:?} at width {width}: height");
            }
        }
    }
}

#[test]
fn whitespace_before_a_forced_break_in_the_next_span_does_not_hang() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    let tails: [(Parts, [f32; 2]); 20] = [
        (&[(W, " "), (W, "\n")], [160.0, 160.0]),
        (&[(W, "\t"), (W, "\n")], [160.0, 160.0]),
        (&[(W, " "), (W, "\nc")], [240.0, 240.0]),
        (&[(W, "\t"), (W, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (W, " "), (W, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (W, "\r\nc")], [240.0, 240.0]),
        (&[(W, " "), (P, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (B, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (N, " "), (W, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (N, "\t\t"), (W, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (N, "\n"), (W, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (N, "\r"), (W, "\nc")], [240.0, 240.0]),
        (
            &[(W, " "), (N, " "), (N, "\t\t"), (N, "\n"), (W, "\nc")],
            [240.0, 240.0],
        ),
        (&[(W, " "), (P, " "), (W, "\nc")], [240.0, 240.0]),
        (&[(W, " "), (B, " "), (W, "\n")], [240.0, 160.0]),
        (&[(W, " "), (B, " "), (W, "\nc")], [320.0, 240.0]),
        (&[(W, " "), (B, "\t\t"), (W, "\nc")], [400.0, 320.0]),
        (&[(W, " "), (B, " "), (N, " "), (W, "\nc")], [320.0, 240.0]),
        (
            &[(W, " "), (N, " "), (B, " "), (N, " "), (W, "\nc")],
            [320.0, 240.0],
        ),
        (
            &[(W, " "), (B, " "), (B, " "), (B, " "), (W, "\nc")],
            [480.0, 400.0],
        ),
    ];
    for strut in [N, W, B] {
        for direction in [Direction::Ltr, Direction::Rtl] {
            for (width, column) in [(1.0, 0), (7.0, 1)] {
                for (tail, heights) in tails {
                    let height = heights[column];
                    let parts: Vec<_> = [(P, "ab")].iter().chain(tail).copied().collect();
                    let (h, placed) = mono_run(
                        strut,
                        OverflowWrap::Anywhere,
                        direction,
                        10.0,
                        &parts,
                        width,
                    );
                    let what = format!("{parts:?} in {strut:?} {direction:?} at width {width}");
                    assert_eq!(h, height, "{what}: height");
                    if parts.last().is_some_and(|p| p.1.ends_with('c')) {
                        let x = if matches!(direction, Direction::Rtl) {
                            width - 6.015625
                        } else {
                            0.0
                        };
                        assert_at(&what, mono_at(&placed, "c"), (x, height - 38.0));
                    }
                }
            }
        }
    }
    type Spans = &'static [(WhiteSpace, &'static str, f32)];
    type Mixed = ((WhiteSpace, f32), OverflowWrap, f32, Spans, (f32, f32));
    let mixed: [Mixed; 28] = [
        (
            (W, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(N, "a", 80.0), (B, " ", 80.0), (W, "\nc", 80.0)],
            (160.0, 122.0),
        ),
        (
            (W, 80.0),
            OverflowWrap::Normal,
            7.0,
            &[(N, "a", 80.0), (B, " ", 80.0), (W, "\nc", 80.0)],
            (240.0, 202.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(N, "a", 80.0), (B, " ", 80.0), (W, "\nc", 80.0)],
            (320.0, 282.0),
        ),
        (
            (P, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(N, "a", 80.0), (B, " ", 80.0), (W, "\nc", 80.0)],
            (320.0, 282.0),
        ),
        (
            (B, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(N, "a", 80.0), (B, " ", 80.0), (W, "\nc", 80.0)],
            (160.0, 122.0),
        ),
        (
            (W, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(N, "a", 80.0), (B, " ", 80.0), (W, "c", 80.0)],
            (160.0, 122.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[
                (B, "a\t", 80.0),
                (W, "\t", 80.0),
                (W, " ", 80.0),
                (N, " ", 80.0),
                (W, "\n\nc", 80.0),
            ],
            (240.0, 202.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[
                (B, "a\t", 80.0),
                (W, " ", 80.0),
                (N, " ", 80.0),
                (W, "\nc", 80.0),
            ],
            (160.0, 122.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(B, "a\t", 80.0), (W, " ", 80.0), (W, "\nc", 80.0)],
            (160.0, 122.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[
                (B, "a\t", 80.0),
                (W, " ", 80.0),
                (N, "", 80.0),
                (W, "\nc", 80.0),
            ],
            (160.0, 122.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[
                (B, "a\t", 80.0),
                (W, "\t", 80.0),
                (W, " ", 80.0),
                (N, " ", 80.0),
                (W, "\nc", 80.0),
            ],
            (160.0, 122.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(B, "a\t", 80.0), (N, " ", 80.0), (W, "\nc", 80.0)],
            (160.0, 122.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[
                (B, "a\t", 80.0),
                (W, " ", 80.0),
                (P, " ", 80.0),
                (W, "\nc", 80.0),
            ],
            (240.0, 202.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Anywhere,
            1.0,
            &[
                (P, "ab", 80.0),
                (W, " ", 80.0),
                (B, " ", 80.0),
                (W, "\nc", 80.0),
            ],
            (320.0, 282.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Anywhere,
            1.0,
            &[(P, "ab", 80.0), (B, " ", 80.0), (W, "\nc", 80.0)],
            (320.0, 282.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Anywhere,
            1.0,
            &[(P, "ab", 80.0), (W, " ", 80.0), (B, " \nc", 80.0)],
            (240.0, 202.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Anywhere,
            1.0,
            &[
                (P, "ab", 80.0),
                (W, " ", 80.0),
                (B, " ", 80.0),
                (W, "\nc", 80.0),
            ],
            (320.0, 282.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Anywhere,
            1.0,
            &[(B, " \nc", 80.0)],
            (240.0, 202.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Anywhere,
            1.0,
            &[(B, " ", 80.0), (W, "\nc", 80.0)],
            (240.0, 202.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Anywhere,
            1.0,
            &[(B, " ", 80.0), (W, "\nc", 120.0)],
            (320.0, 262.0),
        ),
        (
            (N, 0.0),
            OverflowWrap::Anywhere,
            1.0,
            &[
                (P, "ab", 16.0),
                (W, " ", 16.0),
                (N, " ", 16.0),
                (W, "\nc", 0.0),
            ],
            (16.0, 18.0),
        ),
        (
            (N, 0.0),
            OverflowWrap::Anywhere,
            1.0,
            &[
                (P, "ab", 16.0),
                (W, " ", 16.0),
                (N, " ", 16.0),
                (W, "\nc", 16.0),
            ],
            (48.0, 42.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Normal,
            1.0,
            &[(W, " ", 16.0), (W, "\nc", 80.0)],
            (160.0, 122.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Normal,
            1.0,
            &[(P, "ab", 16.0), (B, " \nc", 16.0)],
            (32.0, 26.0),
        ),
        (
            (N, 80.0),
            OverflowWrap::Normal,
            1.0,
            &[(B, " ", 80.0), (W, "\rc", 80.0)],
            (160.0, 122.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Normal,
            1.0,
            &[(N, "a", 16.0), (B, " ", 16.0), (W, " \nc", 80.0)],
            (192.0, 154.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Anywhere,
            1.0,
            &[(P, "ab", 16.0), (W, " \nc", 80.0)],
            (176.0, 138.0),
        ),
        (
            (N, 16.0),
            OverflowWrap::Normal,
            1.0,
            &[(P, "ab", 16.0), (B, " ", 16.0), (W, " \nc", 80.0)],
            (160.0, 122.0),
        ),
    ];
    for (strut, overflow_wrap, width, parts, (height, baseline)) in mixed {
        let (h, placed) = mono_spans(strut, overflow_wrap, Direction::Ltr, 10.0, parts, width);
        let what = format!("{parts:?} in {strut:?} {overflow_wrap:?} at width {width}");
        assert_eq!(h, height, "{what}: height");
        assert_at(&what, mono_at(&placed, "c"), (0.0, baseline));
    }
    for (line_height, width, padding_x, (height, baseline)) in [
        (16.0, 1.0, 1.015625, (64.0, 58.0)),
        (16.0, 1.0, 1.03125, (48.0, 42.0)),
        (16.0, 1.0, 3.0, (48.0, 42.0)),
        (16.0, 7.0, 7.0, (64.0, 58.0)),
        (16.0, 7.0, 8.0, (48.0, 42.0)),
        (16.0, 13.0, 13.0, (64.0, 58.0)),
        (16.0, 13.0, 16.0, (48.0, 42.0)),
        (80.0, 1.0, 3.0, (240.0, 202.0)),
    ] {
        let parts = [
            (N, "a", line_height, None),
            (B, " ", line_height, Some(padding_x)),
            (W, "\nc", line_height, None),
        ];
        let (h, placed) = mono_padded(
            (N, line_height),
            OverflowWrap::Normal,
            Direction::Ltr,
            10.0,
            &parts,
            width,
        );
        let what = format!("{parts:?} at width {width}");
        assert_eq!(h, height, "{what}: height");
        assert_at(&what, mono_at(&placed, "c"), (0.0, baseline));
    }
    type Padded = &'static [(WhiteSpace, &'static str, f32, Option<f32>)];
    let padded: [(f32, Padded, (f32, f32)); 14] = [
        (0.75, &[(B, " \nc", 16.0, Some(0.0))], (48.0, 42.0)),
        (0.75, &[(B, "  \nc", 16.0, Some(0.0))], (64.0, 58.0)),
        (1.25, &[(B, "\t \nc", 16.0, None)], (48.0, 42.0)),
        (1.25, &[(B, "a   \nc", 16.0, None)], (64.0, 58.0)),
        (0.75, &[(B, " \u{200b} \nc", 16.0, None)], (48.0, 42.0)),
        (1.25, &[(B, "   \nc", 16.0, None)], (80.0, 74.0)),
        (7.25, &[(B, "   \nc", 16.0, None)], (64.0, 58.0)),
        (
            1.25,
            &[
                (B, " ", 16.0, Some(1.265625)),
                (B, "  \nc", 16.0, Some(1.265625)),
            ],
            (80.0, 74.0),
        ),
        (
            1.25,
            &[
                (B, " ", 16.0, Some(1.28125)),
                (B, "  \nc", 16.0, Some(1.28125)),
            ],
            (64.0, 58.0),
        ),
        (
            1.0,
            &[(B, " ", 16.0, None), (B, " \nc", 16.0, None)],
            (48.0, 42.0),
        ),
        (
            0.75,
            &[(N, "a", 16.0, None), (B, " \nc", 16.0, Some(0.0))],
            (48.0, 42.0),
        ),
        (
            6.5,
            &[
                (N, "a", 16.0, None),
                (B, "\t", 16.0, Some(6.515625)),
                (W, " \nc", 16.0, None),
            ],
            (64.0, 58.0),
        ),
        (
            6.5,
            &[
                (N, "a", 16.0, None),
                (B, "\t", 16.0, Some(6.53125)),
                (W, " \nc", 16.0, None),
            ],
            (48.0, 42.0),
        ),
        (
            6.5,
            &[
                (N, "a", 16.0, None),
                (B, "\t", 16.0, Some(6.53125)),
                (W, " ", 16.0, None),
                (W, "\nc", 16.0, None),
            ],
            (48.0, 42.0),
        ),
    ];
    for (width, parts, (height, baseline)) in padded {
        let (h, placed) = mono_padded(
            (N, 16.0),
            OverflowWrap::Normal,
            Direction::Ltr,
            10.0,
            parts,
            width,
        );
        let what = format!("{parts:?} at width {width}");
        assert_eq!(h, height, "{what}: height");
        assert_at(&what, mono_at(&placed, "c"), (0.0, baseline));
    }
}

type ChromiumRow = (
    ((WhiteSpace, f32), OverflowWrap, Direction),
    f32,
    Vec<Inline>,
    f32,
    (&'static str, (f32, f32)),
);

fn padded(
    white_space: WhiteSpace,
    t: &str,
    size: f32,
    line_height: f32,
    padding_x: Option<f32>,
) -> Inline {
    Inline::Text {
        text: t.to_string(),
        style: TextStyle {
            family: FamilyList::parse(MONO),
            size,
            line_height,
            tab_size: 1,
            background: padding_x.map(|padding_x| sosie_core::InlineBackground {
                padding_x,
                padding_y: 0.0,
                radius: 0.0,
                color: hex("#000000"),
            }),
            ..style(MONO, white_space)
        },
    }
}

fn inline_box(width: f32, height: f32) -> Inline {
    Inline::Box(InlineBox { width, height })
}

fn assert_chromium_rows(rows: impl IntoIterator<Item = ChromiumRow>) {
    use WhiteSpace::Pre as P;
    for (((strut_ws, strut), overflow_wrap, direction), width, inlines, height, c) in rows {
        let p = Paragraph {
            width,
            strut: TextStyle {
                family: FamilyList::parse(MONO),
                size: 10.0,
                line_height: strut,
                tab_size: 1,
                ..style(MONO, strut_ws)
            },
            overflow_wrap,
            direction,
        };
        let t = layout(shared_fonts(), &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
        let what = format!("{} inlines {direction:?} at width {width}", inlines.len());
        assert_eq!(t.height, height, "{what}: height");
        let (ch, at) = c;
        if ch == "box" {
            let last = t
                .lines
                .iter()
                .flat_map(|l| &l.boxes)
                .map(|b| (b.x, b.y))
                .max_by(|p, q| p.1.total_cmp(&q.1))
                .unwrap_or((f32::NAN, f32::NAN));
            assert_at(&format!("{what}: last box"), last, at);
            continue;
        }
        let mut list = DisplayList::new(100.0, 400.0, hex("#ffffff"));
        t.paint(&mut list, 0.0, 0.0);
        let (_, ids) = mono_run(
            P,
            OverflowWrap::Normal,
            Direction::Ltr,
            10.0,
            &[(P, ch)],
            100.0,
        );
        let last = list
            .items
            .iter()
            .flat_map(|i| match i {
                Item::Glyphs {
                    x,
                    baseline,
                    glyphs,
                    ..
                } => glyphs
                    .iter()
                    .filter(|g| g.id == ids[0].0)
                    .map(|g| (x + g.x, baseline + g.y))
                    .collect(),
                _ => Vec::new(),
            })
            .max_by(|p, q| p.1.total_cmp(&q.1))
            .unwrap_or((f32::NAN, f32::NAN));
        assert_at(&format!("{what}: last {ch}"), last, at);
    }
}

#[test]
fn padding_tabs_and_collapsed_spans_break_like_chromium() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    let (text, inline_box) = (padded, inline_box);
    let (any, ltr) = (OverflowWrap::Anywhere, Direction::Ltr);
    let rows = [
        (
            ((N, 16.0), any, ltr),
            5.984375,
            vec![
                text(N, " ", 10.0, 16.0, Some(6.5)),
                text(B, "\na", 10.0, 16.0, Some(6.53125)),
                text(P, "c", 10.0, 16.0, None),
            ],
            48.0,
            ("c", (0.0, 42.0)),
        ),
        (
            ((N, 16.0), any, ltr),
            19.0,
            vec![
                inline_box(0.0, 6.0),
                text(B, " \r \n", 10.0, 80.0, None),
                text(N, "  \n", 10.0, 16.0, Some(0.5)),
                text(B, "\ta", 10.0, 80.0, None),
                text(B, "c", 10.0, 16.0, None),
            ],
            160.0,
            ("c", (12.03125, 122.0)),
        ),
        (
            ((N, 16.0), any, ltr),
            7.0,
            vec![
                text(N, "a", 10.0, 16.0, None),
                text(B, "\t", 0.0, 16.0, None),
                text(W, "\n", 10.0, 80.0, None),
                text(N, "c", 10.0, 16.0, None),
            ],
            112.0,
            ("c", (0.0, 106.0)),
        ),
        (
            ((N, 80.0), OverflowWrap::Normal, Direction::Rtl),
            12.0,
            vec![
                inline_box(6.0, 6.0),
                text(N, "\na \na", 10.0, 16.0, None),
                text(W, "\r\n", 10.0, 16.0, Some(6.53125)),
                text(P, "\ta ", 10.0, 16.0, Some(3.0)),
                inline_box(6.0, 6.0),
                text(P, "\nc", 10.0, 16.0, None),
            ],
            400.0,
            ("c", (5.984375, 362.0)),
        ),
        (
            ((P, 16.0), any, ltr),
            6.015625,
            vec![
                text(P, "  a", 10.0, 16.0, None),
                text(B, "\u{200b}\u{200b}\n", 10.0, 16.0, None),
                text(B, "a\u{200b}", 10.0, 80.0, None),
                inline_box(0.0, 6.0),
            ],
            96.0,
            ("a", (0.0, 58.0)),
        ),
    ];
    assert_chromium_rows(rows);
}

#[test]
fn zero_width_space_segment_breaks_collapse_like_chromium() {
    use WhiteSpace::Normal as N;
    let text = padded;
    assert_chromium_rows([
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Ltr),
            400.0,
            vec![text(N, "x\u{200b} \ny", 10.0, 16.0, None)],
            16.0,
            ("y", (6.0, 10.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Ltr),
            400.0,
            vec![
                text(N, "x\u{200b}", 10.0, 16.0, None),
                text(N, " \ny", 10.0, 16.0, None),
            ],
            16.0,
            ("y", (6.015625, 10.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Ltr),
            400.0,
            vec![
                text(N, "x\u{200b} ", 10.0, 16.0, None),
                text(N, "\ny", 10.0, 16.0, None),
            ],
            16.0,
            ("y", (6.015625, 10.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Ltr),
            400.0,
            vec![
                text(N, "x \n", 10.0, 16.0, None),
                text(N, "\u{200b}y", 10.0, 16.0, None),
            ],
            16.0,
            ("y", (6.015625, 10.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Ltr),
            400.0,
            vec![
                text(N, "x\u{200b} ", 10.0, 16.0, None),
                text(N, "\n", 10.0, 16.0, None),
                text(N, "\n", 10.0, 16.0, None),
                text(N, "y", 10.0, 16.0, None),
            ],
            16.0,
            ("y", (12.015625, 10.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Ltr),
            400.0,
            vec![
                text(N, "x\u{200b} ", 10.0, 16.0, None),
                text(N, "\n", 10.0, 16.0, Some(6.0)),
                text(N, "y", 10.0, 16.0, None),
            ],
            16.0,
            ("y", (24.015625, 10.0)),
        ),
    ]);
}

#[test]
fn padding_of_empty_and_trimmed_spans_follows_chromium() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    let (text, inline_box) = (padded, inline_box);
    let any = OverflowWrap::Anywhere;
    assert_chromium_rows([
        (
            ((P, 80.0), any, Direction::Ltr),
            24.0,
            vec![
                text(N, " \n", 10.0, 16.0, Some(0.5)),
                text(B, "c", 10.0, 16.0, None),
            ],
            80.0,
            ("c", (1.0, 42.0)),
        ),
        (
            ((N, 80.0), OverflowWrap::Normal, Direction::Rtl),
            17.5,
            vec![
                text(N, "\n", 10.0, 16.0, Some(0.5)),
                text(P, " a ", 10.0, 80.0, None),
                text(W, "\nc", 10.0, 16.0, None),
            ],
            240.0,
            ("a", (5.46875, 122.0)),
        ),
        (
            ((N, 80.0), any, Direction::Rtl),
            18.015625,
            vec![
                text(N, " ", 10.0, 16.0, Some(6.53125)),
                text(N, " \r\n", 10.0, 16.0, Some(6.5)),
                text(W, " c", 10.0, 16.0, None),
            ],
            160.0,
            ("c", (5.984375, 122.0)),
        ),
        (
            ((B, 16.0), OverflowWrap::Normal, Direction::Rtl),
            12.25,
            vec![
                text(N, "\n", 10.0, 80.0, Some(6.53125)),
                text(W, " a ", 10.0, 16.0, None),
            ],
            96.0,
            ("a", (6.234375, 90.0)),
        ),
        (
            ((P, 16.0), any, Direction::Rtl),
            12.015625,
            vec![text(W, "\na", 10.0, 16.0, Some(3.0))],
            32.0,
            ("a", (6.0, 26.0)),
        ),
        (
            ((P, 16.0), any, Direction::Rtl),
            25.0,
            vec![
                text(N, "  \n", 10.0, 16.0, Some(3.0)),
                inline_box(6.0, 20.0),
            ],
            26.0,
            ("box", (13.0, 0.0)),
        ),
        (
            ((B, 80.0), OverflowWrap::Normal, Direction::Rtl),
            5.5,
            vec![
                inline_box(1.0, 20.0),
                text(W, "  x", 10.0, 16.0, Some(6.53125)),
                text(N, "a ", 10.0, 16.0, Some(6.53125)),
                text(B, "\na\na", 10.0, 80.0, None),
                text(W, "\n\n", 10.0, 16.0, Some(3.0)),
                text(P, "c", 10.0, 16.0, None),
            ],
            560.0,
            ("x", (-19.59375, 202.0)),
        ),
        (
            ((B, 16.0), any, Direction::Rtl),
            18.0,
            vec![
                text(N, " ", 10.0, 80.0, Some(3.0)),
                text(N, "c", 10.0, 16.0, None),
            ],
            80.0,
            ("c", (11.984375, 42.0)),
        ),
    ]);
}

#[test]
fn carriage_returns_and_removed_spaces_take_chromium_levels() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    let text = padded;
    let any = OverflowWrap::Anywhere;
    assert_chromium_rows([
        (
            ((W, 16.0), OverflowWrap::Normal, Direction::Rtl),
            24.0,
            vec![
                text(B, "aa\r", 10.0, 16.0, None),
                text(W, "c", 10.0, 16.0, None),
            ],
            16.0,
            ("c", (17.984375, 10.0)),
        ),
        (
            ((P, 16.0), OverflowWrap::Normal, Direction::Rtl),
            13.0,
            vec![
                text(P, "a\n\r", 10.0, 16.0, Some(6.53125)),
                text(P, "x", 10.0, 16.0, None),
                text(P, " a\r\r", 10.0, 16.0, Some(6.5)),
                text(W, " \n", 10.0, 16.0, None),
                text(P, " c", 10.0, 16.0, None),
            ],
            48.0,
            ("x", (-18.03125, 26.0)),
        ),
        (
            ((B, 16.0), any, Direction::Rtl),
            0.25,
            vec![
                text(N, "\r\u{200b}x", 10.0, 16.0, None),
                text(P, "\raa\t ", 10.0, 16.0, Some(6.5)),
                text(P, "\u{200b}\u{200b}", 10.0, 16.0, None),
                text(N, "\raa\n", 10.0, 16.0, None),
            ],
            80.0,
            ("x", (-5.765625, 26.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Rtl),
            40.0,
            vec![
                text(N, "\n", 10.0, 16.0, Some(3.0)),
                text(W, "\u{200b}", 10.0, 16.0, None),
                text(P, "ab", 10.0, 16.0, None),
            ],
            16.0,
            ("b", (33.984375, 10.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Rtl),
            40.0,
            vec![
                text(W, "\u{200b}", 10.0, 16.0, Some(3.0)),
                text(P, "ab", 10.0, 16.0, None),
            ],
            16.0,
            ("b", (33.984375, 10.0)),
        ),
        (
            ((P, 80.0), OverflowWrap::Normal, Direction::Rtl),
            24.0,
            vec![
                text(B, "\n\r", 10.0, 16.0, Some(3.0)),
                text(N, "\nc", 10.0, 16.0, None),
            ],
            160.0,
            ("c", (17.984375, 122.0)),
        ),
        (
            ((B, 16.0), OverflowWrap::Normal, Direction::Ltr),
            12.25,
            vec![
                text(B, "\na", 10.0, 16.0, Some(6.53125)),
                text(N, "\nc", 10.0, 16.0, None),
            ],
            48.0,
            ("c", (0.0, 42.0)),
        ),
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Rtl),
            5.5,
            vec![
                text(B, "\ra\na", 10.0, 16.0, None),
                text(N, " c", 10.0, 16.0, None),
            ],
            48.0,
            ("c", (-0.515625, 42.0)),
        ),
        (
            ((N, 16.0), any, Direction::Rtl),
            6.015625,
            vec![
                text(P, " a ", 10.0, 16.0, None),
                text(N, "\r\u{200b}", 10.0, 80.0, None),
                inline_box(1.0, 20.0),
                text(W, "a\u{200b} \n\n", 10.0, 80.0, None),
                text(N, "c", 10.0, 16.0, None),
            ],
            272.0,
            ("c", (0.0, 266.0)),
        ),
        (
            ((W, 80.0), any, Direction::Rtl),
            11.984375,
            vec![
                text(W, "\t", 10.0, 80.0, None),
                text(B, " \n", 10.0, 16.0, None),
                text(N, "\t", 10.0, 80.0, Some(6.53125)),
                text(P, " c", 10.0, 16.0, None),
            ],
            240.0,
            ("c", (-13.109375, 202.0)),
        ),
    ]);
}

#[test]
fn lf_line_metrics_and_crlf_splits_follow_chromium() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    let text = padded;
    let any = OverflowWrap::Anywhere;
    assert_chromium_rows([
        (
            ((N, 16.0), OverflowWrap::Normal, Direction::Ltr),
            12.015625,
            vec![
                text(B, "\t\t\t \r", 10.0, 16.0, Some(6.5)),
                text(P, "\nc", 10.0, 16.0, None),
            ],
            80.0,
            ("c", (0.0, 74.0)),
        ),
        (
            ((B, 80.0), any, Direction::Ltr),
            6.25,
            vec![
                text(B, "  ", 10.0, 80.0, None),
                text(P, "a\n\t\r", 10.0, 80.0, None),
                text(W, "\n", 10.0, 16.0, Some(3.0)),
                text(N, "c", 10.0, 16.0, None),
            ],
            400.0,
            ("c", (0.0, 362.0)),
        ),
        (
            ((W, 16.0), any, Direction::Ltr),
            5.5,
            vec![
                text(P, "\r", 10.0, 16.0, Some(6.53125)),
                text(W, "\n\t\t ", 10.0, 16.0, None),
                text(B, "a  \t", 10.0, 16.0, None),
            ],
            96.0,
            ("a", (0.0, 42.0)),
        ),
        (
            ((P, 16.0), any, Direction::Rtl),
            0.25,
            vec![text(W, "\na", 10.0, 80.0, None)],
            160.0,
            ("a", (-5.765625, 122.0)),
        ),
    ]);
}

#[test]
fn an_inline_box_before_a_break_spaces_space_keeps_its_line() {
    use WhiteSpace::{BreakSpaces as B, Normal as N, Pre as P, PreWrap as W};
    let mono = |white_space, line_height| TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    let text = |white_space, t: &str, line_height| Inline::Text {
        text: t.to_string(),
        style: mono(white_space, line_height),
    };
    for head in [
        text(N, "a", 16.0),
        text(P, "ab", 16.0),
        text(B, "a\t", 16.0),
    ] {
        let inlines = vec![
            head,
            Inline::Box(InlineBox {
                width: 6.0,
                height: 20.0,
            }),
            text(B, " ", 16.0),
            text(W, "\n", 80.0),
            text(N, "c", 16.0),
        ];
        let p = Paragraph {
            width: 1.0,
            strut: mono(W, 16.0),
            overflow_wrap: OverflowWrap::Normal,
            direction: Direction::Ltr,
        };
        let t = layout(shared_fonts(), &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
        let what = format!("{:?}", inlines[0]);
        assert_eq!(t.height, 154.0, "{what}: height");
        let placed = t
            .lines
            .iter()
            .find_map(|l| l.boxes.first().map(|b| (b.x, b.y)));
        assert_eq!(placed, Some((0.0, 16.0)), "{what}: box");
    }
}

#[test]
fn an_inline_box_before_pre_whitespace_breaks_like_chromium() {
    use WhiteSpace::{Normal as N, Pre as P};
    let mono = |white_space, size, line_height| TextStyle {
        family: FamilyList::parse(MONO),
        size,
        line_height,
        tab_size: 1,
        ..style(MONO, white_space)
    };
    let inline_box = |side| {
        Inline::Box(InlineBox {
            width: side,
            height: side,
        })
    };
    let text = |white_space, t: &str| Inline::Text {
        text: t.to_string(),
        style: mono(white_space, 10.0, 80.0),
    };
    for (strut, inlines, width, height, b, at) in [
        (
            (10.0, 80.0),
            vec![inline_box(0.0), text(P, " b")],
            7.0,
            160.0,
            (6.0, 122.0),
            (0.0, 42.0),
        ),
        (
            (10.0, 80.0),
            vec![text(N, "a"), inline_box(0.0), text(P, " b")],
            13.0,
            160.0,
            (6.0, 122.0),
            (6.015625, 42.0),
        ),
        (
            (10.0, 80.0),
            vec![inline_box(6.0), text(P, " b")],
            7.0,
            160.0,
            (6.0, 122.0),
            (0.0, 36.0),
        ),
        (
            (0.0, 0.0),
            vec![inline_box(6.0), text(P, " b")],
            7.0,
            86.0,
            (6.0, 48.0),
            (0.0, 0.0),
        ),
        (
            (10.0, 0.0),
            vec![inline_box(6.0), text(P, " b")],
            7.0,
            86.0,
            (6.0, 48.0),
            (0.0, 0.0),
        ),
        (
            (10.0, 0.0),
            vec![inline_box(0.0), text(P, " b")],
            7.0,
            82.0,
            (6.0, 44.0),
            (0.0, 2.0),
        ),
    ] {
        let p = Paragraph {
            width,
            strut: mono(N, strut.0, strut.1),
            overflow_wrap: OverflowWrap::Normal,
            direction: Direction::Ltr,
        };
        let t = layout(&all_fonts(), &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
        let mut list = DisplayList::new(100.0, 400.0, hex("#ffffff"));
        t.paint(&mut list, 0.0, 0.0);
        let got = list
            .items
            .iter()
            .filter_map(|i| match i {
                Item::Glyphs {
                    x,
                    baseline,
                    glyphs,
                    ..
                } => glyphs.last().map(|g| (x + g.x, baseline + g.y)),
                _ => None,
            })
            .next_back()
            .unwrap_or((f32::NAN, f32::NAN));
        let what = format!(
            "{} inlines under a {strut:?} strut at width {width}",
            inlines.len()
        );
        assert_eq!(t.height, height, "{what}: height");
        assert_at(&what, got, b);
        let placed = t
            .lines
            .iter()
            .find_map(|l| l.boxes.first().map(|b| (b.x, b.y)));
        assert!(
            placed.is_some_and(|(x, y)| (x - at.0).abs() <= 1.0 / 64.0 && y == at.1),
            "{what}: box at {placed:?}, Chromium {at:?}"
        );
    }
}

#[test]
fn many_preserved_newlines_lay_out_in_linear_time() {
    let fonts = all_fonts();
    let st = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 16.0,
        ..style(MONO, WhiteSpace::Pre)
    };
    let p = Paragraph {
        width: 100.0,
        strut: st.clone(),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
    };
    let start = std::time::Instant::now();
    let t = layout(
        &fonts,
        &p,
        &[Inline::Text {
            text: "\n".repeat(800_000),
            style: st,
        }],
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let elapsed = start.elapsed();
    assert_eq!(t.lines.len(), 800_000);
    assert!(
        elapsed < std::time::Duration::from_secs(10),
        "800,000 newlines took {elapsed:?}"
    );
}

#[test]
fn long_carriage_return_runs_scale_linearly() {
    let fonts = shared_fonts();
    let st = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 16.0,
        ..style(MONO, WhiteSpace::Pre)
    };
    let p = Paragraph {
        width: 100.0,
        strut: st.clone(),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Rtl,
    };
    assert_scales_linearly("carriage returns", 20_000, |n| {
        let inlines = [Inline::Text {
            text: format!("a{}b", "\r".repeat(n)),
            style: st.clone(),
        }];
        let start = std::time::Instant::now();
        let t = layout(fonts, &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
        let elapsed = start.elapsed();
        assert_eq!(t.lines.len(), 1);
        elapsed
    });
}

#[test]
fn long_lines_and_unbreakable_words_lay_out_in_linear_time() {
    let fonts = all_fonts();
    let st = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 16.0,
        ..style(MONO, WhiteSpace::Normal)
    };
    let tiny_cjk = TextStyle {
        family: FamilyList::parse("Noto Sans CJK JP"),
        size: 0.01,
        ..st.clone()
    };
    let text = |text: String, style: &TextStyle| Inline::Text {
        text,
        style: style.clone(),
    };
    for (inlines, width, lines) in [
        (vec![text("a".repeat(150_000), &st)], 7.0, 150_000),
        (vec![text("a ".repeat(100_000), &st)], 1.0e9, 1),
        (
            vec![
                text("a".repeat(100_000), &st),
                text("。。".into(), &tiny_cjk),
            ],
            7.0,
            100_000,
        ),
    ] {
        let p = Paragraph {
            width,
            strut: st.clone(),
            overflow_wrap: OverflowWrap::Anywhere,
            direction: Direction::Ltr,
        };
        let start = std::time::Instant::now();
        let t = layout(&fonts, &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
        let elapsed = start.elapsed();
        assert_eq!(t.lines.len(), lines);
        assert!(
            elapsed < std::time::Duration::from_secs(10),
            "{lines} lines at width {width} took {elapsed:?}"
        );
    }
}

#[test]
fn trimmed_cjk_punctuation_never_moves_the_pen_backwards() {
    let st = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 16.0,
        ..style(MONO, WhiteSpace::Normal)
    };
    let cjk = |size| TextStyle {
        family: FamilyList::parse("Noto Sans CJK JP"),
        size,
        ..st.clone()
    };
    let text = |text: &str, style: &TextStyle| Inline::Text {
        text: text.to_string(),
        style: style.clone(),
    };
    let para = |width| Paragraph {
        width,
        strut: st.clone(),
        overflow_wrap: OverflowWrap::Anywhere,
        direction: Direction::Ltr,
    };
    for size in [0.01, 0.02, 0.05] {
        let inlines = [text("x", &st), text("。。", &cjk(size)), text("y", &st)];
        let t = layout(shared_fonts(), &para(400.0), &inlines).unwrap_or_else(|e| panic!("{e}"));
        let mut list = DisplayList::new(400.0, 100.0, hex("#ffffff"));
        t.paint(&mut list, 0.0, 0.0);
        let xs: Vec<f32> = list
            .items
            .iter()
            .flat_map(|i| match i {
                Item::Glyphs { x, glyphs, .. } => glyphs.iter().map(|g| x + g.x).collect(),
                _ => Vec::new(),
            })
            .collect();
        assert!(
            xs.len() == 4 && xs.windows(2).all(|w| w[0] <= w[1]),
            "x, two full stops and y at size {size}: glyphs at {xs:?}"
        );
    }
    let inlines = [
        text(&"a".repeat(100), &st),
        text(&"。".repeat(50_000), &cjk(0.01)),
    ];
    let t = layout(shared_fonts(), &para(350.03125), &inlines).unwrap_or_else(|e| panic!("{e}"));
    for line in &t.lines {
        let ascii = t.line_text(line).matches('a').count();
        assert!(
            line.advance >= 6.0 * ascii as f32,
            "a line with {ascii} \"a\" has advance {}",
            line.advance
        );
    }
}

fn assert_linear(
    what: &str,
    mut witness: impl FnMut() -> std::time::Duration,
    mut control: impl FnMut() -> std::time::Duration,
) {
    let mut fastest = (std::time::Duration::MAX, std::time::Duration::MAX);
    for _ in 0..5 {
        fastest.1 = fastest.1.min(control());
        fastest.0 = fastest.0.min(witness());
        if fastest.0 < fastest.1 * 4 {
            return;
        }
    }
    panic!(
        "{what} took at best {:?} against its linear control's {:?}",
        fastest.0, fastest.1
    );
}

fn assert_scales_linearly(what: &str, n: usize, mut run: impl FnMut(usize) -> std::time::Duration) {
    let mut fastest = (std::time::Duration::MAX, std::time::Duration::MAX);
    for _ in 0..5 {
        fastest.0 = fastest.0.min(run(n));
        fastest.1 = fastest.1.min(run(4 * n));
        if fastest.1 < fastest.0 * 8 {
            return;
        }
    }
    panic!(
        "{what} took at best {:?} at {} units against {:?} at {n}",
        fastest.1,
        4 * n,
        fastest.0
    );
}

#[test]
fn spread_negative_trims_and_many_spans_cost_like_linear_controls() {
    let st = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 16.0,
        ..style(MONO, WhiteSpace::Normal)
    };
    let tiny_mono = TextStyle {
        size: 1.0 / 64.0,
        ..st.clone()
    };
    let cjk = |size| TextStyle {
        family: FamilyList::parse("Noto Sans CJK JP"),
        size,
        ..st.clone()
    };
    let text = |text: &str, style: &TextStyle| Inline::Text {
        text: text.to_string(),
        style: style.clone(),
    };
    let spread = |size| -> Vec<Inline> {
        (0..50_000)
            .flat_map(|_| {
                [
                    text("a\u{2060}", &tiny_mono),
                    text("。。\u{2060}", &cjk(size)),
                ]
            })
            .collect()
    };
    let pre_wrap = TextStyle {
        white_space: WhiteSpace::PreWrap,
        ..st.clone()
    };
    let timed = |inlines: &[Inline], strut: &TextStyle, width: f32, lines: usize| {
        let p = Paragraph {
            width,
            strut: strut.clone(),
            overflow_wrap: OverflowWrap::Anywhere,
            direction: Direction::Ltr,
        };
        let fonts = shared_fonts();
        let start = std::time::Instant::now();
        let t = layout(fonts, &p, inlines).unwrap_or_else(|e| panic!("{e}"));
        let elapsed = start.elapsed();
        assert_eq!(t.lines.len(), lines);
        elapsed
    };
    for (what, inlines, control, strut, width, lines) in [
        (
            "50,000 tiny trimmed periods",
            spread(0.01),
            (spread(1.0 / 64.0), 150_000),
            &st,
            0.01,
            50_000,
        ),
        (
            "500,000 one-character spans",
            vec![text("a", &st); 500_000],
            (vec![text(&"a".repeat(500_000), &st)], 1),
            &st,
            1.0e9,
            1,
        ),
        (
            "40,000 letters before 40,000 preserved spaces",
            vec![text(&("a".repeat(40_000) + &" ".repeat(40_000)), &pre_wrap)],
            (vec![text(&("a".repeat(80_000) + " "), &pre_wrap)], 80_000),
            &pre_wrap,
            7.0,
            40_000,
        ),
    ] {
        assert_linear(
            what,
            || timed(&inlines, strut, width, lines),
            || timed(&control.0, strut, width, control.1),
        );
    }
}

#[test]
fn long_break_spaces_whitespace_runs_scale_linearly() {
    let st = TextStyle {
        family: FamilyList::parse(MONO),
        size: 10.0,
        line_height: 16.0,
        tab_size: 1,
        ..style(MONO, WhiteSpace::Normal)
    };
    let p = Paragraph {
        width: 0.75,
        strut: st.clone(),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
    };
    for (what, unit) in [("break-spaces spaces", " "), ("break-spaces tabs", "\t")] {
        let run = |n: usize| {
            let inlines = [Inline::Text {
                text: format!("a{}\nc", unit.repeat(n)),
                style: TextStyle {
                    white_space: WhiteSpace::BreakSpaces,
                    ..st.clone()
                },
            }];
            let t0 = std::time::Instant::now();
            let t = layout(shared_fonts(), &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
            let took = t0.elapsed();
            assert!(t.lines.len() > n, "{what}: {} lines", t.lines.len());
            took
        };
        assert_scales_linearly(what, 4_000, run);
    }
}

#[test]
fn unsafe_shaping_cuts_scale_linearly() {
    let st = TextStyle {
        family: FamilyList::parse(SANS),
        size: 10.0,
        line_height: 16.0,
        ..style(SANS, WhiteSpace::Normal)
    };
    for (what, unit, count, ends, overflow_wrap, width, lines) in [
        (
            "a kerned pair across word joiners",
            "\u{2060}",
            2_000,
            ("A", "V"),
            OverflowWrap::Anywhere,
            6.1,
            2,
        ),
        (
            "a kerned pair across break opportunities",
            "\u{200b}\u{2060}",
            2_000,
            ("A", "V"),
            OverflowWrap::Normal,
            6.1,
            2,
        ),
        (
            "a kerned pair across marks on word joiners",
            "\u{2060}\u{301}",
            2_000,
            ("A", "V"),
            OverflowWrap::Anywhere,
            6.1,
            2,
        ),
        (
            "a ligature across word joiners",
            "\u{2060}",
            32_000,
            ("f", "i"),
            OverflowWrap::Anywhere,
            400.0,
            1,
        ),
    ] {
        let p = Paragraph {
            width,
            strut: st.clone(),
            overflow_wrap,
            direction: Direction::Ltr,
        };
        assert_scales_linearly(what, count, |n| {
            let inlines = [Inline::Text {
                text: format!("{}{}{}", ends.0, unit.repeat(n), ends.1),
                style: st.clone(),
            }];
            let start = std::time::Instant::now();
            let t = layout(shared_fonts(), &p, &inlines).unwrap_or_else(|e| panic!("{e}"));
            let elapsed = start.elapsed();
            assert_eq!(t.lines.len(), lines, "{what} at {n}");
            elapsed
        });
    }
}
