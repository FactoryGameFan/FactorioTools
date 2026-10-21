//! Replays `fixtures/fbe_pipes.jsonl` (recorded from the C# by `oracle/FbePipesRecorder.cs`,
//! removed in stage 13b; format in `FIXTURES.md`): `AddPipesFbe.Execute` on each recorded
//! context must give the recorded pipes, final strategy or error, and leave the terminal
//! maps as the C# did.

use std::path::PathBuf;

use planner::Location;
use planner::containers::LocationDictionary;
use planner::context::Context;
use planner::data::{Blueprint, Direction};
use planner::error::ErrorKind;
use planner::grid::{EntityKind, GridEntity, SquareGrid, TerminalLocation};
use planner::models::PipeStrategy;
use planner::options::OilFieldOptions;
use planner::steps::add_pipes_fbe;
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
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

fn strategy(value: i32) -> PipeStrategy {
    match value {
        0 => PipeStrategy::FbeOriginal,
        1 => PipeStrategy::Fbe,
        other => panic!("not an FBE strategy: {other}"),
    }
}

/// One recorded terminal's fields: center, terminal, direction value.
type TerminalFields = (Location, Location, i32);

/// `[[key x, key y, [cx, cy, tx, ty, direction, ...]], ...]` as field tuples, in order.
fn terminal_fields(value: &Value) -> Vec<(Location, Vec<TerminalFields>)> {
    arr(value)
        .iter()
        .map(|pair| {
            let pair = arr(pair);
            let key = Location::new(int(&pair[0]), int(&pair[1]));
            let list = arr(&pair[2])
                .chunks(5)
                .map(|t| {
                    (
                        Location::new(int(&t[0]), int(&t[1])),
                        Location::new(int(&t[2]), int(&t[3])),
                        int(&t[4]),
                    )
                })
                .collect();
            (key, list)
        })
        .collect()
}

/// Both terminal maps, rebuilt the way the recorder's `Clone` built them: dictionaries sized to
/// the count, pairs added in order.
///
/// In the C# the two maps hold the same `TerminalLocation` objects (`PopulateCenterToTerminals`
/// creates each once, `PopulateLocationToTerminals` and the clones copy the references), and
/// `EliminateOtherTerminals` relies on that when it removes a terminal from
/// `LocationToTerminals` by reference. So each terminal is created once here, in the
/// center map, and the location map holds copies of those same terminals.
fn terminal_maps(
    center_value: &Value,
    location_value: &Value,
) -> (
    LocationDictionary<Vec<TerminalLocation>>,
    LocationDictionary<Vec<TerminalLocation>>,
) {
    let center_pairs = terminal_fields(center_value);
    let mut created: Vec<(TerminalFields, TerminalLocation)> = Vec::new();
    let mut center_to_terminals = LocationDictionary::with_capacity(center_pairs.len());
    for (key, list) in center_pairs {
        let list = list
            .into_iter()
            .map(|fields @ (center, terminal, direction)| {
                let t = TerminalLocation::new(
                    center,
                    terminal,
                    Direction::from_value(direction).expect("a Direction value"),
                );
                created.push((fields, t));
                t
            })
            .collect();
        center_to_terminals.add(key, list);
    }

    let location_pairs = terminal_fields(location_value);
    let mut location_to_terminals = LocationDictionary::with_capacity(location_pairs.len());
    for (key, list) in location_pairs {
        let list = list
            .into_iter()
            .map(|fields| {
                created
                    .iter()
                    .find(|(f, _)| *f == fields)
                    .map(|(_, t)| *t)
                    .expect("every location-map terminal is also in the center map")
            })
            .collect();
        location_to_terminals.add(key, list);
    }

    (center_to_terminals, location_to_terminals)
}

/// A terminal map as field tuples, in enumeration order, for comparing with the recording.
fn flatten(
    dictionary: &LocationDictionary<Vec<TerminalLocation>>,
) -> Vec<(Location, Vec<TerminalFields>)> {
    dictionary
        .iter()
        .map(|(k, v)| {
            (
                k,
                v.iter()
                    .map(|t| (t.center, t.terminal, t.direction.value()))
                    .collect(),
            )
        })
        .collect()
}

