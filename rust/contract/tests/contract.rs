//! The Rust contract against `PlanOrchestrator.PlanJson` and `NormalizeJson`, replayed from
//! `fixtures/contract.jsonl` (see `FIXTURES.md`).
//!
//! A response must match the recorded text exactly, except that blueprint strings are
//! compared after decompressing: zlib's output bytes differ between .NET and Rust, while the
//! JSON inside must be the same text. An error envelope must match exactly too, except the
//! inner messages that come from `System.Text.Json`'s reader, whose wording the Rust port
//! does not copy: there the outer message, title and status must match.

use std::path::PathBuf;

use blueprint::json_out::write_string;
use planner::models::AvoidLocation;
use planner::options::OilFieldOptions;
use serde_json::Value;

fn fixture() -> Vec<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/contract.jsonl");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
        .lines()
        .map(|line| serde_json::from_str(line).expect("a JSON line"))
        .collect()
}

/// Every top-level or `request.blueprint` string value that is a blueprint string, replaced
/// by a placeholder, with the decoded JSON texts returned beside it.
fn split_blueprints(response: &str) -> (String, Vec<String>) {
    let value: Value = serde_json::from_str(response).expect("a JSON response");
    let mut text = response.to_string();
    let mut decoded = Vec::new();
    let mut candidates = Vec::new();
    if let Some(s) = value.get("blueprint").and_then(Value::as_str) {
        candidates.push(s.to_string());
    }
    for s in candidates {
        let mut escaped = String::new();
        write_string(&mut escaped, &s);
        let needle = format!("}},\"blueprint\":{escaped}");
        assert!(
            text.contains(&needle),
            "the output blueprint follows the request object"
        );
        text = text.replacen(&needle, "},\"blueprint\":\"<blueprint>\"", 1);
        decoded.push(blueprint::decode_blueprint_json(&s).expect("a decodable blueprint"));
    }
    (text, decoded)
}

fn is_envelope(response: &str) -> bool {
    response.starts_with("{\"title\":")
}

/// Inner messages written by `System.Text.Json`'s reader, which the port does not copy.
fn has_json_reader_inner(expected: &Value) -> bool {
    expected["errors"]["FactorioToolsException"]
        .as_array()
        .is_some_and(|m| {
            m.iter()
                .skip(1)
                .any(|s| s.as_str().is_some_and(|s| s.contains("LineNumber")))
        })
}

/// Slow in a debug build (about two minutes, against nine seconds in release), so it runs in
/// release, with the snapshot gate: `cargo test --release -p contract -- --include-ignored`.
#[test]
#[ignore]
fn contract_matches_the_csharp() {
    let cases = fixture();
    let mut exact = 0;
    let mut envelopes_outer_only = 0;
    let mut thrown = 0;
    let mut grids = 0;

    for case in &cases {
        let name = case["name"].as_str().unwrap();
        let op = case["op"].as_str().unwrap();

        if op == "grid_to_blueprint_string" {
            let parsed = blueprint::parse_blueprint(case["blueprint"].as_str().unwrap()).unwrap();
            let avoid: Vec<AvoidLocation> = case["avoid"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| {
                    AvoidLocation::new(a[0].as_f64().unwrap() as f32, a[1].as_f64().unwrap() as f32)
                })
                .collect();
            let result = planner::planner::execute_with_avoid(
                OilFieldOptions::for_medium_electric_pole(),
                parsed,
                &avoid,
            );
            match case["outcome"].as_str().unwrap() {
                "ok" => {
                    let result = result.unwrap_or_else(|e| panic!("{name}: {e}"));
                    let text = blueprint::grid_to_blueprint_string(
                        &result.context,
                        case["add_fbe_offset"].as_bool().unwrap(),
                        true,
                    );
                    let json = blueprint::decode_blueprint_json(&text).unwrap();
                    assert_eq!(json, case["json"].as_str().unwrap(), "{name}");
                }
                _ => assert!(result.is_err(), "{name}: the C# threw"),
            }
            grids += 1;
            continue;
        }

        let request = case["request"].as_str().unwrap();
        let actual = match op {
            "plan" => contract::plan_json(request),
            "normalize" => contract::normalize_json(request),
            other => panic!("{name}: unknown op {other}"),
        };

        if case["outcome"] == "threw" {
            let expected = case["exception"].as_str().unwrap();
            match actual {
                Err(t) => assert_eq!(t.exception, expected, "{name} ({op}): exception type"),
                Ok(text) => panic!("{name} ({op}): the C# threw {expected}, Rust returned {text}"),
            }
            thrown += 1;
            continue;
        }

        let expected = case["response"].as_str().unwrap();
        let actual = actual.unwrap_or_else(|t| panic!("{name} ({op}): Rust threw {t}"));

        if is_envelope(expected) {
            let expected_value: Value = serde_json::from_str(expected).unwrap();
            if has_json_reader_inner(&expected_value) {
                let actual_value: Value = serde_json::from_str(&actual).unwrap();
                assert_eq!(
                    actual_value["title"], expected_value["title"],
                    "{name} ({op})"
                );
                assert_eq!(
                    actual_value["status"], expected_value["status"],
                    "{name} ({op})"
                );
                assert_eq!(
                    actual_value["errors"]["FactorioToolsException"][0],
                    expected_value["errors"]["FactorioToolsException"][0],
                    "{name} ({op})"
                );
                envelopes_outer_only += 1;
            } else {
                assert_eq!(actual, expected, "{name} ({op})");
                exact += 1;
            }
            continue;
        }

        let (expected_text, expected_blueprints) = split_blueprints(expected);
        let (actual_text, actual_blueprints) = split_blueprints(&actual);
        assert_eq!(actual_text, expected_text, "{name} ({op}): response text");
        assert_eq!(
            actual_blueprints, expected_blueprints,
            "{name} ({op}): decoded blueprint JSON"
        );
        exact += 1;
    }

    println!(
        "{} cases: {exact} exact, {envelopes_outer_only} envelopes compared on the outer message, {thrown} thrown, {grids} grid_to_blueprint_string",
        cases.len()
    );
    assert!(exact > 300, "the fixture is mostly exact comparisons");
}
