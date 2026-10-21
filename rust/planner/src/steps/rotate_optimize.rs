//! `RotateOptimize`, from `src/FactorioTools/OilField/Steps/RotateOptimize.cs`: shortens a
//! strategy's pipes by trying each pumpjack's other terminals and by rerouting the pipe runs
//! between intersections.
//!
//! It changes `pipes` and the context's two terminal maps in place, and only reads the grid.
//! The C# `ChildContext` becomes [`State`] plus the `context` and `pipes` arguments, and its
//! `ExistingPipeGrid` is the live `pipes` set, read through [`pipe_neighbors`].
//!
//! Order matters throughout: `CenterToTerminals` and the intersections are walked in .NET's
//! enumeration order, and every add and remove on `pipes` goes in the C#'s order, because
//! `pipes` is a .NET-ordered hash set and later steps walk it.

use std::collections::VecDeque;

use crate::Location;
use crate::algorithms::{a_star, a_star_into};
use crate::collection_ext::single;
use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;
use crate::error::FtResult;
use crate::grid::{EntityKind, TerminalLocation};
use crate::helpers::{self, TERMINAL_OFFSETS, add_path, count_turns};

/// The C# `ChildContext`'s own sets. `intersections` is enumerable; `goals` is the
/// non-enumerable bit set `GetLocationSet(pipes.Count)` gives.
struct State {
    intersections: LocationSet,
    goals: LocationSet,
}

/// `ExploredPaths`: a breadth-first walk along the pipes from `start`.
struct ExploredPaths {
    came_from: LocationDictionary<Location>,
    reached_goals: Vec<Location>,
}

impl ExploredPaths {
    /// `ExploredPaths.AddPath`: the path from `goal` back to the start, goal first.
    fn add_path(&self, goal: Location, output: &mut Vec<Location>) {
        add_path(&self.came_from, goal, output);
    }
}

/// `RotateOptimize.Execute(parentContext, pipes)`.
///
/// Errors where the C# throws: `Single` on a center with more or fewer than one terminal or
/// on an exploration that reached more or fewer than one goal, and `AStarResult.Path` when a
/// reroute finds no path.
pub fn execute(context: &mut Context, pipes: &mut LocationSet) -> FtResult<()> {
    if context.location_to_terminals.len() < 2 {
        return Ok(());
    }

    // `new ChildContext(parentContext, pipes)`.
    let mut state = State {
        intersections: context.get_location_set_with_capacity(pipes.len(), true),
        goals: context.get_location_set_with_capacity(pipes.len(), false),
    };
    update_intersections_and_goals(context, pipes, &mut state);

    let mut modified = true;
    let mut previous_pipe_count = i32::MAX;

    // Some oil fields have multiple optimal configurations with the same pipe count. Allow up
    // to 5 of these to be attempted before settling on one.
    let mut allowed_same_pipe_counts = 5;

    while modified && (pipe_count(pipes) < previous_pipe_count || allowed_same_pipe_counts > 0) {
        let mut changed_terminal = false;

        // `foreach (var terminals in context.CenterToTerminals.Values)`. The loop changes the
        // lists but never the dictionary's keys, so walking the keys and reading each list when
        // its turn comes sees what the C# sees.
        let centers: Vec<Location> = context.center_to_terminals.keys().collect();
        for center in centers {
            let current_terminal =
                single(context.center_to_terminals.value(center).iter().copied())?;

            if context
                .location_to_terminals
                .value(current_terminal.terminal)
                .len()
                > 1
                || state.intersections.contains(current_terminal.terminal)
            {
                continue;
            }

            if use_best_terminal(context, pipes, &mut state, current_terminal)? {
                changed_terminal = true;
            }
        }

        let mut shortened_path = false;
        for intersection in state.intersections.to_vec() {
            if !state.intersections.contains(intersection) {
                continue;
            }

            state.goals.remove(intersection);
            let explored_paths = explore_paths(pipes, &state, intersection);
            state.goals.add(intersection);

            for &goal in &explored_paths.reached_goals {
                if use_shortest_path(
                    context,
                    pipes,
                    &mut state,
                    &explored_paths,
                    intersection,
                    goal,
                )? {
                    shortened_path = true;
                }
            }
        }

        modified = changed_terminal || shortened_path;
        if previous_pipe_count == pipe_count(pipes) {
            allowed_same_pipe_counts -= 1;
        } else {
            previous_pipe_count = pipe_count(pipes);
        }
    }

    Ok(())
}

