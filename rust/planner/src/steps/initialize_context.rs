//! `InitializeContext`, from `src/FactorioTools/OilField/Steps/InitializeContext.cs`: turns a
//! blueprint into a grid with every pumpjack and avoided cell placed, and works out each
//! pumpjack's candidate terminals.
//!
//! The translation math is `float`, as in the C#: blueprint positions are `float`, and the
//! grid size and offsets are found with `float` arithmetic before `ToInt` checks that they
//! landed on whole numbers.

use dotnet_compat::net_sort;

use crate::Location;
use crate::collection_ext;
use crate::containers::LocationDictionary;
use crate::context::Context;
use crate::data::{Blueprint, Direction, Icon, SignalId, entity_names, item_names, signal_types};
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity, SquareGrid};
use crate::helpers::{
    PUMPJACK_HEIGHT, PUMPJACK_WIDTH, add_pumpjack, populate_center_to_terminals,
    populate_location_to_terminals, to_int,
};
use crate::models::AvoidLocation;
use crate::options::OilFieldOptions;

/// The most pumpjacks a blueprint may hold.
const MAX_PUMPJACKS: usize = 150;
const MAX_WIDTH: i32 = 1000;
const MAX_HEIGHT: i32 = 1000;
const MAX_AREA: i32 = 500 * 500;

/// `Execute(options, blueprint, avoid)`.
pub fn execute(
    options: OilFieldOptions,
    blueprint: Blueprint,
    avoid: &[AvoidLocation],
) -> FtResult<Context> {
    execute_with_min(options, blueprint, avoid, 0, 0)
}

/// `GetEmpty(options, width, height)`: a context for an empty blueprint, at least
/// `width` by `height`.
pub fn get_empty(options: OilFieldOptions, width: i32, height: i32) -> FtResult<Context> {
    let blueprint = Blueprint {
        entities: Vec::new(),
        icons: vec![Icon {
            index: 1,
            signal: SignalId {
                name: entity_names::vanilla::PUMPJACK.to_string(),
                signal_type: signal_types::vanilla::ITEM.to_string(),
            },
        }],
        item: item_names::vanilla::BLUEPRINT.to_string(),
        version: 0,
    };

    execute_with_min(options, blueprint, &[], width, height)
}

/// `Execute(options, blueprint, avoid, minWidth, minHeight)`.
pub fn execute_with_min(
    options: OilFieldOptions,
    blueprint: Blueprint,
    avoid: &[AvoidLocation],
    min_width: i32,
    min_height: i32,
) -> FtResult<Context> {
    let translated = translate_locations(&options, &blueprint, avoid, min_width, min_height)?;

    let grid = initialize_grid(
        &translated.center_and_original_directions,
        &translated.avoid_locations,
        translated.width,
        translated.height,
    )?;

    let mut centers: Vec<Location> = translated
        .center_and_original_directions
        .iter()
        .map(|(center, _)| *center)
        .collect();
    net_sort(&mut centers, |a, b| a.y.cmp(&b.y).then(a.x.cmp(&b.x)));

    let count = translated.center_and_original_directions.len();
    let mut center_to_original_direction = LocationDictionary::with_capacity(count);
    let mut center_to_terminals = LocationDictionary::with_capacity(count);
    let mut location_to_terminals = LocationDictionary::new();

    for &(center, direction) in &translated.center_and_original_directions {
        center_to_original_direction.add(center, direction);
    }
    populate_center_to_terminals(&mut center_to_terminals, &grid, &centers)?;
    populate_location_to_terminals(&mut location_to_terminals, &center_to_terminals);

    let location_to_adjacent_count = get_location_to_adjacent_count(&grid);

    let pole_level = options.electric_pole_quality.level();
    let electric_pole_supply_width_with_quality =
        options.electric_pole_supply_width + 2 * pole_level;
    let electric_pole_supply_height_with_quality =
        options.electric_pole_supply_height + 2 * pole_level;
    let electric_pole_wire_reach_with_quality =
        options.electric_pole_wire_reach + f64::from(2 * pole_level);

    Ok(Context {
        options,
        input_blueprint: blueprint,
        delta_x: translated.delta_x,
        delta_y: translated.delta_y,
        grid,
        centers,
        center_to_terminals,
        center_to_original_direction,
        location_to_terminals,
        location_to_adjacent_count,
        electric_pole_supply_width_with_quality,
        electric_pole_supply_height_with_quality,
        electric_pole_wire_reach_with_quality,
        electric_pole_wire_reach_squared_with_quality: electric_pole_wire_reach_with_quality
            * electric_pole_wire_reach_with_quality,
        heat_pipes: None,
        heat_dropped_pumpjacks: 0,
    })
}

