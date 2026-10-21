//! Rebuilds every grid in `fixtures/grids.jsonl` (recorded from the C# planner, see
//! `FIXTURES.md`) and checks that it enumerates its entities in the recorded order and
//! draws the recorded text byte for byte.

use std::path::PathBuf;

use dotnet_compat::NetHashSet;
use planner::Location;
use planner::data::Direction;
use planner::grid::{EntityKind, GridEntity, SquareGrid};
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
}

fn direction(value: &Value) -> Direction {
    Direction::from_value(int(value)).expect("a Direction value")
}

/// One recorded entity: `[x, y, kind, id, ...extra]`.
fn entity(fields: &[Value]) -> (Location, GridEntity) {
    let location = Location::new(int(&fields[0]), int(&fields[1]));
    let kind_name = fields[2].as_str().expect("a kind name");
    let id = int(&fields[3]);
    let kind = match kind_name {
        "PumpjackCenter" => EntityKind::PumpjackCenter {
            direction: direction(&fields[4]),
        },
        "PumpjackSide" => EntityKind::PumpjackSide {
            center: int(&fields[4]),
        },
        "BeaconCenter" => EntityKind::BeaconCenter,
        "BeaconSide" => EntityKind::BeaconSide {
            center: int(&fields[4]),
        },
        "ElectricPoleCenter" => {
            let mut neighbors = NetHashSet::new();
            for neighbor in fields[4].as_array().expect("a neighbor array") {
                neighbors.add(int(neighbor));
            }
            EntityKind::ElectricPoleCenter { neighbors }
        }
        "ElectricPoleSide" => EntityKind::ElectricPoleSide {
            center: int(&fields[4]),
        },
        "Pipe" => EntityKind::Pipe,
        "Terminal" => EntityKind::Terminal,
        "UndergroundPipe" => EntityKind::UndergroundPipe {
            direction: direction(&fields[4]),
        },
        "HeatPipe" => EntityKind::HeatPipe,
        "TemporaryEntity" => EntityKind::TemporaryEntity,
        "AvoidEntity" => EntityKind::AvoidEntity,
        other => panic!("unknown entity kind {other}"),
    };
    (location, GridEntity::new(id, kind))
}

#[test]
fn grids_match_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/grids.jsonl");
    let text = std::fs::read_to_string(&path).expect("read grids.jsonl");

    let mut cases = 0;
    let mut entities_checked = 0;
    for line in text.lines().filter(|l| !l.is_empty()) {
        let case: Value = serde_json::from_str(line).expect("valid JSON");
        let name = case["name"].as_str().expect("a name");
        let width = int(&case["width"]);
        let height = int(&case["height"]);

        let mut grid = SquareGrid::new(width, height);
        let middle = &case["middle"];
        assert_eq!(
            grid.middle(),
            Location::new(int(&middle[0]), int(&middle[1])),
            "{name}: middle"
        );

        let recorded = case["entities"].as_array().expect("an entity array");
        let mut order = Vec::with_capacity(recorded.len());
        for fields in recorded {
            let (location, entity) = entity(fields.as_array().expect("an entity"));
            let id = entity.id;
            grid.add_entity(location, entity)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(grid.location_of(id), location, "{name}: id {id}");
            order.push(location);
        }

        assert_eq!(
            grid.entity_locations().to_vec(),
            order,
            "{name}: EntityLocations order"
        );

        // A side's center must be on the grid and of the right kind.
        for &location in &order {
            let entity = grid.get(location).unwrap();
            if let Some(center) = entity.kind.center() {
                let center_kind = &grid.get_entity(center).kind;
                let ok = matches!(
                    (&entity.kind, center_kind),
                    (
                        EntityKind::PumpjackSide { .. },
                        EntityKind::PumpjackCenter { .. }
                    ) | (EntityKind::BeaconSide { .. }, EntityKind::BeaconCenter)
                        | (
                            EntityKind::ElectricPoleSide { .. },
                            EntityKind::ElectricPoleCenter { .. }
                        )
                );
                assert!(
                    ok,
                    "{name}: side at {location:?} has center {center_kind:?}"
                );
            }
        }

        let expected = case["text"].as_str().expect("the grid text");
        let actual = grid.to_string();
        if actual != expected {
            let first_bad = actual
                .lines()
                .zip(expected.lines())
                .position(|(a, e)| a != e)
                .unwrap_or(0);
            panic!(
                "{name}: grid text differs at row {first_bad}\n  rust: {:?}\n  c#:   {:?}",
                actual.lines().nth(first_bad),
                expected.lines().nth(first_bad)
            );
        }

        // `new PipeGrid(grid)` keeps the order and the drawing.
        let clone = grid.clone_grid();
        assert_eq!(
            clone.entity_locations().to_vec(),
            order,
            "{name}: clone order"
        );
        assert_eq!(clone.to_string(), expected, "{name}: clone text");

        cases += 1;
        entities_checked += order.len();
    }

    assert!(cases > 0, "no cases in {}", path.display());
    eprintln!("grids.jsonl: {cases} cases, {entities_checked} entities");
}
