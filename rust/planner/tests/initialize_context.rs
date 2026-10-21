//! Replays every case in `fixtures/initialize_context.jsonl` (recorded from the C#
//! `InitializeContext.Execute`, see `FIXTURES.md`) and checks the Rust context field by
//! field, in .NET's enumeration order. Also checks the Rust option presets against the C#.

use std::path::PathBuf;

use planner::Location;
use planner::context::Context;
use planner::data::{Blueprint, Direction, Entity, Position};
use planner::grid::{EntityKind, TerminalLocation};
use planner::models::{AvoidLocation, BeaconStrategy, PipeStrategy};
use planner::options::OilFieldOptions;
use planner::quality::Quality;
use planner::steps::initialize_context::execute_with_min;
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
}

fn float_bits(value: &Value) -> f32 {
    f32::from_bits(value.as_i64().expect("float bits") as i32 as u32)
}

fn double_bits(value: &Value) -> f64 {
    f64::from_bits(value.as_i64().expect("double bits") as u64)
}

fn direction(value: &Value) -> Direction {
    Direction::from_value(int(value)).expect("a Direction value")
}

fn string(value: &Value) -> String {
    value.as_str().expect("a string").to_string()
}

fn array(value: &Value) -> &Vec<Value> {
    value.as_array().expect("an array")
}

fn modules(value: &Value) -> Vec<(String, i32)> {
    array(value)
        .iter()
        .map(|m| (string(&m[0]), int(&m[1])))
        .collect()
}

fn pipe_strategy(value: &Value) -> PipeStrategy {
    PipeStrategy::ALL[usize::try_from(int(value)).expect("a PipeStrategy value")]
}

fn beacon_strategy(value: &Value) -> BeaconStrategy {
    BeaconStrategy::ALL[usize::try_from(int(value)).expect("a BeaconStrategy value")]
}

fn options(o: &Value) -> OilFieldOptions {
    OilFieldOptions {
        use_underground_pipes: o["use_underground_pipes"].as_bool().unwrap(),
        add_beacons: o["add_beacons"].as_bool().unwrap(),
        optimize_pipes: o["optimize_pipes"].as_bool().unwrap(),
        overlap_beacons: o["overlap_beacons"].as_bool().unwrap(),
        add_electric_poles: o["add_electric_poles"].as_bool().unwrap(),
        add_heat_pipes: o["add_heat_pipes"].as_bool().unwrap(),
        heat_pipe_entity_name: string(&o["heat_pipe_entity_name"]),
        pipe_strategies: array(&o["pipe_strategies"])
            .iter()
            .map(pipe_strategy)
            .collect(),
        beacon_strategies: array(&o["beacon_strategies"])
            .iter()
            .map(beacon_strategy)
            .collect(),
        electric_pole_entity_name: string(&o["electric_pole_entity_name"]),
        electric_pole_supply_width: int(&o["electric_pole_supply_width"]),
        electric_pole_supply_height: int(&o["electric_pole_supply_height"]),
        electric_pole_wire_reach: double_bits(&o["electric_pole_wire_reach_bits"]),
        electric_pole_width: int(&o["electric_pole_width"]),
        electric_pole_height: int(&o["electric_pole_height"]),
        beacon_entity_name: string(&o["beacon_entity_name"]),
        beacon_supply_width: int(&o["beacon_supply_width"]),
        beacon_supply_height: int(&o["beacon_supply_height"]),
        beacon_width: int(&o["beacon_width"]),
        beacon_height: int(&o["beacon_height"]),
        validate_solution: o["validate_solution"].as_bool().unwrap(),
        pumpjack_modules: modules(&o["pumpjack_modules"]),
        beacon_modules: modules(&o["beacon_modules"]),
        pumpjack_quality: Quality(int(&o["pumpjack_quality"])),
        beacon_quality: Quality(int(&o["beacon_quality"])),
        electric_pole_quality: Quality(int(&o["electric_pole_quality"])),
        pumpjack_module_quality: Quality(int(&o["pumpjack_module_quality"])),
        beacon_module_quality: Quality(int(&o["beacon_module_quality"])),
    }
}

