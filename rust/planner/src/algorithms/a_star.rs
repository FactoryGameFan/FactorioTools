//! A*, from `Algorithms/AStar.cs` and `AStarResult.cs`, with `Helpers.GetPath` and
//! `Helpers.AddPath`.
//!
//! The result depends on exactly how ties are broken: the frontier is .NET's 4-ary
//! `PriorityQueue` ([`NetPriorityQueue`]), neighbors come in the grid's order (east, north,
//! west, south), and turns cost an extra 0.0001. The C# `USE_VECTORS` heuristic is an exact
//! integer minimum, the same as the plain loop here.

use dotnet_compat::NetPriorityQueue;

use super::LocationMap;
use crate::Location;
use crate::containers::LocationSet;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{GridNeighbors, NEIGHBOR_COST};

/// `AStarResult`.
#[derive(Debug)]
pub struct AStarResult {
    pub success: bool,
    /// `ReachedGoal`: the goal reached, or `Location.Invalid` when none was.
    pub reached_goal: Location,
    /// The path list. On success it is the path (appended to the caller's list, if one was
    /// passed). On failure it is the caller's list unchanged, or empty.
    list: Vec<Location>,
}

impl AStarResult {
    /// `Path`: the path from the reached goal back to the start (goal first), or the C#'s
    /// `FactorioToolsException` when no goal was reached.
    pub fn path(&self) -> FtResult<&[Location]> {
        if self.success {
            Ok(&self.list)
        } else {
            Err(FactorioToolsError::new("No goal location was reached."))
        }
    }

    /// The list itself, whether or not a goal was reached. Hands a caller's list back.
    pub fn into_list(self) -> Vec<Location> {
        self.list
    }
}

/// `AStar.GetShortestPath(context, grid, start, goals, preferNoTurns, xWeight, yWeight)`
/// with no `outputList`: the path goes in a new list.
pub fn get_shortest_path(
    grid: &impl GridNeighbors,
    start: Location,
    goals: &LocationSet,
    prefer_no_turns: bool,
    x_weight: i32,
    y_weight: i32,
) -> AStarResult {
    run(
        grid,
        start,
        goals,
        prefer_no_turns,
        x_weight,
        y_weight,
        Vec::new(),
    )
}

/// `AStar.GetShortestPath` with an `outputList`: on success the path is appended to
/// `output`, after whatever it already holds, as `Helpers.AddPath` does.
pub fn get_shortest_path_into(
    grid: &impl GridNeighbors,
    start: Location,
    goals: &LocationSet,
    prefer_no_turns: bool,
    x_weight: i32,
    y_weight: i32,
    output: Vec<Location>,
) -> AStarResult {
    run(
        grid,
        start,
        goals,
        prefer_no_turns,
        x_weight,
        y_weight,
        output,
    )
}

