//! `AddPipesFbe`, from `src/FactorioTools/OilField/Steps/AddPipes.1.FBE.cs`: the
//! `FbeOriginal` and `Fbe` pipe strategies, copied from Teoxoy's Factorio Blueprint Editor.
//!
//! The C# groups are objects compared by reference and shared between several lists
//! (`groups`, `aloneGroups`, the per-centroid buckets, `TwoConnectedGroups.FirstGroup`). Here
//! every group lives once in an arena and the lists hold its index, so index equality is the
//! C#'s reference equality.
//!
//! Ties decide the pipes, so the sorts that can tie use `net_sort` (.NET's introsort):
//! a pumpjack connection's terminal pairs (`AddPipes.1.FBE.cs:91`) and the leftover pumpjacks
//! by distance to the middle (`:307`, the default build's `MemoryExtensions.Sort` branch).
//! `MinBy` keeps the first of several equal keys, and the group centroid rounds half to even.

use std::cmp::Ordering;

use dotnet_compat::{net_sort, round_to_i32};

use crate::Location;
use crate::algorithms::{a_star, breadth_first_shortest_path};
use crate::collection_ext::{average, min_by, sum};
use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::TerminalLocation;
use crate::helpers::{
    count_turns, eliminate_other_terminals, make_straight_line_on_empty, points_to_lines,
    points_to_lines_sorted,
};
use crate::models::{Endpoints, PipeStrategy};

/// `AddPipesFbe.FbeResult`: the pipes and the strategy that produced them. `FbeOriginal`
/// falls back to `Fbe` when it cannot connect every pumpjack, and then reports `Fbe`.
#[derive(Debug)]
pub struct FbeResult {
    pub pipes: LocationSet,
    pub final_strategy: PipeStrategy,
}

/// `AddPipesFbe.Execute(context, strategy)`. On success, every pumpjack keeps only the
/// terminal the pipes reach (`EliminateOtherTerminals`). The C# returns some failures in a
/// `Result` (no path between two terminals) and throws others; both come back as `Err` here,
/// and in both cases the context is left as it was.
pub fn execute(context: &mut Context, strategy: PipeStrategy) -> FtResult<FbeResult> {
    // HACK: it appears FBE does not adjust the grid middle by the 2 cell buffer added to the
    // side of the grid. We'll apply this hack for now to reproduce FBE results.
    let middle = context.grid.middle().translate(-2, -2);

    let info = delaunay_triangulation(context, middle, strategy)?;

    for terminal in &info.terminals {
        eliminate_other_terminals(context, *terminal);
    }

    Ok(FbeResult {
        pipes: info.pipes,
        final_strategy: info.final_strategy,
    })
}

/// `FbeResultInfo`.
struct FbeResultInfo {
    terminals: Vec<TerminalLocation>,
    pipes: LocationSet,
    final_strategy: PipeStrategy,
}

/// `TerminalPair`.
#[derive(Clone)]
struct TerminalPair {
    terminal_a: TerminalLocation,
    terminal_b: TerminalLocation,
    line: Vec<Location>,
    center_distance: i32,
}

impl TerminalPair {
    fn new(
        terminal_a: TerminalLocation,
        terminal_b: TerminalLocation,
        line: Vec<Location>,
        middle: Location,
    ) -> Self {
        TerminalPair {
            terminal_a,
            terminal_b,
            line,
            center_distance: terminal_a.terminal.manhattan_distance(middle),
        }
    }
}

/// `PumpjackConnection`.
struct PumpjackConnection {
    endpoints: Endpoints,
    connections: Vec<TerminalPair>,
    endpoint_distance: i32,
}

impl PumpjackConnection {
    fn new(endpoints: Endpoints, connections: Vec<TerminalPair>, middle: Location) -> Self {
        PumpjackConnection {
            endpoints,
            connections,
            endpoint_distance: endpoints.a.manhattan_distance(middle)
                + endpoints.b.manhattan_distance(middle),
        }
    }