/// `[entity_number, name, x_bits, y_bits, direction or null, mirror or null]`.
fn blueprint(entities: &Value) -> Blueprint {
    Blueprint {
        entities: array(entities)
            .iter()
            .map(|e| Entity {
                entity_number: int(&e[0]),
                name: string(&e[1]),
                position: Position {
                    x: float_bits(&e[2]),
                    y: float_bits(&e[3]),
                },
                direction: (!e[4].is_null()).then(|| direction(&e[4])),
                mirror: e[5].as_bool(),
                ..Entity::default()
            })
            .collect(),
        ..Blueprint::default()
    }
}

/// `[center_x, center_y, terminal_x, terminal_y, direction]`.
fn terminal(t: &Value) -> TerminalLocation {
    TerminalLocation::new(
        Location::new(int(&t[0]), int(&t[1])),
        Location::new(int(&t[2]), int(&t[3])),
        direction(&t[4]),
    )
}

fn terminal_dictionary(value: &Value) -> Vec<(Location, Vec<TerminalLocation>)> {
    array(value)
        .iter()
        .map(|pair| {
            (
                Location::new(int(&pair[0]), int(&pair[1])),
                array(&pair[2]).iter().map(terminal).collect(),
            )
        })
        .collect()
}

/// `[x, y, kind, id, ...extra]` for the kinds `InitializeContext` places.
fn recorded_entity(fields: &Value) -> (Location, i32, EntityKind) {
    let location = Location::new(int(&fields[0]), int(&fields[1]));
    let id = int(&fields[3]);
    let kind = match fields[2].as_str().expect("a kind") {
        "PumpjackCenter" => EntityKind::PumpjackCenter {
            direction: direction(&fields[4]),
        },
        "PumpjackSide" => EntityKind::PumpjackSide {
            center: int(&fields[4]),
        },
        "AvoidEntity" => EntityKind::AvoidEntity,
        other => panic!("unexpected kind {other}"),
    };
    (location, id, kind)
}

fn same_kind(a: &EntityKind, b: &EntityKind) -> bool {
    match (a, b) {
        (
            EntityKind::PumpjackCenter { direction: x },
            EntityKind::PumpjackCenter { direction: y },
        ) => x == y,
        (EntityKind::PumpjackSide { center: x }, EntityKind::PumpjackSide { center: y }) => x == y,
        (EntityKind::AvoidEntity, EntityKind::AvoidEntity) => true,
        _ => false,
    }
}

fn check_context(name: &str, context: &mut Context, expected: &Value) {
    assert_eq!(
        context.delta_x.to_bits(),
        float_bits(&expected["delta_x_bits"]).to_bits(),
        "{name}: delta_x ({} vs {})",
        context.delta_x,
        float_bits(&expected["delta_x_bits"])
    );
    assert_eq!(
        context.delta_y.to_bits(),
        float_bits(&expected["delta_y_bits"]).to_bits(),
        "{name}: delta_y"
    );
    assert_eq!(
        context.grid.width(),
        int(&expected["width"]),
        "{name}: width"
    );
    assert_eq!(
        context.grid.height(),
        int(&expected["height"]),
        "{name}: height"
    );
    let middle = &expected["middle"];
    assert_eq!(
        context.grid.middle(),
        Location::new(int(&middle[0]), int(&middle[1])),
        "{name}: middle"
    );

    let recorded: Vec<_> = array(&expected["entities"])
        .iter()
        .map(recorded_entity)
        .collect();
    let actual_order = context.grid.entity_locations().to_vec();
    let expected_order: Vec<_> = recorded.iter().map(|(l, _, _)| *l).collect();
    assert_eq!(
        actual_order, expected_order,
        "{name}: EntityLocations order"
    );
    for (location, id, kind) in &recorded {
        let entity = context.grid.get(*location).expect("an entity");
        assert_eq!(entity.id, *id, "{name}: id at {location:?}");
        assert!(
            same_kind(&entity.kind, kind),
            "{name}: {location:?} is {:?}, C# has {kind:?}",
            entity.kind
        );
    }

    assert_eq!(
        context.grid.get_id(),
        int(&expected["next_id"]),
        "{name}: next id"
    );

    let centers: Vec<_> = array(&expected["centers"])
        .iter()
        .map(|c| Location::new(int(&c[0]), int(&c[1])))
        .collect();
    assert_eq!(context.centers, centers, "{name}: centers");

    let actual: Vec<_> = context
        .center_to_terminals
        .iter()
        .map(|(k, v)| (k, v.clone()))
        .collect();
    assert_eq!(
        actual,
        terminal_dictionary(&expected["center_to_terminals"]),
        "{name}: center_to_terminals"
    );

    let actual: Vec<_> = context
        .center_to_original_direction
        .iter()
        .map(|(k, v)| (k, *v))
        .collect();
    let original: Vec<_> = array(&expected["center_to_original_direction"])
        .iter()
        .map(|p| (Location::new(int(&p[0]), int(&p[1])), direction(&p[2])))
        .collect();
    assert_eq!(actual, original, "{name}: center_to_original_direction");

    let actual: Vec<_> = context
        .location_to_terminals
        .iter()
        .map(|(k, v)| (k, v.clone()))
        .collect();
    assert_eq!(
        actual,
        terminal_dictionary(&expected["location_to_terminals"]),
        "{name}: location_to_terminals"
    );

    let mut counts = vec![0; usize::try_from(int(&expected["adjacent_count_length"])).unwrap()];
    for pair in array(&expected["adjacent_counts"]) {
        counts[usize::try_from(int(&pair[0])).unwrap()] = int(&pair[1]);
    }
    assert_eq!(
        context.location_to_adjacent_count, counts,
        "{name}: adjacent counts"
    );

    assert_eq!(
        context.electric_pole_supply_width_with_quality,
        int(&expected["electric_pole_supply_width_with_quality"]),
        "{name}: pole supply width"
    );
    assert_eq!(
        context.electric_pole_supply_height_with_quality,
        int(&expected["electric_pole_supply_height_with_quality"]),
        "{name}: pole supply height"
    );
    assert_eq!(
        context.electric_pole_wire_reach_with_quality.to_bits(),
        double_bits(&expected["electric_pole_wire_reach_with_quality_bits"]).to_bits(),
        "{name}: pole wire reach"
    );
    assert_eq!(
        context
            .electric_pole_wire_reach_squared_with_quality
            .to_bits(),
        double_bits(&expected["electric_pole_wire_reach_squared_with_quality_bits"]).to_bits(),
        "{name}: pole wire reach squared"
    );
}

