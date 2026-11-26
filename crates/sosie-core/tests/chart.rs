mod common;

use common::{all_fonts, hex, json, metric, reference_chart, reference_metrics, SANS};
use serde::Deserialize;
use sosie_core::chart::{
    format_change, format_value, percent_change, render, tone, FONT_STACK, PRIMER,
};
use sosie_core::{
    layout, rasterize, Chart, ChartError, Direction, DisplayList, Family, FamilyList, Fonts,
    Inline, Item, LimitError, MetricError, OverflowWrap, Palette, Paragraph, Pixmap, Radii, Rect,
    Rendered, Rgba, Shape, TextStyle, Theme, Tone, WhiteSpace, WrappedName,
};
use std::collections::BTreeMap;

const W: u32 = 814;
const WIDTH: f32 = 814.0;

#[derive(Deserialize)]
struct Format {
    #[serde(rename = "formatValue")]
    values: Vec<ValueCase>,
    #[serde(rename = "formatChange")]
    changes: Vec<ChangeCase>,
}

#[derive(Deserialize)]
struct ValueCase {
    value: f64,
    unit: String,
    out: String,
}

#[derive(Deserialize)]
struct ChangeCase {
    before: f64,
    after: f64,
    percent: serde_json::Value,
    out: String,
    #[serde(rename = "toneLowerIsBetter")]
    tone_lower: Option<String>,
    #[serde(rename = "toneHigherIsBetter")]
    tone_higher: Option<String>,
}

fn format_cases() -> Format {
    json("crates/sosie-core/tests/chart-format.json")
}

fn tone_name(t: Tone) -> &'static str {
    match t {
        Tone::Success => "success",
        Tone::Danger => "danger",
        Tone::Neutral => "neutral",
    }
}

#[test]
fn format_value_matches_intl_number_format() {
    for c in format_cases().values {
        assert_eq!(
            format_value(c.value, &c.unit),
            c.out,
            "{} {:?}",
            c.value,
            c.unit
        );
    }
}

#[test]
fn change_and_tone_match_metric_bars() {
    for c in format_cases().changes {
        let at = format!("{} -> {}", c.before, c.after);
        let Some(percent) = c.percent.as_f64() else {
            assert!(percent_change(c.before, c.after).is_err(), "{at}");
            continue;
        };
        let change = percent_change(c.before, c.after).unwrap_or_else(|e| panic!("{at}: {e}"));
        assert_eq!(change, percent, "{at}");
        assert_eq!(format_change(change), c.out, "{at}");
        assert_eq!(
            c.tone_lower.as_deref(),
            Some(tone_name(tone(change, true))),
            "{at}"
        );
        assert_eq!(
            c.tone_higher.as_deref(),
            Some(tone_name(tone(change, false))),
            "{at}"
        );
    }
}

#[test]
fn the_change_rounds_the_exact_binary_value() {
    assert_eq!(percent_change(2.0, 2.117), Ok(5.8));
    assert_eq!(percent_change(2.0, 1.873), Ok(-6.3));
    assert_eq!(percent_change(4.0, 3.758), Ok(-6.0));
}

#[test]
fn a_change_without_a_finite_percentage_is_an_error() {
    assert_eq!(percent_change(0.0, 5.0), Err(MetricError::BeforeZero));
    assert_eq!(percent_change(1.0, 1e19), Err(MetricError::Change));
    assert_eq!(percent_change(1e-300, 1e300), Err(MetricError::Change));
    assert_eq!(percent_change(1.0, 1e18 + 1.0), Ok(1e20));
}

#[derive(Deserialize)]
struct Tokens {
    package: String,
    #[serde(rename = "fontStack")]
    font_stack: String,
    themes: BTreeMap<String, BTreeMap<String, String>>,
}

fn named(p: &Palette) -> BTreeMap<&'static str, Rgba> {
    BTreeMap::from([
        ("bgColor-default", p.bg_default),
        ("bgColor-muted", p.bg_muted),
        ("bgColor-neutral-muted", p.bg_neutral_muted),
        ("bgColor-success-emphasis", p.bg_success_emphasis),
        ("bgColor-danger-emphasis", p.bg_danger_emphasis),
        ("borderColor-default", p.border_default),
        ("borderColor-muted", p.border_muted),
        ("borderColor-emphasis", p.border_emphasis),
        ("fgColor-default", p.fg_default),
        ("fgColor-muted", p.fg_muted),
        ("fgColor-success", p.fg_success),
        ("fgColor-danger", p.fg_danger),
    ])
}

