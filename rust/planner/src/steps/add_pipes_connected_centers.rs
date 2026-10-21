//! `AddPipesConnectedCenters`, from `OilField/Steps/AddPipes.2.ConnectedCenters.cs`: the
//! "connected centers" pipe planner.
//!
//! `GetConnectedPumpjacks` decides which pumpjacks should be joined (a Delaunay graph, its
//! minimum spanning tree, or a FLUTE tree). `FindTrunksAndConnect` then lays straight
//! "trunks" through rows of pumpjacks, groups them, and joins the groups with A* until one
//! group holds every pumpjack.
//!
//! **Errors.** The C# fails two ways here: some failures come back inside a `Result`
//! (`AddPipes.0.cs` inspects those and may retry), and others are thrown and end the plan.
//! [`find_trunks_and_connect`] keeps the two apart: the outer `FtResult` is a thrown
//! exception, the inner one a `Result` exception.
//!
//! **Shared state.** In C#, every `PumpjackGroup` holds the same `allIncludedCenters` set and
//! the same connected-centers graph. Here they are passed to the group's methods instead.

use std::cmp::Ordering;
use std::collections::VecDeque;

use dotnet_compat::{NetCompare, net_sort};

use crate::Location;
use crate::algorithms::a_star;
use crate::collection_ext::{average, first_where, max, single_where};
use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::TerminalLocation;
use crate::helpers::{are_locations_collinear, eliminate_other_terminals, make_straight_line};
use crate::models::PipeStrategy;
use crate::steps::{add_pipes_connected_centers_dt, add_pipes_connected_centers_flute};

/// `Translations`: trunks run right (+X) or down (+Y).
const TRANSLATIONS: [Location; 2] = [Location::new(1, 0), Location::new(0, 1)];

/// `GetConnectedPumpjacks`: which pumpjacks each pumpjack should be connected to.
///
/// The only error is the C#'s thrown "not bidirectional" check, which runs when
/// `ValidateSolution` is on.
pub fn get_connected_pumpjacks(
    context: &Context,
    strategy: PipeStrategy,
) -> FtResult<LocationDictionary<LocationSet>> {
    let centers = &context.centers;

    if centers.len() == 2 {
        let mut simple_connected_centers = context.get_location_dictionary();
        simple_connected_centers.add(centers[0], context.get_single_location_set(centers[1]));
        simple_connected_centers.add(centers[1], context.get_single_location_set(centers[0]));
        return Ok(simple_connected_centers);
    }

    // Check that nodes are not collinear.
    if are_locations_collinear(centers) {
        let mut connected = context.get_location_dictionary_with_capacity(centers.len());
        for &center in centers {
            connected.add(center, context.get_location_set_enumerable(true));
        }

        for j in 1..centers.len() {
            connected.value_mut(centers[j - 1]).add(centers[j]);
            connected.value_mut(centers[j]).add(centers[j - 1]);
        }

        return Ok(connected);
    }

    let connected_centers = match strategy {
        PipeStrategy::ConnectedCentersDelaunay => {
            add_pipes_connected_centers_dt::execute_with_delaunay(context, centers)
        }
        PipeStrategy::ConnectedCentersDelaunayMst => {
            add_pipes_connected_centers_dt::execute_with_delaunay_mst(context, centers)
        }
        PipeStrategy::ConnectedCentersFlute => add_pipes_connected_centers_flute::execute(context),
        other => panic!("NotImplementedException: {other:?} is not a connected-centers strategy"),
    };

    // Check if all connected centers have edges in both directions.
    if context.options.validate_solution {
        for (center, others) in connected_centers.iter() {
            for other in others.iter() {
                if !connected_centers.value(other).contains(center) {
                    return Err(FactorioToolsError::new(
                        "The edges in the connected centers graph are not bidirectional.",
                    ));
                }
            }
        }
    }

    Ok(connected_centers)
}

