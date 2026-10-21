//! Breadth-first search, from `Algorithms/BreadthFirstFinder.cs`.

use std::collections::VecDeque;

use super::{LocationLookup, LocationMap};
use crate::Location;
use crate::grid::GridNeighbors;

/// `BreadthFirstFinder.GetShortestPath(context, start, goal)`: the path from `start` to
/// `goal`, start first, or `None` if there is none. The C# walks `context.Grid`; pass that
/// grid here.
pub fn get_shortest_path(
    grid: &impl GridNeighbors,
    start: Location,
    goal: Location,
) -> Option<Vec<Location>> {
    let mut to_explore = VecDeque::new();
    let mut parents: LocationMap<Location> = LocationMap::default();
    let mut visited = LocationLookup::default();

    to_explore.push_back(start);

    while let Some(current) = to_explore.pop_front() {
        if !visited.insert(current) {
            continue;
        }

        if current == goal {
            let mut output = vec![current];
            let mut current = current;
            while let Some(&parent) = parents.get(&current) {
                output.push(parent);
                current = parent;
            }

            output.reverse();
            return Some(output);
        }

        for next in grid.get_neighbors(current) {
            // `parents.TryAdd` keeps the first parent a cell was found from.
            if !next.is_valid() || visited.contains(&next) || parents.contains_key(&next) {
                continue;
            }

            parents.insert(next, current);
            to_explore.push_back(next);
        }
    }

    None
}