/// `Pipes.Count`, as the C#'s `int`.
fn pipe_count(pipes: &LocationSet) -> i32 {
    i32::try_from(pipes.len()).expect("pipe count fits in an int")
}

/// `UseBestTerminal`: try each of the pumpjack's four terminals and keep the one with the
/// shortest path to the other pipes, then the fewest turns.
fn use_best_terminal(
    context: &mut Context,
    pipes: &mut LocationSet,
    state: &mut State,
    original_terminal: TerminalLocation,
) -> FtResult<bool> {
    state.goals.remove(original_terminal.terminal);
    let explored_paths = explore_paths(pipes, state, original_terminal.terminal);

    let original_goal = single(explored_paths.reached_goals.iter().copied())?;

    let mut original_path = Vec::new();
    explored_paths.add_path(original_goal, &mut original_path);

    for &location in &original_path[1..] {
        pipes.remove(location);
    }

    let mut min_terminal = original_terminal;
    let mut min_path = original_path;
    let mut min_path_turns = count_turns(&min_path);
    let mut changed_path = false;

    for (direction, translation) in TERMINAL_OFFSETS {
        let terminal_candidate = original_terminal.center.translate_by(translation);
        if !context.grid.is_empty(terminal_candidate)
            && !context
                .grid
                .is_entity_type(terminal_candidate, EntityKind::is_pipe)
        {
            continue;
        }

        let result = a_star_into(
            &context.grid,
            terminal_candidate,
            pipes,
            true,
            1,
            1,
            Vec::new(),
        );
        if result.success {
            let terminal =
                TerminalLocation::new(original_terminal.center, terminal_candidate, direction);
            let new_path = result.into_list();
            let path_turns = count_turns(&new_path);

            if new_path.len() < min_path.len()
                || (new_path.len() == min_path.len() && path_turns < min_path_turns)
            {
                min_terminal = terminal;
                min_path = new_path;
                min_path_turns = path_turns;
                changed_path = true;
            }
        }
    }

    pipes.union_with_items(min_path.iter().copied());

    if changed_path {
        // The C# checks `minTerminal != originalTerminal`, which compares references, and
        // `minTerminal` is always a new object once `changedPath` is set. So the branch always
        // runs, even when the new terminal is in the same place as the old one. Comparing the
        // values here would skip it in that case.
        context
            .center_to_terminals
            .value_mut(original_terminal.center)
            .push(min_terminal);

        match context.location_to_terminals.get_mut(min_terminal.terminal) {
            Some(location_terminals) => location_terminals.push(min_terminal),
            None => context
                .location_to_terminals
                .add(min_terminal.terminal, vec![min_terminal]),
        }

        // `minTerminal` is a new object, so this keeps it and drops the original even when
        // the two are equal in value; `eliminate_other_terminals` compares identities.
        helpers::eliminate_other_terminals(context, min_terminal);

        update_intersections_and_goals(context, pipes, state);

        Ok(true)
    } else {
        state.goals.add(original_terminal.terminal);
        Ok(false)
    }
}