/// `GetLocationToAdjacentCount`: for each cell, how many pumpjack side cells border it.
fn get_location_to_adjacent_count(grid: &SquareGrid) -> Vec<i32> {
    let mut counts = vec![0; (grid.width() * grid.height()) as usize];

    for location in grid.entity_locations().iter() {
        let is_side = matches!(
            grid.get(location).map(|e| &e.kind),
            Some(EntityKind::PumpjackSide { .. })
        );
        if !is_side {
            continue;
        }

        for neighbor in grid.get_adjacent(location) {
            if !neighbor.is_valid() {
                continue;
            }

            counts[(neighbor.y * grid.width() + neighbor.x) as usize] += 1;
        }
    }

    counts
}

/// `TranslatedLocations`.
struct TranslatedLocations {
    center_and_original_directions: Vec<(Location, Direction)>,
    avoid_locations: Vec<Location>,
    delta_x: f32,
    delta_y: f32,
    width: i32,
    height: i32,
}

/// `Math.Min(float, float)`: NaN wins, and of two equal values a negative zero wins.
fn net_min_f32(a: f32, b: f32) -> f32 {
    if a != b {
        if a.is_nan() {
            return a;
        }
        return if b < a { b } else { a };
    }
    if a.is_sign_negative() { a } else { b }
}

/// `Math.Max(float, float)`: NaN wins, and of two equal values a positive zero wins.
fn net_max_f32(a: f32, b: f32) -> f32 {
    if a != b {
        if a.is_nan() {
            return a;
        }
        return if b < a { a } else { b };
    }
    if a.is_sign_negative() { b } else { a }
}