/// `GroupCandidate`. `group` is an index into the list of groups.
struct GroupCandidate {
    group: usize,
    center: Location,
    included_center: Location,
    terminal: TerminalLocation,
    path: Vec<Location>,
}

/// `FindTrunksAndConnect`: the pipes joining every pumpjack.
///
/// The outer `Err` is an exception the C# throws; the inner `Err` one it returns in a
/// `Result` (a `NoPathBetweenTerminalsException` from joining a group).
pub fn find_trunks_and_connect(
    context: &mut Context,
    center_to_connected_centers: &LocationDictionary<LocationSet>,
) -> FtResult<FtResult<LocationSet>> {
    let selected_trunks = find_trunks(context, center_to_connected_centers);

    // `context.GetLocationSet(selectedTrunks.Count * 2)`: a capacity without `allowEnumerate`
    // gives a bit set.
    let mut all_included_centers =
        context.get_location_set_with_capacity(selected_trunks.len() * 2, false);
    for trunk in &selected_trunks {
        for center in trunk.centers.iter() {
            all_included_centers.add(center);
        }
    }

    let mut groups: Vec<PumpjackGroup> = Vec::with_capacity(selected_trunks.len());
    for trunk in &selected_trunks {
        groups.push(PumpjackGroup::new(
            context,
            center_to_connected_centers,
            &all_included_centers,
            trunk.centers.iter(),
            &make_straight_line(trunk.start(), trunk.end()),
        ));
    }

    if groups.is_empty() {
        let group = connect_two_closest_pumpjacks(
            context,
            center_to_connected_centers,
            &all_included_centers,
        )?;

        groups.push(group);
    }

    while groups.len() > 1 || groups[0].included_centers.len() < context.center_to_terminals.len() {
        let mut shortest_distance: Option<f64> = None;
        let mut candidate: Option<GroupCandidate> = None;

        for (group_index, group) in groups.iter().enumerate() {
            let centroid_x = average(group.pipes.iter().map(|l| l.x));
            let centroid_y = average(group.pipes.iter().map(|l| l.y));

            for center in group.frontier_centers.iter() {
                let included_center = first_where(
                    group.included_center_to_child_centers.iter(),
                    |(_, children)| children.contains(center),
                )?
                .0;

                // Prefer the terminal that has the shortest path, then prefer the terminal
                // closer to the centroid of the child (unconnected) pumpjacks.
                for &terminal in context.center_to_terminals.value(center) {
                    let path = match get_shortest_path_to_group(
                        context, terminal, group, centroid_x, centroid_y,
                    )? {
                        Ok(path) => path,
                        Err(error) => return Ok(Err(error)),
                    };

                    let Some(current) = &candidate else {
                        candidate = Some(GroupCandidate {
                            group: group_index,
                            center,
                            included_center,
                            terminal,
                            path,
                        });
                        continue;
                    };

                    let comparison = path.len().cmp(&current.path.len());
                    if comparison == Ordering::Less {
                        candidate = Some(GroupCandidate {
                            group: group_index,
                            center,
                            included_center,
                            terminal,
                            path,
                        });
                        shortest_distance = None;
                        continue;
                    }

                    if comparison == Ordering::Greater {
                        continue;
                    }

                    let best = match shortest_distance {
                        Some(best) => best,
                        None => {
                            let best = groups[current.group].get_child_centroid_distance_squared(
                                current.included_center,
                                current.terminal.terminal,
                            );
                            shortest_distance = Some(best);
                            best
                        }
                    };

                    let distance = group
                        .get_child_centroid_distance_squared(included_center, terminal.terminal);
                    if distance.net_cmp(&best) == Ordering::Less {
                        candidate = Some(GroupCandidate {
                            group: group_index,
                            center,
                            included_center,
                            terminal,
                            path,
                        });
                        shortest_distance = Some(distance);
                    }
                }
            }
        }

        let Some(candidate) = candidate else {
            return Err(FactorioToolsError::new("No group candidate was found."));
        };

        if all_included_centers.contains(candidate.terminal.center) {
            // `groups.Single(g => g.IncludedCenters.Contains(...))`, then `MergeGroup` and
            // `groups.Remove(otherGroup)`. The merge does not depend on the list, so taking
            // the other group out first changes nothing.
            let other_index = single_where(0..groups.len(), |&i| {
                groups[i]
                    .included_centers
                    .contains(candidate.terminal.center)
            })?;
            let other = groups.remove(other_index);
            let group_index = if other_index < candidate.group {
                candidate.group - 1
            } else {
                candidate.group
            };
            groups[group_index].merge_group(
                context,
                center_to_connected_centers,
                &all_included_centers,
                other,
                &candidate.path,
            );
        } else {
            // Add the newly connected pumpjack to the current group.
            groups[candidate.group].connect_pumpjack(
                context,
                center_to_connected_centers,
                &mut all_included_centers,
                candidate.center,
                &candidate.path,
            );
            eliminate_other_terminals(context, candidate.terminal);
        }
    }

    let group = groups.pop().expect("the loop above ends with one group");
    Ok(Ok(group.pipes))
}