#[test]
fn the_palette_equals_the_primer_tokens() {
    let tokens: Tokens = json("assets/primer-tokens.json");
    assert_eq!(tokens.package, PRIMER);
    assert_eq!(tokens.font_stack, FONT_STACK);
    for (name, theme) in [("light", Theme::Light), ("dark", Theme::Dark)] {
        let want: BTreeMap<&str, Rgba> = tokens.themes[name]
            .iter()
            .map(|(k, v)| (k.as_str(), hex(v)))
            .collect();
        assert_eq!(named(theme.palette()), want, "{name}");
    }
}

fn fonts() -> Fonts {
    Fonts::new()
}

fn draw(chart: &Chart, width: u32) -> Result<Rendered, ChartError> {
    render(&fonts(), chart, Theme::Light, width)
}

fn list(chart: &Chart) -> DisplayList {
    draw(chart, W).unwrap_or_else(|e| panic!("{e}")).list
}

fn chart(title: Option<&str>, metrics: Vec<sosie_core::Metric>) -> Chart {
    Chart::new(title.map(str::to_string), metrics).unwrap_or_else(|e| panic!("{e}"))
}

fn measure(text: &str, size: f32, weight: u16, white_space: WhiteSpace, width: f32) -> Vec<f32> {
    let style = TextStyle {
        family: FamilyList::parse(SANS),
        size,
        line_height: size * 1.5,
        weight,
        italic: false,
        color: Rgba::BLACK,
        white_space,
        tab_size: 8,
        background: None,
    };
    let para = Paragraph {
        width,
        strut: style.clone(),
        overflow_wrap: OverflowWrap::Normal,
        direction: Direction::Ltr,
        dpr: 2.0,
    };
    let inlines = [Inline::Text {
        text: text.to_string(),
        style,
    }];
    layout(&fonts(), &para, &inlines)
        .unwrap_or_else(|e| panic!("{e}"))
        .lines
        .iter()
        .map(|l| l.advance)
        .collect()
}

fn unwrapped(text: &str, size: f32, weight: u16) -> f32 {
    measure(text, size, weight, WhiteSpace::Pre, 1e4)[0]
}

const ARROW_ADVANCE: f32 = 5.0 + 8.75 + 5.0;

fn before_and_arrow(before: &str) -> f32 {
    unwrapped(before, 12.0, 400) + ARROW_ADVANCE
}

fn name_lines(name: &str) -> usize {
    measure(name, 14.0, 600, WhiteSpace::Normal, 250.0).len()
}

fn glyph_runs(list: &DisplayList) -> Vec<(f32, f32)> {
    list.items
        .iter()
        .filter_map(|i| match i {
            Item::Glyphs { x, baseline, .. } => Some((*x, *baseline)),
            _ => None,
        })
        .collect()
}

fn tracks(list: &DisplayList) -> Vec<Rect> {
    list.items
        .iter()
        .filter_map(|i| match i {
            Item::Fill { shape, .. } if shape.rect.h == 10.0 => Some(shape.rect),
            _ => None,
        })
        .collect()
}

#[derive(Debug, PartialEq)]
enum Ink {
    Solid(Rgba),
    Stripes(Rgba),
    Outline(Rgba),
}

#[derive(Debug)]
struct Segment {
    ink: Ink,
    x: f32,
    w: f32,
    shape: Shape,
    clip: Shape,
}

fn segments(list: &DisplayList) -> Vec<Segment> {
    list.items
        .iter()
        .filter_map(|i| {
            let (ink, shape, clip) = match i {
                Item::ClippedFill { shape, clip, color } => (Ink::Solid(*color), shape, clip),
                Item::Hatch {
                    shape,
                    clip,
                    color,
                    stripe,
                    period,
                } => {
                    assert_eq!((*stripe, *period), (3.0, 6.0));
                    (Ink::Stripes(*color), shape, clip)
                }
                Item::ClippedBorder {
                    shape,
                    clip,
                    width,
                    color,
                } => {
                    assert_eq!(*width, 1.0);
                    (Ink::Outline(*color), shape, clip)
                }
                _ => return None,
            };
            Some(Segment {
                ink,
                x: shape.rect.x,
                w: shape.rect.w,
                shape: *shape,
                clip: *clip,
            })
        })
        .collect()
}

