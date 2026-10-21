//! Replays every case in `fixtures/pipes.jsonl` (recorded from the C# planner, see
//! `FIXTURES.md`): `PlanUndergroundPipes` on the recorded pipe set, then `AddPipeEntities`,
//! and checks the underground pipes, the pipe set left behind and every entity added, in
//! .NET's order and with the same ids.

use std::path::PathBuf;

use planner::Location;
use planner::containers::{LocationDictionary, LocationSet};
use planner::context::Context;
use planner::data::Direction;
use planner::grid::{EntityKind, GridEntity, SquareGrid, TerminalLocation};
use planner::models::{BeaconSolution, BeaconStrategy};
use planner::options::OilFieldOptions;
use planner::steps::{add_pipe_entities, initialize_context, plan_underground_pipes, validate};
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
}

fn direction(value: &Value) -> Direction {
    Direction::from_value(int(value)).expect("a Direction value")
}

fn locations(value: &Value) -> Vec<Location> {
    let flat = value.as_array().expect("a flat location array");
    flat.chunks(2)
        .map(|p| Location::new(int(&p[0]), int(&p[1])))
        .collect()
}

/// `[x, y, kind, id, ...extra]`, the grids.jsonl encoding.
fn entity(fields: &[Value]) -> (Location, GridEntity) {
    let location = Location::new(int(&fields[0]), int(&fields[1]));
    let id = int(&fields[3]);
    let kind = match fields[2].as_str().expect("a kind name") {
        "PumpjackCenter" => EntityKind::PumpjackCenter {
            direction: direction(&fields[4]),
        },
        "PumpjackSide" => EntityKind::PumpjackSide {
            center: int(&fields[4]),
        },
        "Pipe" => EntityKind::Pipe,
        "Terminal" => EntityKind::Terminal,
        "UndergroundPipe" => EntityKind::UndergroundPipe {
            direction: direction(&fields[4]),
        },
        "AvoidEntity" => EntityKind::AvoidEntity,
        "TemporaryEntity" => EntityKind::TemporaryEntity,
        other => panic!("unknown entity kind {other}"),
    };
    (location, GridEntity::new(id, kind))
}

/// A short, comparable form of an entity: kind, id and the kind's own field.
fn describe(location: Location, entity: &GridEntity) -> String {
    let extra = match &entity.kind {
        EntityKind::UndergroundPipe { direction } => format!(" {}", direction.value()),
        EntityKind::PumpjackCenter { direction } => format!(" {}", direction.value()),
        EntityKind::PumpjackSide { center } => format!(" {center}"),
        _ => String::new(),
    };
    format!(
        "({}, {}) {} {}{extra}",
        location.x,
        location.y,
        entity.label(),
        entity.id
    )
}

fn terminal_dictionary(value: &Value) -> LocationDictionary<Vec<TerminalLocation>> {
    let mut dictionary = LocationDictionary::new();
    for pair in value.as_array().expect("a terminal dictionary") {
        let key = Location::new(int(&pair[0]), int(&pair[1]));
        let terminals = pair[2]
            .as_array()
            .expect("a terminal list")
            .iter()
            .map(|t| {
                TerminalLocation::new(
                    Location::new(int(&t[0]), int(&t[1])),
                    Location::new(int(&t[2]), int(&t[3])),
                    direction(&t[4]),
                )
            })
            .collect();
        dictionary.add(key, terminals);
    }
    dictionary
}

fn build_context(case: &Value) -> Context {
    let width = int(&case["width"]);
    let height = int(&case["height"]);
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = case["validate"].as_bool().expect("validate");

    let mut context =
        initialize_context::get_empty(options, width, height).expect("an empty context");

    let mut grid = SquareGrid::new(width, height);
    for fields in case["grid"].as_array().expect("grid entities") {
        let (location, entity) = entity(fields.as_array().expect("an entity"));
        grid.add_entity(location, entity).expect("a free cell");
    }

    // `GetId` hands out the recorded next id from here on.
    let next_id = int(&case["next_id"]);
    for _ in 1..next_id {
        grid.get_id();
    }

    context.grid = grid;
    context.center_to_terminals = terminal_dictionary(&case["center_to_terminals"]);
    context.location_to_terminals = terminal_dictionary(&case["location_to_terminals"]);
    context
}

