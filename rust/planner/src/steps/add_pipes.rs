//! `AddPipes`, from `src/FactorioTools/OilField/Steps/AddPipes.0.cs`: runs every pipe
//! strategy, plans underground pipes, heat pipes and beacons for each distinct pipe layout,
//! ranks the resulting plans and commits the best one to the grid.
//!
//! **Errors.** The C# keeps two kinds of failure apart. Some exceptions travel inside a
//! `Result` and are inspected: `SelectBestSolution` retries once, after removing stranded
//! terminals, when the returned exception is a `NoPathBetweenTerminalsException`. Others are
//! thrown and end the plan at once, even a `NoPathBetweenTerminalsException`. So the
//! functions up to `SelectBestSolution` return `FtResult<FtResult<T>>`: the outer error is
//! the thrown kind, the inner one the returned kind. `AddPipesFbe` returns every no-path error
//! and only throws other kinds, so its no-path errors count as returned and the rest as
//! thrown; for any other kind the two end the same way. `FindTrunksAndConnect` keeps the two
//! apart itself.
//!
//! **Shared objects.** The C# passes the same terminal maps and solutions around by
//! reference. Solutions live in one store here and the lists hold indexes into it. The
//! terminal maps move between `Context` and the callers' locals so that every mutation lands
//! where the C#'s would; where the C# lets a solution keep a reference to a map that nothing
//! changes afterwards, the solution keeps a copy.

use std::cmp::Ordering;
use std::mem;

use dotnet_compat::net_sort;

use crate::Location;
use crate::algorithms::dijkstras;
use crate::collection_ext::{first, single};
use crate::containers::{CountedBitArray, LocationDictionary, LocationSet};
use crate::context::Context;
use crate::data::Direction;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity, TerminalLocation};
use crate::helpers::{eliminate_other_terminals, remove_pumpjack};
use crate::models::{BeaconSolution, OilFieldPlan, PipeStrategy};
use crate::providers::{add_beacons_to_grid, remove_entity};
use crate::steps::{
    add_heat_pipes, add_pipe_entities, add_pipes_connected_centers, add_pipes_fbe, plan_beacons,
    plan_underground_pipes, rotate_optimize, validate,
};

/// The result of `AddPipes.Execute`: the selected, alternate and unused plans.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddPipesResult {
    pub selected_plans: Vec<OilFieldPlan>,
    pub alternate_plans: Vec<OilFieldPlan>,
    pub unused_plans: Vec<OilFieldPlan>,
}

type TerminalMap = LocationDictionary<Vec<TerminalLocation>>;

/// A pair of terminal maps: `CenterToTerminals` and `LocationToTerminals`.
struct TerminalMaps {
    center_to_terminals: TerminalMap,
    location_to_terminals: TerminalMap,
}

impl TerminalMaps {
    /// Moves the context's maps out, leaving empty ones behind.
    fn take(context: &mut Context) -> Self {
        TerminalMaps {
            center_to_terminals: mem::take(&mut context.center_to_terminals),
            location_to_terminals: mem::take(&mut context.location_to_terminals),
        }
    }

    /// Moves these maps into the context.
    fn put(self, context: &mut Context) {
        context.center_to_terminals = self.center_to_terminals;
        context.location_to_terminals = self.location_to_terminals;
    }

    /// `original.EnumeratePairs().ToDictionary(context, x => x.Key, x => x.Value.ToList())`
    /// for both maps: new dictionaries and new lists that hold the same terminal objects.
    fn copy(&self) -> Self {
        TerminalMaps {
            center_to_terminals: copy_terminal_map(&self.center_to_terminals),
            location_to_terminals: copy_terminal_map(&self.location_to_terminals),
        }
    }

    /// A copy of the context's maps, for a solution to keep.
    fn copy_of(context: &Context) -> Self {
        TerminalMaps {
            center_to_terminals: copy_terminal_map(&context.center_to_terminals),
            location_to_terminals: copy_terminal_map(&context.location_to_terminals),
        }
    }
}

fn copy_terminal_map(map: &TerminalMap) -> TerminalMap {
    let mut copy = LocationDictionary::with_capacity(map.len());
    for (key, terminals) in map.iter() {
        copy.add(key, terminals.clone());
    }
    copy
}

