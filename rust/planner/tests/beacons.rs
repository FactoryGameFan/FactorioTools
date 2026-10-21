//! Replays `fixtures/beacons.jsonl` (recorded from the C# by `oracle/BeaconsRecorder.cs`,
//! removed in stage 13b; format in `FIXTURES.md`): each `PlanBeacons.Execute` call, on a
//! context rebuilt from its recorded snapshot the same way the recorder rebuilt it, must
//! give the recorded solutions (or error), and leave the grid's entity order and next id
//! where the C# left them.

use std::path::PathBuf;

use dotnet_compat::NetHashSet;
use planner::Location;
use planner::containers::{LocationDictionary, LocationSet};
use planner::context::Context;
use planner::data::{Blueprint, Direction};
use planner::grid::{EntityKind, GridEntity, SquareGrid};
use planner::models::BeaconStrategy;
use planner::options::OilFieldOptions;
use planner::steps::plan_beacons;
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
}

fn boolean(value: &Value) -> bool {
    value.as_bool().expect("a bool")
}

fn arr(value: &Value) -> &Vec<Value> {
    value.as_array().expect("an array")
}

/// A flat `[x0, y0, x1, y1, ...]` array.
fn points(value: &Value) -> Vec<Location> {
    arr(value)
        .chunks(2)
        .map(|p| Location::new(int(&p[0]), int(&p[1])))
        .collect()
}

fn strategy(value: i32) -> BeaconStrategy {
    BeaconStrategy::ALL
        .into_iter()
        .find(|s| s.value() == value)
        .expect("a BeaconStrategy value")
}

fn entity(fields: &[Value]) -> (Location, GridEntity) {
    let location = Location::new(int(&fields[0]), int(&fields[1]));
    let id = int(&fields[3]);
    let kind = match fields[2].as_str().expect("a kind") {
        "PumpjackCenter" => EntityKind::PumpjackCenter {
            direction: Direction::from_value(int(&fields[4])).expect("a Direction value"),
        },
        "PumpjackSide" => EntityKind::PumpjackSide {
            center: int(&fields[4]),
        },
        "ElectricPoleCenter" => {
            let mut neighbors = NetHashSet::new();
            for n in arr(&fields[4]) {
                neighbors.add(int(n));
            }
            EntityKind::ElectricPoleCenter { neighbors }
        }
        "ElectricPoleSide" => EntityKind::ElectricPoleSide {
            center: int(&fields[4]),
        },
        "TemporaryEntity" => EntityKind::TemporaryEntity,
        "AvoidEntity" => EntityKind::AvoidEntity,
        other => panic!("unexpected entity kind {other}"),
    };
    (location, GridEntity::new(id, kind))
}

fn location_set(locations: &[Location]) -> LocationSet {
    let mut set = LocationSet::new_hash();
    for &l in locations {
        set.add(l);
    }
    set
}

