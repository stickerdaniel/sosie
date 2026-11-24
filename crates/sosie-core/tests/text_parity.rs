mod common;

use common::{all_fonts, json, lay_out, write_report, ChromeLayout, LayoutIn};
use serde_json::json;
use std::collections::{BTreeMap, HashMap};

const WIDTH_TOLERANCE: f32 = 4.0 / 64.0;
const HEIGHT_TOLERANCE: f32 = 1.0 / 64.0;
const REPORTED: [&str; 1] = ["cjk"];
const SETS: [(&str, &str); 2] = [
    (
        "tests/golden/synthetic/text/corpus-1.json",
        "tests/golden/synthetic/text/chrome-1.json",
    ),
    (
        "tests/golden/synthetic/text/corpus-2.json",
        "tests/golden/synthetic/text/chrome-2.json",
    ),
];

#[derive(Default, Clone, serde::Serialize)]
struct Stratum {
    layouts: usize,
    lines: usize,
    break_mismatches: usize,
    width_mismatch_lines: usize,
    max_width_diff: f32,
    height_mismatches: usize,
    max_height_diff: f32,
    failing: Vec<String>,
}

fn trim(block: &str, s: &str) -> String {
    let t = s.trim_end_matches([' ', '\t']);
    if block == "p" {
        t.trim_start_matches(' ').to_string()
    } else {
        t.to_string()
    }
}

#[test]
fn line_breaks_and_widths_match_chromium() {
    let fonts = all_fonts();
    let mut strata: BTreeMap<String, Stratum> = BTreeMap::new();
    let mut total = 0;
    for (set, (corpus, chrome)) in SETS.iter().enumerate() {
        let set = set + 1;
        let corpus: Vec<LayoutIn> = json(corpus);
        let chrome: HashMap<String, ChromeLayout> = json::<Vec<ChromeLayout>>(chrome)
            .into_iter()
            .map(|c| (c.id.clone(), c))
            .collect();
        let ids: std::collections::HashSet<&str> = corpus.iter().map(|l| l.id.as_str()).collect();
        assert_eq!(ids.len(), corpus.len(), "set-{set}: layout ids repeat");
        assert_eq!(
            chrome.len(),
            corpus.len(),
            "set-{set}: Chromium layouts and corpus differ"
        );
        for l in &corpus {
            let key = format!("set-{set}/{}", l.id);
            total += 1;
            let s = strata.entry(l.stratum.clone()).or_default();
            s.layouts += 1;
            let c = &chrome[&l.id];
            let ours = lay_out(&fonts, l);
            let painted: Vec<_> = ours
                .lines
                .iter()
                .filter(|x| !ours.line_text(x).is_empty())
                .collect();
            let cl: Vec<String> = c.lines.iter().map(|x| trim(&l.block, &x.text)).collect();
            let ol: Vec<String> = painted
                .iter()
                .map(|x| trim(&l.block, ours.line_text(x)))
                .collect();
            s.lines += cl.len();
            let dh = (c.height - ours.height).abs();
            s.max_height_diff = s.max_height_diff.max(dh);
            if dh > HEIGHT_TOLERANCE {
                s.height_mismatches += 1;
                s.failing.push(key);
                continue;
            }
            if cl != ol {
                s.break_mismatches += 1;
                s.failing.push(key);
                continue;
            }
            let mut bad = false;
            for (cline, oline) in c.lines.iter().zip(&painted) {
                let d = (cline.w - oline.ink_width).abs();
                s.max_width_diff = s.max_width_diff.max(d);
                if d > WIDTH_TOLERANCE {
                    s.width_mismatch_lines += 1;
                    bad = true;
                }
            }
            if bad {
                s.failing.push(key);
            }
        }
    }
    let gated: Vec<(&String, &Stratum)> = strata
        .iter()
        .filter(|(k, _)| !REPORTED.contains(&k.as_str()))
        .collect();
    let gated_layouts: usize = gated.iter().map(|(_, s)| s.layouts).sum();
    write_report(
        "text.json",
        &json!({ "layouts": total, "gated_layouts": gated_layouts, "reported": REPORTED, "strata": strata }),
    );
    for (k, s) in &strata {
        println!(
            "{k:12} layouts {:5} lines {:6} heights {:3} breaks {:3} widths {:3} max {:.4} max height {:.4}",
            s.layouts,
            s.lines,
            s.height_mismatches,
            s.break_mismatches,
            s.width_mismatch_lines,
            s.max_width_diff,
            s.max_height_diff
        );
    }
    assert_eq!(
        gated.len(),
        11,
        "expected the 11 strata that passed stage 0"
    );
    assert!(gated_layouts >= 2000, "only {gated_layouts} gated layouts");
    let failing: Vec<String> = gated
        .iter()
        .filter(|(_, s)| {
            s.height_mismatches > 0 || s.break_mismatches > 0 || s.width_mismatch_lines > 0
        })
        .map(|(k, s)| {
            format!(
                "{k}: {} height, {} break, {} width, first {:?}",
                s.height_mismatches,
                s.break_mismatches,
                s.width_mismatch_lines,
                s.failing.iter().take(5).collect::<Vec<_>>()
            )
        })
        .collect();
    assert!(failing.is_empty(), "text gate failed: {failing:#?}");
}