fn preset(name: &str) -> OilFieldOptions {
    match name {
        "Default" => OilFieldOptions::default(),
        "ForSmallIronElectricPole" => OilFieldOptions::for_small_iron_electric_pole(),
        "ForSmallElectricPole" => OilFieldOptions::for_small_electric_pole(),
        "ForMediumElectricPole" => OilFieldOptions::for_medium_electric_pole(),
        "ForBigElectricPole" => OilFieldOptions::for_big_electric_pole(),
        "ForSubstation" => OilFieldOptions::for_substation(),
        other => panic!("unknown preset {other}"),
    }
}

#[test]
fn initialize_context_matches_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/initialize_context.jsonl");
    let text = std::fs::read_to_string(&path).expect("read initialize_context.jsonl");

    let (mut presets, mut contexts, mut errors) = (0, 0, 0);
    for line in text.lines().filter(|l| !l.is_empty()) {
        let case: Value = serde_json::from_str(line).expect("valid JSON");
        let name = case["name"].as_str().expect("a name");
        let recorded_options = options(&case["options"]);

        if case["kind"] == "preset" {
            assert_eq!(preset(name), recorded_options, "preset {name}");
            presets += 1;
            continue;
        }

        let avoid: Vec<_> = array(&case["avoid"])
            .iter()
            .map(|a| AvoidLocation::new(float_bits(&a[0]), float_bits(&a[1])))
            .collect();
        let min = &case["min"];
        let result = execute_with_min(
            recorded_options,
            blueprint(&case["entities"]),
            &avoid,
            int(&min[0]),
            int(&min[1]),
        );

        match (result, case.get("error")) {
            (Ok(mut context), None) => {
                check_context(name, &mut context, &case["context"]);
                contexts += 1;
            }
            (Err(e), Some(expected)) => {
                assert_eq!(e.message, string(&expected["message"]), "{name}: message");
                assert_eq!(
                    e.bad_input,
                    expected["bad_input"].as_bool().unwrap(),
                    "{name}: bad_input"
                );
                errors += 1;
            }
            (Ok(_), Some(expected)) => panic!("{name}: C# failed with {expected}, Rust succeeded"),
            (Err(e), None) => panic!("{name}: Rust failed with {e:?}, C# succeeded"),
        }
    }

    assert!(contexts > 0, "no cases in {}", path.display());
    eprintln!("initialize_context.jsonl: {presets} presets, {contexts} contexts, {errors} errors");
}
