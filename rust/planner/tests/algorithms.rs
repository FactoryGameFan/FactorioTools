//! Replays every call in `fixtures/algorithms.jsonl` (recorded from the C# planner's
//! algorithms, see `FIXTURES.md`) and checks each result exactly, including the order of
//! every set and dictionary the C# enumerates.

use std::collections::HashMap;
use std::path::PathBuf;

use planner::Location;
use planner::algorithms::{
    a_star, a_star_into, breadth_first_shortest_path, bresenhams_line, dijkstras,
    get_minimum_spanning_tree,
};
use planner::containers::{LocationDictionary, LocationSet};
use planner::grid::{EntityKind, ExistingPipeGrid, GridEntity, SquareGrid};
use serde_json::Value;

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
}

fn point(value: &Value) -> Location {
    let a = value.as_array().expect("a point");
    Location::new(int(&a[0]), int(&a[1]))
}

/// A flat `[x0, y0, x1, y1, ...]` array.
fn points(value: &Value) -> Vec<Location> {
    let a = value.as_array().expect("a point array");
    a.chunks(2)
        .map(|c| Location::new(int(&c[0]), int(&c[1])))
        .collect()
}

fn hash_set(locations: &[Location]) -> LocationSet {
    let mut set = LocationSet::new_hash();
    for &l in locations {
        set.add(l);
    }
    set
}

struct Pipes {
    set: LocationSet,
    width: i32,
    height: i32,
}

/// FNV-1a over the little-endian bytes of each int, as the recorder computes it.
fn fnv(values: &[i32]) -> u64 {
    let mut hash: u64 = 14695981039346656037;
    for value in values {
        for b in (*value as u32).to_le_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(1099511628211);
        }
    }
    hash
}

#[test]
fn algorithms_match_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/algorithms.jsonl");
    let text = std::fs::read_to_string(&path).expect("read algorithms.jsonl");

    let mut grids: HashMap<String, SquareGrid> = HashMap::new();
    let mut pipes: HashMap<String, Pipes> = HashMap::new();
    let mut counts: HashMap<&'static str, usize> = HashMap::new();

    for (line_number, line) in text.lines().enumerate() {
        let case: Value = serde_json::from_str(line).expect("valid JSON");
        let at = format!("algorithms.jsonl line {}", line_number + 1);
        match case["kind"].as_str().expect("a kind") {
            "grid" => {
                let mut grid = SquareGrid::new(int(&case["width"]), int(&case["height"]));
                for location in points(&case["occupied"]) {
                    let id = grid.get_id();
                    grid.add_entity(location, GridEntity::new(id, EntityKind::TemporaryEntity))
                        .expect("an empty cell");
                }
                grids.insert(case["id"].as_str().unwrap().to_owned(), grid);
            }
            "pipes" => {
                pipes.insert(
                    case["id"].as_str().unwrap().to_owned(),
                    Pipes {
                        set: hash_set(&points(&case["pipes"])),
                        width: int(&case["width"]),
                        height: int(&case["height"]),
                    },
                );
            }
            "astar" => {
                check_a_star(&case, &grids, &pipes, &at);
                *counts.entry("astar").or_default() += 1;
            }
            "dijkstra" => {
                check_dijkstra(&case, &grids, &pipes, &at);
                *counts.entry("dijkstra").or_default() += 1;
            }
            "bfs" => {
                let grid = &grids[case["grid"].as_str().unwrap()];
                let actual =
                    breadth_first_shortest_path(grid, point(&case["start"]), point(&case["goal"]));
                let expected = if case["path"].is_null() {
                    None
                } else {
                    Some(points(&case["path"]))
                };
                assert_eq!(actual, expected, "{at}: BFS path");
                *counts.entry("bfs").or_default() += 1;
            }
            "prims" => {
                check_prims(&case, &at);
                *counts.entry("prims").or_default() += 1;
            }
            "bresenham" => {
                let actual = bresenhams_line(point(&case["a"]), point(&case["b"]));
                assert_eq!(actual, points(&case["path"]), "{at}: Bresenham path");
                *counts.entry("bresenham").or_default() += 1;
            }
            other => panic!("{at}: unknown kind {other}"),
        }
    }

    // Guards against a fixture that silently lost a kind of call.
    for kind in ["astar", "dijkstra", "bfs", "prims", "bresenham"] {
        assert!(
            counts.get(kind).copied().unwrap_or(0) > 100,
            "too few {kind} calls: {counts:?}"
        );
    }
}