    /// `GetAverageDistance`.
    fn average_distance(&self) -> f64 {
        if self.connections.is_empty() {
            0.0
        } else {
            average(self.connections.iter().map(|x| x.line.len() as i32 - 1))
        }
    }
}

/// `Group`.
struct Group {
    /// `_terminals`: `context.GetLocationSet()`, a bit set.
    terminals: LocationSet,
    entities: Vec<TerminalLocation>,
    paths: Vec<Vec<Location>>,
    sum_x: f64,
    sum_y: f64,
    location: Location,
}

impl Group {
    fn empty(context: &Context, paths: Vec<Vec<Location>>) -> Self {
        Group {
            terminals: context.get_location_set(),
            entities: Vec::new(),
            paths,
            sum_x: 0.0,
            sum_y: 0.0,
            location: Location::INVALID,
        }
    }

    /// `Group(context, terminal, paths)`.
    fn from_terminal(
        context: &Context,
        terminal: TerminalLocation,
        paths: Vec<Vec<Location>>,
    ) -> Self {
        let mut group = Group::empty(context, paths);
        group.add(terminal);
        group.update_location();
        group
    }

    /// `Group(context, pair, paths)`.
    fn from_pair(context: &Context, pair: &TerminalPair, paths: Vec<Vec<Location>>) -> Self {
        let mut group = Group::empty(context, paths);
        group.add(pair.terminal_a);
        group.add(pair.terminal_b);
        group.update_location();
        group
    }

    /// `HasTerminal`.
    fn has_terminal(&self, location: &TerminalLocation) -> bool {
        self.terminals.contains(location.terminal)
    }

    /// `Add`.
    fn add(&mut self, entity: TerminalLocation) {
        self.entities.push(entity);
        self.terminals.add(entity.terminal);
        self.sum_x += f64::from(entity.center.x);
        self.sum_y += f64::from(entity.center.y);
        self.update_location();
    }

    /// `AddRange(group)`, given the other group's entities and terminal set.
    fn add_range(&mut self, entities: &[TerminalLocation], terminals: &LocationSet) {
        self.entities.extend_from_slice(entities);
        self.terminals.union_with(terminals);
        for entity in entities {
            self.sum_x += f64::from(entity.center.x);
            self.sum_y += f64::from(entity.center.y);
        }
        self.update_location();
    }

    /// `UpdateLocation`: the centroid of the pumpjack centers, `(int)Math.Round` half to even.
    fn update_location(&mut self) {
        let count = self.entities.len() as f64;
        self.location = Location::new(
            round_to_i32(self.sum_x / count),
            round_to_i32(self.sum_y / count),
        );
    }

    /// `Paths.Sum(p => p.Count)`.
    fn path_length_sum(&self) -> i32 {
        sum(self.paths.iter().map(|p| p.len() as i32))
    }
}

/// `TwoConnectedGroups`.
struct TwoConnectedGroups {
    lines: Vec<Vec<Location>>,
    min_distance: i32,
    first_group: usize,
}

/// `PathAndTurns`.
struct PathAndTurns {
    path: Vec<Location>,
    original_index: usize,
}

/// The C#'s Delaunator throws a plain `Exception`; the planner has no `catch`, so it ends the
/// plan the way a `FactorioToolsException` does.
fn lines_from(
    result: Result<Vec<Endpoints>, delaunator_net::DelaunatorError>,
) -> FtResult<Vec<Endpoints>> {
    result.map_err(|e| FactorioToolsError::new(e.to_string()))
}

