//! The `Emits*` and `OmitsQualityFieldWhenNormal` tests from `PlannerTest.cs`: the blueprint
//! string written for a plan.

use std::path::PathBuf;

use blueprint::write::parse_version;
use blueprint::{decode_blueprint_json, grid_to_blueprint_string, parse_blueprint};
use planner::data::entity_names;
use planner::options::OilFieldOptions;
use planner::quality::Quality;

fn first_small_list_blueprint() -> planner::data::Blueprint {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../testdata/oil-field/small-list.txt");
    let lines = blueprint::read_blueprint_file(path).unwrap();
    parse_blueprint(&lines[0]).unwrap()
}

fn plan(options: OilFieldOptions) -> String {
    let result = planner::planner::execute(options, first_small_list_blueprint()).unwrap();
    grid_to_blueprint_string(&result.context, false, false)
}

#[test]
fn emits_heat_pipes_in_two_point_zero_blueprint() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.add_heat_pipes = true;
    let parsed = parse_blueprint(&plan(options)).unwrap();
    assert_eq!(parse_version(parsed.version).0, 2);
    assert!(
        parsed
            .entities
            .iter()
            .any(|e| e.name == entity_names::vanilla::HEAT_PIPE)
    );
}

#[test]
fn emits_two_point_zero_and_entity_quality_without_heat() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.pumpjack_quality = Quality::LEGENDARY;
    options.beacon_quality = Quality::RARE;
    options.electric_pole_quality = Quality::UNCOMMON;
    let pole = options.electric_pole_entity_name.clone();
    let parsed = parse_blueprint(&plan(options)).unwrap();
    assert_eq!(parse_version(parsed.version).0, 2);
    let has = |name: &str, quality: &str| {
        parsed
            .entities
            .iter()
            .any(|e| e.name == name && e.quality.as_deref() == Some(quality))
    };
    assert!(has(entity_names::vanilla::PUMPJACK, "legendary"));
    assert!(has(entity_names::vanilla::BEACON, "rare"));
    assert!(has(&pole, "uncommon"));
}

#[test]
fn omits_quality_field_when_normal() {
    let parsed = parse_blueprint(&plan(OilFieldOptions::for_medium_electric_pole())).unwrap();
    assert!(parsed.entities.iter().all(|e| e.quality.is_none()));
}

#[test]
fn emits_module_quality_in_items_array() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.pumpjack_module_quality = Quality::EPIC;
    options.beacon_module_quality = Quality::LEGENDARY;
    let json = decode_blueprint_json(&plan(options)).unwrap();
    assert!(json.contains("\"quality\":\"epic\""));
    assert!(json.contains("\"quality\":\"legendary\""));
}
