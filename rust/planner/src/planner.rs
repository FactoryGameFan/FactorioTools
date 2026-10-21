//! `Planner`, from `src/FactorioTools/OilField/Planner.cs`: the whole pipeline, from a
//! blueprint to a planned grid and a summary of the plans tried.

use crate::collection_ext::single;
use crate::containers::LocationSet;
use crate::context::Context;
use crate::data::{
    Blueprint, Direction, Entity, Icon, Position, SignalId, entity_names, item_names, signal_types,
};
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::EntityKind;
use crate::models::{AvoidLocation, BeaconStrategy, OilFieldPlanSummary, PipeStrategy};
use crate::options::OilFieldOptions;
use crate::steps::{add_electric_poles, add_heat_pipes, add_pipes, initialize_context, validate};

/// `PlannerResult`.
#[derive(Debug)]
pub struct PlannerResult {
    pub context: Context,
    pub summary: OilFieldPlanSummary,
}

/// `EletricPolesMode` (sic). The variant names keep the C#'s.
#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ElectricPolesMode {
    AddLast,
    AddFirstAndAvoidSpecificTerminals,
    AddFirstAndAvoidAllTerminals,
}

/// `ExecuteSample`: the fixed four-pumpjack sample with every strategy.
pub fn execute_sample() -> FtResult<PlannerResult> {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.pipe_strategies = PipeStrategy::ALL.to_vec();
    options.beacon_strategies = BeaconStrategy::ALL.to_vec();
    options.validate_solution = true;

    execute(options, get_sample_blueprint())
}

/// `GetSampleBlueprint`.
pub fn get_sample_blueprint() -> Blueprint {
    let pumpjack = |entity_number: i32, direction: Option<Direction>, x: f32, y: f32| Entity {
        entity_number,
        name: entity_names::vanilla::PUMPJACK.to_string(),
        position: Position { x, y },
        direction,
        ..Entity::default()
    };

    Blueprint {
        entities: vec![
            pumpjack(1, None, -3.0, -5.0),
            pumpjack(2, Some(Direction::Down), 4.0, 5.0),
            pumpjack(3, Some(Direction::Right), 12.0, -2.0),
            pumpjack(4, Some(Direction::Down), -8.0, 7.0),
        ],
        icons: vec![Icon {
            index: 1,
            signal: SignalId {
                name: entity_names::vanilla::PUMPJACK.to_string(),
                signal_type: signal_types::vanilla::ITEM.to_string(),
            },
        }],
        item: item_names::vanilla::BLUEPRINT.to_string(),
        version: 0,
    }
}

/// `Execute(options, inputBlueprint)`.
pub fn execute(options: OilFieldOptions, blueprint: Blueprint) -> FtResult<PlannerResult> {
    execute_with_avoid(options, blueprint, &[])
}

/// `Execute(options, inputBlueprint, avoid)`.
pub fn execute_with_avoid(
    options: OilFieldOptions,
    blueprint: Blueprint,
    avoid: &[AvoidLocation],
) -> FtResult<PlannerResult> {
    execute_mode(
        &options,
        &blueprint,
        avoid,
        LocationSet::Empty,
        ElectricPolesMode::AddLast,
    )
}