/// `DelaunayTriangulation`. Reads the context and never changes it.
fn delaunay_triangulation(
    context: &Context,
    middle: Location,
    strategy: PipeStrategy,
) -> FtResult<FbeResultInfo> {
    // GENERATE LINES
    let mut lines: Vec<PumpjackConnection> = Vec::new();
    let all_lines = lines_from(points_to_lines_sorted(&context.centers, false))?;
    for line in &all_lines {
        let mut connections = Vec::new();

        for &t_a in context.center_to_terminals.value(line.a) {
            for &t_b in context.center_to_terminals.value(line.b) {
                if t_a.terminal.x != t_b.terminal.x && t_a.terminal.y != t_b.terminal.y {
                    continue;
                }

                let Some(straight_line) =
                    make_straight_line_on_empty(&context.grid, t_a.terminal, t_b.terminal)
                else {
                    continue;
                };

                connections.push(TerminalPair::new(t_a, t_b, straight_line, middle));
            }
        }

        if connections.is_empty() {
            continue;
        }

        lines.push(PumpjackConnection::new(
            Endpoints::new(line.a, line.b),
            connections,
            middle,
        ));
    }

    // GENERATE GROUPS
    let mut arena: Vec<Group> = Vec::new();
    let mut groups: Vec<usize> = Vec::new();
    let mut added_pumpjacks: Vec<TerminalLocation> = Vec::new();
    let mut leftover_pumpjacks =
        context.get_location_set_from(context.centers.iter().copied(), true);
    while !lines.is_empty() {
        let mut line = get_next_line(&mut lines, &added_pumpjacks);

        let added_a = added_pumpjacks
            .iter()
            .find(|x| x.center == line.endpoints.a)
            .copied();
        let added_b = added_pumpjacks
            .iter()
            .find(|x| x.center == line.endpoints.b)
            .copied();

        net_sort(&mut line.connections, |a, b| {
            a.center_distance
                .cmp(&b.center_distance)
                .then(a.line.len().cmp(&b.line.len()))
        });

        for connection in &line.connections {
            if added_a.is_none() && added_b.is_none() {
                arena.push(Group::from_pair(
                    context,
                    connection,
                    vec![connection.line.clone()],
                ));
                groups.push(arena.len() - 1);
                added_pumpjacks.push(connection.terminal_a);
                added_pumpjacks.push(connection.terminal_b);
                leftover_pumpjacks.remove(connection.terminal_a.center);
                leftover_pumpjacks.remove(connection.terminal_b.center);
                break;
            }

            if let (None, Some(added_b)) = (added_a, added_b)
                && added_b.direction == connection.terminal_b.direction
            {
                let group = first_group_with_terminal(&arena, &groups, &added_b)?;
                arena[group].add(connection.terminal_a);
                arena[group].paths.push(connection.line.clone());
                added_pumpjacks.push(connection.terminal_a);
                leftover_pumpjacks.remove(connection.terminal_a.center);
                break;
            }

            if let (Some(added_a), None) = (added_a, added_b)
                && added_a.direction == connection.terminal_a.direction
            {
                let group = first_group_with_terminal(&arena, &groups, &added_a)?;
                arena[group].add(connection.terminal_b);
                arena[group].paths.push(connection.line.clone());
                added_pumpjacks.push(connection.terminal_b);
                leftover_pumpjacks.remove(connection.terminal_b.center);
                break;
            }
        }
    }

    // if no LINES were generated, add 2 pumpjacks to a group here
    // this will only happen when only a few pumpjacks need to be connected
    if groups.is_empty() {
        let mut connection: Option<TerminalPair> = None;
        for line in &all_lines {
            let terminals_a = context.center_to_terminals.value(line.a);
            for &t_a in terminals_a {
                let terminals_b = context.center_to_terminals.value(line.b);
                for &t_b in terminals_b {
                    let goals = context.get_single_location_set(t_b.terminal);

                    if let Some(best) = &connection {
                        // don't perform a shortest path search if the Manhattan distance
                        // (minimum possible) is longer than the best.
                        let min_distance = t_a.terminal.manhattan_distance(t_b.terminal);
                        if min_distance >= best.line.len() as i32 {
                            continue;
                        }
                    }

                    let result = a_star(&context.grid, t_a.terminal, &goals, true, 1, 1);

                    if !result.success {
                        return Err(FactorioToolsError::new("A goal should have been reached."));
                    }

                    let path = result.into_list();
                    match &connection {
                        None => connection = Some(TerminalPair::new(t_a, t_b, path, middle)),
                        Some(best) => {
                            if path.len() < best.line.len() {
                                connection = Some(TerminalPair::new(t_a, t_b, path, middle));
                            }
                        }
                    }
                }
            }
        }

        let Some(connection) = connection else {
            return Err(FactorioToolsError::new(
                "A connection between terminals should have been found.",
            ));
        };

        arena.push(Group::from_pair(
            context,
            &connection,
            vec![connection.line.clone()],
        ));
        groups.push(arena.len() - 1);
    }

    // CONNECT GROUPS
    let max_tries = if strategy == PipeStrategy::FbeOriginal {
        3
    } else {
        20
    };
    let mut tries = max_tries;
    let mut alone_groups: Vec<usize> = Vec::new();
    let mut final_group: Option<usize> = None;

    while !groups.is_empty() {
        let group = min_by(groups.iter().copied(), |&g| arena[g].path_length_sum())
            .expect("groups is not empty");
        // `groups.Remove(group)`: the first occurrence, by reference.
        let index = groups
            .iter()
            .position(|&g| g == group)
            .expect("the group came from the list");
        groups.remove(index);

        if groups.is_empty() {
            if !alone_groups.is_empty() && tries > 0 {
                groups.append(&mut alone_groups);
                groups.push(group);
                tries -= 1;
                continue;
            }

            final_group = Some(group);
            break;
        }

        // A group's Location is the rounded centroid of its pumpjack centers, so two
        // different groups can land on the same tile. Keying a dictionary by it therefore has
        // to allow more than one group per key.
        let mut location_to_groups: LocationDictionary<Vec<usize>> =
            context.get_location_dictionary_with_capacity(groups.len() + 1);
        for &g in &groups {
            index_group(&mut location_to_groups, &arena, g);
        }

        index_group(&mut location_to_groups, &arena, group);

        let keys: Vec<Location> = location_to_groups.keys().collect();
        let group_lines = lines_from(points_to_lines(&keys))?;
        let group_location = arena[group].location;
        let mut par: Vec<usize> = Vec::with_capacity(group_lines.len());
        for line in &group_lines {
            if line.a == group_location {
                add_partners(&mut par, location_to_groups.value(line.b), group);
            } else if line.b == group_location {
                add_partners(&mut par, location_to_groups.value(line.a), group);
            }
        }

        // The triangulation sees one point for every group sharing this group's own
        // centroid, so those groups can never turn up as the far end of a line. They are also
        // the closest partners there are, so add them directly.
        add_partners(&mut par, location_to_groups.value(group_location), group);

        let connection = get_path_between_groups(
            context,
            &arena,
            &par,
            group,
            2 + max_tries - tries,
            strategy,
        )?;

        match connection {
            Some(connection) => {
                let first = connection.first_group;
                let entities = arena[group].entities.clone();
                let terminals = arena[group].terminals.copy();
                let paths = arena[group].paths.clone();
                arena[first].add_range(&entities, &terminals);
                arena[first].paths.extend(paths);
                arena[first].paths.push(
                    connection
                        .lines
                        .into_iter()
                        .next()
                        .expect("lines is not empty"),
                );
            }
            None => alone_groups.push(group),
        }
    }

    let Some(final_group) = final_group else {
        return Err(FactorioToolsError::new(
            "The final group should be initialized at this point.",
        ));
    };

    if !alone_groups.is_empty() {
        // Fallback to the modified FBE algorithm if the original cannot connect all of the
        // groups. Related to https://github.com/teoxoy/factorio-blueprint-editor/issues/254
        if strategy == PipeStrategy::FbeOriginal {
            return delaunay_triangulation(context, middle, PipeStrategy::Fbe);
        }

        return Err(FactorioToolsError::new(
            "There should be no more alone groups at this point.",
        ));
    }

    let mut sorted_leftover_pumpjacks = leftover_pumpjacks.to_vec();
    net_sort(&mut sorted_leftover_pumpjacks, |a, b| {
        let a_c = a.manhattan_distance(middle);
        let b_c = b.manhattan_distance(middle);
        a_c.cmp(&b_c)
    });

    for center in sorted_leftover_pumpjacks {
        let center_terminals = context.center_to_terminals.value(center);
        let mut terminal_groups = Vec::with_capacity(center_terminals.len());
        for &terminal in center_terminals {
            arena.push(Group::from_terminal(
                context,
                terminal,
                vec![vec![terminal.terminal]],
            ));
            terminal_groups.push(arena.len() - 1);
        }

        let mut max_turns = 2;
        loop {
            let connection = get_path_between_groups(
                context,
                &arena,
                &terminal_groups,
                final_group,
                max_turns,
                strategy,
            )?;

            let Some(connection) = connection else {
                // Allow more max turns with the modified FBE algorithm.
                // Related to https://github.com/teoxoy/factorio-blueprint-editor/issues/253
                if strategy == PipeStrategy::FbeOriginal {
                    return delaunay_triangulation(context, middle, PipeStrategy::Fbe);
                } else if max_turns > max_tries {
                    return Err(FactorioToolsError::new(format!(
                        "There should be at least one connection between a leftover pumpjack and the final group. Max turns: {max_turns}"
                    )));
                }

                max_turns += 1;
                continue;
            };

            // `connection.FirstGroup.Entities.Single()`.
            let first = &arena[connection.first_group].entities;
            let entity = match first.len() {
                1 => first[0],
                0 => {
                    return Err(FactorioToolsError::new(
                        "An item should exist in the source.",
                    ));
                }
                _ => {
                    return Err(FactorioToolsError::new(
                        "Only one item should exist in the source.",
                    ));
                }
            };
            arena[final_group].add(entity);
            arena[final_group].paths.push(
                connection
                    .lines
                    .into_iter()
                    .next()
                    .expect("lines is not empty"),
            );
            break;
        }
    }

    let terminals = arena[final_group].entities.clone();
    let mut pipes = context.get_location_set_enumerable(true);
    for path in &arena[final_group].paths {
        for &location in path {
            pipes.add(location);
        }
    }

    Ok(FbeResultInfo {
        terminals,
        pipes,
        final_strategy: strategy,
    })
}

