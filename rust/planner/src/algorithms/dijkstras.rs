//! Dijkstra's algorithm, from `Algorithms/Dijkstras.cs` and `DijkstrasResult.cs`.
//!
//! Every step costs the same, so a cell can be reached by several equally short paths, and
//! `LocationToPrevious` keeps all of their previous cells. A cell that is reached again at
//! the same cost after leaving the queue goes back into it, exactly as in the C#.

use dotnet_compat::NetPriorityQueue;

use super::{LocationLookup, LocationMap};
use crate::Location;
use crate::containers::{LocationDictionary, LocationSet};
use crate::grid::{GridNeighbors, NEIGHBOR_COST};

/// One `LocationToPrevious` value: the cells a cell was reached from at its lowest cost.
///
/// In the C# this is a `LocationBitSet` the size of the whole grid, one per visited cell.
/// A bit set cannot be enumerated (`EnumerateItems` throws), so all anyone can observe is
/// its count and which cells it contains. Those cells are always grid neighbors, so this
/// keeps them in a short list instead, which gives the same answers without allocating a
/// grid-sized bit set per cell.
#[derive(Debug, Default)]
pub struct PreviousSet {
    cells: Vec<Location>,
}

impl PreviousSet {
    fn single(location: Location) -> Self {
        PreviousSet {
            cells: vec![location],
        }
    }

    /// `Add`.
    fn add(&mut self, location: Location) -> bool {
        if self.cells.contains(&location) {
            false
        } else {
            self.cells.push(location);
            true
        }
    }

    /// `Count`.
    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// `Contains`.
    pub fn contains(&self, location: Location) -> bool {
        self.cells.contains(&location)
    }
}

/// `DijkstrasResult`.
#[derive(Debug)]
pub struct DijkstrasResult {
    /// `LocationToPrevious`: every visited cell, in the order it was first reached (the
    /// C#'s `LocationHashDictionary` order), with the cells it was reached from.
    pub location_to_previous: LocationDictionary<PreviousSet>,
    /// `ReachedGoals`, in the order they were reached. A `LocationHashSet` when the caller
    /// allowed enumeration, otherwise a `LocationBitSet`.
    pub reached_goals: LocationSet,
}

impl DijkstrasResult {
    /// `GetStraightPaths`. In the C# every `LocationToPrevious` value is a `LocationBitSet`,
    /// whose `EnumerateItems` throws, so this returns an empty list for an unvisited goal
    /// and throws for any other. No caller uses it. It is kept so the port's surface matches.
    pub fn get_straight_paths(&self, goal: Location) -> Vec<Vec<Location>> {
        if self.location_to_previous.contains_key(goal) {
            panic!("NotSupportedException: LocationBitSet.EnumerateItems");
        }
        Vec::new()
    }
}

/// `Dijkstras.GetShortestPaths(context, grid, start, goals, stopOnFirstGoal,
/// allowGoalEnumerate)`. `width` and `height` are the context grid's, for the
/// `LocationBitSet` that `ReachedGoals` is when `allow_goal_enumerate` is false.
pub fn get_shortest_paths(
    grid: &impl GridNeighbors,
    width: i32,
    height: i32,
    start: Location,
    goals: &LocationSet,
    stop_on_first_goal: bool,
    allow_goal_enumerate: bool,
) -> DijkstrasResult {
    let mut came_from: LocationDictionary<PreviousSet> = LocationDictionary::new();
    came_from.set(start, PreviousSet::default());
    // `context.GetLocationSet(goals)`: a copy with the same implementation.
    let mut remaining_goals = goals.copy();
    let mut reached_goals = if allow_goal_enumerate {
        LocationSet::new_hash()
    } else {
        LocationSet::new_bit(width, height)
    };

    let mut priority_queue: NetPriorityQueue<Location, f64> = NetPriorityQueue::new();
    let mut cost_so_far: LocationMap<f64> = LocationMap::default();
    let mut in_queue = LocationLookup::default();

    cost_so_far.insert(start, 0.0);
    priority_queue.enqueue(start, 0.0);
    in_queue.insert(start);

    while let Some(current) = priority_queue.dequeue() {
        in_queue.remove(&current);
        let current_cost = cost_so_far[&current];

        if remaining_goals.remove(current) {
            reached_goals.add(current);

            if stop_on_first_goal || remaining_goals.is_empty() {
                break;
            }
        }

        for neighbor in grid.get_neighbors(current) {
            if !neighbor.is_valid() {
                continue;
            }

            let alternate_cost = current_cost + f64::from(NEIGHBOR_COST);
            let neighbor_cost = cost_so_far.get(&neighbor).copied();
            let previous_exists = neighbor_cost.is_some();
            if !previous_exists || alternate_cost <= neighbor_cost.unwrap() {
                if !previous_exists || alternate_cost < neighbor_cost.unwrap() {
                    cost_so_far.insert(neighbor, alternate_cost);
                    came_from.set(neighbor, PreviousSet::single(current));
                } else {
                    came_from.value_mut(neighbor).add(current);
                }

                if !in_queue.contains(&neighbor) {
                    priority_queue.enqueue(neighbor, alternate_cost);
                    in_queue.insert(neighbor);
                }
            }
        }
    }

    DijkstrasResult {
        location_to_previous: came_from,
        reached_goals,
    }
}