/// `TranslateLocations`: the grid size, and the offset that moves every blueprint position
/// onto it, leaving room for beacons (when enabled), a pipe and an electric pole on every
/// side.
fn translate_locations(
    options: &OilFieldOptions,
    blueprint: &Blueprint,
    avoid: &[AvoidLocation],
    min_width: i32,
    min_height: i32,
) -> FtResult<TranslatedLocations> {
    let pumpjacks: Vec<_> = blueprint
        .entities
        .iter()
        .filter(|e| e.name == entity_names::vanilla::PUMPJACK)
        .collect();

    if pumpjacks.len() > MAX_PUMPJACKS {
        return Err(FactorioToolsError::new(format!(
            "Having more than {MAX_PUMPJACKS} pumpjacks is not supported. There are {} pumpjacks provided.",
            pumpjacks.len()
        )));
    }

    let mut center_and_original_directions = Vec::with_capacity(pumpjacks.len());
    let mut avoid_locations = Vec::with_capacity(avoid.len());

    let mut delta_x: f32 = 0.0;
    let mut delta_y: f32 = 0.0;

    if pumpjacks.is_empty() && avoid.is_empty() {
        return Ok(TranslatedLocations {
            center_and_original_directions,
            avoid_locations,
            delta_x,
            delta_y,
            width: min_width,
            height: min_height,
        });
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    if !pumpjacks.is_empty() {
        let pumpjack_offset_x = (PUMPJACK_WIDTH / 2) as f32;
        let pumpjack_offset_y = (PUMPJACK_HEIGHT / 2) as f32;

        min_x = collection_ext::min(pumpjacks.iter().map(|p| p.position.x)).unwrap_or(0.0)
            - pumpjack_offset_x;
        min_y = collection_ext::min(pumpjacks.iter().map(|p| p.position.y)).unwrap_or(0.0)
            - pumpjack_offset_y;
        max_x = collection_ext::max(pumpjacks.iter().map(|p| p.position.x)).unwrap_or(0.0)
            + pumpjack_offset_x;
        max_y = collection_ext::max(pumpjacks.iter().map(|p| p.position.y)).unwrap_or(0.0)
            + pumpjack_offset_y;

        if options.add_beacons {
            // Leave room around the pumpjacks for beacons.
            let beacon_offset_x =
                ((options.beacon_supply_width - 1) / 2 + options.beacon_width / 2) as f32;
            let beacon_offset_y =
                ((options.beacon_supply_height - 1) / 2 + options.beacon_height / 2) as f32;
            min_x -= beacon_offset_x;
            min_y -= beacon_offset_y;
            max_x += beacon_offset_x;
            max_y += beacon_offset_y;
        }
    }

    if !avoid.is_empty() {
        min_x = net_min_f32(
            min_x,
            collection_ext::min(avoid.iter().map(|a| a.x)).unwrap_or(0.0),
        );
        min_y = net_min_f32(
            min_y,
            collection_ext::min(avoid.iter().map(|a| a.y)).unwrap_or(0.0),
        );
        max_x = net_max_f32(
            max_x,
            collection_ext::max(avoid.iter().map(|a| a.x)).unwrap_or(0.0),
        );
        max_y = net_max_f32(
            max_y,
            collection_ext::max(avoid.iter().map(|a| a.y)).unwrap_or(0.0),
        );
    }

    // Leave space on every side for a pipe and an electric pole.
    min_x -= (1 + options.electric_pole_width) as f32;
    min_y -= (1 + options.electric_pole_height) as f32;
    max_x += (1 + options.electric_pole_width) as f32;
    max_y += (1 + options.electric_pole_height) as f32;

    delta_x = -min_x;
    delta_y = -min_y;

    // The C# adds with unchecked `int` arithmetic, which wraps.
    let mut width = to_int(max_x - min_x, || {
        "The grid width is not an integer after translation.".to_string()
    })?
    .wrapping_add(1);
    let mut height = to_int(max_y - min_y, || {
        "The grid height is not an integer after translation.".to_string()
    })?
    .wrapping_add(1);

    if min_width > width {
        delta_x += ((min_width - width) / 2) as f32;
        width = min_width;
    }

    if min_height > height {
        delta_y += ((min_height - height) / 2) as f32;
        height = min_height;
    }

    for p in &pumpjacks {
        let x = to_int(p.position.x + delta_x, || {
            format!(
                "Entity {} (a '{}') does not have an integer X value after translation.",
                p.entity_number, p.name
            )
        })?;
        let y = to_int(p.position.y + delta_y, || {
            format!(
                "Entity {} (a '{}') does not have an integer Y value after translation.",
                p.entity_number, p.name
            )
        })?;
        center_and_original_directions
            .push((Location::new(x, y), p.direction.unwrap_or(Direction::Up)));
    }

    for (i, a) in avoid.iter().enumerate() {
        let x = to_int(a.x + delta_x, || {
            format!("Avoided location {i} does not have an integer X value after translation.")
        })?;
        let y = to_int(a.y + delta_y, || {
            format!("Avoided location {i} does not have an integer Y value after translation.")
        })?;
        avoid_locations.push(Location::new(x, y));
    }

    Ok(TranslatedLocations {
        center_and_original_directions,
        avoid_locations,
        delta_x,
        delta_y,
        width,
        height,
    })
}

/// `InitializeGrid`: a `PipeGrid` with every pumpjack placed (in blueprint order), then
/// every avoided cell.
fn initialize_grid(
    center_and_original_directions: &[(Location, Direction)],
    avoid_locations: &[Location],
    width: i32,
    height: i32,
) -> FtResult<SquareGrid> {
    // Unchecked `int` multiplication in the C#, which wraps.
    let area = width.wrapping_mul(height);
    if width > MAX_WIDTH || height > MAX_HEIGHT || area > MAX_AREA {
        return Err(FactorioToolsError::new(format!(
            "The planning grid cannot be larger than {MAX_WIDTH} x {MAX_HEIGHT} or an area larger than {MAX_AREA}. \
             The planning grid for the provided options is {width} x {height} with an area of {area}."
        )));
    }

    let mut grid = SquareGrid::new(width, height);

    for &(center, _) in center_and_original_directions {
        add_pumpjack(&mut grid, center)?;
    }

    for (i, &avoid_location) in avoid_locations.iter().enumerate() {
        let occupied_by_other = grid
            .get(avoid_location)
            .is_some_and(|e| !matches!(e.kind, EntityKind::AvoidEntity));
        if occupied_by_other {
            return Err(FactorioToolsError::new(format!(
                "Avoided location {i} has another entity already placed there (perhaps it's part of a pumpjack spot)."
            )));
        }

        let id = grid.get_id();
        grid.add_entity(avoid_location, GridEntity::new(id, EntityKind::AvoidEntity))?;
    }

    Ok(grid)
}
