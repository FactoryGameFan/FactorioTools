//! `Helpers`, from `src/FactorioTools/OilField/Helpers.cs`.
//!
//! Stage 3 ported what `InitializeContext` calls, and stage 4 of #138 the rest. The provider
//! half (beacon and pole candidates and coverage) is in [`crate::providers`] and re-exported
//! here, so `helpers::` reaches every C# `Helpers` member.

use delaunator_net::{Delaunator, DelaunatorError};
use dotnet_compat::net_sort;

use crate::Location;
use crate::containers::LocationDictionary;
use crate::context::Context;
use crate::data::Direction;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity, SquareGrid, TerminalLocation};
use crate::models::Endpoints;

pub use crate::providers::*;

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

/// `RemovePumpjack`: clears the pumpjack's 3x3 cells, column by column.
pub fn remove_pumpjack(grid: &mut SquareGrid, center: Location) {
    for x in -1..=1 {
        for y in -1..=1 {
            grid.remove_entity(Location::new(center.x + x, center.y + y));
        }
    }
}

/// `GetCenterToTerminals`.
pub fn get_center_to_terminals(
    context: &Context,
    grid: &SquareGrid,
    centers: &[Location],
) -> FtResult<LocationDictionary<Vec<TerminalLocation>>> {
    let mut center_to_terminals = context.get_location_dictionary();
    populate_center_to_terminals(&mut center_to_terminals, grid, centers)?;
    Ok(center_to_terminals)
}

/// `GetLocationToTerminals`.
pub fn get_location_to_terminals(
    context: &Context,
    center_to_terminals: &LocationDictionary<Vec<TerminalLocation>>,
) -> LocationDictionary<Vec<TerminalLocation>> {
    let mut location_to_terminals = context.get_location_dictionary();
    populate_location_to_terminals(&mut location_to_terminals, center_to_terminals);
    location_to_terminals
}

/// `EliminateOtherTerminals`: keeps only `selected_terminal` for its pumpjack, and drops the
/// pumpjack's other terminals from `location_to_terminals`.
///
/// The C# `TerminalLocation` is a class compared by reference. Here it is compared by value.
/// The two agree when `selected_terminal` is an entry of the pumpjack's list, which is how
/// every caller gets it; a separate but equal object would be removed in the C# and kept here.
pub fn eliminate_other_terminals(context: &mut Context, selected_terminal: TerminalLocation) {
    let terminal_options = context
        .center_to_terminals
        .value(selected_terminal.center)
        .clone();

    if terminal_options.len() == 1 {
        return;
    }

    for other_terminal in &terminal_options {
        if *other_terminal == selected_terminal {
            continue;
        }

        let terminals = context
            .location_to_terminals
            .value_mut(other_terminal.terminal);
        if terminals.len() == 1 {
            context
                .location_to_terminals
                .remove(other_terminal.terminal);
        } else if let Some(index) = terminals.iter().position(|t| t == other_terminal) {
            // `List.Remove`: the first equal item.
            terminals.remove(index);
        }
    }

    let terminal_options = context
        .center_to_terminals
        .value_mut(selected_terminal.center);
    terminal_options.clear();
    terminal_options.push(selected_terminal);
}

/// `GetPath`: the path from `reached_goal` back to the start, goal first.
pub fn get_path(
    came_from: &LocationDictionary<Location>,
    start: Location,
    reached_goal: Location,
) -> Vec<Location> {
    let size_estimate = 2 * start.manhattan_distance(reached_goal);
    let mut path = Vec::with_capacity(size_estimate.max(0) as usize);
    add_path(came_from, reached_goal, &mut path);
    path
}

/// `AddPath`: appends the path from `reached_goal` back to the cell that came from itself.
pub fn add_path(
    came_from: &LocationDictionary<Location>,
    reached_goal: Location,
    output_list: &mut Vec<Location>,
) {
    let mut current = reached_goal;
    loop {
        let next = *came_from.value(current);
        output_list.push(current);
        if next == current {
            break;
        }

        current = next;
    }
}

/// `AreLocationsCollinear`: whether every step between consecutive locations has the same
/// slope, compared as `double` the way the C# does (so a repeated location gives `NaN`, which
/// never equals itself). An empty list is not collinear; a single location is.
pub fn are_locations_collinear(locations: &[Location]) -> bool {
    let mut last_slope = 0.0f64;
    for i in 0..locations.len() {
        if i == locations.len() - 1 {
            return true;
        }

        let node = locations[i];
        let next = locations[i + 1];
        let dx = f64::from((node.x - next.x).abs());
        let dy = f64::from((node.y - next.y).abs());
        if i == 0 {
            last_slope = dy / dx;
        } else if last_slope != dy / dx {
            break;
        }
    }

    false
}

