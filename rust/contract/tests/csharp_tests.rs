//! Ports of `PlanOrchestratorTest`, `ContractSerializationTest` and `CleanBlueprintTest`
//! (test/FactorioTools.Test/OilField). `ContractSerializationTest.PlanRequest_SourceGen_MatchesWebAppReflection`
//! compares two .NET serializers with each other and has no Rust counterpart; the
//! recorded-contract replay in `contract.rs` covers what it protects.

use std::path::PathBuf;

use contract::{PlanRequest, normalize_json, plan_json, read_plan_request};
use serde_json::Value;

/// `PlanOrchestratorTest.SampleBlueprint`: a small valid single-pumpjack blueprint.
const SAMPLE_BLUEPRINT: &str = "0eJyMj70OwjAMhN/lZg8NbHkVhFB/rMrQuFGSIqoq707aMiCVgcWSz+fP5wXNMLEPogl2gbSjRtjLgii91sOqae0YFn5y/l63DxDS7FdFEjtkgmjHL1iTrwTWJEl4Z2zNfNPJNRyKgX6w/BjLwqjrpQI5E+ZSC7WTwO0+qTIdYKc/YKbaaOaAK0G38Pbre8KTQ/wY8hsAAP//AwAEfF3F";

fn request(blueprint: &str) -> String {
    PlanRequest {
        blueprint: Some(blueprint.to_string()),
        ..PlanRequest::default()
    }
    .to_json()
}

// PlanOrchestratorTest.Plan_ReturnsNonEmptyBlueprint and PlanJson_HappyPath_ReturnsResponseJson.
#[test]
fn plan_returns_non_empty_blueprint() {
    let response: Value =
        serde_json::from_str(&plan_json(&request(SAMPLE_BLUEPRINT)).unwrap()).unwrap();
    assert!(!response["blueprint"].as_str().unwrap().is_empty());
    assert!(response["summary"].is_object());
}

// PlanOrchestratorTest.Normalize_ReturnsNonEmptyBlueprint.
#[test]
fn normalize_returns_non_empty_blueprint() {
    let json = format!("{{\"blueprint\":\"{SAMPLE_BLUEPRINT}\"}}");
    let response: Value = serde_json::from_str(&normalize_json(&json).unwrap()).unwrap();
    assert!(!response["blueprint"].as_str().unwrap().is_empty());
}

// PlanOrchestratorTest.Plan_BadInput_ThrowsFactorioToolsException and
// PlanJson_BadInput_ReturnsErrorEnvelope.
#[test]
fn plan_bad_input_returns_error_envelope() {
    let response: Value =
        serde_json::from_str(&plan_json(&request("not-a-blueprint")).unwrap()).unwrap();
    assert_eq!(response["status"], 400);
    let messages = response["errors"]["FactorioToolsException"]
        .as_array()
        .unwrap();
    assert!(!messages.is_empty());
}

// ContractSerializationTest.PlanRequest_RoundTrips_WithStringEnums.
#[test]
fn plan_request_round_trips_with_string_enums() {
    let json = request("BP");
    assert!(
        json.contains("\"ConnectedCentersDelaunay\""),
        "enum as PascalCase string"
    );
    assert!(json.contains("\"blueprint\":\"BP\""), "property camelCase");
    let back = read_plan_request(&json).unwrap();
    assert_eq!(back.blueprint.as_deref(), Some("BP"));
}

fn oil_field_test_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../test/FactorioTools.Test/OilField")
}

/// `CleanBlueprintTest.VerifySameBlueprint`: the corpus is already normalized, so cleaning a
/// blueprint gives it back unchanged (the C# compares the two serialized; this compares the
/// parsed models, which carry the same fields).
fn verify_same_blueprint(input: &str) {
    let blueprint = blueprint::parse_blueprint(input).unwrap();
    let mut clean = planner::steps::clean_blueprint::execute(blueprint.clone()).unwrap();
    clean.version = blueprint.version;
    assert_eq!(clean, blueprint);
}

// CleanBlueprintTest.SmallListBlueprintsAreNormalized.
#[test]
fn small_list_blueprints_are_normalized() {
    for line in blueprint::read_blueprint_file(oil_field_test_dir().join("small-list.txt")).unwrap()
    {
        verify_same_blueprint(&line);
    }
}

// CleanBlueprintTest.BigListBlueprintsAreNormalized.
#[test]
fn big_list_blueprints_are_normalized() {
    for line in blueprint::read_blueprint_file(oil_field_test_dir().join("big-list.txt")).unwrap() {
        verify_same_blueprint(&line);
    }
}
