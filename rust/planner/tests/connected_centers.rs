//! Replays every case in `fixtures/connected_centers.jsonl` (recorded from the C#
//! `AddPipesConnectedCenters`, see `FIXTURES.md`).
//!
//! Each case is a blueprint and options. The test builds the starting context with the Rust
//! `InitializeContext` (checking its centers and terminal maps against the recording), then
//! for each connected-centers strategy does what `AddPipes.GetSolutionGroups` does: fresh
//! copies of the terminal maps, `GetConnectedPumpjacks`, then `FindTrunksAndConnect`. It
//! checks the graph, the pipes or the error (thrown or returned), and the terminal maps left
//! behind, all in .NET's enumeration order.

use std::path::PathBuf;

use planner::Location;
use planner::containers::{LocationDictionary, LocationSet};
use planner::data::{Blueprint, Direction, Entity, Position};
use planner::error::{ErrorKind, FactorioToolsError};
use planner::grid::TerminalLocation;
use planner::models::{AvoidLocation, PipeStrategy};
use planner::options::OilFieldOptions;
use planner::steps::add_pipes_connected_centers::{
    find_trunks_and_connect, get_connected_pumpjacks,
};
use planner::steps::initialize_context;
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
}

fn float_bits(value: &Value) -> f32 {
    f32::from_bits(value.as_i64().expect("float bits") as i32 as u32)
}

fn string(value: &Value) -> String {
    value.as_str().expect("a string").to_string()
}

fn array(value: &Value) -> &Vec<Value> {
    value.as_array().expect("an array")
}

fn direction(value: &Value) -> Direction {
    Direction::from_value(int(value)).expect("a Direction value")
}

/// `[x0, y0, x1, y1, ...]`.
fn flat_locations(value: &Value) -> Vec<Location> {
    array(value)
        .chunks(2)
        .map(|p| Location::new(int(&p[0]), int(&p[1])))
        .collect()
}