/// The context the recorder rebuilt: only what `PlanBeacons` reads is filled in.
fn rebuild(case: &Value) -> Context {
    let o = &case["options"];
    let options = OilFieldOptions {
        beacon_width: int(&o["beacon_width"]),
        beacon_height: int(&o["beacon_height"]),
        beacon_supply_width: int(&o["beacon_supply_width"]),
        beacon_supply_height: int(&o["beacon_supply_height"]),
        electric_pole_width: int(&o["electric_pole_width"]),
        electric_pole_height: int(&o["electric_pole_height"]),
        overlap_beacons: boolean(&o["overlap_beacons"]),
        validate_solution: boolean(&o["validate_solution"]),
        add_heat_pipes: boolean(&o["add_heat_pipes"]),
        add_beacons: boolean(&o["add_beacons"]),
        beacon_strategies: arr(&o["beacon_strategies"])
            .iter()
            .map(|s| strategy(int(s)))
            .collect(),
        ..OilFieldOptions::default()
    };

    let width = int(&case["width"]);
    let height = int(&case["height"]);
    let mut grid = SquareGrid::new(width, height);
    for fields in arr(&case["entities"]) {
        let (location, entity) = entity(arr(fields));
        grid.add_entity(location, entity).unwrap();
    }

    // `GetId()` until the next id is the recorded one.
    let next_id = int(&case["next_id"]);
    while grid.get_id() + 1 < next_id {}

    let centers = points(&case["centers"]);
    let mut center_to_terminals = LocationDictionary::new();
    for &center in centers
        .iter()
        .take(int(&case["center_to_terminals_count"]) as usize)
    {
        center_to_terminals.add(center, Vec::new());
    }

    let supply = arr(&case["pole_supply"]);
    Context {
        options,
        input_blueprint: Blueprint::default(),
        delta_x: 0.0,
        delta_y: 0.0,
        grid,
        centers,
        center_to_terminals,
        center_to_original_direction: LocationDictionary::new(),
        location_to_terminals: LocationDictionary::new(),
        location_to_adjacent_count: vec![0; (width * height) as usize],
        electric_pole_supply_width_with_quality: int(&supply[0]),
        electric_pole_supply_height_with_quality: int(&supply[1]),
        electric_pole_wire_reach_with_quality: 0.0,
        electric_pole_wire_reach_squared_with_quality: 0.0,
        heat_pipes: None,
        heat_dropped_pumpjacks: 0,
    }
}

/// What the C# would see next from `GetId()`, without changing the grid.
fn peek_next_id(grid: &SquareGrid) -> i32 {
    // `get_id` needs `&mut`; reading through a clone keeps the replayed grid untouched.
    let mut clone = grid.clone_grid();
    clone.get_id()
}

#[test]
fn plan_beacons_matches_recorded_dotnet_calls() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/beacons.jsonl");
    let text = std::fs::read_to_string(&path).expect("read beacons.jsonl");

    let mut cases = 0;
    let mut solutions_checked = 0;
    let mut errors = 0;
    for line in text.lines() {
        let case: Value = serde_json::from_str(line).expect("a JSON line");
        let name = case["name"].as_str().expect("a name");
        let mut context = rebuild(&case);

        let pipes = location_set(&points(&case["pipes"]));
        let heat_pipes = match &case["heat_pipes"] {
            Value::Null => None,
            value => Some(location_set(&points(value))),
        };

        let result = plan_beacons::execute(&mut context, &pipes, heat_pipes.as_ref());

        match (&case["error"], result) {
            (Value::String(expected), Err(actual)) => {
                assert_eq!(&actual.message, expected, "{name}: error message");
                errors += 1;
            }
            (Value::Null, Ok(solutions)) => {
                let expected = arr(&case["solutions"]);
                assert_eq!(solutions.len(), expected.len(), "{name}: solution count");
                for (actual, expected) in solutions.iter().zip(expected) {
                    let strategy = strategy(int(&expected["strategy"]));
                    assert_eq!(actual.strategy, strategy, "{name}: strategy order");
                    assert_eq!(
                        actual.beacons,
                        points(&expected["beacons"]),
                        "{name}: {strategy:?} beacons"
                    );
                    assert_eq!(
                        actual.effects,
                        int(&expected["effects"]),
                        "{name}: {strategy:?} effects"
                    );
                    let counts: Vec<i32> = arr(&expected["counts"]).iter().map(int).collect();
                    assert_eq!(
                        actual.effects_given_counts, counts,
                        "{name}: {strategy:?} effect counts"
                    );
                    solutions_checked += 1;
                }
            }
            (expected, actual) => {
                panic!(
                    "{name}: expected error {expected:?}, got {:?}",
                    actual.err()
                )
            }
        }

        assert_eq!(
            context.grid.entity_locations().to_vec(),
            points(&case["after_entity_locations"]),
            "{name}: EntityLocations order afterwards"
        );
        assert_eq!(
            peek_next_id(&context.grid),
            int(&case["after_next_id"]),
            "{name}: next id afterwards"
        );

        cases += 1;
    }

    assert!(cases > 250, "only {cases} cases");
    println!("{cases} cases, {solutions_checked} solutions, {errors} errors");
}
