//! `AddPipeEntities`, from `src/FactorioTools/OilField/Steps/AddPipeEntities.cs`: puts a
//! plan's pipes on a grid. Underground pipes go first, then each pumpjack's terminal, then
//! the plain pipes, each kind skipping cells an earlier one already took. The order of the
//! entities, and so their ids, follows the .NET enumeration order of the inputs.

use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;
use crate::data::Direction;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity, SquareGrid, TerminalLocation};

/// `Execute(context, pipes, undergroundPipes)`: onto `context.grid`, one terminal per
/// pumpjack.
pub fn execute(
    context: &mut Context,
    pipes: &LocationSet,
    underground_pipes: Option<&LocationDictionary<Direction>>,
) -> FtResult<()> {
    let added_pipes = context.get_location_set();
    add(
        &context.center_to_terminals,
        &mut context.grid,
        added_pipes,
        pipes,
        underground_pipes,
        false,
    )
}

/// `Execute(context, grid, pipes, undergroundPipes, allowMultipleTerminals)`: onto another
/// grid, such as a clone. The set that tracks added cells is sized from `context.grid`, as
/// in the C#.
pub fn execute_on_grid(
    context: &Context,
    grid: &mut SquareGrid,
    pipes: &LocationSet,
    underground_pipes: Option<&LocationDictionary<Direction>>,
    allow_multiple_terminals: bool,
) -> FtResult<()> {
    add(
        &context.center_to_terminals,
        grid,
        context.get_location_set(),
        pipes,
        underground_pipes,
        allow_multiple_terminals,
    )
}

fn add(
    center_to_terminals: &LocationDictionary<Vec<TerminalLocation>>,
    grid: &mut SquareGrid,
    mut added_pipes: LocationSet,
    pipes: &LocationSet,
    underground_pipes: Option<&LocationDictionary<Direction>>,
    allow_multiple_terminals: bool,
) -> FtResult<()> {
    if let Some(underground_pipes) = underground_pipes {
        for (location, &direction) in underground_pipes.iter() {
            added_pipes.add(location);
            let id = grid.get_id();
            grid.add_entity(
                location,
                GridEntity::new(id, EntityKind::UndergroundPipe { direction }),
            )?;
        }
    }

    for terminals in center_to_terminals.values() {
        if terminals.len() != 1 && !allow_multiple_terminals {
            return Err(FactorioToolsError::new(
                "Every pumpjack should have a single terminal selected.",
            ));
        }

        for terminal in terminals {
            if added_pipes.add(terminal.terminal) {
                let id = grid.get_id();
                grid.add_entity(terminal.terminal, GridEntity::new(id, EntityKind::Terminal))?;
            }
        }
    }

    for pipe in pipes.iter() {
        if added_pipes.add(pipe) {
            let id = grid.get_id();
            grid.add_entity(pipe, GridEntity::new(id, EntityKind::Pipe))?;
        }
    }

    Ok(())
}
