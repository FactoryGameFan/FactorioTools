//! `Helpers`, from `src/FactorioTools/OilField/Helpers.cs`.
//!
//! Stage 3 ports only what `InitializeContext` calls. Stage 4 of #138 adds the rest.

use crate::Location;
use crate::containers::LocationDictionary;
use crate::data::Direction;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity, SquareGrid, TerminalLocation};

/// `PumpjackWidth`.
pub const PUMPJACK_WIDTH: i32 = 3;

/// `PumpjackHeight`.
pub const PUMPJACK_HEIGHT: i32 = 3;

/// `TerminalOffsets`: where the pipe goes for each pumpjack direction, relative to the
/// pumpjack's center. Measured from Factorio 2.1.14 (issue #81); see the C# comment for the
/// picture and the history.
pub const TERMINAL_OFFSETS: [(Direction, Location); 4] = [
    (Direction::Up, Location::new(1, -2)),
    (Direction::Right, Location::new(2, 1)),
    (Direction::Down, Location::new(-1, 2)),
    (Direction::Left, Location::new(-2, -1)),
];

/// `AddPumpjack`: a center and its eight sides, added column by column (x outer, y inner).
/// The center takes the first id and each side the next, in that order. Returns the
/// center's id, where the C# returns the `PumpjackCenter`.
pub fn add_pumpjack(grid: &mut SquareGrid, center: Location) -> FtResult<i32> {
    let center_id = grid.get_id();
    for x in -1..=1 {
        for y in -1..=1 {
            let entity = if x != 0 || y != 0 {
                GridEntity::new(
                    grid.get_id(),
                    EntityKind::PumpjackSide { center: center_id },
                )
            } else {
                GridEntity::pumpjack_center(center_id)
            };
            grid.add_entity(Location::new(center.x + x, center.y + y), entity)?;
        }
    }

    Ok(center_id)
}

/// `PopulateCenterToTerminals`: each center's candidate terminals, in `TerminalOffsets`
/// order, keeping a spot that is empty or already a pipe.
pub fn populate_center_to_terminals(
    center_to_terminals: &mut LocationDictionary<Vec<TerminalLocation>>,
    grid: &SquareGrid,
    centers: &[Location],
) -> FtResult<()> {
    for &center in centers {
        let mut candidate_terminals = Vec::new();
        for (direction, translation) in TERMINAL_OFFSETS {
            let location = center.translate_by(translation);
            let terminal = TerminalLocation::new(center, location, direction);
            match grid.get(location) {
                None => candidate_terminals.push(terminal),
                Some(existing) if existing.kind.is_pipe() => candidate_terminals.push(terminal),
                Some(_) => {}
            }
        }

        if candidate_terminals.is_empty() {
            return Err(FactorioToolsError::with_bad_input(
                "At least one pumpjack has no room for a pipe connection. Try removing some pumpjacks.",
                true,
            ));
        }

        center_to_terminals.add(center, candidate_terminals);
    }

    Ok(())
}

/// `PopulateLocationToTerminals`: the terminals grouped by the cell they occupy, walking
/// `centerToTerminals` in .NET's order.
pub fn populate_location_to_terminals(
    location_to_terminals: &mut LocationDictionary<Vec<TerminalLocation>>,
    center_to_terminals: &LocationDictionary<Vec<TerminalLocation>>,
) {
    for terminals in center_to_terminals.values() {
        for terminal in terminals {
            match location_to_terminals.get_mut(terminal.terminal) {
                Some(list) => list.push(*terminal),
                None => {
                    let mut list = Vec::with_capacity(2);
                    list.push(*terminal);
                    location_to_terminals.add(terminal.terminal, list);
                }
            }
        }
    }
}

/// `float.Epsilon * 100`: 100 times the smallest positive `float`, a subnormal that the C#
/// compiler folds exactly.
const TO_INT_TOLERANCE: f32 = f32::from_bits(100);

/// `ToInt`: the float as an `int`, or `error` if it has a fractional part.
///
/// The C# rounds with `Math.Round(double, 0)` (half to even) after widening to `double`,
/// then casts. The `as i32` cast saturates out-of-range values, as .NET 9 and later do.
pub fn to_int(x: f32, error: impl FnOnce() -> String) -> FtResult<i32> {
    if (x % 1.0).abs() > TO_INT_TOLERANCE {
        return Err(FactorioToolsError::new(error()));
    }

    Ok(f64::from(x).round_ties_even() as i32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_int_accepts_whole_floats_only() {
        assert_eq!(to_int(7.0, || "bad".into()), Ok(7));
        assert_eq!(to_int(-3.0, || "bad".into()), Ok(-3));
        assert_eq!(to_int(0.5, || "bad".into()).unwrap_err().message, "bad");
    }

    #[test]
    fn add_pumpjack_numbers_the_center_first() {
        let mut grid = SquareGrid::new(5, 5);
        let center = add_pumpjack(&mut grid, Location::new(2, 2)).unwrap();
        assert_eq!(center, 1);
        assert_eq!(grid.get(Location::new(1, 1)).unwrap().id, 2);
        assert_eq!(grid.get(Location::new(2, 2)).unwrap().id, 1);
        assert_eq!(grid.get(Location::new(3, 3)).unwrap().id, 9);
    }
}
