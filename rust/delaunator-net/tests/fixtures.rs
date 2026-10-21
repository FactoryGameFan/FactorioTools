//! Replays `fixtures/delaunator.jsonl`, recorded from DelaunatorSharp by `oracle/` (removed
//! in stage 13b, so the file is frozen), and checks every triangulation matches exactly.

use delaunator_net::{Delaunator, DelaunatorError};
use serde_json::Value;
use std::path::PathBuf;

fn ints(value: &Value) -> Vec<i32> {
    value
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_i64().unwrap() as i32)
        .collect()
}

#[test]
fn matches_delaunator_sharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/delaunator.jsonl");
    let text = std::fs::read_to_string(&path).expect("read fixtures");
    let mut cases = 0;
    let mut failures = Vec::new();
    for line in text.lines() {
        let case: Value = serde_json::from_str(line).unwrap();
        let name = case["name"].as_str().unwrap();
        let points: Vec<(f64, f64)> = case["points"]
            .as_array()
            .unwrap()
            .iter()
            .map(|p| (p[0].as_f64().unwrap(), p[1].as_f64().unwrap()))
            .collect();

        let actual = Delaunator::new(&points);
        let ok = match (&actual, case.get("error").and_then(Value::as_str)) {
            (Err(DelaunatorError::TooFewPoints), Some("ArgumentOutOfRangeException")) => true,
            (Err(DelaunatorError::NoTriangulation), Some("Exception")) => true,
            (Ok(d), None) => {
                d.triangles == ints(&case["triangles"]) && d.halfedges == ints(&case["halfedges"])
            }
            _ => false,
        };
        if !ok {
            failures.push(name.to_string());
        }
        cases += 1;
    }

    assert!(cases > 2000, "only {cases} cases read");
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ from DelaunatorSharp, first: {:?}",
        failures.len(),
        &failures[..failures.len().min(10)]
    );
}
