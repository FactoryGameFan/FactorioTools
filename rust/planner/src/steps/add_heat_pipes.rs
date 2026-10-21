//! `AddHeatPipes`, from `src/FactorioTools/OilField/Steps/AddHeatPipes.cs`.
//!
//! Routes one connected heat pipe network next to every pumpjack and pipe so an oil field
//! survives on Aquilo, where unheated entities freeze. It is a greedy cover: seed with the
//! highest-coverage tile, grow along the network by the best adjacent tile, and bridge
//! through empty tiles with a breadth-first search when nothing adjacent helps.
//!
//! Order matters throughout: candidates are scanned in the order they were found, ties keep
//! the first, and the chosen set enumerates in insertion order (it becomes the order heat
//! pipe entities are added and get their ids). So the candidate list is a `Vec`, and every
//! set the C# enumerates is a .NET-ordered `LocationSet`. The coverage dictionaries are only
//! looked up, never enumerated.

use std::collections::VecDeque;

use crate::Location;
use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;
use crate::error::FtResult;
use crate::grid::{EntityKind, GridEntity, GridNeighbors, SquareGrid};
use crate::steps::validate::PUMPJACK_RING_OFFSETS;

/// `Execute(context)`: routes the network around the pipes and pumpjacks already on the grid,
/// places the heat pipes and stores them in `context.heat_pipes`. Does nothing unless heat
/// pipes are on and none were routed yet (with beacons on, `AddPipes` routes and places them
/// before beacon planning).
pub fn execute(context: &mut Context) -> FtResult<()> {
    if !context.options.add_heat_pipes || context.heat_pipes.is_some() {
        return Ok(());
    }

    // The targets that must be heated: every pipe tile (and every pumpjack center, below).
    let mut pipe_tiles = context.get_location_set_enumerable(true);
    for location in context.grid.entity_locations().iter() {
        if context
            .grid
            .get(location)
            .is_some_and(|entity| entity.kind.is_pipe())
        {
            pipe_tiles.add(location);
        }
    }

    let (chosen, _, _) = route_core(context, &pipe_tiles);

    // Place the heat pipe entities.
    for location in chosen.iter() {
        let id = context.grid.get_id();
        context
            .grid
            .add_entity(location, GridEntity::new(id, EntityKind::HeatPipe))?;
    }

    context.heat_pipes = Some(chosen);
    Ok(())
}

/// What [`route`] returns: the chosen heat tiles, then the pipes and pumpjack centers that no
/// reachable tile could heat (both empty when the layout can be fully heated).
pub struct RouteResult {
    pub chosen: LocationSet,
    pub uncovered_pipes: LocationSet,
    pub uncovered_centers: LocationSet,
}

/// `Route(context, pipeTiles, out uncoveredPipes, out uncoveredCenters)`: routes the network
/// for pipe tiles that are not on the grid yet, without placing anything. The pipe tiles
/// occupy the grid as temporary entities while routing, which takes ids, and are removed
/// again afterwards.
pub fn route(context: &mut Context, pipe_tiles: &LocationSet) -> FtResult<RouteResult> {
    for pipe in pipe_tiles.iter() {
        let id = context.grid.get_id();
        context
            .grid
            .add_entity(pipe, GridEntity::new(id, EntityKind::TemporaryEntity))?;
    }

    let (chosen, uncovered_pipes, uncovered_centers) = route_core(context, pipe_tiles);

    for pipe in pipe_tiles.iter() {
        context.grid.remove_entity(pipe);
    }

    Ok(RouteResult {
        chosen,
        uncovered_pipes,
        uncovered_centers,
    })
}

