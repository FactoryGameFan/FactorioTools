//! `test/FactorioTools.Test/OilField/FactorioOracleTest.cs`, ported: the planner's hardcoded
//! Factorio facts checked against an oracle captured from the game itself
//! (`tools/capture-factorio-oracle.sh`).
//!
//! This reads the COMMITTED fixture, never the game, so CI needs no Factorio install.
//! Re-capture after a Factorio update and commit the diff; a changed fixture failing here is
//! the signal that a constant needs review.
//!
//! `EntityNameToSize` lives in the blueprint crate, so its check is a unit test there
//! (`blueprint::write::tests`). This file checks the footprints the planner itself uses.

use std::collections::HashSet;
use std::path::PathBuf;

use planner::Location;
use planner::data::{Direction, entity_names, item_names};
use planner::helpers::{PUMPJACK_HEIGHT, PUMPJACK_WIDTH, TERMINAL_OFFSETS};
use planner::options::OilFieldOptions;
use planner::steps::plan_underground_pipes::MAX_UNDERGROUND;
use serde_json::Value;

use entity_names::vanilla;

/// The one place this file names the fixture's location.
fn factorio_oracle_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../test/FactorioTools.Test/OilField/factorio-oracle.json")
}

const RE_CAPTURE_HINT: &str = "Two likely causes: either Factorio changed (re-run \
    tools/capture-factorio-oracle.sh and review the diff), or a name was added to \
    entity_names::vanilla without adding it to WANTED_ENTITIES in tools/trim-factorio-oracle.py.";

