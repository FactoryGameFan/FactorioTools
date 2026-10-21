//! A*, from `Algorithms/AStar.cs` and `AStarResult.cs`, with `Helpers.GetPath` and
//! `Helpers.AddPath`.
//!
//! The result depends on exactly how ties are broken: the frontier is .NET's 4-ary
//! `PriorityQueue` ([`NetPriorityQueue`]), neighbors come in the grid's order (east, north,
//! west, south), and turns cost an extra 0.0001. The C# `USE_VECTORS` heuristic is an exact
//! integer minimum, the same as the plain loop here.

use std::cell::RefCell;

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

    let search = Search {
        goals,
        goals_array: &goals_array,
        prefer_no_turns,
        x_weight,
        y_weight,
    };
    match grid.bounds() {
        Some((width, height))
            if start.x >= 0 && start.x < width && start.y >= 0 && start.y < height =>
        {
            DENSE.with_borrow_mut(|dense| {
                dense.begin(width, height);
                search.run(grid, start, dense, list)
            })
        }
        _ => search.run(grid, start, &mut HashScratch::default(), list),
    }
}

/// The inputs that stay fixed for one search.
struct Search<'a> {
    goals: &'a LocationSet,
    goals_array: &'a [Location],
    prefer_no_turns: bool,
    x_weight: i32,
    y_weight: i32,
}

impl Search<'_> {
    fn run(
        &self,
        grid: &impl GridNeighbors,
        start: Location,
        scratch: &mut impl Scratch,
        mut list: Vec<Location>,
    ) -> AStarResult {
        let mut frontier: NetPriorityQueue<Location, f64> = NetPriorityQueue::new();

        frontier.enqueue(start, 0.0);
        scratch.reach(start, 0.0, start);

        let mut reached_goal = Location::INVALID;
        let mut success = false;

        while let Some(current) = frontier.dequeue() {
            if self.goals.contains(current) {
                reached_goal = current;
                success = true;
                break;
            }

            let previous = scratch.came_from(current);
            let current_cost = scratch.cost(current).expect("a dequeued cell was reached");

            for next in grid.get_neighbors(current) {
                if !next.is_valid() {
                    continue;
                }

                let new_cost = current_cost + f64::from(NEIGHBOR_COST);
                let improves = match scratch.cost(next) {
                    None => true,
                    Some(this_cost_so_far) => new_cost < this_cost_so_far,
                };
                if improves {
                    let mut priority = new_cost
                        + f64::from(heuristic(
                            next,
                            self.goals_array,
                            self.x_weight,
                            self.y_weight,
                        ));

                    // Prefer paths without turns.
                    if self.prefer_no_turns
                        && previous != current
                        && is_turn(previous, current, next)
                    {
                        priority += 0.0001;
                    }

                    frontier.enqueue(next, priority);
                    scratch.reach(next, new_cost, current);
                }
            }
        }

        if success {
            // `GetPath` makes a new list and `AddPath` appends to the caller's; both walk
            // `cameFrom` from the goal back to the start, so one function covers both.
            add_path(scratch, reached_goal, &mut list);
        }

        AStarResult {
            success,
            reached_goal,
            list,
        }
    }
}

/// Where a search keeps each reached cell's cost so far (the C#'s `costSoFar`) and the cell
/// it was reached from (`cameFrom`). Both are only looked up, never enumerated, so how they
/// are stored cannot change the path.
trait Scratch {
    /// The cost so far, or `None` for a cell this search has not reached.
    fn cost(&self, location: Location) -> Option<f64>;
    /// The cell `location` was reached from. Only asked of reached cells.
    fn came_from(&self, location: Location) -> Location;
    /// Records that `location` was reached from `from` at `cost`.
    fn reach(&mut self, location: Location, cost: f64, from: Location);
}

/// Hash maps, for grids with no fixed bounds ([`crate::grid::ExistingPipeGrid`]).
#[derive(Default)]
struct HashScratch {
    came_from: LocationMap<Location>,
    cost_so_far: LocationMap<f64>,
}

impl Scratch for HashScratch {
    fn cost(&self, location: Location) -> Option<f64> {
        self.cost_so_far.get(&location).copied()
    }

    fn came_from(&self, location: Location) -> Location {
        self.came_from[&location]
    }

    fn reach(&mut self, location: Location, cost: f64, from: Location) {
        self.cost_so_far.insert(location, cost);
        self.came_from.insert(location, from);
    }
}