/// `Solution`.
struct Solution {
    strategies: Vec<PipeStrategy>,
    optimized: Vec<bool>,
    terminals: TerminalMaps,
    pipe_count_without_underground: i32,
    pipes: LocationSet,
    underground_pipes: Option<LocationDictionary<Direction>>,
    beacon_solutions: Option<Vec<BeaconSolution>>,
    heat_pipes: Option<LocationSet>,
    uncovered_pipes: LocationSet,
    uncovered_centers: LocationSet,
}

/// `SolutionsAndGroupNumber`: indexes into the solution store.
struct SolutionGroup {
    solutions: Vec<usize>,
    group_number: i32,
}

/// What `GetSolutionGroups` builds: the solutions, and `pipesToSolutions.Values` in order.
struct SolutionGroups {
    solutions: Vec<Solution>,
    groups: Vec<SolutionGroup>,
}

/// `PlanInfo`. `solution` indexes the solution store and `beacons` the solution's beacon
/// solutions.
#[derive(Clone, Copy)]
struct PlanInfo {
    group_number: i32,
    group_size: i32,
    plan: OilFieldPlan,
    solution: usize,
    beacons: Option<usize>,
}

/// `SolutionInfo`.
struct SolutionInfo {
    selected_plans: Vec<OilFieldPlan>,
    alternate_plans: Vec<OilFieldPlan>,
    unused_plans: Vec<OilFieldPlan>,
    best_solution: Solution,
    best_beacons: Option<BeaconSolution>,
}

/// `AddPipes.Execute(context, eliminateStrandedTerminals)`.
pub fn execute(
    context: &mut Context,
    eliminate_stranded_terminals: bool,
) -> FtResult<AddPipesResult> {
    if eliminate_stranded_terminals {
        eliminate_stranded_terminals_step(context)?;
    }

    // A returned exception is thrown here, the same as a thrown one.
    let info = select_best_solution(context, eliminate_stranded_terminals)??;
    let SolutionInfo {
        mut selected_plans,
        alternate_plans,
        unused_plans,
        best_solution,
        best_beacons,
    } = info;

    best_solution.terminals.put(context);

    add_pipe_entities::execute(
        context,
        &best_solution.pipes,
        best_solution.underground_pipes.as_ref(),
    )?;

    // Place the heat network before beacons so it claims the contested tiles next to pipes
    // and pumpjacks. `AddHeatPipes.Execute` is then a no-op in the planner.
    if let Some(heat_pipes) = best_solution.heat_pipes {
        for location in heat_pipes.iter() {
            let id = context.grid.get_id();
            context
                .grid
                .add_entity(location, GridEntity::new(id, EntityKind::HeatPipe))?;
        }

        context.heat_pipes = Some(heat_pipes);
    }

    if let Some(best_beacons) = best_beacons {
        add_beacons_to_grid(
            &mut context.grid,
            &context.options,
            best_beacons.beacons.iter().copied(),
        )?;

        // On Aquilo, extend the heat network to reach the placed beacons, then drop any beacon
        // that still cannot be heated, and update the reported counts to match.
        if context.options.add_heat_pipes && context.heat_pipes.is_some() {
            let heated_beacons = add_heat_pipes::extend_to_beacons(context, &best_beacons.beacons)?;

            let mut kept_effects = 0;
            let mut kept_count = 0;
            for (i, &center) in best_beacons.beacons.iter().enumerate() {
                if heated_beacons.contains(center) {
                    kept_effects += best_beacons.effects_given_counts[i];
                    kept_count += 1;
                } else {
                    remove_entity(
                        &mut context.grid,
                        center,
                        context.options.beacon_width,
                        context.options.beacon_height,
                    );
                }
            }

            if let Some(first) = selected_plans.first_mut() {
                first.beacon_effect_count = kept_effects;
                first.beacon_count = kept_count;
            }
        }
    }

    Ok(AddPipesResult {
        selected_plans,
        alternate_plans,
        unused_plans,
    })
}