#[allow(clippy::too_many_arguments)]
fn run(
    grid: &impl GridNeighbors,
    start: Location,
    goals: &LocationSet,
    prefer_no_turns: bool,
    x_weight: i32,
    y_weight: i32,
    mut list: Vec<Location>,
) -> AStarResult {
    if goals.contains(start) {
        list.push(start);
        return AStarResult {
            success: true,
            reached_goal: start,
            list,
        };
    }

    // `goals.CopyTo(goalsArray)`. A `LocationBitSet` throws here in the C#, and so does
    // `copy_to`.
    let mut goals_array = vec![Location::INVALID; goals.len()];
    goals.copy_to(&mut goals_array);

    let mut came_from: LocationMap<Location> = LocationMap::default();
    let mut cost_so_far: LocationMap<f64> = LocationMap::default();
    let mut frontier: NetPriorityQueue<Location, f64> = NetPriorityQueue::new();

    frontier.enqueue(start, 0.0);
    came_from.insert(start, start);
    cost_so_far.insert(start, 0.0);

    let mut reached_goal = Location::INVALID;
    let mut success = false;

    while let Some(current) = frontier.dequeue() {
        if goals.contains(current) {
            reached_goal = current;
            success = true;
            break;
        }

        let previous = came_from[&current];
        let current_cost = cost_so_far[&current];

        for next in grid.get_neighbors(current) {
            if !next.is_valid() {
                continue;
            }

            let new_cost = current_cost + f64::from(NEIGHBOR_COST);
            let improves = match cost_so_far.get(&next) {
                None => true,
                Some(&this_cost_so_far) => new_cost < this_cost_so_far,
            };
            if improves {
                cost_so_far.insert(next, new_cost);
                let mut priority =
                    new_cost + f64::from(heuristic(next, &goals_array, x_weight, y_weight));

                // Prefer paths without turns.
                if prefer_no_turns && previous != current && is_turn(previous, current, next) {
                    priority += 0.0001;
                }

                frontier.enqueue(next, priority);
                came_from.insert(next, current);
            }
        }
    }

    if success {
        // `GetPath` makes a new list and `AddPath` appends to the caller's; both walk
        // `cameFrom` from the goal back to the start, so one function covers both.
        add_path(&came_from, reached_goal, &mut list);
    }

    AStarResult {
        success,
        reached_goal,
        list,
    }
}

/// `AStar.IsTurn`.
fn is_turn(a: Location, b: Location, c: Location) -> bool {
    let direction_a = if a.x == b.x { 0 } else { 1 };
    let direction_b = if b.x == c.x { 0 } else { 1 };
    direction_a != direction_b
}

/// `AStar.Heuristic`: the smallest weighted Manhattan distance to any goal.
fn heuristic(current: Location, goals: &[Location], x_weight: i32, y_weight: i32) -> i32 {
    let mut min = i32::MAX;
    for goal in goals {
        let val = x_weight * (goal.x - current.x).abs() + y_weight * (goal.y - current.y).abs();
        if val < min {
            min = val;
        }
    }
    min
}

/// `Helpers.AddPath`: from the goal back to the start, which maps to itself.
fn add_path(came_from: &LocationMap<Location>, reached_goal: Location, output: &mut Vec<Location>) {
    let mut current = reached_goal;
    loop {
        let next = came_from[&current];
        output.push(current);
        if next == current {
            break;
        }
        current = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grid::SquareGrid;

    #[test]
    fn start_in_goals_returns_just_the_start() {
        let grid = SquareGrid::new(5, 5);
        let mut goals = LocationSet::new_hash();
        goals.add(Location::new(1, 1));
        let result = get_shortest_path(&grid, Location::new(1, 1), &goals, true, 1, 1);
        assert!(result.success);
        assert_eq!(result.path().unwrap(), &[Location::new(1, 1)]);
    }

    #[test]
    fn appends_to_the_output_list() {
        let grid = SquareGrid::new(5, 5);
        let mut goals = LocationSet::new_hash();
        goals.add(Location::new(2, 0));
        let prefix = vec![Location::new(9, 9)];
        let result = get_shortest_path_into(&grid, Location::new(0, 0), &goals, true, 1, 1, prefix);
        assert_eq!(
            result.path().unwrap(),
            &[
                Location::new(9, 9),
                Location::new(2, 0),
                Location::new(1, 0),
                Location::new(0, 0)
            ]
        );
    }

    #[test]
    fn failure_keeps_the_callers_list() {
        let grid = SquareGrid::new(3, 3);
        let mut goals = LocationSet::new_hash();
        goals.add(Location::new(10, 10));
        let result = get_shortest_path_into(
            &grid,
            Location::new(0, 0),
            &goals,
            true,
            1,
            1,
            vec![Location::new(7, 7)],
        );
        assert!(!result.success);
        assert_eq!(result.reached_goal, Location::INVALID);
        assert!(result.path().is_err());
        assert_eq!(result.into_list(), vec![Location::new(7, 7)]);
    }
}
