//! `PlanBeacons`, from `src/FactorioTools/OilField/Steps/PlanBeacons.0.cs`: runs each beacon
//! strategy once on a grid that temporarily holds the pipes (and heat pipes).

use crate::Location;
use crate::containers::{CountedBitArray, LocationSet};
use crate::context::Context;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity};
use crate::models::{BeaconSolution, BeaconStrategy};
use crate::steps::{plan_beacons_fbe, plan_beacons_snug, validate};

/// `BeaconPlannerResult`: one strategy's beacons, in placement order, the total effects, and
/// each beacon's effect count.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BeaconPlannerResult {
    pub beacons: Vec<Location>,
    pub effects: i32,
    pub effects_given_counts: Vec<i32>,
}

/// `PlanBeacons.Execute(context, pipes, heatPipes)`.
///
/// The pipes and heat pipes go on the grid as `TemporaryEntity`s, in their enumeration
/// order, so the strategies route around them, and are removed again afterwards. That uses
/// up entity ids and, through the entity set's free list, changes the order `EntityLocations`
/// lists the remaining entities in, exactly as the C# does. As in the C#, an error from a
/// strategy leaves the temporary entities on the grid.
pub fn execute(
    context: &mut Context,
    pipes: &LocationSet,
    heat_pipes: Option<&LocationSet>,
) -> FtResult<Vec<BeaconSolution>> {
    for pipe in pipes.iter() {
        let id = context.grid.get_id();
        context
            .grid
            .add_entity(pipe, GridEntity::new(id, EntityKind::TemporaryEntity))?;
    }

    // On Aquilo the heat network is routed first, and the contested cells next to pipes and
    // pumpjacks are reserved for it, so the beacon strategies route around the heat pipes.
    if let Some(heat_pipes) = heat_pipes {
        for heat_pipe in heat_pipes.iter() {
            let id = context.grid.get_id();
            context
                .grid
                .add_entity(heat_pipe, GridEntity::new(id, EntityKind::TemporaryEntity))?;
        }
    }

    let mut solutions = Vec::with_capacity(context.options.beacon_strategies.len());

    let mut completed_strategies = CountedBitArray::new(BeaconStrategy::Snug.value() + 1);
    let strategies = context.options.beacon_strategies.clone();
    for strategy in strategies {
        if completed_strategies.get(strategy.value()) {
            continue;
        }

        let result = match strategy {
            BeaconStrategy::FbeOriginal | BeaconStrategy::Fbe => {
                plan_beacons_fbe::execute(context, strategy)
            }
            BeaconStrategy::Snug => plan_beacons_snug::execute(context)?,
        };

        completed_strategies.set(strategy.value(), true);

        solutions.push(BeaconSolution {
            strategy,
            beacons: result.beacons,
            effects: result.effects,
            effects_given_counts: result.effects_given_counts,
        });
    }

    for pipe in pipes.iter() {
        context.grid.remove_entity(pipe);
    }

    if let Some(heat_pipes) = heat_pipes {
        for heat_pipe in heat_pipes.iter() {
            context.grid.remove_entity(heat_pipe);
        }
    }

    if solutions.is_empty() {
        return Err(FactorioToolsError::new(
            "At least one beacon strategy must be used.",
        ));
    }

    validate::beacons_do_not_overlap(context, &solutions)?;

    Ok(solutions)
}