/// `GetShortestPathToGroup`: the shorter of two A* paths from `terminal` to the group's
/// pipes, one that prefers vertical moves (`xWeight: 2`) and one that prefers horizontal
/// ones (`yWeight: 2`). Ties go to the path touching more cells beside pumpjacks, then to the
/// one closer to the group's centroid, then to the horizontal one.
///
/// The outer `Err` is thrown in the C#, the inner one returned in a `Result`.
fn get_shortest_path_to_group(
    context: &Context,
    terminal: TerminalLocation,
    group: &PumpjackGroup,
    group_centroid_x: f64,
    group_centroid_y: f64,
) -> FtResult<FtResult<Vec<Location>>> {
    let a_star_result_v = a_star(&context.grid, terminal.terminal, &group.pipes, true, 2, 1);
    let a_star_result_h = a_star(&context.grid, terminal.terminal, &group.pipes, true, 1, 2);

    if !a_star_result_v.success {
        let first = crate::collection_ext::first(group.pipes.iter())?;
        return Ok(Err(FactorioToolsError::no_path_between_terminals(
            terminal.terminal,
            first,
        )));
    }

    let path_v = a_star_result_v.path()?;
    let path_h = a_star_result_h.path()?;

    if path_v == path_h {
        return Ok(Ok(path_v.to_vec()));
    }

    let mut adjacent_pipes_v = 0;
    let mut centroid_distance_squared_v = 0.0f64;

    let mut adjacent_pipes_h = 0;
    let mut centroid_distance_squared_h = 0.0f64;

    // The C# caches each location's distance in a dictionary; the value is the same either
    // way, so this computes it directly. The sums keep the C#'s order: V then H, step by step.
    let width = context.grid.width();
    for i in 0..path_v.len().max(path_h.len()) {
        if let Some(&location) = path_v.get(i) {
            if context.location_to_adjacent_count[(location.y * width + location.x) as usize] > 0 {
                adjacent_pipes_v += 1;
            }

            centroid_distance_squared_v +=
                location.euclidean_distance_squared_to(group_centroid_x, group_centroid_y);
        }

        if let Some(&location) = path_h.get(i) {
            if context.location_to_adjacent_count[(location.y * width + location.x) as usize] > 0 {
                adjacent_pipes_h += 1;
            }

            centroid_distance_squared_h +=
                location.euclidean_distance_squared_to(group_centroid_x, group_centroid_y);
        }
    }

    let path = if adjacent_pipes_v > adjacent_pipes_h {
        path_v
    } else if adjacent_pipes_v < adjacent_pipes_h {
        path_h
    } else if centroid_distance_squared_v < centroid_distance_squared_h {
        path_v
    } else {
        path_h
    };

    Ok(Ok(path.to_vec()))
}