fn preset(name: &str, validate_solution: bool) -> OilFieldOptions {
    let mut options = match name {
        "ForSmallElectricPole" => OilFieldOptions::for_small_electric_pole(),
        "ForMediumElectricPole" => OilFieldOptions::for_medium_electric_pole(),
        "ForBigElectricPole" => OilFieldOptions::for_big_electric_pole(),
        "ForSubstation" => OilFieldOptions::for_substation(),
        other => panic!("unknown preset {other}"),
    };
    options.validate_solution = validate_solution;
    options
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

/// A terminal's fields. Terminals are compared by these, not with `==`: in the C#
/// `TerminalLocation` is a class compared by reference, so the port's `==` is about identity,
/// and a terminal parsed here is never the same object as one the planner made.
type TerminalFields = (Location, Location, Direction);

fn fields(t: &TerminalLocation) -> TerminalFields {
    (t.center, t.terminal, t.direction)
}

/// `[center_x, center_y, terminal_x, terminal_y, direction]`.
fn terminal(t: &Value) -> TerminalFields {
    (
        Location::new(int(&t[0]), int(&t[1])),
        Location::new(int(&t[2]), int(&t[3])),
        direction(&t[4]),
    )
}

fn terminal_dictionary(value: &Value) -> Vec<(Location, Vec<TerminalFields>)> {
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

fn actual_terminals(
    dictionary: &LocationDictionary<Vec<TerminalLocation>>,
) -> Vec<(Location, Vec<TerminalFields>)> {
    dictionary
        .iter()
        .map(|(k, v)| (k, v.iter().map(fields).collect()))
        .collect()
}

/// `GetSolutionGroups`' copy: a new dictionary sized to the original, the same keys and the
/// same terminal objects (copies of the port's terminals, which keep their identity) in the
/// same order.
fn copy_terminals(
    original: &LocationDictionary<Vec<TerminalLocation>>,
) -> LocationDictionary<Vec<TerminalLocation>> {
    let mut copy = LocationDictionary::with_capacity(original.len());
    for (key, terminals) in original.iter() {
        copy.add(key, terminals.clone());
    }
    copy
}

fn check_error(name: &str, what: &str, actual: &FactorioToolsError, expected: &Value) {
    assert_eq!(
        actual.message,
        string(&expected["message"]),
        "{name}: {what} message"
    );
    match string(&expected["type"]).as_str() {
        "NoPathBetweenTerminalsException" => {
            let a = Location::new(int(&expected["a"][0]), int(&expected["a"][1]));
            let b = Location::new(int(&expected["b"][0]), int(&expected["b"][1]));
            assert_eq!(
                actual.kind,
                ErrorKind::NoPathBetweenTerminals { a, b },
                "{name}: {what} kind"
            );
        }
        "FactorioToolsException" => {
            assert_eq!(actual.kind, ErrorKind::General, "{name}: {what} kind");
        }
        other => panic!("{name}: {what}: the C# threw {other}, which the port does not model"),
    }
}

fn graph_pairs(graph: &LocationDictionary<LocationSet>) -> Vec<(Location, Vec<Location>)> {
    graph.iter().map(|(k, v)| (k, v.to_vec())).collect()
}

fn expected_graph(value: &Value) -> Vec<(Location, Vec<Location>)> {
    array(value)
        .iter()
        .map(|pair| {
            (
                Location::new(int(&pair[0]), int(&pair[1])),
                flat_locations(&pair[2]),
            )
        })
        .collect()
}

#[test]
fn connected_centers_match_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/connected_centers.jsonl");
    let text = std::fs::read_to_string(&path).expect("read connected_centers.jsonl");

    let mut cases = 0;
    let mut runs = 0;
    let mut pipes_checked = 0;
    let mut returned_errors = 0;
    let mut thrown_errors = 0;

    for line in text.lines() {
        let case: Value = serde_json::from_str(line).expect("valid JSON");
        let name = string(&case["name"]);
        let options = preset(
            &string(&case["preset"]),
            case["validate_solution"].as_bool().unwrap(),
        );
        let avoid: Vec<AvoidLocation> = array(&case["avoid"])
            .iter()
            .map(|a| AvoidLocation::new(float_bits(&a[0]), float_bits(&a[1])))
            .collect();

        let mut context =
            initialize_context::execute(options, blueprint(&case["entities"]), &avoid)
                .unwrap_or_else(|e| panic!("{name}: InitializeContext failed: {e:?}"));

        assert_eq!(
            context.centers,
            flat_locations(&case["centers"]),
            "{name}: centers"
        );
        assert_eq!(
            actual_terminals(&context.center_to_terminals),
            terminal_dictionary(&case["center_to_terminals"]),
            "{name}: starting center_to_terminals"
        );
        assert_eq!(
            actual_terminals(&context.location_to_terminals),
            terminal_dictionary(&case["location_to_terminals"]),
            "{name}: starting location_to_terminals"
        );

        let original_center_to_terminals = copy_terminals(&context.center_to_terminals);
        let original_location_to_terminals = copy_terminals(&context.location_to_terminals);

        for run in array(&case["runs"]) {
            runs += 1;
            let strategy = PipeStrategy::ALL[int(&run["strategy"]) as usize];
            let what = format!("{name} {strategy:?}");

            context.center_to_terminals = copy_terminals(&original_center_to_terminals);
            context.location_to_terminals = copy_terminals(&original_location_to_terminals);

            let graph = match get_connected_pumpjacks(&context, strategy) {
                Ok(graph) => {
                    assert!(
                        run.get("graph_error").is_none(),
                        "{what}: the C# threw building the graph, the port did not"
                    );
                    graph
                }
                Err(error) => {
                    let expected = run.get("graph_error").unwrap_or_else(|| {
                        panic!("{what}: the port failed building the graph: {error:?}")
                    });
                    check_error(&what, "graph error", &error, expected);
                    continue;
                }
            };

            assert_eq!(
                graph_pairs(&graph),
                expected_graph(&run["graph"]),
                "{what}: connected centers graph"
            );

            match find_trunks_and_connect(&mut context, &graph) {
                Ok(Ok(pipes)) => {
                    let expected = run
                        .get("pipes")
                        .unwrap_or_else(|| panic!("{what}: the C# failed, the port found pipes"));
                    assert_eq!(pipes.to_vec(), flat_locations(expected), "{what}: pipes");
                    pipes_checked += 1;
                }
                Ok(Err(error)) => {
                    let expected = run.get("returned_error").unwrap_or_else(|| {
                        panic!("{what}: the port returned an error the C# did not: {error:?}")
                    });
                    check_error(&what, "returned error", &error, expected);
                    returned_errors += 1;
                }
                Err(error) => {
                    let expected = run.get("thrown_error").unwrap_or_else(|| {
                        panic!("{what}: the port threw an error the C# did not: {error:?}")
                    });
                    check_error(&what, "thrown error", &error, expected);
                    thrown_errors += 1;
                }
            }

            assert_eq!(
                actual_terminals(&context.center_to_terminals),
                terminal_dictionary(&run["center_to_terminals_after"]),
                "{what}: center_to_terminals after"
            );
            assert_eq!(
                actual_terminals(&context.location_to_terminals),
                terminal_dictionary(&run["location_to_terminals_after"]),
                "{what}: location_to_terminals after"
            );
        }

        cases += 1;
    }

    assert!(cases > 250, "expected the full fixture, got {cases} cases");
    println!(
        "connected_centers: {cases} cases, {runs} runs, {pipes_checked} pipe sets, {returned_errors} returned and {thrown_errors} thrown errors"
    );
}