/// `RouteCore`: builds the candidate tiles and what each covers, then grows the network.
/// Assumes the pipes and pumpjacks already occupy the grid.
fn route_core(
    context: &Context,
    pipe_tiles: &LocationSet,
) -> (LocationSet, LocationSet, LocationSet) {
    let grid = &context.grid;

    // Candidate heat tiles (empty tiles that cover at least one target), in the order found.
    let mut candidates = Vec::new();
    let mut covered_pipes: LocationDictionary<Vec<Location>> = context.get_location_dictionary();
    let mut covered_centers: LocationDictionary<Vec<Location>> = context.get_location_dictionary();

    for pipe in pipe_tiles.iter() {
        for candidate in grid.get_adjacent(pipe) {
            if !candidate.is_valid() || !grid.is_empty(candidate) {
                continue;
            }

            add_coverage(&mut covered_pipes, &mut candidates, candidate, pipe);
        }
    }

    for &center in &context.centers {
        for offset in PUMPJACK_RING_OFFSETS {
            let candidate = center.translate_by(offset);
            if !grid.is_in_bounds(candidate) || !grid.is_empty(candidate) {
                continue;
            }

            add_coverage(&mut covered_centers, &mut candidates, candidate, center);
        }
    }

    let mut chosen = context.get_location_set_enumerable(true);

    let mut uncovered_pipes = context.get_location_set_enumerable(true);
    uncovered_pipes.union_with(pipe_tiles);
    let mut uncovered_centers = context.get_location_set_enumerable(true);
    for &center in &context.centers {
        uncovered_centers.add(center);
    }

    if !candidates.is_empty() {
        let mut coverage = Coverage {
            pipes: &covered_pipes,
            centers: &covered_centers,
            uncovered_pipes: &mut uncovered_pipes,
            uncovered_centers: &mut uncovered_centers,
        };
        grow(grid, &mut chosen, &candidates, &mut coverage);
    }

    (chosen, uncovered_pipes, uncovered_centers)
}

/// The two coverage channels the router tracks: what each candidate tile heats, and what is
/// still unheated. `AddHeatPipes` uses pipes and pumpjack centers; `ExtendToBeacons` passes
/// an empty pipe channel and beacons in the second.
struct Coverage<'a> {
    pipes: &'a LocationDictionary<Vec<Location>>,
    centers: &'a LocationDictionary<Vec<Location>>,
    uncovered_pipes: &'a mut LocationSet,
    uncovered_centers: &'a mut LocationSet,
}

impl Coverage<'_> {
    /// `Gain`: how many still-unheated targets a candidate would heat.
    fn gain(&self, candidate: Location) -> i32 {
        count_covered(self.pipes, candidate, self.uncovered_pipes)
            + count_covered(self.centers, candidate, self.uncovered_centers)
    }

    fn any_uncovered(&self) -> bool {
        !self.uncovered_pipes.is_empty() || !self.uncovered_centers.is_empty()
    }

    /// `AddTile`: adds a tile to the network and marks what it heats.
    fn add_tile(&mut self, chosen: &mut LocationSet, location: Location) {
        if !chosen.add(location) {
            return;
        }

        remove_covered(self.pipes, location, self.uncovered_pipes);
        remove_covered(self.centers, location, self.uncovered_centers);
    }
}

/// `Grow`: seeds the network with the best candidate, preferring one with an empty neighbor
/// so the network can actually expand from it, then grows it.
fn grow(
    grid: &SquareGrid,
    chosen: &mut LocationSet,
    candidates: &[Location],
    coverage: &mut Coverage<'_>,
) {
    let mut seed = Location::INVALID;
    let mut seed_gain = -1;
    let mut expandable_seed = Location::INVALID;
    let mut expandable_seed_gain = -1;
    for &candidate in candidates {
        let gain = coverage.gain(candidate);
        if gain > seed_gain {
            seed_gain = gain;
            seed = candidate;
        }

        if gain > expandable_seed_gain && has_empty_neighbor(grid, candidate) {
            expandable_seed_gain = gain;
            expandable_seed = candidate;
        }
    }

    if expandable_seed.is_valid() {
        seed = expandable_seed;
    }

    coverage.add_tile(chosen, seed);

    grow_from(grid, chosen, candidates, coverage);
}

