use serde_json::{Map, Value};
use sosie_core::Metric;

pub struct Input {
    pub title: Option<String>,
    pub metrics: Vec<Metric>,
}

pub fn parse(json: &[u8]) -> Result<Input, String> {
    let fail = |why: &str| format!("metrics.json: {why}");
    let raw: Value = serde_json::from_slice(json).map_err(|e| fail(&e.to_string()))?;
    let empty = Map::new();
    let input = match &raw {
        Value::Object(fields) => fields,
        Value::Array(_) => &empty,
        _ => return Err(fail("expected an object")),
    };
    let title = match input.get("title") {
        None => None,
        Some(Value::String(t)) => Some(t.clone()),
        Some(_) => return Err(fail("title must be a string")),
    };
    let entries = match input.get("metrics") {
        Some(Value::Array(m)) if !m.is_empty() => m,
        _ => return Err(fail("metrics must be a non-empty array")),
    };
    let metrics = entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            metric(entry).ok_or_else(|| {
                fail(&format!(
                    "metrics[{i}] needs a name, a unit (\"\" for none), numbers for before (not 0) and after, and lowerIsBetter as true or false"
                ))
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(Input { title, metrics })
}

fn metric(entry: &Value) -> Option<Metric> {
    let name = entry.get("name")?.as_str().filter(|n| !n.is_empty())?;
    let unit = entry.get("unit")?.as_str()?;
    let before = entry.get("before")?.as_f64().filter(|b| *b != 0.0)?;
    let after = entry.get("after")?.as_f64()?;
    let lower_is_better = entry.get("lowerIsBetter")?.as_bool()?;
    Some(Metric {
        name: name.to_string(),
        unit: unit.to_string(),
        before,
        after,
        lower_is_better,
    })
}
