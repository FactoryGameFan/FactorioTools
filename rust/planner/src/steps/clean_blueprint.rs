//! `CleanBlueprint`, from `src/FactorioTools/OilField/Steps/CleanBlueprint.cs`: a blueprint
//! reduced to its pumpjacks, each turned to the first direction (Up, then clockwise) whose
//! terminal does not overlap another pumpjack. The contract's `Normalize` runs it.

use crate::collection_ext::min_by;
use crate::data::{
    Blueprint, Entity, Icon, Position, SignalId, entity_names, item_names, signal_types,
};
use crate::error::FtResult;
use crate::options::OilFieldOptions;
use crate::steps::initialize_context;

/// `CleanBlueprint.Execute`. The version is left at 0: it is stamped when the blueprint is
/// serialized, as in the C#.
pub fn execute(blueprint: Blueprint) -> FtResult<Blueprint> {
    let context = initialize_context::execute(OilFieldOptions::default(), blueprint, &[])?;
    let mut entities = Vec::with_capacity(context.centers.len());

    for &center in &context.centers {
        // `MinBy(x => x.Direction)`: the first terminal with the smallest direction value.
        let terminal = min_by(context.center_to_terminals.value(center).iter(), |t| {
            t.direction.value()
        })
        .expect("every pumpjack has at least one terminal");

        entities.push(Entity {
            entity_number: entities.len() as i32 + 1,
            direction: Some(terminal.direction),
            name: entity_names::vanilla::PUMPJACK.to_string(),
            position: Position {
                x: center.x as f32,
                y: center.y as f32,
            },
            ..Entity::default()
        });
    }

    Ok(Blueprint {
        entities,
        icons: vec![Icon {
            index: 1,
            signal: SignalId {
                name: entity_names::vanilla::PUMPJACK.to_string(),
                signal_type: signal_types::vanilla::ITEM.to_string(),
            },
        }],
        item: item_names::vanilla::BLUEPRINT.to_string(),
        version: 0,
    })
}