/// `GrowFrom`: grows a seeded network until every reachable target is heated, bridging
/// through empty tiles when no adjacent candidate helps.
fn grow_from(
    grid: &SquareGrid,
    chosen: &mut LocationSet,
    candidates: &[Location],
    coverage: &mut Coverage<'_>,
) {
    // Candidates bridging could not reach; skipped on later passes so the loop ends.
    let mut unreachable = LocationSet::new_bit(grid.width(), grid.height());

    while coverage.any_uncovered() {
        // The best candidate next to the network (it stays connected for free).
        let mut best_adjacent = Location::INVALID;
        let mut best_adjacent_gain = 0;

        // The bridge fallback: the best candidate anywhere that still heats something.
        let mut best_any = Location::INVALID;
        let mut best_any_gain = 0;

        for &candidate in candidates {
            if chosen.contains(candidate) {
                continue;
            }

            let gain = coverage.gain(candidate);
            if gain == 0 {
                continue;
            }

            if !unreachable.contains(candidate) && gain > best_any_gain {
                best_any_gain = gain;
                best_any = candidate;
            }

            if gain > best_adjacent_gain && is_adjacent_to_network(grid, chosen, candidate) {
                best_adjacent_gain = gain;
                best_adjacent = candidate;
            }
        }

        if best_adjacent_gain > 0 {
            coverage.add_tile(chosen, best_adjacent);
            continue;
        }

        if best_any_gain == 0 {
            // What is left has no empty tile next to it at all: boxed in, cannot be heated.
            break;
        }

        // Bridge the network out to the best remaining candidate.
        match bridge_to_tile(grid, chosen, best_any) {
            None => {
                unreachable.add(best_any);
            }
            Some(path) => {
                for location in path {
                    coverage.add_tile(chosen, location);
                }
            }
        }
    }
}

/// `ExtendToBeacons(context, beaconCenters)`: grows the placed heat network
/// (`context.heat_pipes`) to reach the placed beacons, places the new heat pipes, and returns
/// the beacon centers that ended up heated. A beacon is heated when a heat pipe is next to any
/// cell of its footprint.
pub fn extend_to_beacons(
    context: &mut Context,
    beacon_centers: &[Location],
) -> FtResult<LocationSet> {
    let mut heated_beacons = context.get_location_set_enumerable(true);

    if context.heat_pipes.is_none() || beacon_centers.is_empty() {
        return Ok(heated_beacons);
    }

    let width = context.options.beacon_width;
    let height = context.options.beacon_height;

    let mut uncovered_beacons = context.get_location_set_enumerable(true);
    for &center in beacon_centers {
        uncovered_beacons.add(center);
    }

    let mut candidates = Vec::new();
    let mut covered_beacons: LocationDictionary<Vec<Location>> = context.get_location_dictionary();

    // Empty stand-ins for the pipe channel GrowFrom shares with the pipe and pumpjack router.
    let no_pipe_coverage: LocationDictionary<Vec<Location>> = context.get_location_dictionary();
    let mut no_pipes = context.get_location_set_enumerable(true);

    let grid = &context.grid;
    for &center in beacon_centers {
        let min_x = center.x - ((width - 1) / 2);
        let max_x = center.x + (width / 2);
        let min_y = center.y - ((height - 1) / 2);
        let max_y = center.y + (height / 2);

        // First pass: is this beacon already next to the network?
        let mut already_heated = false;
        let mut x = min_x;
        while x <= max_x && !already_heated {
            let mut y = min_y;
            while y <= max_y && !already_heated {
                for n in grid.get_adjacent(Location::new(x, y)) {
                    if n.is_valid()
                        && grid
                            .get(n)
                            .is_some_and(|e| matches!(e.kind, EntityKind::HeatPipe))
                    {
                        already_heated = true;
                        break;
                    }
                }
                y += 1;
            }
            x += 1;
        }

        if already_heated {
            uncovered_beacons.remove(center);
            continue;
        }

        // Second pass: the empty tiles next to the footprint would heat this beacon.
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                for n in grid.get_adjacent(Location::new(x, y)) {
                    if !n.is_valid() {
                        continue;
                    }

                    let inside_footprint =
                        n.x >= min_x && n.x <= max_x && n.y >= min_y && n.y <= max_y;
                    if !inside_footprint && grid.is_empty(n) {
                        add_coverage(&mut covered_beacons, &mut candidates, n, center);
                    }
                }
            }
        }
    }

    // Seed the grow with the whole existing network so the extension stays one component.
    let mut chosen = context.get_location_set_enumerable(true);
    chosen.union_with(context.heat_pipes.as_ref().expect("checked above"));

    if !candidates.is_empty() && !uncovered_beacons.is_empty() {
        let mut coverage = Coverage {
            pipes: &no_pipe_coverage,
            centers: &covered_beacons,
            uncovered_pipes: &mut no_pipes,
            uncovered_centers: &mut uncovered_beacons,
        };
        grow_from(&context.grid, &mut chosen, &candidates, &mut coverage);
    }

    // Place the newly chosen tiles and grow the network set.
    for location in chosen.iter() {
        if context
            .heat_pipes
            .as_ref()
            .expect("checked above")
            .contains(location)
        {
            continue;
        }

        let id = context.grid.get_id();
        context
            .grid
            .add_entity(location, GridEntity::new(id, EntityKind::HeatPipe))?;
        context
            .heat_pipes
            .as_mut()
            .expect("checked above")
            .add(location);
    }

    for &center in beacon_centers {
        if !uncovered_beacons.contains(center) {
            heated_beacons.add(center);
        }
    }

    Ok(heated_beacons)
}