/// `CountTurns`: how many times the path switches between horizontal and vertical steps.
pub fn count_turns(path: &[Location]) -> i32 {
    let mut previous_direction = -1;
    let mut turns = 0;
    for i in 1..path.len() {
        let current_direction = if path[i].x == path[i - 1].x { 0 } else { 1 };
        if previous_direction != -1 && previous_direction != current_direction {
            turns += 1;
        }

        previous_direction = current_direction;
    }

    turns
}

/// The C#'s `ArgumentException` message for two points that are not in a line.
const NOT_IN_A_LINE: &str = "ArgumentException: The two points must be one the same line either horizontally or vertically.";

/// `MakeStraightLineOnEmpty`: the cells from `a` to `b`, lowest first, or `None` if any is
/// occupied. Panics, as the C# throws `ArgumentException`, if `a` and `b` share neither
/// a column nor a row.
pub fn make_straight_line_on_empty(
    grid: &SquareGrid,
    a: Location,
    b: Location,
) -> Option<Vec<Location>> {
    if a.x == b.x {
        let (min, max) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
        let mut line = Vec::with_capacity((max - min + 1) as usize);
        for y in min..=max {
            if !grid.is_empty(Location::new(a.x, y)) {
                return None;
            }

            line.push(Location::new(a.x, y));
        }

        return Some(line);
    }

    if a.y == b.y {
        let (min, max) = if a.x < b.x { (a.x, b.x) } else { (b.x, a.x) };
        let mut line = Vec::with_capacity((max - min + 1) as usize);
        for x in min..=max {
            if !grid.is_empty(Location::new(x, a.y)) {
                return None;
            }

            line.push(Location::new(x, a.y));
        }

        return Some(line);
    }

    panic!("{NOT_IN_A_LINE}");
}

/// `MakeStraightLine`: the cells from `a` to `b`, lowest first. Panics, as the C# throws
/// `ArgumentException`, if `a` and `b` share neither a column nor a row.
pub fn make_straight_line(a: Location, b: Location) -> Vec<Location> {
    if a.x == b.x {
        let (min, max) = if a.y < b.y { (a.y, b.y) } else { (b.y, a.y) };
        return (min..=max).map(|y| Location::new(a.x, y)).collect();
    }

    if a.y == b.y {
        let (min, max) = if a.x < b.x { (a.x, b.x) } else { (b.x, a.x) };
        return (min..=max).map(|x| Location::new(x, a.y)).collect();
    }

    panic!("{NOT_IN_A_LINE}");
}

/// `PointsToLines(IReadOnlyCollection<Location>)`: [`points_to_lines_sorted`] with sorting.
pub fn points_to_lines(nodes: &[Location]) -> Result<Vec<Endpoints>, DelaunatorError> {
    points_to_lines_sorted(nodes, true)
}

/// `PointsToLines(IReadOnlyList<Location>, bool sort)`: the lines between neighboring points,
/// from a Delaunay triangulation, after sorting by X then Y if asked. One or two points give
/// one line; collinear points give the line between each consecutive pair. Fails as the C#
/// Delaunator throws: with no points, or with points that are all identical.
///
/// Source: https://github.com/teoxoy/factorio-blueprint-editor/blob/21ab873d8316a41b9a05c719697d461d3ede095d/packages/editor/src/core/generators/util.ts#L62
pub fn points_to_lines_sorted(
    nodes: &[Location],
    sort: bool,
) -> Result<Vec<Endpoints>, DelaunatorError> {
    let mut sorted_xy;
    let filtered_nodes: &[Location] = if sort {
        sorted_xy = nodes.to_vec();
        net_sort(&mut sorted_xy, |a, b| a.x.cmp(&b.x).then(a.y.cmp(&b.y)));
        &sorted_xy
    } else {
        nodes
    };

    if filtered_nodes.len() == 1 {
        return Ok(vec![Endpoints::new(filtered_nodes[0], filtered_nodes[0])]);
    } else if filtered_nodes.len() == 2 {
        return Ok(vec![Endpoints::new(filtered_nodes[0], filtered_nodes[1])]);
    }

    // Check that the nodes are not collinear.
    if are_locations_collinear(filtered_nodes) {
        return Ok(filtered_nodes
            .windows(2)
            .map(|pair| Endpoints::new(pair[0], pair[1]))
            .collect());
    }

    let points: Vec<(f64, f64)> = filtered_nodes
        .iter()
        .map(|node| (f64::from(node.x), f64::from(node.y)))
        .collect();
    let delaunator = Delaunator::new(&points)?;

    let mut lines = Vec::new();
    for e in 0..delaunator.triangles.len() {
        if e as i32 > delaunator.halfedges[e] {
            let p = filtered_nodes[delaunator.triangles[e] as usize];
            let q = filtered_nodes
                [delaunator.triangles[if e % 3 == 2 { e - 2 } else { e + 1 }] as usize];
            lines.push(Endpoints::new(p, q));
        }
    }

    Ok(lines)
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