/// `groups.First(g => g.HasTerminal(terminal))`.
fn first_group_with_terminal(
    arena: &[Group],
    groups: &[usize],
    terminal: &TerminalLocation,
) -> FtResult<usize> {
    groups
        .iter()
        .copied()
        .find(|&g| arena[g].has_terminal(terminal))
        .ok_or_else(|| FactorioToolsError::new("An item should have matched the predicate."))
}

/// `GetNextLine`: picks a line, removes it from `lines` and returns it. Prefers lines touching
/// an added pumpjack, then the larger endpoint distance, then fewer connections, then the
/// smaller average connection length; the earliest line wins a full tie.
fn get_next_line(
    lines: &mut Vec<PumpjackConnection>,
    added_pumpjacks: &[TerminalLocation],
) -> PumpjackConnection {
    let mut next: Option<usize> = None;
    let mut next_index = 0;
    let mut next_contains_added_pumpjack: Option<bool> = None;
    let mut next_average_distance: Option<f64> = None;

    for i in 0..lines.len() {
        let Some(n) = next else {
            next = Some(i);
            continue;
        };

        let next_contains = *next_contains_added_pumpjack
            .get_or_insert_with(|| line_contains_an_added_pumpjack(added_pumpjacks, &lines[n]));

        let line = &lines[i];
        let contains_added_pumpjack = line_contains_an_added_pumpjack(added_pumpjacks, line);
        match contains_added_pumpjack.cmp(&next_contains) {
            Ordering::Greater => {
                next = Some(i);
                next_index = i;
                next_contains_added_pumpjack = Some(contains_added_pumpjack);
                next_average_distance = None;
                continue;
            }
            Ordering::Less => continue,
            Ordering::Equal => {}
        }

        match line.endpoint_distance.cmp(&lines[n].endpoint_distance) {
            Ordering::Greater => {
                next = Some(i);
                next_index = i;
                next_contains_added_pumpjack = Some(contains_added_pumpjack);
                next_average_distance = None;
                continue;
            }
            Ordering::Less => continue,
            Ordering::Equal => {}
        }

        match line.connections.len().cmp(&lines[n].connections.len()) {
            Ordering::Less => {
                next = Some(i);
                next_index = i;
                next_contains_added_pumpjack = Some(contains_added_pumpjack);
                next_average_distance = None;
                continue;
            }
            Ordering::Greater => continue,
            Ordering::Equal => {}
        }

        let next_average =
            *next_average_distance.get_or_insert_with(|| lines[n].average_distance());

        let average_distance = line.average_distance();
        if average_distance < next_average {
            next = Some(i);
            next_index = i;
            next_contains_added_pumpjack = Some(contains_added_pumpjack);
            next_average_distance = Some(average_distance);
        }
    }

    lines.remove(next_index)
}