/// `HasEmptyNeighbor`.
fn has_empty_neighbor(grid: &SquareGrid, location: Location) -> bool {
    grid.get_adjacent(location)
        .into_iter()
        .any(|adjacent| adjacent.is_valid() && grid.is_empty(adjacent))
}

/// `IsAdjacentToNetwork`.
fn is_adjacent_to_network(grid: &SquareGrid, chosen: &LocationSet, candidate: Location) -> bool {
    grid.get_adjacent(candidate)
        .into_iter()
        .any(|adjacent| adjacent.is_valid() && chosen.contains(adjacent))
}

/// `AddCoverage`: records that `candidate` heats `target`, listing a new candidate once.
fn add_coverage(
    covered: &mut LocationDictionary<Vec<Location>>,
    candidates: &mut Vec<Location>,
    candidate: Location,
    target: Location,
) {
    match covered.get_mut(candidate) {
        Some(targets) => targets.push(target),
        None => {
            covered.add(candidate, vec![target]);
            candidates.push(candidate);
        }
    }
}

/// `CountCovered`.
fn count_covered(
    covered: &LocationDictionary<Vec<Location>>,
    candidate: Location,
    uncovered: &LocationSet,
) -> i32 {
    match covered.get(candidate) {
        None => 0,
        Some(targets) => targets.iter().filter(|&&t| uncovered.contains(t)).count() as i32,
    }
}

/// `RemoveCovered`.
fn remove_covered(
    covered: &LocationDictionary<Vec<Location>>,
    candidate: Location,
    uncovered: &mut LocationSet,
) {
    if let Some(targets) = covered.get(candidate) {
        for &target in targets {
            uncovered.remove(target);
        }
    }
}

/// `BridgeToTile`: the shortest path of empty tiles from any chosen tile to `goal`, goal
/// first and network end last, or `None` if `goal` cannot be reached. Chosen tiles are still
/// empty here (placement comes later), so the search can run along the network itself.
fn bridge_to_tile(
    grid: &SquareGrid,
    chosen: &LocationSet,
    goal: Location,
) -> Option<Vec<Location>> {
    let mut queue: VecDeque<Location> = chosen.iter().collect();
    let mut visited = LocationSet::new_bit(grid.width(), grid.height());
    let mut parents: LocationDictionary<Location> = LocationDictionary::new();

    while let Some(current) = queue.pop_front() {
        if !visited.add(current) {
            continue;
        }

        if current == goal {
            let mut path = vec![current];
            let mut current = current;
            while let Some(&parent) = parents.get(current) {
                path.push(parent);
                current = parent;
            }

            return Some(path);
        }

        for next in grid.get_neighbors(current) {
            if !next.is_valid() || visited.contains(next) || !parents.try_add(next, current) {
                continue;
            }

            queue.push_back(next);
        }
    }

    None
}