/// `FindTrunks`: the trunk candidates, best first, keeping each one that shares no pipe and
/// no pumpjack with a better one. Each kept trunk selects its pumpjacks' terminals.
fn find_trunks(
    context: &mut Context,
    center_to_connected_centers: &LocationDictionary<LocationSet>,
) -> Vec<Trunk> {
    let mut trunk_candidates = get_trunk_candidates(context, center_to_connected_centers);

    // The C# computes each trunk's end distance lazily inside the comparison. It depends only
    // on the trunk and the graph, so computing it up front gives the same values.
    for trunk in &mut trunk_candidates {
        trunk.trunk_end_distance =
            trunk.get_trunk_end_distance(context, center_to_connected_centers);
    }

    // Ends on the unique `OriginalIndex`, so the order is total; .NET's sort is used anyway.
    net_sort(&mut trunk_candidates, |a, b| {
        let c = b.terminal_locations.len().cmp(&a.terminal_locations.len());
        if c != Ordering::Equal {
            return c;
        }

        let c = a.length().cmp(&b.length());
        if c != Ordering::Equal {
            return c;
        }

        let c = a.trunk_end_distance.net_cmp(&b.trunk_end_distance);
        if c != Ordering::Equal {
            return c;
        }

        a.original_index.cmp(&b.original_index)
    });

    // Eliminate lower priority trunks that have any pipes shared with higher priority trunks.
    let mut included_pipes = context.get_location_set();
    let mut included_centers = context.get_location_set_enumerable(true);
    let mut selected_trunks = Vec::new();
    for trunk in trunk_candidates {
        let path = make_straight_line(trunk.start(), trunk.end());
        if !included_pipes.overlaps(path.iter().copied())
            && !included_centers.overlaps(trunk.centers.iter())
        {
            included_pipes.union_with_items(path.iter().copied());
            included_centers.union_with(&trunk.centers);
            selected_trunks.push(trunk);
        }
    }

    // Eliminate unused terminals for pumpjacks included in all of the trunks. A pumpjack
    // connected to a trunk has its terminal selected.
    for trunk in &selected_trunks {
        for &terminal in &trunk.terminals {
            eliminate_other_terminals(context, terminal);
        }
    }

    selected_trunks
}

/// `BestConnection`.
struct BestConnection {
    path: Vec<Location>,
    terminal: TerminalLocation,
    best_terminal: TerminalLocation,
}