fn check_a_star(
    case: &Value,
    grids: &HashMap<String, SquareGrid>,
    pipes: &HashMap<String, Pipes>,
    at: &str,
) {
    let start = point(&case["start"]);
    let goal_list = points(&case["goals"]);
    let goals = match case["goalsKind"].as_str().unwrap() {
        "hash" => hash_set(&goal_list),
        "single" => LocationSet::new_single(goal_list[0]),
        other => panic!("{at}: unknown goals kind {other}"),
    };
    let prefer_no_turns = case["preferNoTurns"].as_bool().unwrap();
    let x_weight = int(&case["xWeight"]);
    let y_weight = int(&case["yWeight"]);
    let prefix = if case["prefix"].is_null() {
        None
    } else {
        Some(points(&case["prefix"]))
    };

    let run = |grid: &dyn Fn(Option<Vec<Location>>) -> planner::algorithms::AStarResult| {
        grid(prefix.clone())
    };
    let result = if let Some(id) = case["grid"].as_str() {
        let grid = &grids[id];
        run(&|prefix| match prefix {
            None => a_star(grid, start, &goals, prefer_no_turns, x_weight, y_weight),
            Some(p) => a_star_into(grid, start, &goals, prefer_no_turns, x_weight, y_weight, p),
        })
    } else {
        let existing = ExistingPipeGrid::new(&pipes[case["pipes"].as_str().unwrap()].set);
        run(&|prefix| match prefix {
            None => a_star(
                &existing,
                start,
                &goals,
                prefer_no_turns,
                x_weight,
                y_weight,
            ),
            Some(p) => a_star_into(
                &existing,
                start,
                &goals,
                prefer_no_turns,
                x_weight,
                y_weight,
                p,
            ),
        })
    };

    assert_eq!(
        result.success,
        case["success"].as_bool().unwrap(),
        "{at}: A* success"
    );
    assert_eq!(
        result.reached_goal,
        point(&case["reached"]),
        "{at}: A* reached goal"
    );
    if result.success {
        assert_eq!(
            result.path().unwrap(),
            points(&case["path"]).as_slice(),
            "{at}: A* path"
        );
    } else {
        assert!(case["path"].is_null(), "{at}: A* path should be null");
        // The C# leaves the caller's list untouched on failure; so does the port.
        assert_eq!(
            result.into_list(),
            prefix.unwrap_or_default(),
            "{at}: caller's list on failure"
        );
    }
}

fn check_dijkstra(
    case: &Value,
    grids: &HashMap<String, SquareGrid>,
    pipes: &HashMap<String, Pipes>,
    at: &str,
) {
    let start = point(&case["start"]);
    let goal_list = points(&case["goals"]);
    let goals = hash_set(&goal_list);
    let stop_on_first_goal = case["stopOnFirstGoal"].as_bool().unwrap();
    let allow_goal_enumerate = case["allowGoalEnumerate"].as_bool().unwrap();

    let (result, width, height) = if let Some(id) = case["grid"].as_str() {
        let grid = &grids[id];
        let result = dijkstras(
            grid,
            grid.width(),
            grid.height(),
            start,
            &goals,
            stop_on_first_goal,
            allow_goal_enumerate,
        );
        (result, grid.width(), grid.height())
    } else {
        let p = &pipes[case["pipes"].as_str().unwrap()];
        let existing = ExistingPipeGrid::new(&p.set);
        let result = dijkstras(
            &existing,
            p.width,
            p.height,
            start,
            &goals,
            stop_on_first_goal,
            allow_goal_enumerate,
        );
        (result, p.width, p.height)
    };

    assert_eq!(
        result.reached_goals.len(),
        int(&case["reachedCount"]) as usize,
        "{at}: reached count"
    );
    if allow_goal_enumerate {
        assert_eq!(
            result.reached_goals.to_vec(),
            points(&case["reached"]),
            "{at}: reached goals in order"
        );
    } else {
        let expected: Vec<bool> = case["reachedContains"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_bool().unwrap())
            .collect();
        let actual: Vec<bool> = goal_list
            .iter()
            .map(|&g| result.reached_goals.contains(g))
            .collect();
        assert_eq!(actual, expected, "{at}: reached goals (bit set)");
    }

    // LocationToPrevious, as the recorder flattens it: each key in order, then a mask of
    // which in-bounds neighbors (east, north, west, south) its set holds.
    let mut previous = Vec::new();
    for (location, set) in result.location_to_previous.iter() {
        let mut mask: i32 = 0;
        let neighbors = [
            location.translate(1, 0),
            location.translate(0, -1),
            location.translate(-1, 0),
            location.translate(0, 1),
        ];
        for (i, n) in neighbors.into_iter().enumerate() {
            if n.x >= 0 && n.y >= 0 && n.x < width && n.y < height && set.contains(n) {
                mask |= 1 << i;
            }
        }
        assert_eq!(
            mask.count_ones() as usize,
            set.len(),
            "{at}: previous set at {location:?} holds a non-neighbor"
        );
        previous.extend([location.x, location.y, mask]);
    }

    assert_eq!(
        result.location_to_previous.len(),
        int(&case["previousCount"]) as usize,
        "{at}: previous count"
    );
    if let Some(full) = case.get("previous") {
        let expected: Vec<i32> = full.as_array().unwrap().iter().map(int).collect();
        assert_eq!(previous, expected, "{at}: LocationToPrevious");
    }
    assert_eq!(
        fnv(&previous),
        case["previousHash"].as_u64().unwrap(),
        "{at}: LocationToPrevious hash"
    );
}

fn check_prims(case: &Value, at: &str) {
    let mut graph: LocationDictionary<LocationDictionary<i32>> = LocationDictionary::new();
    for node in case["graph"].as_array().unwrap() {
        let node = node.as_array().unwrap();
        let mut edges = LocationDictionary::new();
        for edge in node[2].as_array().unwrap().chunks(3) {
            edges.set(Location::new(int(&edge[0]), int(&edge[1])), int(&edge[2]));
        }
        graph.add(Location::new(int(&node[0]), int(&node[1])), edges);
    }

    let mst = get_minimum_spanning_tree(
        &graph,
        point(&case["first"]),
        case["digraph"].as_bool().unwrap(),
    );

    let actual: Vec<(Location, Vec<Location>)> = mst.iter().map(|(k, v)| (k, v.to_vec())).collect();
    let expected: Vec<(Location, Vec<Location>)> = case["mst"]
        .as_array()
        .unwrap()
        .iter()
        .map(|node| {
            let node = node.as_array().unwrap();
            (
                Location::new(int(&node[0]), int(&node[1])),
                points(&node[2]),
            )
        })
        .collect();
    assert_eq!(actual, expected, "{at}: MST in order");
}