fn execute_mode(
    options: &OilFieldOptions,
    blueprint: &Blueprint,
    avoid: &[AvoidLocation],
    mut electric_poles_avoid: LocationSet,
    electric_poles_mode: ElectricPolesMode,
) -> FtResult<PlannerResult> {
    let mut context = initialize_context::execute(options.clone(), blueprint.clone(), avoid)?;
    let initial_pumpjack_count = context.center_to_terminals.len() as i32;
    let add_electric_poles_first = electric_poles_mode != ElectricPolesMode::AddLast;

    if context.center_to_terminals.is_empty() {
        return Err(FactorioToolsError::with_bad_input(
            "The must be at least one pumpjack in the blueprint.",
            true,
        ));
    }

    if add_electric_poles_first {
        if electric_poles_mode == ElectricPolesMode::AddFirstAndAvoidAllTerminals {
            electric_poles_avoid = get_electric_poles_avoid(&context);
        }

        let poles = add_electric_poles::execute(&mut context, &electric_poles_avoid, false)?;

        if poles.is_none() {
            if electric_poles_mode == ElectricPolesMode::AddFirstAndAvoidAllTerminals {
                return Err(FactorioToolsError::with_bad_input(
                    "No valid placement for the electric poles could be found, while adding electric poles first. \
                     Try removing some pumpjacks or using a different electric pole.",
                    true,
                ));
            }

            return execute_mode(
                options,
                blueprint,
                avoid,
                LocationSet::Empty,
                ElectricPolesMode::AddFirstAndAvoidAllTerminals,
            );
        }

        // Remove terminals overlapping with the added poles.
        let centers: Vec<_> = context.center_to_terminals.keys().collect();
        for center in centers {
            let mut i = 0;
            while i < context.center_to_terminals.value(center).len() {
                let terminal = context.center_to_terminals.value(center)[i];
                if context.grid.is_empty(terminal.terminal) {
                    i += 1;
                    continue;
                }

                context.center_to_terminals.value_mut(center).remove(i);
                // `List.Remove` compares references.
                let at_location = context.location_to_terminals.value_mut(terminal.terminal);
                if let Some(index) = at_location.iter().position(|t| t.same_object(&terminal)) {
                    at_location.remove(index);
                }
            }
        }
    }

    let plans = add_pipes::execute(&mut context, add_electric_poles_first)?;

    // Heat pipes are the hard constraint, so they go before the electric poles, which only
    // need to be within supply range.
    add_heat_pipes::execute(&mut context)?;

    if options.add_electric_poles {
        if !add_electric_poles_first || context.options.add_beacons {
            let poles = add_electric_poles::execute(
                &mut context,
                &LocationSet::Empty,
                add_electric_poles_first,
            )?;
            if poles.is_none() {
                if add_electric_poles_first {
                    return Err(FactorioToolsError::with_bad_input(
                        "No valid placement for the electric poles could be found, after adding electric poles first. \
                         Try removing some pumpjacks or using a different electric pole.",
                        true,
                    ));
                }

                let electric_poles_avoid = get_electric_poles_avoid(&context);
                return execute_mode(
                    options,
                    blueprint,
                    avoid,
                    electric_poles_avoid,
                    ElectricPolesMode::AddFirstAndAvoidSpecificTerminals,
                );
            }
        }

        validate::all_entities_have_power(&context)?;
    }

    let missing_pumpjacks = initial_pumpjack_count
        - context.center_to_terminals.len() as i32
        - context.heat_dropped_pumpjacks;
    if missing_pumpjacks > 0 {
        return Err(FactorioToolsError::new(
            "The initial number of pumpjacks does not match the final pumpjack count.",
        ));
    }

    let directions: Vec<_> = context
        .center_to_terminals
        .iter()
        .map(|(center, terminals)| (center, terminals[0].direction))
        .collect();
    for (center, direction) in directions {
        match context.grid.get_mut(center).map(|entity| &mut entity.kind) {
            Some(EntityKind::PumpjackCenter { direction: d }) => *d = direction,
            _ => {
                return Err(FactorioToolsError::new(
                    "A pumpjack center entity was not at the expected location.",
                ));
            }
        }
    }

    let mut rotated_pumpjacks = 0;
    for (location, &original_direction) in context.center_to_original_direction.iter() {
        let final_direction = single(context.center_to_terminals.value(location).iter())?.direction;
        if original_direction != final_direction {
            rotated_pumpjacks += 1;
        }
    }

    let (unheated_pumpjacks, unheated_pipes) = validate::count_unheated_targets(&context);
    validate::heat_pipes_are_connected(&context)?;
    validate::no_unheated_beacons(&context)?;

    let summary = OilFieldPlanSummary {
        missing_pumpjacks,
        rotated_pumpjacks,
        heat_dropped_pumpjacks: context.heat_dropped_pumpjacks,
        unheated_pumpjacks,
        unheated_pipes,
        selected_plans: plans.selected_plans,
        alternate_plans: plans.alternate_plans,
        unused_plans: plans.unused_plans,
    };

    Ok(PlannerResult { context, summary })
}

/// `GetElectricPolesAvoid`: every terminal location.
fn get_electric_poles_avoid(context: &Context) -> LocationSet {
    let mut electric_poles_avoid = context.get_location_set_enumerable(true);
    for terminals in context.center_to_terminals.values() {
        for terminal in terminals {
            electric_poles_avoid.add(terminal.terminal);
        }
    }

    electric_poles_avoid
}