const SQUARE: Radii = Radii {
    tl: 0.0,
    tr: 0.0,
    br: 0.0,
    bl: 0.0,
};
const ROUND_RIGHT: Radii = Radii {
    tl: 0.0,
    tr: 6.0,
    br: 6.0,
    bl: 0.0,
};

fn assert_segment(got: &Segment, ink: Ink, x: f64, w: f64, radii: Radii, track: Rect) {
    assert_eq!(got.ink, ink, "{got:?}");
    assert!((f64::from(got.x) - x).abs() < 1e-4, "x {} vs {x}", got.x);
    assert!((f64::from(got.w) - w).abs() < 1e-4, "w {} vs {w}", got.w);
    assert_eq!(
        (got.shape.rect.y, got.shape.rect.h, got.shape.radii),
        (track.y, 10.0, radii)
    );
    assert_eq!(
        got.clip,
        Shape {
            rect: track,
            radii: Radii::all(6.0)
        }
    );
}

const SUCCESS: &str = "#1f883d";
const DANGER: &str = "#cf222e";
const EMPHASIS: &str = "#818b98";

#[test]
fn the_reference_chart_has_the_css_geometry() {
    let list = list(&reference_chart());
    assert_eq!(list.height, 215.0);
    let track = |y| Rect {
        x: 283.0,
        y,
        w: 268.0,
        h: 10.0,
    };
    let rows = [62.5, 104.5, 146.5, 188.5].map(track);
    assert_eq!(tracks(&list), rows);
    let segments = segments(&list);
    assert_eq!(segments.len(), 9);
    let ms = |v: f64| 268.0 * v / 1136.0;
    let (stripes, outline) = (hex("#1f883d73"), hex("#1f883d8c"));
    let decrease = |at: usize, row: Rect, after: f64, before: f64| {
        assert_segment(
            &segments[at],
            Ink::Solid(hex(SUCCESS)),
            283.0,
            ms(after),
            SQUARE,
            row,
        );
        for (s, ink) in [(1, Ink::Stripes(stripes)), (2, Ink::Outline(outline))] {
            assert_segment(
                &segments[at + s],
                ink,
                283.0 + ms(after),
                ms(before - after),
                ROUND_RIGHT,
                row,
            );
        }
    };
    decrease(0, rows[0], 588.0, 1136.0);
    decrease(3, rows[1], 774.0, 1136.0);
    assert_segment(
        &segments[6],
        Ink::Solid(hex(EMPHASIS)),
        283.0,
        268.0 * 292.5 / 297.2,
        SQUARE,
        rows[2],
    );
    assert_segment(
        &segments[7],
        Ink::Solid(hex(DANGER)),
        283.0 + 268.0 * 292.5 / 297.2,
        268.0 * (297.2 - 292.5) / 297.2,
        SQUARE,
        rows[2],
    );
    assert_segment(
        &segments[8],
        Ink::Solid(hex(EMPHASIS)),
        283.0,
        268.0,
        SQUARE,
        rows[3],
    );
}

#[test]
fn metrics_with_one_unit_share_a_scale() {
    let metrics = vec![
        metric("Small", "ms", 50.0, 40.0, true),
        metric("Large", "ms", 400.0, 500.0, true),
        metric("Bundle", "kB", 2.0, 3.0, true),
    ];
    let segments = segments(&list(&chart(None, metrics)));
    let ms = |v: f64| 268.0 * v / 500.0;
    let kb = |v: f64| 268.0 * v / 3.0;
    let widths: Vec<f64> = segments.iter().map(|s| f64::from(s.w)).collect();
    let want = [
        ms(40.0),
        ms(10.0),
        ms(10.0),
        ms(400.0),
        ms(100.0),
        kb(2.0),
        kb(1.0),
    ];
    assert_eq!(widths.len(), want.len());
    for (got, want) in widths.into_iter().zip(want) {
        assert!((got - want).abs() < 1e-4, "{got} vs {want}");
    }
}

#[test]
fn a_tiny_after_keeps_half_a_percent_of_the_track() {
    let thin = list(&chart(
        None,
        vec![metric("Requests", "", 1000.0, 1.0, true)],
    ));
    let segments = segments(&thin);
    let track = tracks(&thin)[0];
    assert_segment(
        &segments[0],
        Ink::Solid(hex(SUCCESS)),
        283.0,
        1.34,
        SQUARE,
        track,
    );
    assert_segment(
        &segments[1],
        Ink::Stripes(hex("#1f883d73")),
        283.0 + 0.268,
        268.0 * 0.999,
        ROUND_RIGHT,
        track,
    );
}