/// `ConnectTwoClosestPumpjacks`: when no trunk was found, the shortest A* path between two
/// connected pumpjacks starts the only group. Every failure here is thrown in the C#.
fn connect_two_closest_pumpjacks(
    context: &mut Context,
    center_to_connected_centers: &LocationDictionary<LocationSet>,
    all_included_centers: &LocationSet,
) -> FtResult<PumpjackGroup> {
    let mut center_to_goals: LocationDictionary<LocationSet> = context.get_location_dictionary();
    let mut best_connection: Option<BestConnection> = None;
    let mut best_connected_centers_count: usize = 0;
    let mut best_other_connected_centers_count: Option<usize> = None;
    let mut best_terminal_distance: Option<f64> = None;
    let mut best_other_terminal_distance: Option<f64> = None;

    for i in 0..context.centers.len() {
        let center = context.centers[i];
        let terminals = context.center_to_terminals.value(center).clone();

        for &terminal in &terminals {
            let connected_centers = center_to_connected_centers.value(center);

            for other_center in connected_centers.iter() {
                let other_terminals = context.center_to_terminals.value(other_center);

                if !center_to_goals.contains_key(other_center) {
                    let mut goals = context.get_location_set_enumerable(true);
                    for other_terminal in other_terminals {
                        goals.add(other_terminal.terminal);
                    }

                    center_to_goals.add(other_center, goals);
                }

                let goals = center_to_goals.value(other_center);

                let result = a_star(&context.grid, terminal.terminal, goals, true, 1, 1);
                if !result.success {
                    let first = crate::collection_ext::first(goals.iter())?;
                    return Err(FactorioToolsError::no_path_between_terminals(
                        terminal.terminal,
                        first,
                    ));
                }

                let reached_goal = result.reached_goal;
                let closest_terminal =
                    *single_where(other_terminals.iter(), |t| t.terminal == reached_goal)?;
                let path = result.into_list();

                let replace = |path: Vec<Location>| BestConnection {
                    path,
                    terminal,
                    best_terminal: closest_terminal,
                };

                let Some(best) = &best_connection else {
                    best_connection = Some(replace(path));
                    best_connected_centers_count = connected_centers.len();
                    best_other_connected_centers_count = None;
                    best_terminal_distance = None;
                    best_other_terminal_distance = None;
                    continue;
                };

                let c = path.len().cmp(&best.path.len());
                if c == Ordering::Less {
                    best_connection = Some(replace(path));
                    best_connected_centers_count = connected_centers.len();
                    best_other_connected_centers_count = None;
                    best_terminal_distance = None;
                    best_other_terminal_distance = None;
                    continue;
                } else if c == Ordering::Greater {
                    continue;
                }

                let c = connected_centers.len().cmp(&best_connected_centers_count);
                if c == Ordering::Greater {
                    best_connection = Some(replace(path));
                    best_connected_centers_count = connected_centers.len();
                    best_other_connected_centers_count = None;
                    best_terminal_distance = None;
                    best_other_terminal_distance = None;
                    continue;
                } else if c == Ordering::Less {
                    continue;
                }

                let best_other_count =
                    *best_other_connected_centers_count.get_or_insert_with(|| {
                        center_to_connected_centers
                            .value(best.best_terminal.center)
                            .len()
                    });

                let other_connected_centers_count =
                    center_to_connected_centers.value(other_center).len();
                let c = other_connected_centers_count.cmp(&best_other_count);
                if c == Ordering::Greater {
                    best_connection = Some(replace(path));
                    best_connected_centers_count = connected_centers.len();
                    best_other_connected_centers_count = Some(other_connected_centers_count);
                    best_terminal_distance = None;
                    best_other_terminal_distance = None;
                    continue;
                } else if c == Ordering::Less {
                    continue;
                }

                // The C# compares a Euclidean distance with a *squared* one here (and below).
                // Copied as it is.
                let middle = context.grid.middle();
                let best_distance = *best_terminal_distance.get_or_insert_with(|| {
                    f64::from(best.terminal.terminal.euclidean_distance_squared(middle))
                });

                let terminal_distance = terminal.terminal.euclidean_distance(middle);
                let c = terminal_distance.net_cmp(&best_distance);
                if c == Ordering::Less {
                    best_connection = Some(replace(path));
                    best_connected_centers_count = connected_centers.len();
                    best_other_connected_centers_count = Some(other_connected_centers_count);
                    best_terminal_distance = Some(terminal_distance);
                    best_other_terminal_distance = None;
                    continue;
                } else if c == Ordering::Greater {
                    continue;
                }

                let best_other_distance = *best_other_terminal_distance.get_or_insert_with(|| {
                    f64::from(
                        best.best_terminal
                            .terminal
                            .euclidean_distance_squared(middle),
                    )
                });

                let other_terminal_distance = closest_terminal.terminal.euclidean_distance(middle);
                let c = other_terminal_distance.net_cmp(&best_other_distance);
                if c == Ordering::Less {
                    best_connection = Some(replace(path));
                    best_connected_centers_count = connected_centers.len();
                    best_other_connected_centers_count = Some(other_connected_centers_count);
                    best_terminal_distance = Some(terminal_distance);
                    best_other_terminal_distance = Some(other_terminal_distance);
                }
            }
        }
    }

    let Some(best_connection) = best_connection else {
        return Err(FactorioToolsError::new(
            "A new connection should have been found.",
        ));
    };

    eliminate_other_terminals(context, best_connection.terminal);
    eliminate_other_terminals(context, best_connection.best_terminal);

    // Note that, unlike the trunk groups, these two pumpjacks are not added to
    // `allIncludedCenters`. Copied as it is.
    Ok(PumpjackGroup::new(
        context,
        center_to_connected_centers,
        all_included_centers,
        [
            best_connection.terminal.center,
            best_connection.best_terminal.center,
        ],
        &best_connection.path,
    ))
}

