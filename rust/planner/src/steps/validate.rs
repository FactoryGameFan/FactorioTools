//! `Validate`, from `src/FactorioTools/OilField/Steps/Validate.cs`: checks the planner runs on
//! its own results. Most do nothing unless `options.validate_solution` is set.

use std::collections::VecDeque;

use crate::Location;
use crate::algorithms::dijkstras;
use crate::collection_ext::first;
use crate::containers::{CountedBitArray, LocationDictionary, LocationSet};
use crate::context::Context;
use crate::data::Direction;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, ExistingPipeGrid};
use crate::helpers::{
    add_beacons_to_grid, get_covered_center_to_provider_centers, get_electric_pole_coverage,
    get_powered_entities, get_provider_center_to_covered_centers,
};
use crate::models::{BeaconSolution, ElectricPoleCandidateInfo, ProviderRecipient};
use crate::steps::add_pipe_entities;

/// `AddHeatPipes.PumpjackRingOffsets`: the twelve cells next to a pumpjack's 3x3 footprint, in
/// the C#'s order. `AddHeatPipes` is ported in stage 10, which should use this one.
pub const PUMPJACK_RING_OFFSETS: [Location; 12] = [
    Location::new(-1, -2),
    Location::new(0, -2),
    Location::new(1, -2),
    Location::new(-1, 2),
    Location::new(0, 2),
    Location::new(1, 2),
    Location::new(-2, -1),
    Location::new(-2, 0),
    Location::new(-2, 1),
    Location::new(2, -1),
    Location::new(2, 0),
    Location::new(2, 1),
];

/// `PipesAreConnected`: every pumpjack has one terminal, and the pipes join them all.
pub fn pipes_are_connected(context: &Context, optimized_pipes: &LocationSet) -> FtResult<()> {
    if !context.options.validate_solution {
        return Ok(());
    }

    for terminals in context.center_to_terminals.values() {
        if terminals.len() != 1 {
            return Err(FactorioToolsError::new(
                "A pumpjack has more than one terminal.",
            ));
        }
    }

    let mut goals = context.get_location_set_enumerable(true);
    for terminals in context.center_to_terminals.values() {
        for terminal in terminals {
            goals.add(terminal.terminal);
        }
    }

    let clone = ExistingPipeGrid::new(optimized_pipes);
    let start = first(goals.iter())?;
    goals.remove(start);
    let result = dijkstras(
        &clone,
        context.grid.width(),
        context.grid.height(),
        start,
        &goals,
        false,
        true,
    );
    let mut reached_goals = result.reached_goals;
    reached_goals.add(start);
    let mut unreached_goals = context.get_location_set_copy(&goals);
    unreached_goals.except_with(&reached_goals);
    if !unreached_goals.is_empty() {
        return Err(FactorioToolsError::new(
            "The pipes are not fully connected.",
        ));
    }

    Ok(())
}

/// `UndergroundPipesArePipes`: every underground pipe is also in the pipe set.
pub fn underground_pipes_are_pipes(
    context: &Context,
    pipes: &LocationSet,
    location_to_direction: &LocationDictionary<Direction>,
) -> FtResult<()> {
    if context.options.validate_solution {
        let mut missing = context.get_location_set_from(location_to_direction.keys(), true);
        missing.except_with(pipes);

        if !missing.is_empty() {
            return Err(FactorioToolsError::new(
                "The underground pipes should be in the pipe set.",
            ));
        }
    }

    Ok(())
}

/// `PipesDoNotMatch`: the two pipe sets differ. With a `LocationBitSet` first, the C#
/// `SetEquals` always answers false (see `LocationSet::set_equals`), so this always passes.
pub fn pipes_do_not_match(
    context: &Context,
    pipes1: &LocationSet,
    pipes2: &LocationSet,
) -> FtResult<()> {
    if context.options.validate_solution && pipes1.set_equals(pipes2) {
        return Err(FactorioToolsError::new(
            "The two pipe configurations should not match.",
        ));
    }

    Ok(())
}