/// `UseShortestPath`: try to replace the pipe run from `start` (an intersection) to
/// `original_goal` with a shorter one, or one as short with fewer turns.
fn use_shortest_path(
    context: &Context,
    pipes: &mut LocationSet,
    state: &mut State,
    explored_paths: &ExploredPaths,
    start: Location,
    original_goal: Location,
) -> FtResult<bool> {
    let mut original_path = Vec::new();
    explored_paths.add_path(original_goal, &mut original_path);

    for i in 1..original_path.len() {
        // Does the path contain an intersection as an intermediate point? This can happen if a
        // previous call of this method with the same exploration changed the intersections.
        if i < original_path.len() - 1 && state.intersections.contains(original_path[i]) {
            // Add the path back.
            for j in (1..i).rev() {
                pipes.add(original_path[j]);
            }

            return Ok(false);
        }

        pipes.remove(original_path[i]);
    }

    let mut connection_points = context.get_location_set_with_capacity(pipes.len(), true);
    explore_pipes(pipes, original_goal, &mut connection_points);

    let result = a_star(&context.grid, start, &connection_points, true, 1, 1);
    let path = result.path()?;
    if path.len() > original_path.len()
        || (path.len() == original_path.len() && count_turns(path) >= count_turns(&original_path))
    {
        pipes.union_with_items(original_path.iter().copied());
        return Ok(false);
    }

    pipes.union_with_items(path.iter().copied());
    update_intersections_and_goals(context, pipes, state);

    Ok(true)
}

/// `ExistingPipeGrid.GetNeighbors` over the live pipe set: east, north, west, south, each the
/// cell itself if it is a pipe and `Location.Invalid` otherwise.
fn pipe_neighbors(pipes: &LocationSet, location: Location) -> [Location; 4] {
    [
        location.translate(1, 0),
        location.translate(0, -1),
        location.translate(-1, 0),
        location.translate(0, 1),
    ]
    .map(|l| {
        if pipes.contains(l) {
            l
        } else {
            Location::INVALID
        }
    })
}

/// `ExplorePipes`: every pipe connected to `start`, added to `output` breadth first.
fn explore_pipes(pipes: &LocationSet, start: Location, output: &mut LocationSet) {
    let mut to_explore = VecDeque::new();
    to_explore.push_back(start);
    output.add(start);

    while let Some(current) = to_explore.pop_front() {
        for neighbor in pipe_neighbors(pipes, current) {
            if neighbor.is_valid() && output.add(neighbor) {
                to_explore.push_back(neighbor);
            }
        }
    }
}

/// `ExplorePaths`: walk the pipes breadth first from `start`, stopping at each goal reached.
fn explore_paths(pipes: &LocationSet, state: &State, start: Location) -> ExploredPaths {
    let mut to_explore = VecDeque::new();
    to_explore.push_back(start);
    let mut came_from = LocationDictionary::new();
    came_from.set(start, start);

    let mut reached_goals = Vec::new();

    while let Some(current) = to_explore.pop_front() {
        if current != start && state.goals.contains(current) {
            reached_goals.push(current);
            continue;
        }

        for neighbor in pipe_neighbors(pipes, current) {
            if !neighbor.is_valid() || came_from.contains_key(neighbor) {
                continue;
            }

            came_from.add(neighbor, current);
            to_explore.push_back(neighbor);
        }
    }

    ExploredPaths {
        came_from,
        reached_goals,
    }
}

/// `ChildContext.UpdateIntersectionsAndGoals`: an intersection is a pipe with more than two
/// pipe neighbors, or a terminal pipe with more than one. The goals are every terminal cell
/// plus every intersection.
fn update_intersections_and_goals(context: &Context, pipes: &LocationSet, state: &mut State) {
    state.intersections.clear();

    state.goals.clear();
    state
        .goals
        .union_with_items(context.location_to_terminals.keys());

    for pipe in pipes.iter() {
        let mut neighbors = 0;

        if pipes.contains(pipe.translate(1, 0)) {
            neighbors += 1;
        }

        if pipes.contains(pipe.translate(0, -1)) {
            neighbors += 1;
        }

        if pipes.contains(pipe.translate(-1, 0)) {
            neighbors += 1;
        }

        if pipes.contains(pipe.translate(0, 1)) {
            neighbors += 1;
        }

        if neighbors > 2 || context.location_to_terminals.contains_key(pipe) && neighbors > 1 {
            state.intersections.add(pipe);
            state.goals.add(pipe);
        }
    }
}