/// `GetChildCenters`: a breadth-first walk of the graph from `starting_center`, skipping
/// `ignore_centers` and not walking on from `shallow_explore_centers`. Returns every center
/// reached except the start, in the order reached.
fn get_child_centers(
    context: &Context,
    center_to_connected_centers: &LocationDictionary<LocationSet>,
    ignore_centers: &LocationSet,
    shallow_explore_centers: &LocationSet,
    starting_center: Location,
) -> LocationSet {
    let mut queue = VecDeque::new();
    let mut visited = context.get_location_set_enumerable(true);
    queue.push_back((starting_center, true));

    while let Some((current, should_recurse)) = queue.pop_front() {
        if !visited.add(current) || !should_recurse {
            continue;
        }

        for other in center_to_connected_centers.value(current).iter() {
            if ignore_centers.contains(other) {
                continue;
            }

            // If the other center is in another group, don't recursively explore.
            queue.push_back((other, !shallow_explore_centers.contains(other)));
        }
    }

    visited.remove(starting_center);
    visited
}

/// The C#'s nested `Max`: the largest terminal X (or Y) among the pumpjacks in `centers`.
/// `Max` returns 0 for an empty source.
fn max_terminal_coordinate(
    context: &Context,
    centers: &LocationSet,
    coordinate: impl Fn(Location) -> i32,
) -> i32 {
    max(centers.iter().map(|c| {
        max(context
            .center_to_terminals
            .value(c)
            .iter()
            .map(|t| coordinate(t.terminal)))
        .unwrap_or(0)
    }))
    .unwrap_or(0)
}

/// `GetTrunkCandidates`: from every terminal, walk right and walk down while the cells are
/// empty, collecting the terminals of connected pumpjacks met on the way.
fn get_trunk_candidates(
    context: &Context,
    center_to_connected_centers: &LocationDictionary<LocationSet>,
) -> Vec<Trunk> {
    let mut center_to_max_x: LocationDictionary<i32> =
        context.get_location_dictionary_with_capacity(context.centers.len());
    let mut center_to_max_y: LocationDictionary<i32> =
        context.get_location_dictionary_with_capacity(context.centers.len());
    for &c in &context.centers {
        center_to_max_x.add(
            c,
            max_terminal_coordinate(context, center_to_connected_centers.value(c), |l| l.x),
        );
    }
    for &c in &context.centers {
        center_to_max_y.add(
            c,
            max_terminal_coordinate(context, center_to_connected_centers.value(c), |l| l.y),
        );
    }

    // Find paths that connect the most terminals of neighboring pumpjacks.
    let mut trunk_candidates: Vec<Trunk> = Vec::new();
    for translation in TRANSLATIONS {
        for &starting_center in &context.centers {
            for &terminal in context.center_to_terminals.value(starting_center) {
                let mut current_center = starting_center;
                let mut expanded_child_centers = false;
                // `nextCenters` starts as the graph's own set and is replaced by a new one.
                let mut expanded_next_centers: Option<LocationSet> = None;
                let mut max_x = *center_to_max_x.value(current_center);
                let mut max_y = *center_to_max_y.value(current_center);

                let mut location = terminal.terminal.translate_by(translation);

                let mut trunk: Option<Trunk> = None;

                while location.x <= max_x && location.y <= max_y && context.grid.is_empty(location)
                {
                    if let Some(terminals) = context.location_to_terminals.get(location) {
                        let next_centers = expanded_next_centers
                            .as_ref()
                            .unwrap_or_else(|| center_to_connected_centers.value(current_center));

                        let Some(next_center) = terminals
                            .iter()
                            .map(|t| t.center)
                            .find(|&c| next_centers.contains(c))
                        else {
                            // The pumpjack terminal we ran into does not belong to a pumpjack
                            // that the current pumpjack should be connected to.
                            break;
                        };

                        if !expanded_child_centers {
                            let children = get_child_centers(
                                context,
                                center_to_connected_centers,
                                &context.get_single_location_set(current_center),
                                &context.get_single_location_set(next_center),
                                next_center,
                            );

                            if children.is_empty() {
                                break;
                            }

                            max_x = max_terminal_coordinate(context, &children, |l| l.x);
                            max_y = max_terminal_coordinate(context, &children, |l| l.y);
                            expanded_next_centers = Some(children);
                            expanded_child_centers = true;
                        }

                        let trunk = trunk
                            .get_or_insert_with(|| Trunk::new(context, terminal, current_center));

                        trunk.terminals.extend(terminals.iter().copied());
                        for other in terminals {
                            trunk.terminal_locations.add(other.terminal);
                        }
                        for next_terminal in terminals {
                            trunk.centers.add(next_terminal.center);
                        }

                        current_center = next_center;
                    }

                    location = location.translate_by(translation);
                }

                if let Some(mut trunk) = trunk
                    && trunk.centers.len() > 1
                {
                    trunk.original_index = trunk_candidates.len();
                    trunk_candidates.push(trunk);
                }
            }
        }
    }

    trunk_candidates
}