/// `BeaconsDoNotOverlap`: with overlapping beacons turned off, no pumpjack gets an effect
/// from two beacons.
pub fn beacons_do_not_overlap(context: &Context, solutions: &[BeaconSolution]) -> FtResult<()> {
    if context.options.validate_solution && !context.options.overlap_beacons {
        for solution in solutions {
            let beacon_center_to_covered_centers = get_provider_center_to_covered_centers(
                context,
                context.options.beacon_width,
                context.options.beacon_height,
                context.options.beacon_supply_width,
                context.options.beacon_supply_height,
                solution.beacons.iter().copied(),
                true,
                false,
            );

            let covered_center_to_pole_centers =
                get_covered_center_to_provider_centers(context, &beacon_center_to_covered_centers);

            for (_, beacon_centers) in covered_center_to_pole_centers.iter() {
                if beacon_centers.len() > 1 {
                    return Err(FactorioToolsError::new(
                        "Multiple beacons are providing an effect to a pumpjack.",
                    ));
                }
            }
        }
    }

    Ok(())
}

/// `NoExistingBeacons`. `existing_beacons` maps beacon centers to entity ids.
pub fn no_existing_beacons(
    context: &Context,
    existing_beacons: &LocationDictionary<i32>,
) -> FtResult<()> {
    if context.options.validate_solution && !existing_beacons.is_empty() {
        return Err(FactorioToolsError::new(
            "There should not be any existing beacons.",
        ));
    }

    Ok(())
}

/// `NoOverlappingEntities`: the pipes, and each beacon solution in turn, can all be placed on
/// a copy of the grid. A clash surfaces as `AddEntity`'s "There is already an entity at ..."
/// error, which is how the C# reports it too.
pub fn no_overlapping_entities(
    context: &Context,
    optimized_pipes: &LocationSet,
    underground_pipes: Option<&LocationDictionary<Direction>>,
    beacon_solutions: Option<&[BeaconSolution]>,
) -> FtResult<()> {
    if context.options.validate_solution {
        match beacon_solutions {
            None => {
                let mut clone = context.grid.clone_grid();
                add_pipe_entities::execute_on_grid(
                    context,
                    &mut clone,
                    optimized_pipes,
                    underground_pipes,
                    false,
                )?;
            }
            Some(solutions) => {
                for solution in solutions {
                    let mut clone = context.grid.clone_grid();
                    add_pipe_entities::execute_on_grid(
                        context,
                        &mut clone,
                        optimized_pipes,
                        underground_pipes,
                        false,
                    )?;
                    add_beacons_to_grid(
                        &mut clone,
                        &context.options,
                        solution.beacons.iter().copied(),
                    )?;
                }
            }
        }
    }

    Ok(())
}

/// `CandidateCoversMoreEntities`: a pole candidate covers at least one entity that is not
/// covered yet.
pub fn candidate_covers_more_entities(
    context: &Context,
    powered_entities: &[ProviderRecipient],
    covered_entities: &CountedBitArray,
    candidate: Location,
    candidate_info: &ElectricPoleCandidateInfo,
) -> FtResult<()> {
    if context.options.validate_solution {
        let covered = &candidate_info.base.covered;
        let mut is_subset_of = true;
        let mut i = 0;
        while i < powered_entities.len() && is_subset_of {
            if covered.get(i as i32) {
                is_subset_of = covered_entities.get(i as i32);
            }
            i += 1;
        }

        if is_subset_of {
            return Err(FactorioToolsError::new(format!(
                "Candidate {candidate} should have been eliminated."
            )));
        }
    }

    Ok(())
}

