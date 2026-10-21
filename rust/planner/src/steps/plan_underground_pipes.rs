//! `PlanUndergroundPipes`, from `src/FactorioTools/OilField/Steps/PlanUndergroundPipes.cs`:
//! turns straight runs of plain pipe into pairs of underground pipes.
//!
//! The candidate cells are sorted before runs are formed. Both C# comparisons order by one
//! axis and then the other, and the cells in a set are distinct, so the order is total and
//! std's sort gives the same list as .NET's introsort. What does depend on .NET is the pipe
//! set itself: the cells removed from it change where later additions land, so `pipes` must
//! be the .NET-ordered `LocationSet` the caller passed in.

use crate::Location;
use crate::collection_ext;
use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;
use crate::data::Direction;
use crate::error::FtResult;
use crate::steps::validate;

/// The ends each take one cell. A run shorter than 3 would use as many underground pipes as
/// plain ones, so it is not worth converting.
const MIN_UNDERGROUND: usize = 3;

/// The longest underground pipe, ends included (9 cells between the ends, per the wiki).
/// Public only so `tests/factorio_oracle.rs` can check it against the game.
pub const MAX_UNDERGROUND: usize = 11;

/// `Execute(context, pipes)`: the underground pipes and the direction each end faces. The
/// cells between each pair of ends are removed from `pipes`.
pub fn execute(
    context: &Context,
    pipes: &mut LocationSet,
) -> FtResult<LocationDictionary<Direction>> {
    let mut location_to_direction = context.get_location_dictionary();

    convert_in_one_direction(
        context,
        pipes,
        &mut location_to_direction,
        Location::new(0, 1),
    )?;
    convert_in_one_direction(
        context,
        pipes,
        &mut location_to_direction,
        Location::new(1, 0),
    )?;

    validate::underground_pipes_are_pipes(context, pipes, &location_to_direction)?;

    Ok(location_to_direction)
}

/// `ConvertInOneDirection`.
fn convert_in_one_direction(
    context: &Context,
    pipes: &mut LocationSet,
    location_to_direction: &mut LocationDictionary<Direction>,
    forward: Location,
) -> FtResult<()> {
    let (forward_direction, backward_direction) = match (forward.x, forward.y) {
        (1, 0) => (Direction::Right, Direction::Left),
        (0, 1) => (Direction::Down, Direction::Up),
        _ => panic!("NotImplementedException: forward must be (1, 0) or (0, 1)"),
    };

    // Candidates are pipes with another pipe ahead or behind them on this axis and none
    // beside them.
    let mut candidates = context.get_location_set_enumerable(true);
    let backward = Location::new(-forward.x, -forward.y);
    let right = Location::new(forward.y, forward.x);
    let left = Location::new(-right.x, -right.y);

    for goal in pipes.iter() {
        if (pipes.contains(goal.translate_by(forward))
            || pipes.contains(goal.translate_by(backward)))
            && !pipes.contains(goal.translate_by(right))
            && !pipes.contains(goal.translate_by(left))
        {
            if let Some(terminals) = context.location_to_terminals.get(goal) {
                if terminals.len() > 1 {
                    continue;
                }

                let direction = collection_ext::single(terminals.iter())?.direction;
                if direction != forward_direction && direction != backward_direction {
                    continue;
                }
            }

            candidates.add(goal);
        }
    }

    if candidates.is_empty() {
        return Ok(());
    }

    let mut sorted = candidates.to_vec();
    if forward.x == 1 {
        sorted.sort_by(|a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));
    } else {
        sorted.sort_by(|a, b| a.x.cmp(&b.x).then(a.y.cmp(&b.y)));
    }

    let mut current_run = vec![sorted[0]];
    for &current in &sorted[1..] {
        let previous = current_run[current_run.len() - 1];
        if previous.x + forward.x == current.x
            && previous.y + forward.y == current.y
            && current_run.len() < MAX_UNDERGROUND
        {
            current_run.push(current);
            continue;
        }

        add_run_and_clear(
            pipes,
            location_to_direction,
            forward_direction,
            backward_direction,
            &mut current_run,
        );

        current_run.push(current);
    }

    add_run_and_clear(
        pipes,
        location_to_direction,
        forward_direction,
        backward_direction,
        &mut current_run,
    );

    Ok(())
}

/// `AddRunAndClear`: a long enough run becomes two underground ends, and the cells between
/// them leave the pipe set.
fn add_run_and_clear(
    pipes: &mut LocationSet,
    location_to_direction: &mut LocationDictionary<Direction>,
    forward_direction: Direction,
    backward_direction: Direction,
    current_run: &mut Vec<Location>,
) {
    if current_run.len() >= MIN_UNDERGROUND {
        location_to_direction.add(current_run[0], backward_direction);
        location_to_direction.add(current_run[current_run.len() - 1], forward_direction);

        for &location in &current_run[1..current_run.len() - 1] {
            pipes.remove(location);
        }
    }

    current_run.clear();
}