/// `Trunk`: a straight line of pipe from one terminal through other pumpjacks' terminals.
struct Trunk {
    original_index: usize,
    terminals: Vec<TerminalLocation>,
    /// A bit set: `GetLocationSet(location, capacity)` without `allowEnumerate`.
    terminal_locations: LocationSet,
    centers: LocationSet,
    /// `GetTrunkEndDistance`, filled in before sorting.
    trunk_end_distance: f64,
}

impl Trunk {
    fn new(context: &Context, starting_terminal: TerminalLocation, center: Location) -> Self {
        let mut terminals = Vec::with_capacity(2);
        terminals.push(starting_terminal);
        Trunk {
            original_index: 0,
            terminals,
            terminal_locations: context.get_location_set_with_capacity_and(
                starting_terminal.terminal,
                2,
                false,
            ),
            centers: context.get_location_set_with_capacity_and(center, 2, true),
            trunk_end_distance: 0.0,
        }
    }

    fn length(&self) -> i32 {
        self.start().manhattan_distance(self.end()) + 1
    }

    fn start(&self) -> Location {
        self.terminals[0].terminal
    }

    fn end(&self) -> Location {
        self.terminals[self.terminals.len() - 1].terminal
    }

    /// `GetTrunkEndDistance`: how far the trunk's ends are from the centroid of the
    /// pumpjacks the trunk's pumpjacks connect to, outside the trunk. 0 when there are none.
    fn get_trunk_end_distance(
        &self,
        context: &Context,
        center_to_connected_centers: &LocationDictionary<LocationSet>,
    ) -> f64 {
        let mut neighbors = context.get_location_set_enumerable(true);
        for center in self.centers.iter() {
            for other_center in center_to_connected_centers.value(center).iter() {
                neighbors.add(other_center);
            }
        }

        neighbors.except_with(&self.centers);

        if neighbors.is_empty() {
            return 0.0;
        }

        let centroid_x = average(neighbors.iter().map(|l| l.x));
        let centroid_y = average(neighbors.iter().map(|l| l.y));
        self.start().euclidean_distance_to(centroid_x, centroid_y)
            + self.end().euclidean_distance_to(centroid_x, centroid_y)
    }
}

