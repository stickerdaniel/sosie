mod common;

use common::{hex, json, metric, reference_chart, reference_metrics, SANS};
use serde::Deserialize;
use sosie_core::chart::{
    format_change, format_value, percent_change, render, tone, FONT_STACK, PRIMER,
};
use sosie_core::{
    layout, rasterize, Chart, ChartError, Direction, DisplayList, FamilyList, Fonts, Inline, Item,
    LimitError, MetricError, OverflowWrap, Palette, Paragraph, Pixmap, Radii, Rect, Rgba, Shape,
    TextStyle, Theme, Tone, WhiteSpace,
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

fn draw(chart: &Chart, width: u32) -> Result<DisplayList, ChartError> {
    render(&fonts(), chart, Theme::Light, width)
}

fn list(chart: &Chart) -> DisplayList {
    draw(chart, W).unwrap_or_else(|e| panic!("{e}"))
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

fn clipped(list: &DisplayList) -> Vec<(Shape, Shape)> {
    list.items
        .iter()
        .filter_map(|i| match i {
            Item::ClippedFill { shape, clip, .. } => Some((*shape, *clip)),
            _ => None,
        })
        .collect()
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

#[test]
fn the_reference_chart_has_the_css_geometry() {
    let list = list(&reference_chart());
    assert_eq!(list.height, 439.0);
    let bars = clipped(&list);
    let track_tops = [92.0, 114.0, 190.0, 212.0, 288.0, 310.0, 386.0, 408.0];
    let fills: [f64; 8] = [
        620.0, 320.915493, 620.0, 422.429577, 610.195155, 620.0, 620.0, 620.0,
    ];
    assert_eq!(bars.len(), 8);
    for ((fill, track), (y, w)) in bars.iter().zip(track_tops.into_iter().zip(fills)) {
        let want = Shape {
            rect: Rect {
                x: 17.0,
                y,
                w: 620.0,
                h: 10.0,
            },
            radii: Radii::all(6.0),
        };
        assert_eq!(*track, want);
        assert!(list
            .items
            .iter()
            .any(|i| matches!(i, Item::Fill { shape, .. } if *shape == want)));
        assert!(
            (f64::from(fill.rect.w) - w).abs() < 1e-4,
            "fill {} at {y}",
            fill.rect.w
        );
        assert_eq!(
            Shape {
                rect: Rect {
                    w: 620.0,
                    ..fill.rect
                },
                ..*fill
            },
            want
        );
    }
}

#[test]
fn the_change_ends_at_the_inner_right_edge_on_the_name_baseline() {
    let list = list(&reference_chart());
    let runs = glyph_runs(&list);
    for change in ["−48 %", "−32 %", "+1.6 %", "0 %"] {
        let x = 797.0 - unwrapped(change, 14.0, 600);
        let (_, baseline) = *runs
            .iter()
            .find(|(rx, _)| (rx - x).abs() < 1e-4)
            .unwrap_or_else(|| panic!("no run ends {change} at 797"));
        assert!(
            runs.iter().any(|&(rx, b)| rx == 17.0 && b == baseline),
            "{change} is off the name baseline"
        );
    }
}

#[test]
fn the_header_and_rows_take_their_css_heights() {
    assert_eq!(list(&chart(Some(""), reference_metrics())).height, 393.0);
    assert_eq!(list(&chart(None, reference_metrics())).height, 393.0);
    let one = vec![reference_metrics().remove(0)];
    assert_eq!(list(&chart(Some("Performance"), one)).height, 145.0);

    let title = "Render times and bundle sizes before and after the change across every page of the site that we measured in the lab";
    assert_eq!(
        measure(title, 14.0, 600, WhiteSpace::Normal, WIDTH - 34.0).len(),
        2
    );
    assert_eq!(list(&chart(Some(title), reference_metrics())).height, 460.0);

    let room = WIDTH - 50.0 - unwrapped("−48 %", 14.0, 600);
    let lines = |name: &str| measure(name, 14.0, 600, WhiteSpace::Normal, room).len();
    let mut name = "First Contentful Paint measured on the slowest page of the whole site with a cold cache and x".to_string();
    while lines(&name) == 1 {
        name.push('x');
    }
    assert_eq!(lines(&name), 2);
    assert!(unwrapped(&name, 14.0, 600) < room + 16.0);
    let mut metrics = reference_metrics();
    metrics[0].name = name;
    assert_eq!(list(&chart(Some("Performance"), metrics)).height, 460.0);
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
    let long = "x".repeat(200);
    let mut metrics = reference_metrics();
    metrics[2].name = format!("Bundle {long}");
    assert_eq!(
        draw(&chart(None, metrics), W),
        Err(ChartError::Metric {
            index: 2,
            error: MetricError::NameTooWide
        })
    );
    assert_eq!(
        draw(&chart(Some(&long), reference_metrics()), W),
        Err(ChartError::TitleTooWide)
    );
}

#[test]
fn the_width_lies_between_310_and_4096_px() {
    for width in [309, 4097] {
        assert_eq!(
            draw(&reference_chart(), width),
            Err(ChartError::Width { width })
        );
    }
    for width in [310, 4096] {
        assert_eq!(
            draw(&reference_chart(), width).map(|l| l.width),
            Ok(width as f32)
        );
    }
}

#[test]
fn a_chart_beyond_the_dpr_2_limits_is_rejected_before_rasterizing() {
    let row = reference_metrics().remove(0);
    let tall = |rows: usize| chart(None, vec![row.clone(); rows]);
    let height = |rows: usize| 98 * rows as u64 + 1;
    assert_eq!(
        draw(&tall(167), W).map(|l| l.height),
        Ok(height(167) as f32)
    );
    assert_eq!(
        draw(&tall(168), W),
        Err(ChartError::Size(LimitError::Height {
            px: 2 * height(168),
            max: 32_768
        }))
    );
    assert!(draw(&tall(39), 4096).is_ok());
    assert_eq!(
        draw(&tall(40), 4096),
        Err(ChartError::Size(LimitError::Pixels {
            pixels: 8_192 * 2 * height(40),
            max: 64_000_000
        }))
    );
}

fn unit_at(width: f32) -> String {
    let (before_key, after_key) = (
        unwrapped("Before", 12.0, 400),
        unwrapped("After", 12.0, 400),
    );
    let column = |unit: &str| {
        let before = before_key + unwrapped(&format!("1 {unit}"), 12.0, 400);
        let after = after_key + unwrapped(&format!("7 {unit}"), 12.0, 600);
        before.max(after)
    };
    let letters: Vec<char> = ('a'..='z').collect();
    for len in 0..30 {
        let stem = "m".repeat(len);
        let widest = column(&format!("{stem}mmm")).max(column(&format!("{stem}www")));
        if widest < width - 1.0 {
            continue;
        }
        for a in &letters {
            for b in &letters {
                for c in &letters {
                    let unit = format!("{stem}{a}{b}{c}");
                    if column(&unit) == width {
                        return unit;
                    }
                }
            }
        }
    }
    panic!("no unit gives a {width} px value column");
}

#[test]
fn the_value_column_holds_exactly_144_px() {
    let draw_unit = |unit: &str| draw(&chart(None, vec![metric("Size", unit, 1.0, 7.0, true)]), W);
    let padded = |unit: &str| format!(" \t{unit}\n ");
    let fits = unit_at(144.0);
    assert!(draw_unit(&fits).is_ok(), "{fits}");
    assert_eq!(draw_unit(&padded(&fits)), draw_unit(&fits), "{fits}");
    let over = unit_at(144.0 + 1.0 / 64.0);
    for unit in [over.clone(), padded(&over)] {
        assert_eq!(
            draw_unit(&unit),
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
    let list = render(&fonts, chart, theme, W).unwrap_or_else(|e| panic!("{e}"));
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
fn a_thin_fill_stays_inside_the_rounded_track() {
    let thin = chart(None, vec![metric("Requests", "", 1000.0, 1.0, true)]);
    let after = clipped(&list(&thin))[1].0;
    assert!((after.rect.w - 3.1).abs() < 1e-4, "{}", after.rect.w);
    let pm = raster(&thin, Theme::Light);
    let bg = hex("#ffffff");
    let fill = hex("#1f883d");
    let track = Rect {
        x: 34.0,
        y: 136.0,
        w: 1240.0,
        h: 20.0,
    };
    let mut checked = 0;
    for y in 124..168 {
        for x in 20..80 {
            let (cx, cy) = (x as f32 + 0.5, y as f32 + 0.5);
            if outside_rounded_rect(cx, cy, track, 10.0) > 0.75 {
                let p = pm.pixel(x, y).expect("pixel");
                assert_eq!(p, bg, "device pixel {x},{y}");
                checked += 1;
            }
        }
    }
    assert!(checked > 1000, "{checked}");
    assert!(near(pm.pixel(37, 146).expect("pixel"), fill, 2));
}

#[test]
fn a_half_fill_has_a_rounded_right_end() {
    let pm = raster(
        &chart(None, vec![metric("Requests", "", 100.0, 50.0, true)]),
        Theme::Light,
    );
    let track = over(hex("#818b981f"), hex("#ffffff"));
    let corner = pm.pixel(653, 136).expect("pixel");
    assert!(near(corner, track, 2), "{corner:?} vs track {track:?}");
    assert!(near(pm.pixel(650, 146).expect("pixel"), hex("#1f883d"), 2));
}

#[test]
fn after_fills_take_the_tone_color() {
    let centers = [
        (17.0 + 320.915493 / 2.0, 119.0),
        (17.0 + 422.429577 / 2.0, 217.0),
        (327.0, 315.0),
        (327.0, 413.0),
    ];
    for (theme, colors) in [
        (Theme::Light, ["#1f883d", "#1f883d", "#cf222e", "#818b98"]),
        (Theme::Dark, ["#238636", "#238636", "#da3633", "#656c76"]),
    ] {
        let pm = raster(&reference_chart(), theme);
        for ((x, y), color) in centers.into_iter().zip(colors) {
            let p = pm.pixel((x * 2.0) as u32, (y * 2.0) as u32).expect("pixel");
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