#[test]
fn a_decrease_where_higher_is_better_is_hatched_in_danger() {
    let segments = segments(&list(&chart(
        None,
        vec![metric("Score", "", 90.0, 60.0, false)],
    )));
    let inks: Vec<&Ink> = segments.iter().map(|s| &s.ink).collect();
    assert_eq!(
        inks,
        [
            &Ink::Solid(hex(DANGER)),
            &Ink::Stripes(hex("#cf222e73")),
            &Ink::Outline(hex("#cf222e8c"))
        ]
    );
}

#[test]
fn the_columns_sit_where_the_css_grid_puts_them() {
    let list = list(&reference_chart());
    let runs = glyph_runs(&list);
    let rows = [
        ("−48 %", "1,136", "588 ms"),
        ("−32 %", "1,136", "774 ms"),
        ("+1.6 %", "292.5", "297.2 kB"),
        ("0 %", "91", "91"),
    ];
    for (change, before, after) in rows {
        assert!(unwrapped(change, 14.0, 600) < 64.0, "{change}");
        let change_x = 797.0 - unwrapped(change, 14.0, 600);
        let (_, baseline) = *runs
            .iter()
            .find(|(x, _)| (x - change_x).abs() < 1e-3)
            .unwrap_or_else(|| panic!("no run ends {change} at 797"));
        assert!(
            runs.iter().any(|&(x, b)| x == 17.0 && b == baseline),
            "{change} is off the name baseline"
        );
        let values_x = 717.0 - before_and_arrow(before) - unwrapped(after, 12.0, 600);
        assert!(
            runs.iter().any(|(x, _)| (x - values_x).abs() < 1e-3),
            "{before}{after} does not end at 717"
        );
    }
}

fn arrow(left: f64, middle: f64) -> Vec<Vec<[f64; 2]>> {
    let (length, thickness, head) = (8.75, 0.95, 3.35);
    let miter = thickness * std::f64::consts::SQRT_2;
    let (tip, half) = (left + length, thickness / 2.0);
    let (notch, arm) = (tip - miter, tip - head);
    vec![
        vec![
            [left, middle - half],
            [notch, middle - half],
            [notch, middle + half],
            [left, middle + half],
        ],
        vec![
            [arm - miter, middle - head],
            [arm, middle - head],
            [tip, middle],
            [notch, middle],
        ],
        vec![
            [notch, middle],
            [tip, middle],
            [arm, middle + head],
            [arm - miter, middle + head],
        ],
    ]
}

#[test]
fn the_arrow_is_a_drawn_chevron_between_before_and_after() {
    let list = list(&reference_chart());
    let runs = glyph_runs(&list);
    let arrows: Vec<(&Vec<Vec<[f32; 2]>>, Rgba)> = list
        .items
        .iter()
        .filter_map(|i| match i {
            Item::Polygons { polygons, color } => Some((polygons, *color)),
            _ => None,
        })
        .collect();
    let rows = [
        ("1,136", "588 ms"),
        ("1,136", "774 ms"),
        ("292.5", "297.2 kB"),
        ("91", "91"),
    ];
    assert_eq!(arrows.len(), rows.len());
    let tracks = tracks(&list);
    for (((polygons, color), (before, after)), track) in arrows.into_iter().zip(rows).zip(tracks) {
        assert_eq!(color, hex("#59636e"));
        let values_x = 717.0 - before_and_arrow(before) - unwrapped(after, 12.0, 600);
        let &(_, baseline) = runs
            .iter()
            .find(|(x, b)| (x - values_x).abs() < 1e-3 && (b - track.y).abs() < 20.0)
            .unwrap_or_else(|| panic!("no value run for {before}"));
        let left = values_x + unwrapped(before, 12.0, 400) + 5.0;
        let want = arrow(f64::from(left), f64::from(baseline) - 4.25);
        assert_eq!(polygons.len(), want.len());
        for (got, want) in polygons.iter().zip(&want) {
            assert_eq!(got.len(), want.len());
            for (g, w) in got.iter().zip(want) {
                assert!(
                    (f64::from(g[0]) - w[0]).abs() < 1e-3 && (f64::from(g[1]) - w[1]).abs() < 1e-3,
                    "{before}: {g:?} vs {w:?}"
                );
            }
        }
    }
}

