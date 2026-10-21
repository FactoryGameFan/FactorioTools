//! `AddPipesConnectedCentersFLUTE`, from `OilField/Steps/AddPipes.4.ConnectedCenters.FLUTE.cs`:
//! the pumpjack graph the `ConnectedCentersFlute` strategy starts from.
//!
//! FLUTE builds a rectilinear Steiner tree over every pumpjack terminal. Two pumpjacks are
//! connected when one's terminals reach the other's through the tree, passing only Steiner
//! points or its own terminals on the way.
//!
//! FLUTE is by Dr. Chris C. N. Chu, Iowa State University,
//! <http://home.engineering.iastate.edu/~cnchu/>, used under his Attribution Assurance
//! License (see the `flute` crate).

use std::collections::VecDeque;

use flute::{Point, Tree};

use crate::Location;
use crate::containers::{LocationDictionary, LocationSet};
use crate::context::Context;
use crate::grid::TerminalLocation;
use crate::steps::initialize_flute;

/// `FlutePoint`: one node of the FLUTE tree.
///
/// The C# also has `IsEliminated` and `IsSteinerPoint`, which nothing reads.
struct FlutePoint {
    location: Location,
    /// The pumpjacks with a terminal at this point. Empty for a Steiner point.
    centers: LocationSet,
    /// `Terminals`: filled in, but never read.
    #[allow(dead_code)]
    terminals: Vec<TerminalLocation>,
    neighbors: LocationSet,
}

impl FlutePoint {
    fn new(context: &Context, location: Location) -> Self {
        FlutePoint {
            location,
            centers: context.get_location_set_enumerable(true),
            terminals: Vec::new(),
            neighbors: context.get_location_set_enumerable(true),
        }
    }
}

/// `Execute`.
pub fn execute(context: &Context) -> LocationDictionary<LocationSet> {
    let location_to_point = get_location_to_flute_point(context);

    // Determine which terminals should be connected to each other either directly or via
    // only Steiner points.
    let mut center_to_centers = context.get_location_dictionary();
    for (center, terminals) in context.center_to_terminals.iter() {
        let mut other_centers = context.get_location_set_enumerable(true);
        let mut visited_points = context.get_location_set();
        // The C# queues the `FlutePoint` objects; this queues their locations and looks each
        // one up when it comes off the queue.
        let mut queue = VecDeque::new();
        for terminal in terminals {
            queue.push_back(terminal.terminal);
        }

        while let Some(location) = queue.pop_front() {
            let point = location_to_point.value(location);

            if !visited_points.add(point.location) {
                continue;
            }

            if !point.centers.contains(center) && !point.centers.is_empty() {
                other_centers.union_with(&point.centers);
            } else {
                other_centers.union_with(&point.centers);

                for neighbor in point.neighbors.iter() {
                    queue.push_back(neighbor);
                }
            }
        }

        other_centers.remove(center);
        center_to_centers.add(center, other_centers);
    }

    center_to_centers
}

/// `GetLocationToFlutePoint`: the FLUTE tree as points with neighbor sets.
fn get_location_to_flute_point(context: &Context) -> LocationDictionary<FlutePoint> {
    let flute_tree = get_flute_tree(context);

    // Map the FLUTE tree into a more useful object graph.
    let mut location_to_point: LocationDictionary<FlutePoint> = context.get_location_dictionary();

    // Explore the branches. Each walk follows `N` until it reaches the root, whose `N` is
    // its own index.
    for branch in &flute_tree.branch {
        let mut current = *branch;
        loop {
            let next = flute_tree.branch[current.n as usize];

            let current_location = Location::new(current.x, current.y);
            let next_location = Location::new(next.x, next.y);
            get_or_add_point(context, &mut location_to_point, current_location);
            get_or_add_point(context, &mut location_to_point, next_location);

            location_to_point
                .value_mut(current_location)
                .neighbors
                .add(next_location);
            location_to_point
                .value_mut(next_location)
                .neighbors
                .add(current_location);

            if current.n == next.n {
                break;
            }

            current = next;
        }
    }

    // Remove self from neighbors.
    let locations: Vec<Location> = location_to_point.keys().collect();
    for location in locations {
        let point = location_to_point.value_mut(location);
        let own = point.location;
        point.neighbors.remove(own);
    }

    // Add in pumpjack information.
    for (center, terminals) in context.center_to_terminals.iter() {
        for terminal in terminals {
            let point = location_to_point.value_mut(terminal.terminal);
            point.terminals.push(*terminal);
            point.centers.add(center);
        }
    }

    location_to_point
}

/// The local `GetOrAddPoint` function.
fn get_or_add_point(
    context: &Context,
    location_to_point: &mut LocationDictionary<FlutePoint>,
    location: Location,
) {
    if !location_to_point.contains_key(location) {
        location_to_point.add(location, FlutePoint::new(context, location));
    }
}

/// `GetFluteTree`: FLUTE over every terminal, in `CenterToTerminals` order.
///
/// FluteSharp throws `OverflowException` for no points; the planner never asks for that (a
/// context has at least one pumpjack here), so the port panics.
fn get_flute_tree(context: &Context) -> Tree {
    let mut terminal_points = Vec::new();
    for terminals in context.center_to_terminals.values() {
        for terminal in terminals {
            terminal_points.push(Point::new(terminal.terminal.x, terminal.terminal.y));
        }
    }

    let flute = initialize_flute::execute(6);
    match flute.execute(&terminal_points) {
        Ok(tree) => tree,
        Err(error) => panic!("FLUTE: {error:?}"),
    }
}