/// `SelectBestSolution`.
fn select_best_solution(
    context: &mut Context,
    eliminate_stranded_terminals: bool,
) -> FtResult<FtResult<SolutionInfo>> {
    // The full pumpjack set's maps. The drop loop changes these in place, so each replan
    // sees the reduced set.
    let mut originals = TerminalMaps::take(context);

    loop {
        let mut result = get_best_solution(context, &mut originals)?;
        if let Err(error) = &result
            && error.is_no_path_between_terminals()
            && !eliminate_stranded_terminals
        {
            // GetBestSolution leaves a per-strategy copy in the context when it fails, so
            // put the originals back before removing the stranded terminals from them.
            originals.put(context);
            eliminate_stranded_terminals_step(context)?;
            originals = TerminalMaps::take(context);
            result = get_best_solution(context, &mut originals)?;
        }

        let info = match result {
            Ok(info) => info,
            Err(error) => return Ok(Err(error)),
        };

        let best = &info.best_solution;
        if !context.options.add_heat_pipes
            || best.uncovered_pipes.len() + best.uncovered_centers.len() == 0
        {
            return Ok(Ok(info));
        }

        let drop_center = choose_drop_candidate(context, best);
        if !drop_center.is_valid() {
            // Nothing left to drop. Accept the residual; the summary reports it.
            return Ok(Ok(info));
        }

        drop_pumpjack(context, &mut originals, drop_center);
        context.heat_dropped_pumpjacks += 1;
    }
}

/// `ChooseDropCandidate`: a pumpjack that cannot be heated itself, or else the pumpjack
/// nearest a stuck pipe tile, ties broken by (Y, X).
fn choose_drop_candidate(context: &Context, best: &Solution) -> Location {
    if !best.uncovered_centers.is_empty() {
        let mut chosen_center = Location::INVALID;
        for center in best.uncovered_centers.iter() {
            if is_before(center, chosen_center) {
                chosen_center = center;
            }
        }

        return chosen_center;
    }

    if best.uncovered_pipes.is_empty() {
        return Location::INVALID;
    }

    let mut chosen = Location::INVALID;
    let mut chosen_distance = i32::MAX;
    for &center in &context.centers {
        let mut distance = i32::MAX;
        for pipe in best.uncovered_pipes.iter() {
            let d = (center.x - pipe.x).abs() + (center.y - pipe.y).abs();
            if d < distance {
                distance = d;
            }
        }

        if distance < chosen_distance || (distance == chosen_distance && is_before(center, chosen))
        {
            chosen_distance = distance;
            chosen = center;
        }
    }

    chosen
}

/// `IsBefore`.
fn is_before(a: Location, b: Location) -> bool {
    if !b.is_valid() {
        return true;
    }

    if a.y != b.y {
        return a.y < b.y;
    }

    a.x < b.x
}

/// `DropPumpjack`.
fn drop_pumpjack(context: &mut Context, originals: &mut TerminalMaps, center: Location) {
    if let Some(terminals) = originals.center_to_terminals.get(center) {
        for terminal in terminals {
            if let Some(at_location) = originals.location_to_terminals.get_mut(terminal.terminal) {
                // `List.Remove` compares references.
                if let Some(index) = at_location.iter().position(|t| t.same_object(terminal)) {
                    at_location.remove(index);
                }

                if at_location.is_empty() {
                    originals.location_to_terminals.remove(terminal.terminal);
                }
            }
        }

        originals.center_to_terminals.remove(center);
    }

    if let Some(index) = context.centers.iter().position(|&c| c == center) {
        context.centers.remove(index);
    }

    context.center_to_original_direction.remove(center);
    remove_pumpjack(&mut context.grid, center);
}