#[test]
fn value_glyphs_come_from_noto_sans_even_with_every_pack_loaded() {
    let fonts = all_fonts();
    let list = render(&fonts, &reference_chart(), Theme::Light, W)
        .unwrap_or_else(|e| panic!("{e}"))
        .list;
    let values_x = [
        ("1,136", "588 ms"),
        ("1,136", "774 ms"),
        ("292.5", "297.2 kB"),
        ("91", "91"),
    ]
    .map(|(before, after)| 717.0 - before_and_arrow(before) - unwrapped(after, 12.0, 600));
    let mut runs = 0;
    for item in &list.items {
        if let Item::Glyphs { face, x, .. } = item {
            if values_x.iter().any(|v| (v - x).abs() < 1e-3) {
                let family = fonts.face(*face).map(|f| f.family());
                assert_eq!(family, Some(Family::NotoSans), "value run at {x}");
                runs += 1;
            }
        }
    }
    assert_eq!(runs, 8);
}

#[test]
fn the_header_and_rows_take_their_css_heights() {
    assert_eq!(list(&chart(Some(""), reference_metrics())).height, 169.0);
    assert_eq!(list(&chart(None, reference_metrics())).height, 169.0);
    let one = vec![reference_metrics().remove(0)];
    assert_eq!(list(&chart(Some("Performance"), one)).height, 89.0);

    let title = "Render times and bundle sizes before and after the change across every page of the site that we measured in the lab";
    assert_eq!(
        measure(title, 14.0, 600, WhiteSpace::Normal, WIDTH - 34.0).len(),
        2
    );
    assert_eq!(list(&chart(Some(title), reference_metrics())).height, 236.0);
}

#[test]
fn a_wrapped_name_grows_its_row_and_centers_the_other_cells() {
    let two = "First Contentful Paint on a cold cache";
    let three = "First Contentful Paint on the slowest page of the site with a cold cache";
    assert_eq!((name_lines(two), name_lines(three)), (2, 3));
    let mut metrics = reference_metrics();
    metrics[0].name = two.to_string();
    metrics[2].name = three.to_string();
    let rendered = draw(&chart(Some("Performance"), metrics), W).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(rendered.list.height, 215.0 + 21.0 + 42.0);
    assert_eq!(
        tracks(&rendered.list)
            .iter()
            .map(|r| r.y)
            .collect::<Vec<_>>(),
        [73.0, 125.5, 188.5, 251.5]
    );
    assert_eq!(
        rendered.wrapped,
        [
            WrappedName { index: 0, lines: 2 },
            WrappedName { index: 2, lines: 3 }
        ]
    );
    let runs = glyph_runs(&rendered.list);
    let first_name_baseline = runs
        .iter()
        .filter(|&&(x, b)| x == 17.0 && b > 47.0)
        .map(|&(_, b)| b)
        .fold(f32::INFINITY, f32::min);
    let change_x = 797.0 - unwrapped("−48 %", 14.0, 600);
    let (_, change_baseline) = *runs
        .iter()
        .find(|(x, _)| (x - change_x).abs() < 1e-3)
        .expect("the change run");
    assert_eq!(change_baseline - first_name_baseline, 10.5);
    let values_baseline = |before: &str, after: &str, top: f32| {
        let x = 717.0 - before_and_arrow(before) - unwrapped(after, 12.0, 600);
        runs.iter()
            .find(|&&(rx, b)| (rx - x).abs() < 1e-3 && b > top && b < top + 60.0)
            .map(|&(_, b)| b - top)
            .unwrap_or_else(|| panic!("no value run for {after} below {top}"))
    };
    assert_eq!(
        values_baseline("1,136", "588 ms", 47.0) - values_baseline("1,136", "774 ms", 110.0),
        10.5
    );
}

#[test]
fn names_on_one_line_are_not_reported() {
    assert_eq!(
        draw(&reference_chart(), W).map(|r| r.wrapped),
        Ok(Vec::new())
    );
}

