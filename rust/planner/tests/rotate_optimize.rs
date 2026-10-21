//! Replays `fixtures/rotate_optimize.jsonl` (recorded from the C# by
//! `oracle/RotateOptimizeRecorder.cs`, format in `FIXTURES.md`): every `RotateOptimize.Execute`
//! call must leave the pipes and both terminal maps exactly as the C# did, in .NET's order.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::PathBuf;

use planner::Location;
use planner::containers::{LocationDictionary, LocationSet};
use planner::context::Context;
use planner::data::{Blueprint, Direction};
use planner::grid::{EntityKind, GridEntity, SquareGrid, TerminalLocation};
use planner::options::OilFieldOptions;
use planner::steps::rotate_optimize;
use serde_json::Value;

fn int(value: &Value) -> i32 {
    i32::try_from(value.as_i64().expect("an integer")).expect("fits in an int")
}

fn arr(value: &Value) -> &Vec<Value> {
    value.as_array().expect("an array")
}

fn points(value: &Value) -> Vec<Location> {
    arr(value)
        .chunks(2)
        .map(|p| Location::new(int(&p[0]), int(&p[1])))
        .collect()
}

fn terminals(value: &Value) -> LocationDictionary<Vec<TerminalLocation>> {
    let mut dictionary = LocationDictionary::new();
    for pair in arr(value) {
        let pair = arr(pair);
        let key = Location::new(int(&pair[0]), int(&pair[1]));
        let list = arr(&pair[2])
            .chunks(5)
            .map(|t| {
                TerminalLocation::new(
                    Location::new(int(&t[0]), int(&t[1])),
                    Location::new(int(&t[2]), int(&t[3])),
                    Direction::from_value(int(&t[4])).expect("a Direction value"),
                )
            })
            .collect();
        dictionary.add(key, list);
    }
    dictionary
}

/// The same text `TerminalsText` builds in the recorder.
fn terminals_text(dictionary: &LocationDictionary<Vec<TerminalLocation>>) -> String {
    let mut text = String::new();
    for (key, list) in dictionary.iter() {
        write!(text, "{},{}:", key.x, key.y).unwrap();
        for t in list {
            write!(
                text,
                "{},{},{},{},{};",
                t.center.x,
                t.center.y,
                t.terminal.x,
                t.terminal.y,
                t.direction.value()
            )
            .unwrap();
        }
        text.push('|');
    }
    text
}

/// A grid holding what `RotateOptimize` reads: which cells are taken, and which hold pipes.
fn grid(record: &Value) -> SquareGrid {
    let mut grid = SquareGrid::new(int(&record["width"]), int(&record["height"]));
    for cell in arr(&record["cells"]).chunks(3) {
        let location = Location::new(int(&cell[0]), int(&cell[1]));
        let kind = if int(&cell[2]) == 1 {
            EntityKind::Pipe
        } else {
            EntityKind::TemporaryEntity
        };
        let id = grid.get_id();
        grid.add_entity(location, GridEntity::new(id, kind))
            .unwrap();
    }
    grid
}

/// In C#, `LocationToTerminals` holds the same terminal objects as `CenterToTerminals`,
/// and `EliminateOtherTerminals` compares them by reference. The case records both maps
/// as values, so rebuild the second from the first's objects.
fn share_terminals(
    center_to_terminals: &LocationDictionary<Vec<TerminalLocation>>,
    mut location_to_terminals: LocationDictionary<Vec<TerminalLocation>>,
) -> LocationDictionary<Vec<TerminalLocation>> {
    let keys: Vec<_> = location_to_terminals.iter().map(|(key, _)| key).collect();
    for key in keys {
        for terminal in location_to_terminals.value_mut(key).iter_mut() {
            *terminal = *center_to_terminals
                .value(terminal.center)
                .iter()
                .find(|t| *t == terminal)
                .expect("every terminal in LocationToTerminals is one of its center's");
        }
    }
    location_to_terminals
}

fn context(grid: SquareGrid, case: &Value) -> Context {
    let width = grid.width();
    let height = grid.height();
    let center_to_terminals = terminals(&case["center_to_terminals"]);
    Context {
        options: OilFieldOptions::default(),
        input_blueprint: Blueprint::default(),
        delta_x: 0.0,
        delta_y: 0.0,
        grid,
        centers: Vec::new(),
        center_to_original_direction: LocationDictionary::new(),
        location_to_terminals: share_terminals(
            &center_to_terminals,
            terminals(&case["location_to_terminals"]),
        ),
        center_to_terminals,
        location_to_adjacent_count: vec![0; (width * height) as usize],
        electric_pole_supply_width_with_quality: 0,
        electric_pole_supply_height_with_quality: 0,
        electric_pole_wire_reach_with_quality: 0.0,
        electric_pole_wire_reach_squared_with_quality: 0.0,
        heat_pipes: None,
        heat_dropped_pumpjacks: 0,
    }
}

#[test]
fn rotate_optimize_matches_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/rotate_optimize.jsonl");
    let text = std::fs::read_to_string(&path).expect("read rotate_optimize.jsonl");

    let mut grids: HashMap<i32, Value> = HashMap::new();
    let mut cases = 0;
    let mut errors = 0;
    for line in text.lines() {
        let record: Value = serde_json::from_str(line).expect("valid JSON");
        match record["kind"].as_str() {
            Some("grid") => {
                grids.insert(int(&record["id"]), record);
                continue;
            }
            Some("case") => {}
            other => panic!("unknown line kind {other:?}"),
        }

        let name = record["name"].as_str().unwrap();
        let mut context = context(grid(&grids[&int(&record["grid"])]), &record);

        let input = points(&record["pipes"]);
        let mut pipes = match record["pipes_type"].as_str() {
            Some("single") => {
                assert_eq!(input.len(), 1, "{name}: a single set holds one pipe");
                LocationSet::new_single(input[0])
            }
            Some("hash") => {
                let mut set = context.get_location_set_with_capacity(input.len(), true);
                for pipe in &input {
                    set.add(*pipe);
                }
                set
            }
            other => panic!("{name}: unknown pipes_type {other:?}"),
        };

        let result = rotate_optimize::execute(&mut context, &mut pipes);
        let out = &record["out"];
        match (&result, out["error"].as_str()) {
            (Ok(()), None) => {}
            (Err(error), Some(expected)) => {
                errors += 1;
                assert!(
                    expected.ends_with(&error.message),
                    "{name}: error {:?}, expected {expected:?}",
                    error.message
                );
            }
            (result, expected) => panic!("{name}: got {result:?}, expected error {expected:?}"),
        }

        assert_eq!(pipes.to_vec(), points(&out["pipes"]), "{name}: pipes");
        assert_eq!(
            terminals_text(&context.center_to_terminals),
            out["center_to_terminals"].as_str().unwrap(),
            "{name}: center_to_terminals"
        );
        assert_eq!(
            terminals_text(&context.location_to_terminals),
            out["location_to_terminals"].as_str().unwrap(),
            "{name}: location_to_terminals"
        );
        cases += 1;
    }

    assert!(cases > 700, "only {cases} cases");
    println!("{cases} cases matched ({errors} errors)");
}