/// The context `AddPipes.GetSolutionGroups` hands FBE. FBE only asks the grid which cells are
/// empty, so every recorded occupied cell gets a `TemporaryEntity`.
fn rebuild(case: &Value) -> Context {
    let width = int(&case["width"]);
    let height = int(&case["height"]);
    let mut grid = SquareGrid::new(width, height);
    for location in points(&case["occupied"]) {
        let id = grid.get_id();
        grid.add_entity(location, GridEntity::new(id, EntityKind::TemporaryEntity))
            .expect("recorded cells are distinct");
    }

    let middle = points(&case["middle"])[0];
    assert_eq!(grid.middle(), middle, "{}: grid middle", case["name"]);

    let (center_to_terminals, location_to_terminals) =
        terminal_maps(&case["center_to_terminals"], &case["location_to_terminals"]);

    Context {
        options: OilFieldOptions::default(),
        input_blueprint: Blueprint::default(),
        delta_x: 0.0,
        delta_y: 0.0,
        grid,
        centers: points(&case["centers"]),
        center_to_terminals,
        center_to_original_direction: LocationDictionary::new(),
        location_to_terminals,
        location_to_adjacent_count: vec![0; (width * height) as usize],
        electric_pole_supply_width_with_quality: 0,
        electric_pole_supply_height_with_quality: 0,
        electric_pole_wire_reach_with_quality: 0.0,
        electric_pole_wire_reach_squared_with_quality: 0.0,
        heat_pipes: None,
        heat_dropped_pumpjacks: 0,
    }
}

fn check(case: &Value) -> Result<(), String> {
    let name = case["name"].as_str().expect("a name");
    let mut context = rebuild(case);
    let result = add_pipes_fbe::execute(&mut context, strategy(int(&case["strategy"])));

    match (&case["error"], result) {
        (Value::Null, Ok(result)) => {
            let expected_strategy = strategy(int(&case["final_strategy"]));
            if result.final_strategy != expected_strategy {
                return Err(format!(
                    "{name}: final strategy {:?}, expected {expected_strategy:?}",
                    result.final_strategy
                ));
            }

            let pipes = result.pipes.to_vec();
            let expected = points(&case["pipes"]);
            if pipes != expected {
                return Err(format!(
                    "{name}: pipes differ ({} vs {} expected)\n  got      {pipes:?}\n  expected {expected:?}",
                    pipes.len(),
                    expected.len()
                ));
            }
        }
        (Value::Null, Err(error)) => {
            return Err(format!("{name}: failed with {error:?}, expected pipes"));
        }
        (expected, Ok(result)) => {
            return Err(format!(
                "{name}: returned {} pipes, expected the error {expected}",
                result.pipes.len()
            ));
        }
        (expected, Err(error)) => {
            let message = expected["message"].as_str().expect("a message");
            if error.message != message {
                return Err(format!(
                    "{name}: error {:?}, expected {message:?}",
                    error.message
                ));
            }

            if let Some(bad_input) = expected["bad_input"].as_bool()
                && error.bad_input != bad_input
            {
                return Err(format!(
                    "{name}: bad_input {}, expected {bad_input}",
                    error.bad_input
                ));
            }

            let no_path = match error.kind {
                ErrorKind::NoPathBetweenTerminals { a, b } => Some(vec![a.x, a.y, b.x, b.y]),
                ErrorKind::General => None,
            };
            let expected_no_path = expected
                .get("no_path")
                .map(|v| arr(v).iter().map(int).collect::<Vec<_>>());
            if no_path != expected_no_path {
                return Err(format!(
                    "{name}: no-path locations {no_path:?}, expected {expected_no_path:?}"
                ));
            }
        }
    }

    let after = flatten(&context.center_to_terminals);
    let expected = terminal_fields(&case["center_to_terminals_after"]);
    if after != expected {
        return Err(format!("{name}: center_to_terminals afterwards differ"));
    }

    let after = flatten(&context.location_to_terminals);
    let expected = terminal_fields(&case["location_to_terminals_after"]);
    if after != expected {
        return Err(format!("{name}: location_to_terminals afterwards differ"));
    }

    Ok(())
}

#[test]
fn fbe_pipes_match_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/fbe_pipes.jsonl");
    let text = std::fs::read_to_string(&path).expect("read fbe_pipes.jsonl");

    let mut cases = 0;
    let mut failures = Vec::new();
    for line in text.lines() {
        let case: Value = serde_json::from_str(line).expect("a JSON line");
        cases += 1;
        if let Err(failure) = check(&case) {
            failures.push(failure);
        }
    }

    assert!(cases > 0, "no cases in {}", path.display());
    assert!(
        failures.is_empty(),
        "{} of {cases} cases differ from the C#:\n{}",
        failures.len(),
        failures
            .iter()
            .take(5)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