#[test]
fn invalid_metrics_are_rejected_with_their_index() {
    let at = |index, error| Err(ChartError::Metric { index, error });
    let with = |i: usize, f: &dyn Fn(&mut sosie_core::Metric)| {
        let mut metrics = reference_metrics();
        f(&mut metrics[i]);
        Chart::new(None, metrics)
    };
    assert_eq!(Chart::new(None, vec![]), Err(ChartError::NoMetrics));
    assert_eq!(with(1, &|m| m.name.clear()), at(1, MetricError::EmptyName));
    assert_eq!(
        with(2, &|m| m.after = f64::NAN),
        at(2, MetricError::NonFiniteValue)
    );
    assert_eq!(
        with(3, &|m| m.before = f64::INFINITY),
        at(3, MetricError::NonFiniteValue)
    );
    assert_eq!(with(0, &|m| m.before = 0.0), at(0, MetricError::BeforeZero));
    assert_eq!(
        with(1, &|m| (m.before, m.after) = (1.0, 1e19)),
        at(1, MetricError::Change)
    );
}

#[test]
fn text_that_cannot_fit_is_rejected() {
    let word = "x".repeat(40);
    assert!(unwrapped(&word, 14.0, 600) > 250.0);
    let mut metrics = reference_metrics();
    metrics[2].name = format!("Bundle {word}");
    assert_eq!(
        draw(&chart(None, metrics), W),
        Err(ChartError::Metric {
            index: 2,
            error: MetricError::NameTooWide
        })
    );
    let long = "x".repeat(200);
    assert_eq!(
        draw(&chart(Some(&long), reference_metrics()), W),
        Err(ChartError::TitleTooWide)
    );
}

#[test]
fn the_width_lies_between_646_and_4096_px() {
    for width in [645, 4097] {
        assert_eq!(
            draw(&reference_chart(), width),
            Err(ChartError::Width { width })
        );
    }
    for width in [646, 4096] {
        assert_eq!(
            draw(&reference_chart(), width).map(|r| r.list.width),
            Ok(width as f32)
        );
    }
    let narrow = draw(&reference_chart(), 646).unwrap_or_else(|e| panic!("{e}"));
    assert_eq!(tracks(&narrow.list)[0].w, 100.0);
}

#[test]
fn the_widest_change_sets_the_change_column() {
    let mut metrics = reference_metrics();
    metrics.push(metric("Requests", "", 1.0, 1e6, true));
    let wide = unwrapped("+99999900 %", 14.0, 600);
    assert!(wide > 64.0, "{wide}");
    let list = list(&chart(None, metrics.clone()));
    let track = 814.0 - 34.0 - 250.0 - 150.0 - 48.0 - wide;
    assert!((tracks(&list)[0].w - track).abs() < 1e-3);
    let runs = glyph_runs(&list);
    let values_end = 797.0 - wide - 16.0;
    let values_x = values_end - before_and_arrow("1,136") - unwrapped("588 ms", 12.0, 600);
    assert!(runs.iter().any(|(x, _)| (x - values_x).abs() < 1e-3));
    assert_eq!(
        draw(&chart(None, metrics), 646),
        Err(ChartError::Metric {
            index: 4,
            error: MetricError::ChangeTooWide
        })
    );
}

#[test]
fn a_chart_beyond_the_dpr_2_limits_is_rejected_before_rasterizing() {
    let row = reference_metrics().remove(0);
    let tall = |rows: usize| chart(None, vec![row.clone(); rows]);
    let height = |rows: usize| 42 * rows as u64 + 1;
    assert_eq!(
        draw(&tall(390), W).map(|r| r.list.height),
        Ok(height(390) as f32)
    );
    assert_eq!(
        draw(&tall(391), W),
        Err(ChartError::Size(LimitError::Height {
            px: 2 * height(391),
            max: 32_768
        }))
    );
    assert!(draw(&tall(92), 4096).is_ok());
    assert_eq!(
        draw(&tall(93), 4096),
        Err(ChartError::Size(LimitError::Pixels {
            pixels: 8_192 * 2 * height(93),
            max: 64_000_000
        }))
    );
}

