//! Replays `fixtures/poles_and_heat.jsonl` (recorded from the C# by
//! `oracle/PolesAndHeatRecorder.cs`, removed in stage 13b; format in `FIXTURES.md`): every
//! `AddElectricPoles` and `AddHeatPipes` case, on a context rebuilt from its recorded
//! snapshot the same way the recorder rebuilt it, must give the recorded result and leave
//! the grid in the recorded state (entities, ids, pole wiring and the next id).

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;

use dotnet_compat::NetHashSet;
use planner::Location;
use planner::containers::{LocationDictionary, LocationSet};
use planner::context::Context;
use planner::data::{Blueprint, Direction};
use planner::grid::{EntityKind, GridEntity, SquareGrid};
use planner::options::OilFieldOptions;
use planner::steps::{add_electric_poles, add_heat_pipes};
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

fn flat(locations: impl IntoIterator<Item = Location>) -> Vec<i32> {
    locations.into_iter().flat_map(|l| [l.x, l.y]).collect()
}

fn ints(value: &Value) -> Vec<i32> {
    arr(value).iter().map(int).collect()
}

fn direction(value: i32) -> Direction {
    Direction::from_value(value).expect("a Direction value")
}

/// One entity, `[x, y, kind code, id, extra?]`, kind codes as in [`kind_code`].
fn entity(fields: &[Value]) -> (Location, GridEntity) {
    let location = Location::new(int(&fields[0]), int(&fields[1]));
    let id = int(&fields[3]);
    let kind = match int(&fields[2]) {
        0 => EntityKind::PumpjackCenter {
            direction: direction(int(&fields[4])),
        },
        1 => EntityKind::PumpjackSide {
            center: int(&fields[4]),
        },
        2 => EntityKind::BeaconCenter,
        3 => EntityKind::BeaconSide {
            center: int(&fields[4]),
        },
        4 => {
            let mut neighbors = NetHashSet::new();
            for n in arr(&fields[4]) {
                neighbors.add(int(n));
            }
            EntityKind::ElectricPoleCenter { neighbors }
        }
        5 => EntityKind::ElectricPoleSide {
            center: int(&fields[4]),
        },
        6 => EntityKind::Pipe,
        7 => EntityKind::Terminal,
        8 => EntityKind::UndergroundPipe {
            direction: direction(int(&fields[4])),
        },
        9 => EntityKind::HeatPipe,
        10 => EntityKind::TemporaryEntity,
        11 => EntityKind::AvoidEntity,
        other => panic!("unknown entity kind code {other}"),
    };
    (location, GridEntity::new(id, kind))
}

/// The context `PolesAndHeatRecorder.Rebuild` makes from a snapshot.
fn rebuild(s: &Value) -> Context {
    let sizes = ints(&s["sizes"]);
    let wire = arr(&s["wire_reach"]);
    let options = OilFieldOptions {
        beacon_width: sizes[0],
        beacon_height: sizes[1],
        electric_pole_width: sizes[2],
        electric_pole_height: sizes[3],
        validate_solution: s["validate_solution"].as_bool().unwrap(),
        add_heat_pipes: s["add_heat_pipes"].as_bool().unwrap(),
        ..OilFieldOptions::default()
    };

    let width = int(&s["width"]);
    let height = int(&s["height"]);
    let mut grid = SquareGrid::new(width, height);
    for fields in arr(&s["entities"]) {
        let (location, entity) = entity(arr(fields));
        grid.add_entity(location, entity).unwrap();
    }

    // `GetId()` until the next id is the recorded one.
    let next_id = int(&s["next_id"]);
    while grid.get_id() + 1 < next_id {}

    let heat_pipes = match &s["heat_pipes"] {
        Value::Null => None,
        value => {
            let mut set = LocationSet::new_hash();
            for l in points(value) {
                set.add(l);
            }
            Some(set)
        }
    };

    Context {
        options,
        input_blueprint: Blueprint::default(),
        delta_x: 0.0,
        delta_y: 0.0,
        grid,
        centers: points(&s["centers"]),
        center_to_terminals: LocationDictionary::new(),
        center_to_original_direction: LocationDictionary::new(),
        location_to_terminals: LocationDictionary::new(),
        location_to_adjacent_count: vec![0; (width * height) as usize],
        electric_pole_supply_width_with_quality: sizes[4],
        electric_pole_supply_height_with_quality: sizes[5],
        electric_pole_wire_reach_with_quality: wire[0].as_f64().unwrap(),
        electric_pole_wire_reach_squared_with_quality: wire[1].as_f64().unwrap(),
        heat_pipes,
        heat_dropped_pumpjacks: 0,
    }
}