/// `CountUnheatedTargets`: how many pumpjacks and pipe tiles have no heat pipe next to them.
/// Both are 0 unless heat pipes were routed. Returns `(unheated_pumpjacks, unheated_pipes)`.
pub fn count_unheated_targets(context: &Context) -> (i32, i32) {
    let mut unheated_pumpjacks = 0;
    let mut unheated_pipes = 0;

    if !context.options.add_heat_pipes || context.heat_pipes.is_none() {
        return (unheated_pumpjacks, unheated_pipes);
    }

    let grid = &context.grid;
    let is_heat_pipe = |location: Location| matches!(grid.get(location), Some(e) if matches!(e.kind, EntityKind::HeatPipe));

    for location in grid.entity_locations().iter() {
        if !grid.get(location).is_some_and(|e| e.kind.is_pipe()) {
            continue;
        }

        let heated = grid
            .get_adjacent(location)
            .into_iter()
            .any(|adjacent| adjacent.is_valid() && is_heat_pipe(adjacent));

        if !heated {
            unheated_pipes += 1;
        }
    }

    for &center in &context.centers {
        let heated = PUMPJACK_RING_OFFSETS.iter().any(|&offset| {
            let ring_location = center.translate_by(offset);
            grid.is_in_bounds(ring_location) && is_heat_pipe(ring_location)
        });

        if !heated {
            unheated_pumpjacks += 1;
        }
    }

    (unheated_pumpjacks, unheated_pipes)
}

/// `NoUnheatedBeacons`: on Aquilo, every beacon has a heat pipe next to one of its cells.
pub fn no_unheated_beacons(context: &Context) -> FtResult<()> {
    if !context.options.validate_solution
        || !context.options.add_heat_pipes
        || !context.options.add_beacons
        || context.heat_pipes.is_none()
    {
        return Ok(());
    }

    let grid = &context.grid;
    let width = context.options.beacon_width;
    let height = context.options.beacon_height;

    for location in grid.entity_locations().iter() {
        if !grid
            .get(location)
            .is_some_and(|e| matches!(e.kind, EntityKind::BeaconCenter))
        {
            continue;
        }

        let min_x = location.x - ((width - 1) / 2);
        let max_x = location.x + (width / 2);
        let min_y = location.y - ((height - 1) / 2);
        let max_y = location.y + (height / 2);

        let mut heated = false;
        let mut x = min_x;
        while x <= max_x && !heated {
            let mut y = min_y;
            while y <= max_y && !heated {
                for n in grid.get_adjacent(Location::new(x, y)) {
                    if n.is_valid()
                        && grid
                            .get(n)
                            .is_some_and(|e| matches!(e.kind, EntityKind::HeatPipe))
                    {
                        heated = true;
                        break;
                    }
                }
                y += 1;
            }
            x += 1;
        }

        if !heated {
            return Err(FactorioToolsError::new(
                "A beacon was left without an adjacent heat pipe on Aquilo, so it would freeze.",
            ));
        }
    }

    Ok(())
}

/// `HeatPipesAreConnected`: the heat pipes form one network.
pub fn heat_pipes_are_connected(context: &Context) -> FtResult<()> {
    let Some(heat_pipes) = &context.heat_pipes else {
        return Ok(());
    };

    if context.options.validate_solution && context.options.add_heat_pipes && !heat_pipes.is_empty()
    {
        let mut visited = context.get_location_set();
        let mut queue = VecDeque::new();
        let start = first(heat_pipes.iter())?;
        queue.push_back(start);
        visited.add(start);
        let mut reached = 1usize;

        while let Some(current) = queue.pop_front() {
            for next in context.grid.get_adjacent(current) {
                if next.is_valid() && heat_pipes.contains(next) && visited.add(next) {
                    reached += 1;
                    queue.push_back(next);
                }
            }
        }

        if reached != heat_pipes.len() {
            return Err(FactorioToolsError::new(
                "The heat pipe network is not fully connected, so heat from a single source would not reach all of it.",
            ));
        }
    }

    Ok(())
}

/// `AllEntitiesHavePower`: every pumpjack and beacon is covered by a pole.
pub fn all_entities_have_power(context: &Context) -> FtResult<()> {
    if context.options.validate_solution {
        let (powered_entities, _) = get_powered_entities(context);

        let electric_pole_centers: Vec<Location> = context
            .grid
            .entity_locations()
            .iter()
            .filter(|&location| {
                context
                    .grid
                    .get(location)
                    .is_some_and(|e| matches!(e.kind, EntityKind::ElectricPoleCenter { .. }))
            })
            .collect();

        get_electric_pole_coverage(context, &powered_entities, electric_pole_centers)?;
    }

    Ok(())
}