fn values_at(width: f32) -> (f64, String) {
    let befores = [
        (1.0, "1"),
        (1.5, "1.5"),
        (1000.0, "1,000"),
        (-1.0, "−1"),
        (1.25, "1.25"),
        (-1.5, "−1.5"),
    ]
    .map(|(value, text)| (value, before_and_arrow(text)));
    let after = |unit: &str| unwrapped(&format!("7 {unit}"), 12.0, 600);
    let letters: Vec<char> = ('a'..='z').collect();
    for len in 0..30 {
        let stem = "m".repeat(len);
        let widest = after(&format!("{stem}mmm")).max(after(&format!("{stem}www")));
        if widest + befores.iter().map(|b| b.1).fold(0.0, f32::max) < width - 1.0 {
            continue;
        }
        for a in &letters {
            for b in &letters {
                for c in &letters {
                    let unit = format!("{stem}{a}{b}{c}");
                    let w = after(&unit);
                    if let Some(&(before, _)) = befores.iter().find(|b| b.1 + w == width) {
                        return (before, unit);
                    }
                }
            }
        }
    }
    panic!("no unit gives a {width} px value column");
}

#[test]
fn the_value_column_holds_exactly_150_px() {
    let draw_unit = |before: f64, unit: &str| {
        draw(
            &chart(None, vec![metric("Size", unit, before, 7.0, true)]),
            W,
        )
    };
    let padded = |unit: &str| format!(" \t{unit}\n ");
    let (before, fits) = values_at(150.0);
    assert!(draw_unit(before, &fits).is_ok(), "{before} {fits}");
    assert_eq!(
        draw_unit(before, &padded(&fits)),
        draw_unit(before, &fits),
        "{fits}"
    );
    let (before, over) = values_at(150.0 + 1.0 / 64.0);
    for unit in [over.clone(), padded(&over)] {
        assert_eq!(
            draw_unit(before, &unit),
            Err(ChartError::Metric {
                index: 0,
                error: MetricError::ValueTooWide
            }),
            "{unit:?}"
        );
    }
}

fn raster(chart: &Chart, theme: Theme) -> Pixmap {
    let fonts = fonts();
    let list = render(&fonts, chart, theme, W)
        .unwrap_or_else(|e| panic!("{e}"))
        .list;
    rasterize(&fonts, &list, 2.0, &Default::default()).expect("rasterize")
}

fn near(a: Rgba, b: Rgba, tol: u8) -> bool {
    [(a.r, b.r), (a.g, b.g), (a.b, b.b), (a.a, b.a)]
        .iter()
        .all(|(x, y)| x.abs_diff(*y) <= tol)
}

fn over(top: Rgba, bottom: Rgba) -> Rgba {
    let a = top.a as u32;
    let mix = |s: u8, d: u8| ((s as u32 * a + d as u32 * (255 - a) + 127) / 255) as u8;
    Rgba::rgb(
        mix(top.r, bottom.r),
        mix(top.g, bottom.g),
        mix(top.b, bottom.b),
    )
}

fn outside_rounded_rect(x: f32, y: f32, r: Rect, radius: f32) -> f32 {
    let qx = (x - (r.x + r.w / 2.0)).abs() - (r.w / 2.0 - radius);
    let qy = (y - (r.y + r.h / 2.0)).abs() - (r.h / 2.0 - radius);
    let (ox, oy) = (qx.max(0.0), qy.max(0.0));
    (ox * ox + oy * oy).sqrt() + qx.max(qy).min(0.0) - radius
}

#[test]
fn the_removed_part_has_135_degree_stripes_inside_an_outline() {
    let pm = raster(&reference_chart(), Theme::Light);
    let track = over(hex("#818b981f"), hex("#ffffff"));
    let stripe = over(hex("#1f883d73"), track);
    let left = 2.0 * (283.0 + 268.0 * 588.0 / 1136.0);
    let (mut stripes, mut gaps) = (0, 0);
    for y in 127..143u32 {
        for x in 846..1080u32 {
            let along = ((f64::from(x) + 0.5 - left) + (f64::from(y) + 0.5 - 125.0)) / 2.0
                * std::f64::consts::FRAC_1_SQRT_2;
            let phase = along.rem_euclid(6.0);
            if [0.0, 3.0, 6.0]
                .iter()
                .any(|edge| (phase - edge).abs() < 0.01)
            {
                continue;
            }
            let (want, count) = if phase < 3.0 {
                (stripe, &mut stripes)
            } else {
                (track, &mut gaps)
            };
            let p = pm.pixel(x, y).expect("pixel");
            assert!(near(p, want, 1), "{x},{y} at {phase}: {p:?} vs {want:?}");
            *count += 1;
        }
    }
    assert!(stripes > 1500 && gaps > 1500, "{stripes} {gaps}");
    let outline = hex("#1f883d8c");
    for x in 846..1080u32 {
        let p = pm.pixel(x, 125).expect("pixel");
        let options = [over(outline, track), over(outline, stripe)];
        assert!(
            options.iter().any(|&o| near(p, o, 1)),
            "{x},125: {p:?} vs {options:?}"
        );
    }
    assert_eq!(pm.pixel(1101, 144), Some(hex("#ffffff")));
}

