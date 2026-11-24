mod common;

use common::{
    all_fonts, case_height, decode_png, diff_ratio, json, paint_case, root, write_report, CaseBox,
    PaintCase, CASE_PADDING, PRE_PADDING,
};
use serde_json::json;
use sosie_core::{rasterize, Limits};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

const BUDGET_B: f64 = 0.00815;
const CAPTURE_LIMIT: f64 = 0.015;
const DPR: f32 = 2.0;
const TEXT_STRATA: [&str; 3] = ["sans-prose", "mono-code", "emoji"];

#[derive(Default, serde::Serialize)]
struct Agg {
    cases: usize,
    pixels: u64,
    gate_diff: f64,
    strict_diff: f64,
    max_case_gate: f64,
    worst_case: String,
}

impl Agg {
    fn add(&mut self, id: &str, gate: f64, strict: f64, pixels: u64) {
        self.cases += 1;
        self.pixels += pixels;
        self.gate_diff += gate * pixels as f64;
        self.strict_diff += strict * pixels as f64;
        if gate > self.max_case_gate {
            self.max_case_gate = gate;
            self.worst_case = id.to_string();
        }
    }

    fn gate(&self) -> f64 {
        self.gate_diff / self.pixels.max(1) as f64
    }

    fn strict(&self) -> f64 {
        self.strict_diff / self.pixels.max(1) as f64
    }
}

fn goldens() -> Option<PathBuf> {
    let dir = std::env::var_os("SOSIE_GOLDENS")
        .map_or_else(|| root().join("target/goldens"), PathBuf::from);
    let paint = dir.join("synthetic-paint");
    if paint.is_dir() {
        return Some(paint);
    }
    assert!(
        std::env::var_os("CI").is_none(),
        "pixel bundle missing at {}: run tools/fetch-goldens.sh",
        paint.display()
    );
    None
}

#[test]
fn text_and_fills_raster_within_budget_b() {
    let Some(dir) = goldens() else {
        println!("pixel gate skipped: no bundle at target/goldens");
        return;
    };
    let fonts = all_fonts();
    let limits = Limits::default();
    let mut groups: BTreeMap<String, Agg> = BTreeMap::new();
    let mut size_mismatches = Vec::new();
    let mut captures: Vec<(String, f64)> = Vec::new();
    let mut text_fixtures = 0;
    for set in ["1", "2"] {
        let cases: Vec<PaintCase> = json(&format!("tests/golden/synthetic/paint/cases-{set}.json"));
        let box_list: Vec<CaseBox> =
            json(&format!("tests/golden/synthetic/paint/boxes-{set}.json"));
        let boxes: HashMap<String, CaseBox> =
            box_list.into_iter().map(|b| (b.id.clone(), b)).collect();
        let pngs = std::fs::read_dir(dir.join(set))
            .unwrap_or_else(|e| panic!("set-{set}: {e}"))
            .filter(|e| {
                e.as_ref()
                    .is_ok_and(|e| e.path().extension().is_some_and(|x| x == "png"))
            })
            .count();
        let ids: std::collections::HashSet<&str> = cases.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids.len(), cases.len(), "set-{set}: case ids repeat");
        assert_eq!(
            boxes.len(),
            cases.len(),
            "set-{set}: boxes and cases differ"
        );
        assert_eq!(pngs, cases.len(), "set-{set}: bundle PNGs and cases differ");
        for c in &cases {
            let key = format!("set-{set}/{}", c.id);
            let b = &boxes[&c.id];
            let extra = if c.block == "pre" {
                2.0 * PRE_PADDING
            } else {
                0.0
            };
            let ours = (c.width + extra + 2.0 * CASE_PADDING, case_height(&fonts, c));
            if ours != (b.w, b.h) {
                size_mismatches.push(format!("{key}: ours {:?}, chromium {:?}", ours, (b.w, b.h)));
            }
            let png = std::fs::read(dir.join(set).join(format!("{}.png", c.id)))
                .unwrap_or_else(|e| panic!("{key}: {e}"));
            let (cw, ch, expected) = decode_png(&png);
            let list = paint_case(&fonts, c, b.w, b.h);
            let pm =
                rasterize(&fonts, &list, DPR, &limits).unwrap_or_else(|e| panic!("{key}: {e}"));
            assert_eq!((pm.width, pm.height), (cw, ch), "{key}: raster size");
            let (gate, pixels) = diff_ratio(&pm.rgba, &expected, cw, ch, 8, 1);
            let (strict, _) = diff_ratio(&pm.rgba, &expected, cw, ch, 2, 0);
            if TEXT_STRATA.contains(&c.stratum.as_str()) {
                text_fixtures += 1;
            }
            captures.push((key.clone(), gate));
            for group in [
                format!("pool/{}/{}", c.stratum, c.theme),
                format!("set-{set}/{}/{}", c.stratum, c.theme),
            ] {
                groups
                    .entry(group)
                    .or_default()
                    .add(&key, gate, strict, pixels);
            }
        }
    }
    let report: BTreeMap<&String, serde_json::Value> = groups
        .iter()
        .map(|(k, a)| (k, json!({ "cases": a.cases, "pixels": a.pixels, "gate": a.gate(), "strict": a.strict(), "maxCaseGate": a.max_case_gate, "worstCase": a.worst_case })))
        .collect();
    let above_b = captures.iter().filter(|(_, g)| *g > BUDGET_B).count();
    let over_limit: Vec<String> = captures
        .iter()
        .filter(|(_, g)| *g > CAPTURE_LIMIT)
        .map(|(k, g)| format!("{k}: {:.4} %", 100.0 * g))
        .collect();
    let (worst, worst_gate) = captures
        .iter()
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .cloned()
        .unwrap_or_default();
    write_report(
        "pixels.json",
        &json!({
            "budget": BUDGET_B,
            "captureLimit": CAPTURE_LIMIT,
            "dpr": DPR,
            "captures": captures.len(),
            "textFixtures": text_fixtures,
            "capturesAboveBudget": above_b,
            "worstCapture": { "case": worst, "gate": worst_gate },
            "groups": report,
        }),
    );
    println!(
        "{} captures, {text_fixtures} text, {above_b} above B, worst {worst} at {:.4} %",
        captures.len(),
        100.0 * worst_gate
    );
    for (k, a) in &groups {
        println!(
            "{k:34} cases {:3} gate {:.4} % strict {:.4} % max case {:.4} % ({})",
            a.cases,
            100.0 * a.gate(),
            100.0 * a.strict(),
            100.0 * a.max_case_gate,
            a.worst_case
        );
    }
    assert!(
        size_mismatches.is_empty(),
        "surface sizes differ: {size_mismatches:#?}"
    );
    assert!(text_fixtures >= 50, "only {text_fixtures} text fixtures");
    let pools = groups.keys().filter(|k| k.starts_with("pool/")).count();
    assert_eq!(pools, 8, "expected 8 pools of stratum and theme");
    assert!(
        over_limit.is_empty(),
        "captures above {:.1} %: {over_limit:#?}",
        100.0 * CAPTURE_LIMIT
    );
    let over: Vec<String> = groups
        .iter()
        .filter(|(k, a)| k.starts_with("pool/") && a.gate() > BUDGET_B)
        .map(|(k, a)| {
            format!(
                "{k}: {:.4} %, worst {} at {:.4} %",
                100.0 * a.gate(),
                a.worst_case,
                100.0 * a.max_case_gate
            )
        })
        .collect();
    assert!(
        over.is_empty(),
        "pixel gate above budget B ({:.3} %): {over:#?}",
        100.0 * BUDGET_B
    );
}