/// `PumpjackGroup`: pumpjacks already joined by pipes.
struct PumpjackGroup {
    included_centers: LocationSet,
    frontier_centers: LocationSet,
    included_center_to_child_centers: LocationDictionary<LocationSet>,
    pipes: LocationSet,
}

impl PumpjackGroup {
    fn new(
        context: &Context,
        center_to_connected_centers: &LocationDictionary<LocationSet>,
        all_included_centers: &LocationSet,
        included_centers: impl IntoIterator<Item = Location>,
        pipes: &[Location],
    ) -> Self {
        let mut group = PumpjackGroup {
            included_centers: context.get_read_only_location_set(included_centers, true),
            frontier_centers: context.get_location_set_enumerable(true),
            included_center_to_child_centers: context.get_location_dictionary(),
            pipes: context.get_location_set_from(pipes.iter().copied(), true),
        };

        group.update_frontier_centers(center_to_connected_centers);
        group.update_included_center_to_child_centers(
            context,
            center_to_connected_centers,
            all_included_centers,
        );
        group
    }

    /// `GetChildCentroidDistanceSquared`. With no child centers this is 0 / 0, `NaN`, as in
    /// the C#.
    fn get_child_centroid_distance_squared(
        &self,
        included_center: Location,
        terminal_candidate: Location,
    ) -> f64 {
        let mut sum_x = 0i32;
        let mut sum_y = 0i32;
        let mut count = 0i32;
        for center in self
            .included_center_to_child_centers
            .value(included_center)
            .iter()
        {
            sum_x = sum_x.wrapping_add(center.x);
            sum_y = sum_y.wrapping_add(center.y);
            count += 1;
        }

        let centroid_x = f64::from(sum_x) / f64::from(count);
        let centroid_y = f64::from(sum_y) / f64::from(count);

        terminal_candidate.euclidean_distance_squared_to(centroid_x, centroid_y)
    }

    fn connect_pumpjack(
        &mut self,
        context: &Context,
        center_to_connected_centers: &LocationDictionary<LocationSet>,
        all_included_centers: &mut LocationSet,
        center: Location,
        path: &[Location],
    ) {
        all_included_centers.add(center);
        self.included_centers.add(center);
        self.pipes.union_with_items(path.iter().copied());
        self.update_frontier_centers(center_to_connected_centers);
        self.update_included_center_to_child_centers(
            context,
            center_to_connected_centers,
            all_included_centers,
        );
    }

    fn merge_group(
        &mut self,
        context: &Context,
        center_to_connected_centers: &LocationDictionary<LocationSet>,
        all_included_centers: &LocationSet,
        other: PumpjackGroup,
        path: &[Location],
    ) {
        self.included_centers.union_with(&other.included_centers);
        self.pipes.union_with_items(path.iter().copied());
        self.pipes.union_with(&other.pipes);
        self.update_frontier_centers(center_to_connected_centers);
        self.update_included_center_to_child_centers(
            context,
            center_to_connected_centers,
            all_included_centers,
        );
    }

    fn update_frontier_centers(
        &mut self,
        center_to_connected_centers: &LocationDictionary<LocationSet>,
    ) {
        self.frontier_centers.clear();

        for center in self.included_centers.iter() {
            self.frontier_centers
                .union_with(center_to_connected_centers.value(center));
        }

        self.frontier_centers.except_with(&self.included_centers);
    }

    fn update_included_center_to_child_centers(
        &mut self,
        context: &Context,
        center_to_connected_centers: &LocationDictionary<LocationSet>,
        all_included_centers: &LocationSet,
    ) {
        self.included_center_to_child_centers.clear();

        for center in self.included_centers.iter() {
            let visited = get_child_centers(
                context,
                center_to_connected_centers,
                &self.included_centers,
                all_included_centers,
                center,
            );

            self.included_center_to_child_centers.add(center, visited);
        }
    }
}