// -------------------------------------------------------------------------------------------
// The grid hash, the same as `PolesAndHeatRecorder.HashGrid`

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(1469598103934665603)
    }

    fn mix(&mut self, value: i32) -> &mut Self {
        for b in (value as u32).to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(1099511628211);
        }
        self
    }

    fn mix_location(&mut self, l: Location) -> &mut Self {
        self.mix(l.x).mix(l.y)
    }

    fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

fn kind_code(kind: &EntityKind) -> i32 {
    match kind {
        EntityKind::PumpjackCenter { .. } => 0,
        EntityKind::PumpjackSide { .. } => 1,
        EntityKind::BeaconCenter => 2,
        EntityKind::BeaconSide { .. } => 3,
        EntityKind::ElectricPoleCenter { .. } => 4,
        EntityKind::ElectricPoleSide { .. } => 5,
        EntityKind::Pipe => 6,
        EntityKind::Terminal => 7,
        EntityKind::UndergroundPipe { .. } => 8,
        EntityKind::HeatPipe => 9,
        EntityKind::TemporaryEntity => 10,
        EntityKind::AvoidEntity => 11,
    }
}

/// Takes the next id, as the C# `HashGrid` does, so it must come last.
fn hash_grid(grid: &mut SquareGrid) -> String {
    let mut h = Fnv::new();
    for location in grid.entity_locations().iter() {
        let e = grid.get(location).unwrap();
        h.mix_location(location).mix(kind_code(&e.kind)).mix(e.id);
        match &e.kind {
            EntityKind::PumpjackCenter { direction }
            | EntityKind::UndergroundPipe { direction } => {
                h.mix(direction.value());
            }
            EntityKind::PumpjackSide { center }
            | EntityKind::BeaconSide { center }
            | EntityKind::ElectricPoleSide { center } => {
                h.mix(*center);
            }
            EntityKind::ElectricPoleCenter { neighbors } => {
                h.mix(neighbors.len() as i32);
                for n in neighbors.iter() {
                    h.mix(n);
                }
            }
            _ => {}
        }
    }
    h.mix(grid.get_id());
    h.hex()
}

/// Every pole, in grid order: `[x, y, id, [neighbor ids in order]]`.
fn poles(grid: &SquareGrid) -> Vec<(i32, i32, i32, Vec<i32>)> {
    grid.entity_locations()
        .iter()
        .filter_map(|l| match &grid.get(l).unwrap().kind {
            EntityKind::ElectricPoleCenter { neighbors } => Some((
                l.x,
                l.y,
                grid.get(l).unwrap().id,
                neighbors.iter().collect(),
            )),
            _ => None,
        })
        .collect()
}

fn recorded_poles(value: &Value) -> Vec<(i32, i32, i32, Vec<i32>)> {
    arr(value)
        .iter()
        .map(|p| {
            let p = arr(p);
            (int(&p[0]), int(&p[1]), int(&p[2]), ints(&p[3]))
        })
        .collect()
}

fn pole_centers(grid: &SquareGrid) -> Vec<Location> {
    grid.entity_locations()
        .iter()
        .filter(|&l| {
            matches!(
                grid.get(l).unwrap().kind,
                EntityKind::ElectricPoleCenter { .. }
            )
        })
        .collect()
}

fn hash_set(locations: &[Location]) -> LocationSet {
    let mut set = LocationSet::new_hash();
    for &l in locations {
        set.add(l);
    }
    set
}

/// Checks the recorded `error` / `exception` against what the Rust step did.
fn check_error<T>(name: &str, out: &Value, result: &std::thread::Result<planner::FtResult<T>>) {
    match (result, out.get("exception")) {
        (Err(_), Some(_)) => {}
        (Err(_), None) => panic!("{name}: the Rust step panicked, the C# did not throw"),
        (Ok(_), Some(e)) => panic!("{name}: the C# threw {e}, the Rust step did not panic"),
        (Ok(Err(error)), None) => assert_eq!(
            Some(error.message.as_str()),
            out["error"].as_str(),
            "{name}: error"
        ),
        (Ok(Ok(_)), None) => assert!(
            out["error"].is_null(),
            "{name}: expected error {}",
            out["error"]
        ),
    }
}