#[test]
fn pipes_match_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/pipes.jsonl");
    let text = std::fs::read_to_string(&path).expect("read pipes.jsonl");

    let mut cases = 0;
    let mut undergrounds = 0;
    let mut added = 0;
    let mut errors = 0;
    let mut overlap_checks = 0;
    let mut overlap_errors = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let case: Value = serde_json::from_str(line).expect("valid JSON");
        let name = case["name"].as_str().expect("a name");
        let mut context = build_context(&case);
        assert_eq!(context.grid.width(), int(&case["width"]), "{name}: width");

        let mut pipes = LocationSet::new_hash();
        for location in locations(&case["pipes"]) {
            pipes.add(location);
        }

        let underground = if case["plan_underground"]
            .as_bool()
            .expect("plan_underground")
        {
            let result = plan_underground_pipes::execute(&context, &mut pipes)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let actual: Vec<(Location, i32)> = result.iter().map(|(l, d)| (l, d.value())).collect();
            let expected: Vec<(Location, i32)> = case["underground"]
                .as_array()
                .expect("underground")
                .iter()
                .map(|u| (Location::new(int(&u[0]), int(&u[1])), int(&u[2])))
                .collect();
            assert_eq!(actual, expected, "{name}: underground pipes");
            assert_eq!(
                pipes.to_vec(),
                locations(&case["pipes_after"]),
                "{name}: pipes after"
            );
            undergrounds += actual.len();
            Some(result)
        } else {
            None
        };

        // `Validate.NoOverlappingEntities` on this case's pipes, recorded for selected cases.
        if let Some(checks) = case.get("no_overlap") {
            for check in checks.as_array().expect("no_overlap checks") {
                let label = check["label"].as_str().expect("a label");
                let mut check_pipes = LocationSet::new_hash();
                for location in locations(&check["pipes"]) {
                    check_pipes.add(location);
                }
                let solutions = check.get("beacons").map(|b| {
                    vec![BeaconSolution {
                        strategy: BeaconStrategy::Snug,
                        beacons: locations(b),
                        effects: 0,
                        effects_given_counts: Vec::new(),
                    }]
                });
                let result = validate::no_overlapping_entities(
                    &context,
                    &check_pipes,
                    underground.as_ref(),
                    solutions.as_deref(),
                );
                let expected = check["error"].as_str();
                match (result, expected) {
                    (Ok(()), None) => {}
                    (Err(error), Some(message)) => {
                        assert_eq!(
                            error.message.as_str(),
                            message,
                            "{name}: no_overlap {label}"
                        )
                    }
                    (result, expected) => {
                        panic!("{name}: no_overlap {label}: got {result:?}, expected {expected:?}")
                    }
                }
                overlap_checks += 1;
                if expected.is_some() {
                    overlap_errors += 1;
                }
            }
        }

        let mode = case["mode"].as_str().expect("a mode");
        let before = context.grid.entity_locations().len();
        let (result, grid) = match mode {
            "selected" | "multiple_error" => {
                let result = add_pipe_entities::execute(&mut context, &pipes, underground.as_ref());
                (result, None)
            }
            "multiple_on_clone" => {
                let mut clone = context.grid.clone_grid();
                let result = add_pipe_entities::execute_on_grid(
                    &context,
                    &mut clone,
                    &pipes,
                    underground.as_ref(),
                    true,
                );
                (result, Some(clone))
            }
            other => panic!("{name}: unknown mode {other}"),
        };
        let grid = grid.as_ref().unwrap_or(&context.grid);

        match (result, case.get("error")) {
            (Err(error), Some(expected)) => {
                assert_eq!(
                    error.message.as_str(),
                    expected.as_str().expect("a message"),
                    "{name}: error"
                );
                errors += 1;
            }
            (Ok(()), None) => {
                let actual: Vec<String> = grid
                    .entity_locations()
                    .iter()
                    .skip(before)
                    .map(|l| describe(l, grid.get(l).expect("an entity")))
                    .collect();
                let expected: Vec<String> = case["added"]
                    .as_array()
                    .expect("added")
                    .iter()
                    .map(|e| {
                        let (location, entity) = entity(e.as_array().expect("an entity"));
                        describe(location, &entity)
                    })
                    .collect();
                assert_eq!(actual, expected, "{name}: added entities");
                added += actual.len();

                let mut grid_copy = grid.clone_grid();
                assert_eq!(
                    grid_copy.get_id(),
                    int(&case["next_id_after"]),
                    "{name}: next id"
                );
            }
            (Err(error), None) => panic!("{name}: unexpected error {error}"),
            (Ok(()), Some(expected)) => panic!("{name}: expected the error {expected}"),
        }

        cases += 1;
    }

    assert!(cases > 200, "only {cases} cases");
    assert!(
        overlap_checks > 500,
        "only {overlap_checks} NoOverlappingEntities checks"
    );
    println!(
        "{cases} cases, {undergrounds} underground pipes, {added} entities added, {errors} errors, \
         {overlap_checks} NoOverlappingEntities checks ({overlap_errors} errors)"
    );
}