/// `LineContainsAnAddedPumpjack`.
fn line_contains_an_added_pumpjack(
    added_pumpjacks: &[TerminalLocation],
    ent: &PumpjackConnection,
) -> bool {
    for terminal in added_pumpjacks {
        if ent.endpoints.a != terminal.center && ent.endpoints.b != terminal.center {
            continue;
        }

        for pair in &ent.connections {
            if pair.terminal_a.direction == terminal.direction
                || pair.terminal_b.direction == terminal.direction
            {
                return true;
            }
        }
    }

    false
}

/// `GetPathBetweenGroups`: the connection from one of `groups` to `group` with the shortest
/// path, the earliest winning a tie, or `None` if no group connects.
fn get_path_between_groups(
    context: &Context,
    arena: &[Group],
    groups: &[usize],
    group: usize,
    max_turns: i32,
    strategy: PipeStrategy,
) -> FtResult<Option<TwoConnectedGroups>> {
    let mut best: Option<TwoConnectedGroups> = None;
    for &g in groups {
        let connection = connect_two_groups(context, arena, g, group, max_turns, strategy)?;

        if connection.lines.is_empty() {
            continue;
        }

        match &best {
            None => best = Some(connection),
            Some(b) => {
                if connection.min_distance < b.min_distance {
                    best = Some(connection);
                }
            }
        }
    }

    Ok(best)
}

