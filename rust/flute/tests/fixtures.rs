//! Replays every case FluteSharp recorded in `fixtures/flute.jsonl` and checks the Rust
//! port gives the same tree, branch for branch. See `FIXTURES.md`.

use flute::{Branch, Flute, FluteError, LookUpTable, Point};
use serde_json::Value;
use std::path::PathBuf;

fn as_i32(v: &Value) -> i32 {
    i32::try_from(v.as_i64().expect("an integer")).expect("fits in i32")
}

#[test]
fn every_recorded_tree_matches() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/flute.jsonl");
    let text = std::fs::read_to_string(&path).expect("read flute.jsonl");
    let flute = Flute::new(LookUpTable::degree6());

    let mut cases = 0;
    let mut failures = Vec::new();
    for line in text.lines() {
        let case: Value = serde_json::from_str(line).expect("valid JSON");
        let name = case["name"].as_str().expect("a name");
        let points: Vec<Point> = case["points"]
            .as_array()
            .expect("points")
            .iter()
            .map(|p| Point::new(as_i32(&p[0]), as_i32(&p[1])))
            .collect();
        cases += 1;

        let actual = flute.execute(&points);
        if let Some(error) = case.get("error") {
            let error = error.as_str().expect("an error name");
            let expected = match error {
                "OverflowException" => FluteError::NoPoints,
                other => panic!("{name}: no Rust equivalent written for {other}"),
            };
            if actual.as_ref().err() != Some(&expected) {
                failures.push(format!("{name}: expected {error}, got {actual:?}"));
            }
            continue;
        }

        let expected_branches: Vec<Branch> = case["branches"]
            .as_array()
            .expect("branches")
            .iter()
            .map(|b| Branch {
                x: as_i32(&b[0]),
                y: as_i32(&b[1]),
                n: as_i32(&b[2]),
            })
            .collect();
        match actual {
            Err(e) => failures.push(format!("{name}: expected a tree, got {e:?}")),
            Ok(tree) => {
                if tree.deg != as_i32(&case["deg"]) {
                    failures.push(format!("{name}: deg {} != {}", tree.deg, case["deg"]));
                } else if tree.length != as_i32(&case["length"]) {
                    failures.push(format!(
                        "{name}: length {} != {}",
                        tree.length, case["length"]
                    ));
                } else if tree.branch != expected_branches {
                    let first = tree
                        .branch
                        .iter()
                        .zip(&expected_branches)
                        .position(|(a, b)| a != b)
                        .unwrap_or(tree.branch.len().min(expected_branches.len()));
                    failures.push(format!(
                        "{name}: branches differ from index {first} (got {} branches, expected {})",
                        tree.branch.len(),
                        expected_branches.len()
                    ));
                }
            }
        }
    }

    assert!(
        cases > 1000,
        "expected the full fixture, read {cases} cases"
    );
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ:\n{}",
        failures.len(),
        failures
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