fn oracle() -> Value {
    let text = std::fs::read_to_string(factorio_oracle_path()).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// `Names`: the keys of an object or the strings of an array.
fn names(parent: &Value, property: &str) -> HashSet<String> {
    match &parent[property] {
        Value::Object(map) => map.keys().cloned().collect(),
        Value::Array(items) => items
            .iter()
            .map(|x| x.as_str().unwrap().to_string())
            .collect(),
        other => panic!("{property} is neither an object nor an array: {other}"),
    }
}

fn coordinates(point: &Value) -> Vec<f64> {
    point
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

/// `EveryVanillaEntityNameExistsInFactorio`. This checks `entity_names::vanilla` only, so
/// `entity_names::aai_industry` is deliberately NOT checked. Those names come from the AAI
/// Industry mod, and the oracle is captured with mods disabled on purpose, so their absence
/// is expected rather than drift.
#[test]
fn every_vanilla_entity_name_exists_in_factorio() {
    let entities = names(&oracle(), "entities");

    let missing: Vec<_> = vanilla::ALL
        .into_iter()
        .filter(|name| !entities.contains(*name))
        .collect();

    assert!(
        missing.is_empty(),
        "Not in Factorio: {}. {RE_CAPTURE_HINT}",
        missing.join(", ")
    );
}

/// `EveryVanillaModuleNameExistsInFactorio`.
#[test]
fn every_vanilla_module_name_exists_in_factorio() {
    let modules = names(&oracle(), "modules");

    let missing: Vec<_> = item_names::vanilla::ALL
        .into_iter()
        // "blueprint" is an item, not a module, so it is not in the module list.
        .filter(|name| *name != item_names::vanilla::BLUEPRINT)
        .filter(|name| !modules.contains(*name))
        .collect();

    assert!(
        missing.is_empty(),
        "Not a Factorio module: {}. {RE_CAPTURE_HINT}",
        missing.join(", ")
    );
}

/// `TerminalOffsetsMatchTheFactorioOutputCorners`. `TERMINAL_OFFSETS` says which tile a
/// pumpjack's pipe occupies, one entry per rotation. The fixture records the prototype's
/// `output_fluid_box.pipe_connections` positions, also one per rotation in north, east,
/// south, west order. The position is the connection point inside the entity, and the pipe
/// sits one tile further out in the direction the entity faces.
///
/// That step-one-tile-outward rule was measured, not assumed: a probe mod read
/// `PipeConnection.target_position` out of running 2.0.77 and 2.1.14 games and the rule
/// reproduced both. Factorio 2.1 gave each rotation its own output corner (FFF #442), which
/// is why the east and west offsets moved. See issue #81 and the C# comment.
#[test]
fn terminal_offsets_match_the_factorio_output_corners() {
    let cases = [
        (0, Direction::Up, 0, -1),
        (1, Direction::Right, 1, 0),
        (2, Direction::Down, 0, 1),
        (3, Direction::Left, -1, 0),
    ];
    let oracle = oracle();
    let positions = &oracle["entities"][vanilla::PUMPJACK]["output_fluid_box"]["pipe_connections"]
        [0]["positions"];

    for (rotation, direction, step_x, step_y) in cases {
        let position = coordinates(&positions[rotation]);
        let expected = Location::new(position[0] as i32 + step_x, position[1] as i32 + step_y);

        let actual = TERMINAL_OFFSETS
            .iter()
            .find(|(d, _)| *d == direction)
            .map(|(_, offset)| *offset)
            .unwrap();

        assert!(
            expected == actual,
            "Factorio puts the {direction} pumpjack's pipe at {expected:?}, but TERMINAL_OFFSETS \
             says {actual:?}. Every plan would attach the pipe to the wrong corner. \
             {RE_CAPTURE_HINT}"
        );
    }
}

/// `InternalDirectionDoublesToTheFactorioValue`. The internal enum is deliberately 1.1-style
/// four-way (N=0, E=2, S=4, W=6). Factorio 2.0 is 16-way, so the blueprint value is always
/// exactly double.
#[test]
fn internal_direction_doubles_to_the_factorio_value() {
    let cases = [
        (Direction::Up, "north"),
        (Direction::Right, "east"),
        (Direction::Down, "south"),
        (Direction::Left, "west"),
    ];
    let oracle = oracle();

    for (direction, factorio_name) in cases {
        let expected = oracle["directions"][factorio_name].as_i64().unwrap();
        assert_eq!(
            expected,
            i64::from(direction.value() * 2),
            "{direction} vs {factorio_name}"
        );
    }
}

/// `RawGeometryBehindTheOptionsPresetsIsUnchanged`. As in the C#, the values are pinned
/// here rather than read from `OilFieldOptions`: the presets derive tile counts from these,
/// so a move here means the presets need review.
#[test]
fn raw_geometry_behind_the_options_presets_is_unchanged() {
    let cases = [
        (vanilla::SMALL_ELECTRIC_POLE, "supply_area_distance", 2.5),
        (vanilla::MEDIUM_ELECTRIC_POLE, "supply_area_distance", 3.5),
        (vanilla::BIG_ELECTRIC_POLE, "supply_area_distance", 2.0),
        (vanilla::SUBSTATION, "supply_area_distance", 9.0),
        (vanilla::SMALL_ELECTRIC_POLE, "maximum_wire_distance", 7.5),
        (vanilla::MEDIUM_ELECTRIC_POLE, "maximum_wire_distance", 9.0),
        (vanilla::BIG_ELECTRIC_POLE, "maximum_wire_distance", 32.0),
        (vanilla::SUBSTATION, "maximum_wire_distance", 18.0),
        (vanilla::BEACON, "supply_area_distance", 3.0),
        // Beacon strength itself, not just its supply area. distribution_effectivity is the
        // multiplier a beacon applies to the module effects it broadcasts (1.5 = 150%).
        // Nothing in the planner reads this value today, but pinning it means a Factorio
        // balance change that alters beacon strength is noticed here rather than silently
        // changing plan quality.
        (vanilla::BEACON, "distribution_effectivity", 1.5),
    ];
    let oracle = oracle();

    for (entity, field, expected) in cases {
        let actual = oracle["entities"][entity][field].as_f64().unwrap();
        assert!(
            expected == actual,
            "{entity}.{field} moved from {expected} to {actual}, so the OilFieldOptions presets \
             need review. {RE_CAPTURE_HINT}"
        );
    }
}

/// The planner's own footprints, checked the way
/// `EntityNameToSizeMatchesTheDerivedCollisionBoxFootprint` checks the blueprint writer's:
/// the collision box rounded up to whole tiles. The C# has no such test; it is here because
/// the planner keeps these sizes apart from `EntityNameToSize`, in `PUMPJACK_WIDTH` and the
/// `OilFieldOptions` presets. The AAI Industry preset is left out, as its pole is a mod
/// entity.
#[test]
fn planner_footprints_match_the_derived_collision_box_footprint() {
    let mut sizes = vec![(vanilla::PUMPJACK, PUMPJACK_WIDTH, PUMPJACK_HEIGHT)];
    for options in [
        OilFieldOptions::for_small_electric_pole(),
        OilFieldOptions::for_medium_electric_pole(),
        OilFieldOptions::for_big_electric_pole(),
        OilFieldOptions::for_substation(),
    ] {
        sizes.push((
            vanilla::ALL
                .into_iter()
                .find(|name| *name == options.electric_pole_entity_name)
                .unwrap(),
            options.electric_pole_width,
            options.electric_pole_height,
        ));
    }
    let options = OilFieldOptions::default();
    assert_eq!(options.beacon_entity_name, vanilla::BEACON);
    sizes.push((vanilla::BEACON, options.beacon_width, options.beacon_height));

    let oracle = oracle();
    let mut mismatches = Vec::new();
    for (name, width, height) in sizes {
        let collision_box = &oracle["entities"][name]["collision_box"];
        let (min, max) = (
            coordinates(&collision_box[0]),
            coordinates(&collision_box[1]),
        );
        let derived = (
            (max[0] - min[0]).ceil() as i32,
            (max[1] - min[1]).ceil() as i32,
        );
        if (width, height) != derived {
            mismatches.push(format!(
                "{name}: the planner says {width}x{height}, collision_box derives {}x{}",
                derived.0, derived.1
            ));
        }
    }

    assert!(
        mismatches.is_empty(),
        "{}. {RE_CAPTURE_HINT}",
        mismatches.join("; ")
    );
}

/// `MaxUndergroundIsOneMoreThanFactoriosGapDistance`. `MAX_UNDERGROUND` (11) counts both
/// underground ends of a run, while Factorio's `max_underground_distance` (10, on
/// pipe-to-ground) counts only the gap between them. So the two are off by exactly one, and
/// that relationship is the thing worth pinning.
#[test]
fn max_underground_is_one_more_than_factorios_gap_distance() {
    let oracle = oracle();
    let max_underground_distance =
        oracle["entities"][vanilla::PIPE_TO_GROUND]["fluid_box"]["pipe_connections"]
            .as_array()
            .unwrap()
            .iter()
            .find_map(|c| c.get("max_underground_distance"))
            .unwrap()
            .as_u64()
            .unwrap() as usize;

    assert!(
        MAX_UNDERGROUND == max_underground_distance + 1,
        "MAX_UNDERGROUND is {MAX_UNDERGROUND} but max_underground_distance is now \
         {max_underground_distance}, so the relationship (MAX_UNDERGROUND == \
         max_underground_distance + 1) no longer holds. {RE_CAPTURE_HINT}"
    );
}