#[test]
fn poles_and_heat_match_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/poles_and_heat.jsonl");
    let text = std::fs::read_to_string(&path).expect("read poles_and_heat.jsonl");

    let mut contexts: HashMap<i32, Value> = HashMap::new();
    let mut counts: HashMap<String, usize> = HashMap::new();

    for (line_number, line) in text.lines().enumerate() {
        let record: Value = serde_json::from_str(line).expect("valid JSON");
        if record["kind"] == "context" {
            contexts.insert(int(&record["id"]), record);
            continue;
        }

        let snapshot = &contexts[&int(&record["ctx"])];
        let op = record["op"].as_str().unwrap();
        let name = format!(
            "line {} ({op} on {})",
            line_number + 1,
            snapshot["name"].as_str().unwrap()
        );
        let out = &record["out"];
        let mut context = rebuild(snapshot);
        *counts.entry(op.to_string()).or_default() += 1;

        match op {
            "poles" => {
                let avoid_points = points(&record["avoid"]);
                let avoid = if avoid_points.is_empty() {
                    LocationSet::Empty
                } else {
                    hash_set(&avoid_points)
                };
                let allow_retries = record["allow_retries"].as_bool().unwrap();
                let result = catch_unwind(AssertUnwindSafe(|| {
                    add_electric_poles::execute(&mut context, &avoid, allow_retries)
                }));
                check_error(&name, out, &result);

                let returned = match &result {
                    Ok(Ok(returned)) => returned.as_ref(),
                    _ => None,
                };
                match (&out["returned"], returned) {
                    (Value::Null, None) => {}
                    (Value::Null, Some(_)) => panic!("{name}: expected no poles returned"),
                    (expected, None) => panic!("{name}: expected poles {expected}"),
                    (expected, Some(set)) => {
                        assert_eq!(set.len() as i32, int(&expected["count"]), "{name}: count");
                        let members: Vec<Location> = pole_centers(&context.grid)
                            .into_iter()
                            .filter(|&l| set.contains(l))
                            .collect();
                        assert_eq!(flat(members), ints(&expected["members"]), "{name}: members");
                    }
                }

                assert_eq!(
                    poles(&context.grid),
                    recorded_poles(&out["poles"]),
                    "{name}: poles"
                );
            }
            "heat_execute" => {
                let result =
                    catch_unwind(AssertUnwindSafe(|| add_heat_pipes::execute(&mut context)));
                check_error(&name, out, &result);
                match &out["heat_pipes"] {
                    Value::Null => assert!(context.heat_pipes.is_none(), "{name}: heat pipes"),
                    expected => assert_eq!(
                        flat(context.heat_pipes.as_ref().expect("heat pipes").iter()),
                        ints(expected),
                        "{name}: heat pipes"
                    ),
                }
            }
            "heat_route" => {
                let tiles = hash_set(&points(&record["pipe_tiles"]));
                let result = catch_unwind(AssertUnwindSafe(|| {
                    add_heat_pipes::route(&mut context, &tiles)
                }));
                check_error(&name, out, &result);
                if let Ok(Ok(routed)) = &result {
                    assert_eq!(
                        flat(routed.chosen.iter()),
                        ints(&out["chosen"]),
                        "{name}: chosen"
                    );
                    assert_eq!(
                        flat(routed.uncovered_pipes.iter()),
                        ints(&out["uncovered_pipes"]),
                        "{name}: uncovered pipes"
                    );
                    assert_eq!(
                        flat(routed.uncovered_centers.iter()),
                        ints(&out["uncovered_centers"]),
                        "{name}: uncovered centers"
                    );
                }
            }
            "heat_extend" => {
                let beacons = points(&record["beacons"]);
                let result = catch_unwind(AssertUnwindSafe(|| {
                    add_heat_pipes::extend_to_beacons(&mut context, &beacons)
                }));
                check_error(&name, out, &result);
                if let Ok(Ok(heated)) = &result {
                    assert_eq!(flat(heated.iter()), ints(&out["heated"]), "{name}: heated");
                }
                assert_eq!(
                    flat(context.heat_pipes.iter().flat_map(|s| s.iter())),
                    ints(&out["heat_pipes"]),
                    "{name}: heat pipes"
                );
            }
            other => panic!("unknown op {other}"),
        }

        assert_eq!(
            hash_grid(&mut context.grid),
            out["grid"].as_str().unwrap(),
            "{name}: grid"
        );
    }

    let mut summary: Vec<_> = counts.into_iter().collect();
    summary.sort();
    println!("replayed: {summary:?}");
    assert!(summary.iter().map(|(_, n)| n).sum::<usize>() > 400);
}