/// `GetBestSolution`.
fn get_best_solution(
    context: &mut Context,
    originals: &mut TerminalMaps,
) -> FtResult<FtResult<SolutionInfo>> {
    let (mut groups, mut sorted_plans) = match get_all_plans(context, originals)? {
        Ok(result) => result,
        Err(error) => return Ok(Err(error)),
    };

    let add_heat_pipes = context.options.add_heat_pipes;
    let solutions = &groups.solutions;
    net_sort(&mut sorted_plans, |a, b| {
        compare_plans(add_heat_pipes, solutions, a, b)
    });

    let mut best_plan_info: Option<PlanInfo> = None;
    let mut no_more_alternates = false;
    let mut selected_plans = Vec::new();
    let mut alternate_plans = Vec::new();
    let mut unused_plans = Vec::new();

    for plan_info in &sorted_plans {
        if no_more_alternates {
            unused_plans.push(plan_info.plan);
            continue;
        }

        let Some(best) = best_plan_info else {
            best_plan_info = Some(*plan_info);
            selected_plans.push(plan_info.plan);
            continue;
        };

        if plan_info.plan.is_equivalent(&best.plan) {
            if plan_info.group_number == best.group_number {
                selected_plans.push(plan_info.plan);
            } else {
                alternate_plans.push(plan_info.plan);
            }
        } else {
            no_more_alternates = true;
            unused_plans.push(plan_info.plan);
        }
    }

    let Some(best) = best_plan_info else {
        return Err(FactorioToolsError::new(
            "At least one pipe strategy must be used.",
        ));
    };

    let best_beacons = best.beacons.map(|i| {
        groups.solutions[best.solution]
            .beacon_solutions
            .as_ref()
            .expect("a plan with beacons has beacon solutions")[i]
            .clone()
    });
    let best_solution = groups.solutions.swap_remove(best.solution);

    Ok(Ok(SolutionInfo {
        selected_plans,
        alternate_plans,
        unused_plans,
        best_solution,
        best_beacons,
    }))
}

/// The plan sort in `GetBestSolution`. It ends on the group number, so the order is total.
fn compare_plans(
    add_heat_pipes: bool,
    solutions: &[Solution],
    a: &PlanInfo,
    b: &PlanInfo,
) -> Ordering {
    // When heat is on it is the hard constraint: fewer unheated targets first.
    if add_heat_pipes {
        let a_solution = &solutions[a.solution];
        let b_solution = &solutions[b.solution];
        let a_unheated = a_solution.uncovered_pipes.len() + a_solution.uncovered_centers.len();
        let b_unheated = b_solution.uncovered_pipes.len() + b_solution.uncovered_centers.len();
        let heat = a_unheated.cmp(&b_unheated);
        if heat != Ordering::Equal {
            return heat;
        }
    }

    // more effects = better
    b.plan
        .beacon_effect_count
        .cmp(&a.plan.beacon_effect_count)
        // fewer beacons = better (less power)
        .then(a.plan.beacon_count.cmp(&b.plan.beacon_count))
        // fewer pipes = better
        .then(a.plan.pipe_count.cmp(&b.plan.pipe_count))
        // prefer solutions that more algorithms find
        .then(b.group_size.cmp(&a.group_size))
        // the rest is arbitrary tie breaking; `Comparer<BeaconStrategy?>` puts null first,
        // as `Option` does
        .then(a.plan.pipe_strategy.cmp(&b.plan.pipe_strategy))
        .then(a.plan.optimize_pipes.cmp(&b.plan.optimize_pipes))
        .then(a.plan.beacon_strategy.cmp(&b.plan.beacon_strategy))
        .then(a.group_number.cmp(&b.group_number))
}