#[test]
fn a_thin_fill_and_its_stripes_stay_inside_the_rounded_track() {
    let thin = chart(None, vec![metric("Requests", "", 1000.0, 1.0, true)]);
    let pm = raster(&thin, Theme::Light);
    let bg = hex("#ffffff");
    let track = Rect {
        x: 566.0,
        y: 33.0,
        w: 536.0,
        h: 20.0,
    };
    let mut checked = 0;
    for y in 22..64 {
        for x in (556..600).chain(1060..1112) {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            if outside_rounded_rect(cx, cy, track, 10.0) > 0.75 {
                let p = pm.pixel(x, y).expect("pixel");
                assert_eq!(p, bg, "device pixel {x},{y}");
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "{checked}");
    assert_ne!(pm.pixel(566, 43), Some(bg));
}

#[test]
fn segments_take_the_tone_colors() {
    let points = [(700, 135), (635, 303), (1098, 303), (580, 387)];
    for (theme, colors) in [
        (Theme::Light, [SUCCESS, EMPHASIS, DANGER, EMPHASIS]),
        (Theme::Dark, ["#238636", "#656c76", "#da3633", "#656c76"]),
    ] {
        let pm = raster(&reference_chart(), theme);
        for ((x, y), color) in points.into_iter().zip(colors) {
            let p = pm.pixel(x, y).expect("pixel");
            assert!(
                near(p, hex(color), 1),
                "{theme:?} at {x},{y}: {p:?} vs {color}"
            );
        }
    }
}

fn paint(items: Vec<Item>) -> Pixmap {
    let mut list = DisplayList::new(40.0, 40.0, Rgba::WHITE);
    for item in items {
        list.push(item);
    }
    rasterize(&fonts(), &list, 2.0, &Default::default()).expect("rasterize")
}

fn rounded(x: f32, y: f32, w: f32, h: f32, r: f32) -> Shape {
    Shape {
        rect: Rect { x, y, w, h },
        radii: Radii::all(r),
    }
}

#[test]
fn a_fill_clipped_by_itself_squares_its_edge_coverage() {
    let shape = Shape {
        rect: Rect {
            x: 5.25,
            y: 5.0,
            w: 10.0,
            h: 10.0,
        },
        radii: Radii::default(),
    };
    let half = (0.5f32 * 255.0 + 0.5) as u32;
    let squared = (half * half + 127) / 255;
    let ink = ((255 - squared) * 255 + 127) / 255;
    let pm = paint(vec![Item::ClippedFill {
        shape,
        clip: shape,
        color: Rgba::BLACK,
    }]);
    assert_eq!(
        pm.pixel(10, 20),
        Some(Rgba::rgb(ink as u8, ink as u8, ink as u8))
    );
    assert_eq!(pm.pixel(20, 20), Some(Rgba::BLACK));
}

#[test]
fn a_clipped_fill_paints_only_inside_its_clip() {
    let shape = rounded(5.0, 5.0, 20.0, 10.0, 4.0);
    let color = hex("#cf222e");
    assert_eq!(
        paint(vec![Item::ClippedFill {
            shape,
            clip: rounded(28.0, 20.0, 10.0, 10.0, 3.0),
            color
        }]),
        paint(vec![])
    );
    assert_eq!(
        paint(vec![Item::ClippedFill {
            shape,
            clip: rounded(2.0, 2.0, 30.0, 20.0, 2.0),
            color
        }]),
        paint(vec![Item::Fill { shape, color }])
    );
}

#[test]
fn value_white_space_collapses_onto_one_line() {
    let draw_unit = |unit: &str| {
        draw(
            &chart(None, vec![metric("Size", unit, 10.0, 11.0, true)]),
            W,
        )
    };
    let single = draw_unit("m s");
    for unit in ["m\ns", "m  s", "m\ts", " m s ", "\r\nm \t\n s\n"] {
        assert_eq!(draw_unit(unit), single, "{unit:?}");
    }
    assert_eq!(draw_unit(" \t\n"), draw_unit(""));
}