/// One slot per grid cell, for grids whose neighbors never leave their bounds. The arrays
/// are kept between searches, so a search neither allocates nor rehashes. Instead of
/// clearing them, each search takes a new generation number, and a cell counts as reached
/// only when its stamp holds the current one.
#[derive(Default)]
struct DenseScratch {
    width: i32,
    generation: u32,
    stamp: Vec<u32>,
    cost_so_far: Vec<f64>,
    came_from: Vec<Location>,
}

thread_local! {
    static DENSE: RefCell<DenseScratch> = RefCell::new(DenseScratch::default());
}

impl DenseScratch {
    /// Starts a search on a `width` by `height` grid.
    fn begin(&mut self, width: i32, height: i32) {
        let cells = (width * height) as usize;
        if self.stamp.len() < cells {
            self.stamp.resize(cells, 0);
            self.cost_so_far.resize(cells, 0.0);
            self.came_from.resize(cells, Location::INVALID);
        }
        self.width = width;
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            // After 2^32 searches the stamps could repeat, so start them over.
            self.stamp.fill(0);
            self.generation = 1;
        }
    }

    fn index(&self, location: Location) -> usize {
        debug_assert!(location.x >= 0 && location.x < self.width && location.y >= 0);
        (location.y * self.width + location.x) as usize
    }
}

impl Scratch for DenseScratch {
    fn cost(&self, location: Location) -> Option<f64> {
        let i = self.index(location);
        (self.stamp[i] == self.generation).then(|| self.cost_so_far[i])
    }

    fn came_from(&self, location: Location) -> Location {
        let i = self.index(location);
        debug_assert_eq!(
            self.stamp[i], self.generation,
            "{location:?} was not reached"
        );
        self.came_from[i]
    }

    fn reach(&mut self, location: Location, cost: f64, from: Location) {
        let i = self.index(location);
        self.stamp[i] = self.generation;
        self.cost_so_far[i] = cost;
        self.came_from[i] = from;
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
fn add_path(scratch: &impl Scratch, reached_goal: Location, output: &mut Vec<Location>) {
    let mut current = reached_goal;
    loop {
        let next = scratch.came_from(current);
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
    use crate::grid::{EntityKind, GridEntity, SquareGrid};

    /// A grid with about a third of its cells blocked, from a fixed pseudo-random sequence.
    fn blocked_grid(width: i32, height: i32, seed: u64) -> SquareGrid {
        let mut grid = SquareGrid::new(width, height);
        let mut state = seed;
        for y in 0..height {
            for x in 0..width {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1);
                if (state >> 33).is_multiple_of(3) && (x, y) != (0, 0) {
                    let id = grid.get_id();
                    grid.add_entity(
                        Location::new(x, y),
                        GridEntity::new(id, EntityKind::TemporaryEntity),
                    )
                    .unwrap();
                }
            }
        }
        grid
    }

    #[test]
    fn dense_and_hash_scratch_find_the_same_paths() {
        // The grid sizes change from search to search, so the reused dense arrays see both
        // a larger and a smaller grid than the one before.
        let sizes = [(12, 9), (30, 25), (7, 7), (30, 25), (16, 40)];
        for (round, &(width, height)) in sizes.iter().enumerate() {
            let grid = blocked_grid(width, height, round as u64 + 1);
            let mut goals = LocationSet::new_hash();
            goals.add(Location::new(width - 1, height - 1));
            goals.add(Location::new(width / 2, height - 1));
            let goals_array = [
                Location::new(width - 1, height - 1),
                Location::new(width / 2, height - 1),
            ];
            for prefer_no_turns in [false, true] {
                let search = Search {
                    goals: &goals,
                    goals_array: &goals_array,
                    prefer_no_turns,
                    x_weight: 1,
                    y_weight: 2,
                };
                let start = Location::new(0, 0);
                let hash = search.run(&grid, start, &mut HashScratch::default(), Vec::new());
                let dense = DENSE.with_borrow_mut(|dense| {
                    dense.begin(width, height);
                    search.run(&grid, start, dense, Vec::new())
                });
                assert_eq!(hash.success, dense.success, "{width}x{height}");
                assert_eq!(hash.reached_goal, dense.reached_goal, "{width}x{height}");
                assert_eq!(hash.into_list(), dense.into_list(), "{width}x{height}");
            }
        }
    }

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