/// `GetAllPlans`.
fn get_all_plans(
    context: &mut Context,
    originals: &mut TerminalMaps,
) -> FtResult<FtResult<(SolutionGroups, Vec<PlanInfo>)>> {
    let groups = match get_solution_groups(context, originals)? {
        Ok(groups) => groups,
        Err(error) => return Ok(Err(error)),
    };

    let mut plans = Vec::new();
    for group in &groups.groups {
        let group_size = group.solutions.len() as i32;
        for &solution_index in &group.solutions {
            let solution = &groups.solutions[solution_index];
            match &solution.beacon_solutions {
                None => {
                    for &strategy in &solution.strategies {
                        for &optimized in &solution.optimized {
                            plans.push(PlanInfo {
                                group_number: group.group_number,
                                group_size,
                                plan: OilFieldPlan {
                                    pipe_strategy: strategy,
                                    optimize_pipes: optimized,
                                    beacon_strategy: None,
                                    beacon_effect_count: 0,
                                    beacon_count: 0,
                                    pipe_count: solution.pipes.len() as i32,
                                    pipe_count_without_underground: solution
                                        .pipe_count_without_underground,
                                },
                                solution: solution_index,
                                beacons: None,
                            });
                        }
                    }
                }
                Some(beacon_solutions) => {
                    for (beacons_index, beacons) in beacon_solutions.iter().enumerate() {
                        for &strategy in &solution.strategies {
                            for &optimized in &solution.optimized {
                                plans.push(PlanInfo {
                                    group_number: group.group_number,
                                    group_size,
                                    plan: OilFieldPlan {
                                        pipe_strategy: strategy,
                                        optimize_pipes: optimized,
                                        beacon_strategy: Some(beacons.strategy),
                                        beacon_effect_count: beacons.effects,
                                        beacon_count: beacons.beacons.len() as i32,
                                        pipe_count: solution.pipes.len() as i32,
                                        pipe_count_without_underground: solution
                                            .pipe_count_without_underground,
                                    },
                                    solution: solution_index,
                                    beacons: Some(beacons_index),
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(Ok((groups, plans)))
}

/// `pipesToSolutions`, a `Dictionary` keyed by pipe set with `LocationSetComparer`. Nothing
/// is ever removed, so its values list in insertion order. A lookup matches an entry only
/// when the hash codes match and the comparer says equal, as `Dictionary.FindValue` does.
struct PipesToSolutions {
    entries: Vec<(LocationSet, i32, usize)>,
}

impl PipesToSolutions {
    fn find(&self, pipes: &LocationSet) -> Option<usize> {
        let hash = location_set_hash(pipes);
        self.entries
            .iter()
            .position(|(key, key_hash, _)| *key_hash == hash && location_sets_equal(key, pipes))
            .map(|i| self.entries[i].2)
    }
}

/// `LocationSetComparer.Equals(x, y)`.
fn location_sets_equal(x: &LocationSet, y: &LocationSet) -> bool {
    x.len() == y.len() && x.set_equals(y)
}

/// `LocationSetComparer.GetHashCode`, with C#'s wrapping `int` arithmetic.
fn location_set_hash(set: &LocationSet) -> i32 {
    let mut sum_x = 0i32;
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut sum_y = 0i32;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;

    for l in set.iter() {
        sum_x = sum_x.wrapping_add(l.x);
        min_x = min_x.min(l.x);
        max_x = max_x.max(l.x);
        sum_y = sum_y.wrapping_add(l.y);
        min_y = min_y.min(l.y);
        max_y = max_y.max(l.y);
    }

    let mut hash = 17i32;
    for value in [set.len() as i32, sum_x, min_x, max_x, sum_y, min_y, max_y] {
        hash = hash.wrapping_mul(23).wrapping_add(value);
    }
    hash
}

/// `ConnectedCentersComparer.Equals(x, y)`.
fn connected_centers_equal(
    x: &LocationDictionary<LocationSet>,
    y: &LocationDictionary<LocationSet>,
) -> bool {
    if x.len() != y.len() {
        return false;
    }

    for (key, x_value) in x.iter() {
        match y.get(key) {
            Some(y_value) if x_value.set_equals(y_value) => {}
            _ => return false,
        }
    }

    true
}

/// `ConnectedCentersComparer.GetHashCode`. Note that `locationSum` ends as the count of the
/// last set enumerated, not a sum; so two equal graphs listed in different orders can hash
/// differently and miss each other in the dictionary, as in the C#.
fn connected_centers_hash(graph: &LocationDictionary<LocationSet>) -> i32 {
    let mut sum_x = 0i32;
    let mut min_x = i32::MAX;
    let mut max_x = i32::MIN;
    let mut sum_y = 0i32;
    let mut min_y = i32::MAX;
    let mut max_y = i32::MIN;
    let mut location_sum = 0i32;

    for (l, s) in graph.iter() {
        sum_x = sum_x.wrapping_add(l.x);
        min_x = min_x.min(l.x);
        max_x = max_x.max(l.x);
        sum_y = sum_y.wrapping_add(l.y);
        min_y = min_y.min(l.y);
        max_y = max_y.max(l.y);
        location_sum = s.len() as i32;
    }

    let mut hash = 17i32;
    for value in [
        graph.len() as i32,
        sum_x,
        min_x,
        max_x,
        sum_y,
        min_y,
        max_y,
        location_sum,
    ] {
        hash = hash.wrapping_mul(23).wrapping_add(value);
    }
    hash
}

/// The builder state `GetSolutionGroups` and `OptimizeAndAddSolutions` share.
struct GroupsBuilder {
    solutions: Vec<Solution>,
    groups: Vec<SolutionGroup>,
    pipes_to_solutions: PipesToSolutions,
}

/// `GetSolutionGroups`. `originals` are the maps the context held on entry (the C# reads
/// them through `context.CenterToTerminals`).
fn get_solution_groups(
    context: &mut Context,
    originals: &mut TerminalMaps,
) -> FtResult<FtResult<SolutionGroups>> {
    let mut builder = GroupsBuilder {
        solutions: Vec::new(),
        groups: Vec::new(),
        pipes_to_solutions: PipesToSolutions {
            entries: Vec::new(),
        },
    };

    // `connectedCentersToSolutions`, keyed by graph with `ConnectedCentersComparer`.
    let mut connected_centers_to_solutions: Vec<(
        LocationDictionary<LocationSet>,
        i32,
        Vec<usize>,
    )> = Vec::new();

    if originals.center_to_terminals.len() == 1 {
        // The context holds the originals here, and these changes stay in them.
        mem::replace(originals, TerminalMaps::empty()).put(context);

        let terminal = single(context.center_to_terminals.iter())?.1[0];
        eliminate_other_terminals(context, terminal);
        let pipes = context.get_single_location_set(terminal.terminal);
        let solutions = optimize_and_add_solutions(context, &mut builder, None, pipes)?;
        let solution = single(solutions)?;
        let strategies = context.options.pipe_strategies.clone();
        builder.solutions[solution].strategies = strategies;

        *originals = TerminalMaps::take(context);
    } else {
        let max_value = PipeStrategy::ConnectedCentersFlute.value() + 1;
        let mut completed_strategies = CountedBitArray::new(max_value);
        let strategies = context.options.pipe_strategies.clone();
        for strategy in strategies {
            if completed_strategies.get(strategy.value()) {
                continue;
            }

            originals.copy().put(context);

            match strategy {
                PipeStrategy::FbeOriginal | PipeStrategy::Fbe => {
                    let result = match add_pipes_fbe::execute(context, strategy) {
                        Ok(result) => result,
                        // FBE returns its no-path errors and throws the rest.
                        Err(error) if error.is_no_path_between_terminals() => {
                            return Ok(Err(error));
                        }
                        Err(error) => return Err(error),
                    };

                    completed_strategies.set(result.final_strategy.value(), true);
                    optimize_and_add_solutions(
                        context,
                        &mut builder,
                        Some(result.final_strategy),
                        result.pipes,
                    )?;
                }
                PipeStrategy::ConnectedCentersDelaunay
                | PipeStrategy::ConnectedCentersDelaunayMst
                | PipeStrategy::ConnectedCentersFlute => {
                    let center_to_connected_centers =
                        add_pipes_connected_centers::get_connected_pumpjacks(context, strategy)?;
                    completed_strategies.set(strategy.value(), true);

                    let hash = connected_centers_hash(&center_to_connected_centers);
                    if let Some((_, _, solutions)) =
                        connected_centers_to_solutions
                            .iter()
                            .find(|(key, key_hash, _)| {
                                *key_hash == hash
                                    && connected_centers_equal(key, &center_to_connected_centers)
                            })
                    {
                        for &solution in solutions {
                            builder.solutions[solution].strategies.push(strategy);
                        }
                        continue;
                    }

                    let pipes = match add_pipes_connected_centers::find_trunks_and_connect(
                        context,
                        &center_to_connected_centers,
                    )? {
                        Ok(pipes) => pipes,
                        Err(error) => return Ok(Err(error)),
                    };

                    let solutions =
                        optimize_and_add_solutions(context, &mut builder, Some(strategy), pipes)?;
                    connected_centers_to_solutions.push((
                        center_to_connected_centers,
                        hash,
                        solutions,
                    ));
                }
            }
        }
    }

    Ok(Ok(SolutionGroups {
        solutions: builder.solutions,
        groups: builder.groups,
    }))
}

impl TerminalMaps {
    fn empty() -> Self {
        TerminalMaps {
            center_to_terminals: LocationDictionary::new(),
            location_to_terminals: LocationDictionary::new(),
        }
    }
}

/// `OptimizeAndAddSolutions`. `strategy` is `None` for the C#'s `default` (single pumpjack),
/// where the caller replaces the strategies afterwards. Returns the group's solution
/// indexes. The context's maps on entry are this layout's ("original") maps, and they are
/// back in the context on return.
fn optimize_and_add_solutions(
    context: &mut Context,
    builder: &mut GroupsBuilder,
    strategy: Option<PipeStrategy>,
    pipes: LocationSet,
) -> FtResult<Vec<usize>> {
    // `default(PipeStrategy)` is the first member.
    let strategy_value = strategy.unwrap_or(PipeStrategy::FbeOriginal);

    if let Some(group) = builder.pipes_to_solutions.find(&pipes) {
        let solutions = builder.groups[group].solutions.clone();
        for &solution in &solutions {
            builder.solutions[solution].strategies.push(strategy_value);
        }

        return Ok(solutions);
    }

    let original = TerminalMaps::take(context);

    let optimized_pipes = if context.options.optimize_pipes {
        original.copy().put(context);
        let mut optimized_pipes = pipes.copy();
        rotate_optimize::execute(context, &mut optimized_pipes)?;
        Some(optimized_pipes)
    } else {
        // The C# keeps the original maps in the context here; a copy behaves the same,
        // since nothing changes them before the solution copies them.
        original.copy().put(context);
        None
    };

    let set_equal = match &optimized_pipes {
        Some(optimized_pipes) => pipes.set_equals(optimized_pipes),
        None => pipes.set_equals(&pipes),
    };

    let solutions = if set_equal {
        let solution_pipes = if context.options.use_underground_pipes {
            pipes.copy()
        } else {
            copy_without_underground(&pipes)
        };
        let mut solution = get_solution(context, strategy_value, false, solution_pipes)?;

        if context.options.optimize_pipes {
            solution.optimized.push(true);
        }

        builder.solutions.push(solution);
        vec![builder.solutions.len() - 1]
    } else {
        let optimized_pipes = optimized_pipes.unwrap_or_else(|| pipes.copy());
        let solution_a = get_solution(context, strategy_value, true, optimized_pipes)?;

        original.copy().put(context);
        let pipes_b = if context.options.use_underground_pipes {
            pipes.copy()
        } else {
            copy_without_underground(&pipes)
        };
        let solution_b = get_solution(context, strategy_value, false, pipes_b)?;

        validate::pipes_do_not_match(context, &solution_a.pipes, &solution_b.pipes)?;

        builder.solutions.push(solution_a);
        builder.solutions.push(solution_b);
        vec![builder.solutions.len() - 2, builder.solutions.len() - 1]
    };

    let hash = location_set_hash(&pipes);
    let group_number = builder.groups.len() as i32 + 1;
    builder.groups.push(SolutionGroup {
        solutions: solutions.clone(),
        group_number,
    });
    builder
        .pipes_to_solutions
        .entries
        .push((pipes, hash, builder.groups.len() - 1));

    original.put(context);

    Ok(solutions)
}

/// Without underground pipes the C# hands the dictionary key itself to `GetSolution`, which
/// never changes it then. A copy lists the same items in the same order.
fn copy_without_underground(pipes: &LocationSet) -> LocationSet {
    pipes.copy()
}

/// `GetSolution`.
fn get_solution(
    context: &mut Context,
    strategy: PipeStrategy,
    optimized: bool,
    mut optimized_pipes: LocationSet,
) -> FtResult<Solution> {
    validate::pipes_are_connected(context, &optimized_pipes)?;

    let pipe_count_before_underground = optimized_pipes.len() as i32;

    let underground_pipes = if context.options.use_underground_pipes {
        Some(plan_underground_pipes::execute(
            context,
            &mut optimized_pipes,
        )?)
    } else {
        None
    };

    // On Aquilo, route the heat network for this layout up front so plan selection can rank
    // by heat coverage. Layouts that cannot be fully heated are kept but ranked last.
    let mut heat_pipes = None;
    let mut uncovered_pipes = LocationSet::Empty;
    let mut uncovered_centers = LocationSet::Empty;
    let mut heat_feasible = true;
    if context.options.add_heat_pipes {
        let route = add_heat_pipes::route(context, &optimized_pipes)?;
        heat_feasible = route.uncovered_pipes.is_empty() && route.uncovered_centers.is_empty();
        heat_pipes = Some(route.chosen);
        uncovered_pipes = route.uncovered_pipes;
        uncovered_centers = route.uncovered_centers;
    }

    let beacon_solutions = if context.options.add_beacons && heat_feasible {
        Some(plan_beacons::execute(
            context,
            &optimized_pipes,
            heat_pipes.as_ref(),
        )?)
    } else {
        None
    };

    validate::no_overlapping_entities(
        context,
        &optimized_pipes,
        underground_pipes.as_ref(),
        beacon_solutions.as_deref(),
    )?;

    Ok(Solution {
        strategies: vec![strategy],
        optimized: vec![optimized],
        terminals: TerminalMaps::copy_of(context),
        pipe_count_without_underground: pipe_count_before_underground,
        pipes: optimized_pipes,
        underground_pipes,
        beacon_solutions,
        heat_pipes,
        uncovered_pipes,
        uncovered_centers,
    })
}

/// `EliminateStrandedTerminals`: splits the terminal locations into connected components,
/// keeps the one serving the most pumpjacks and eliminates the rest. Throws when a pumpjack
/// loses every terminal.
fn eliminate_stranded_terminals_step(context: &mut Context) -> FtResult<()> {
    let keys: Vec<Location> = context.location_to_terminals.keys().collect();
    let mut remaining = context.get_read_only_location_set(keys, true);

    let mut components: Vec<LocationSet> = Vec::new();
    while !remaining.is_empty() {
        let mut goals = context.get_location_set_copy(&remaining);
        let start = first(goals.iter())?;
        goals.remove(start);

        let result = dijkstras(
            &context.grid,
            context.grid.width(),
            context.grid.height(),
            start,
            &goals,
            false,
            true,
        );

        let mut component = context.get_location_set_copy(&result.reached_goals);
        component.add(start);
        remaining.except_with(&component);
        components.push(component);
    }

    // One component means everything can already reach everything else.
    if components.len() < 2 {
        return Ok(());
    }

    let mut keep = 0;
    let mut most_pumpjacks: i64 = -1;
    for (i, component) in components.iter().enumerate() {
        let mut centers = context.get_location_set();
        for location in component.iter() {
            for terminal in context.location_to_terminals.value(location) {
                centers.add(terminal.center);
            }
        }

        if centers.len() as i64 > most_pumpjacks {
            most_pumpjacks = centers.len() as i64;
            keep = i;
        }
    }

    let mut stranded_terminal = Location::INVALID;
    let mut found_stranded = false;
    for (i, component) in components.iter().enumerate() {
        if i == keep {
            continue;
        }

        for location in component.iter() {
            let terminals = context.location_to_terminals.value(location).clone();
            for terminal in terminals {
                let center_terminals = context.center_to_terminals.value_mut(terminal.center);
                if let Some(index) = center_terminals
                    .iter()
                    .position(|t| t.same_object(&terminal))
                {
                    center_terminals.remove(index);
                }

                if center_terminals.is_empty() {
                    stranded_terminal = terminal.terminal;
                    found_stranded = true;
                }
            }

            context.location_to_terminals.remove(location);
        }
    }

    // A pumpjack whose every terminal sat outside the kept component cannot join the
    // network.
    if found_stranded {
        return Err(FactorioToolsError::no_path_between_terminals(
            stranded_terminal,
            first(components[keep].iter())?,
        ));
    }

    Ok(())
}