/// `ConnectTwoGroups`.
fn connect_two_groups(
    context: &Context,
    arena: &[Group],
    a: usize,
    b: usize,
    max_turns: i32,
    strategy: PipeStrategy,
) -> FtResult<TwoConnectedGroups> {
    let a_locations: Vec<Location> = arena[a].paths.iter().flatten().copied().collect();
    let mut b_locations: Vec<Location> = arena[b].paths.iter().flatten().copied().collect();

    let mut line_info: Vec<PathAndTurns> = Vec::new();
    for &al in &a_locations {
        for &bl in &b_locations {
            if al.x != bl.x && al.y != bl.y {
                continue;
            }

            let Some(line) = make_straight_line_on_empty(&context.grid, al, bl) else {
                continue;
            };

            let original_index = line_info.len();
            line_info.push(PathAndTurns {
                path: line,
                original_index,
            });
        }
    }

    if a_locations.len() == 1 {
        let mut location_to_index: LocationDictionary<usize> =
            context.get_location_dictionary_with_capacity(b_locations.len());
        for (i, &location) in b_locations.iter().enumerate() {
            location_to_index.try_add(location, i);
        }

        let a0 = a_locations[0];
        net_sort(&mut b_locations, |x, y| {
            let x_c = a0.manhattan_distance(*x);
            let y_c = a0.manhattan_distance(*y);
            x_c.cmp(&y_c)
                .then_with(|| location_to_index.value(*x).cmp(location_to_index.value(*y)))
        });

        b_locations.truncate(20);
    }

    let mut a_plus_b =
        context.get_location_set_with_capacity(a_locations.len() + b_locations.len(), true);
    a_plus_b.union_with_items(a_locations.iter().copied());
    a_plus_b.union_with_items(b_locations.iter().copied());

    let all_endpoints = lines_from(points_to_lines(&a_plus_b.to_vec()))?;
    let mut matches: Vec<(Endpoints, i32, usize)> = Vec::with_capacity(all_endpoints.len());
    for pair in all_endpoints {
        if (a_locations.contains(&pair.a) && a_locations.contains(&pair.b))
            || (b_locations.contains(&pair.a) && b_locations.contains(&pair.b))
        {
            continue;
        }

        let index = matches.len();
        matches.push((pair, pair.a.manhattan_distance(pair.b), index));
    }

    // Ends on the unique index, so the order is total and any sort gives the same list.
    matches.sort_by(|x, y| x.1.cmp(&y.1).then(x.2.cmp(&y.2)));

    let take_lines = matches.len().min(5);
    for &(l, _, _) in matches.iter().take(take_lines) {
        let path = if strategy == PipeStrategy::FbeOriginal {
            // We can't terminal early based on max turns because this leads to different
            // results since it allows secondary path options that would have otherwise been
            // not considered for a given start and goal state.
            match breadth_first_shortest_path(&context.grid, l.b, l.a) {
                Some(path) => path,
                None => return Err(FactorioToolsError::no_path_between_terminals(l.a, l.b)),
            }
        } else {
            let goals = context.get_single_location_set(l.b);
            let result = a_star(&context.grid, l.a, &goals, true, 1, 1);
            if !result.success {
                return Err(FactorioToolsError::no_path_between_terminals(l.a, l.b));
            }

            result.into_list()
        };

        let turns = count_turns(&path);
        if turns == 0 || turns > max_turns {
            continue;
        }

        let original_index = line_info.len();
        line_info.push(PathAndTurns {
            path,
            original_index,
        });
    }

    // Ends on the unique original index, so the order is total.
    line_info.sort_by(|x, y| {
        x.path
            .len()
            .cmp(&y.path.len())
            .then(x.original_index.cmp(&y.original_index))
    });

    let min_distance = line_info
        .iter()
        .map(|x| x.path.len() as i32)
        .min()
        .unwrap_or(0);
    let lines = line_info.into_iter().map(|x| x.path).collect();

    Ok(TwoConnectedGroups {
        lines,
        min_distance,
        first_group: a,
    })
}

/// `IndexGroup`: files a group under its centroid, creating the bucket on first use.
fn index_group(
    location_to_groups: &mut LocationDictionary<Vec<usize>>,
    arena: &[Group],
    group: usize,
) {
    let location = arena[group].location;
    match location_to_groups.get_mut(location) {
        Some(list) => list.push(group),
        None => location_to_groups.add(location, vec![group]),
    }
}

/// `AddPartners`: appends every candidate that is not `self_group` and not already present.
fn add_partners(partners: &mut Vec<usize>, candidates: &[usize], self_group: usize) {
    for &candidate in candidates {
        if candidate == self_group {
            continue;
        }

        if !partners.contains(&candidate) {
            partners.push(candidate);
        }
    }
}
