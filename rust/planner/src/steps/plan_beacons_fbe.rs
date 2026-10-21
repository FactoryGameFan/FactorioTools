//! `PlanBeaconsFbe`, from `src/FactorioTools/OilField/Steps/PlanBeacons.1.FBE.cs`: the beacon
//! planner from Teoxoy's Factorio Blueprint Editor, as modified in the C# (`FbeOriginal` and
//! `Fbe`).
//!
//! Every container the result depends on keeps the C# order: the possible beacon areas are
//! made in `context.Centers` order and then column by column, which sets each candidate's
//! `OriginalIndex`, and both sorts end on that index, so their order is total. The entity
//! areas are made in `EntityLocations` order, but an area's index is only a bit position in
//! `EffectsGiven`, so that order cannot change the result.

use std::cmp::Ordering;

use dotnet_compat::{NetCompare, net_sort};

use crate::Location;
use crate::containers::{CountedBitArray, LocationDictionary, LocationSet};
use crate::context::Context;
use crate::grid::EntityKind;
use crate::helpers::{PUMPJACK_HEIGHT, PUMPJACK_WIDTH};
use crate::models::BeaconStrategy;
use crate::steps::plan_beacons::BeaconPlannerResult;

/// `MinAffectedEntities`.
const MIN_AFFECTED_ENTITIES: i32 = 1;

/// `Area`: an entity's cells. `index` is its position among the areas that receive effects,
/// or -1.
struct Area {
    index: i32,
    effect: bool,
    locations: Vec<Location>,
}

/// `BeaconCandidate`.
struct BeaconCandidate {
    original_index: i32,
    center: Location,
    collision_area: Vec<Location>,
    effects_given_count: i32,
    average_distance_to_entities: f64,
    number_of_overlaps: i32,
    effects_given: Option<CountedBitArray>,
}

/// `PlanBeaconsFbe.Execute(context, strategy)`.
pub fn execute(context: &Context, strategy: BeaconStrategy) -> BeaconPlannerResult {
    let entity_areas = get_entity_areas(context);
    let occupied_positions = get_occupied_positions(context, &entity_areas);
    let possible_beacon_areas = get_possible_beacon_areas(context, &occupied_positions);
    let point_to_beacon_count = get_point_to_beacon_count(context, &possible_beacon_areas);
    let effect_entity_areas = get_effect_entity_areas(entity_areas);
    let point_to_entity_area = get_point_to_entity_area(context, &effect_entity_areas);

    let mut possible_beacons = get_possible_beacons(
        context,
        &effect_entity_areas,
        possible_beacon_areas,
        &point_to_beacon_count,
        &point_to_entity_area,
    );

    if strategy == BeaconStrategy::Fbe {
        sort_possible_beacons(context, &mut possible_beacons);
    } else if strategy == BeaconStrategy::FbeOriginal {
        sort_possible_beacons_original(&mut possible_beacons);
    }

    get_beacons(context, effect_entity_areas.len() as i32, possible_beacons)
}

/// `GetBeacons`: takes candidates from the end of the sorted list while they fit.
fn get_beacons(
    context: &Context,
    effect_entity_area_count: i32,
    mut possible_beacons: Vec<BeaconCandidate>,
) -> BeaconPlannerResult {
    let mut beacons = Vec::new();
    let mut effects_given_counts = Vec::new();
    let mut effects = 0;
    let mut collision_area = context.get_location_set();
    let mut covered_entity_areas = if context.options.overlap_beacons {
        None
    } else {
        Some(CountedBitArray::new(effect_entity_area_count))
    };

    while let Some(beacon) = possible_beacons.pop() {
        if collision_area.overlaps(beacon.collision_area.iter().copied()) {
            continue;
        }

        if let Some(covered_entity_areas) = covered_entity_areas.as_mut() {
            let effects_given = beacon
                .effects_given
                .as_ref()
                .expect("EffectsGiven is set when beacons do not overlap");
            let mut overlapping = effects_given.clone();
            overlapping.and(covered_entity_areas);

            if overlapping.true_count() > 0 {
                continue;
            }

            covered_entity_areas.or(effects_given);

            if covered_entity_areas.true_count() == covered_entity_areas.count() {
                break;
            }
        }

        beacons.push(beacon.center);
        effects += beacon.effects_given_count;
        effects_given_counts.push(beacon.effects_given_count);
        collision_area.union_with_items(beacon.collision_area.iter().copied());
    }

    BeaconPlannerResult {
        beacons,
        effects,
        effects_given_counts,
    }
}

/// `SortPossibleBeacons` (the `Fbe` strategy): ascending by effects given, so the best is
/// last. Ties go to fewer overlaps (or, for a single effect, a larger average distance),
/// then to the candidate closer to the grid middle, then to the earlier candidate.
fn sort_possible_beacons(context: &Context, possible_beacons: &mut [BeaconCandidate]) {
    // The C# caches each center's distance in a dictionary; computing it again gives the
    // same value.
    let middle = context.grid.middle();
    net_sort(possible_beacons, |a, b| {
        let c = a.effects_given_count.cmp(&b.effects_given_count);
        if c != Ordering::Equal {
            return c;
        }

        let a_c = if a.effects_given_count == 1 {
            -a.average_distance_to_entities
        } else {
            f64::from(a.number_of_overlaps)
        };
        let b_c = if b.effects_given_count == 1 {
            -b.average_distance_to_entities
        } else {
            f64::from(b.number_of_overlaps)
        };
        let c = b_c.net_cmp(&a_c);
        if c != Ordering::Equal {
            return c;
        }

        let a_c = a.center.euclidean_distance(middle);
        let b_c = b.center.euclidean_distance(middle);
        let c = a_c.net_cmp(&b_c);
        if c != Ordering::Equal {
            return c;
        }

        b.original_index.cmp(&a.original_index)
    });
}

/// `SortPossibleBeaconsOriginal` (the `FbeOriginal` strategy). Not exactly FBE's comparer,
/// which is inconsistent; see the C# comment.
fn sort_possible_beacons_original(possible_beacons: &mut [BeaconCandidate]) {
    net_sort(possible_beacons, |a, b| {
        let c = a.effects_given_count.cmp(&b.effects_given_count);
        if c != Ordering::Equal {
            return c;
        }

        let c = b.number_of_overlaps.cmp(&a.number_of_overlaps);
        if c != Ordering::Equal {
            return c;
        }

        let c = a
            .average_distance_to_entities
            .net_cmp(&b.average_distance_to_entities);
        if c != Ordering::Equal {
            return c;
        }

        a.original_index.cmp(&b.original_index)
    });
}

/// `GetPossibleBeacons`.
fn get_possible_beacons(
    context: &Context,
    effect_entity_areas: &[Area],
    possible_beacon_areas: Vec<Vec<Location>>,
    point_to_beacon_count: &LocationDictionary<i32>,
    point_to_entity_area: &LocationDictionary<usize>,
) -> Vec<BeaconCandidate> {
    let (entity_min_x, entity_min_y, entity_max_x, entity_max_y) =
        get_bounds(point_to_entity_area.keys());

    let o = &context.options;
    let center_x = (o.beacon_width - 1) / 2;
    let center_y = (o.beacon_height - 1) / 2;
    let center_index = (center_y * o.beacon_width + center_x) as usize;

    let mut possible_beacons = Vec::with_capacity(possible_beacon_areas.len());
    let mut effects_given = CountedBitArray::new(effect_entity_areas.len() as i32);
    for collision_area in possible_beacon_areas {
        let center = collision_area[center_index];

        let min_x = entity_min_x.max(center.x - (o.beacon_supply_width / 2));
        let min_y = entity_min_y.max(center.y - (o.beacon_supply_height / 2));
        let max_x = entity_max_x.min(center.x + (o.beacon_supply_width / 2));
        let max_y = entity_max_y.min(center.y + (o.beacon_supply_height / 2));

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if let Some(&area) = point_to_entity_area.get(Location::new(x, y)) {
                    effects_given.set(effect_entity_areas[area].index, true);
                }
            }
        }

        if effects_given.true_count() < MIN_AFFECTED_ENTITIES {
            effects_given.set_all(false);
            continue;
        }

        let mut sum_distance = 0;
        for (j, area) in effect_entity_areas.iter().enumerate() {
            if effects_given.get(j as i32) {
                // The C# indexes the pumpjack's cells with the beacon's center index, so this
                // is not always the pumpjack's center cell.
                sum_distance += area.locations[center_index].manhattan_distance(center);
            }
        }

        let average_distance_to_entities =
            f64::from(sum_distance) / f64::from(effects_given.true_count());

        let mut number_of_overlaps = 0;
        for location in &collision_area {
            number_of_overlaps += *point_to_beacon_count.value(*location);
        }

        let original_index = possible_beacons.len() as i32;
        possible_beacons.push(BeaconCandidate {
            original_index,
            center,
            collision_area,
            effects_given_count: effects_given.true_count(),
            average_distance_to_entities,
            number_of_overlaps,
            effects_given: if o.overlap_beacons {
                None
            } else {
                Some(effects_given.clone())
            },
        });

        effects_given.set_all(false);
    }

    possible_beacons
}

/// `GetBounds`: the smallest and largest X and Y. With no locations the bounds stay at
/// `int.MaxValue` and `int.MinValue`, as in the C#.
fn get_bounds(locations: impl Iterator<Item = Location>) -> (i32, i32, i32, i32) {
    let mut entity_min_x = i32::MAX;
    let mut entity_min_y = i32::MAX;
    let mut entity_max_x = i32::MIN;
    let mut entity_max_y = i32::MIN;

    for location in locations {
        if location.x < entity_min_x {
            entity_min_x = location.x;
        }

        if location.y < entity_min_y {
            entity_min_y = location.y;
        }

        if location.x > entity_max_x {
            entity_max_x = location.x;
        }

        if location.y > entity_max_y {
            entity_max_y = location.y;
        }
    }

    (entity_min_x, entity_min_y, entity_max_x, entity_max_y)
}

/// `GetOccupiedPositions`.
fn get_occupied_positions(context: &Context, entity_areas: &[Area]) -> LocationSet {
    let mut occupied_positions = context.get_location_set();
    for area in entity_areas {
        for location in &area.locations {
            occupied_positions.add(*location);
        }
    }

    occupied_positions
}

/// `GetEntityAreas`: each entity's cells, in `EntityLocations` order. Side cells are skipped;
/// their centers carry the whole area.
fn get_entity_areas(context: &Context) -> Vec<Area> {
    let mut areas = Vec::with_capacity(context.grid.entity_locations().len());

    for location in context.grid.entity_locations().iter() {
        let entity = context
            .grid
            .get(location)
            .expect("an entity location holds an entity");

        let (width, height, effect) = match &entity.kind {
            EntityKind::TemporaryEntity | EntityKind::AvoidEntity => (1, 1, false),
            EntityKind::ElectricPoleCenter { .. } => (
                context.options.electric_pole_width,
                context.options.electric_pole_height,
                false,
            ),
            EntityKind::PumpjackCenter { .. } => (PUMPJACK_WIDTH, PUMPJACK_HEIGHT, true),
            EntityKind::BeaconCenter
            | EntityKind::BeaconSide { .. }
            | EntityKind::ElectricPoleSide { .. }
            | EntityKind::PumpjackSide { .. } => continue,
            other => panic!("NotImplementedException: {other:?} in PlanBeaconsFbe.GetEntityAreas"),
        };

        let min_x = location.x - ((width - 1) / 2);
        let max_x = location.x + (width / 2);
        let min_y = location.y - ((height - 1) / 2);
        let max_y = location.y + (height / 2);

        let mut area = Vec::with_capacity((width * height) as usize);
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                area.push(Location::new(x, y));
            }
        }

        let index = areas.len() as i32;
        areas.push(Area {
            index,
            effect,
            locations: area,
        });
    }

    areas
}

/// `GetEffectEntityAreas`: the areas that receive effects, re-indexed from 0.
fn get_effect_entity_areas(entity_areas: Vec<Area>) -> Vec<Area> {
    let mut effect_entity_areas = Vec::new();
    for mut area in entity_areas {
        if area.effect {
            area.index = effect_entity_areas.len() as i32;
            effect_entity_areas.push(area);
        }
    }

    effect_entity_areas
}

/// `GetPossibleBeaconAreas`: every beacon spot within supply range of a pumpjack whose cells
/// are all free, in `Centers` order and column by column, each listed once.
fn get_possible_beacon_areas(
    context: &Context,
    occupied_positions: &LocationSet,
) -> Vec<Vec<Location>> {
    let o = &context.options;
    let mut valid_beacon_centers = context.get_location_set();
    let mut possible_beacon_areas = Vec::new();

    let grid_min_x = (o.beacon_width - 1) / 2;
    let grid_min_y = (o.beacon_height - 1) / 2;
    let grid_max_x = context.grid.width() - (o.beacon_width / 2) - 1;
    let grid_max_y = context.grid.height() - (o.beacon_height / 2) - 1;

    let supply_left = ((PUMPJACK_WIDTH - 1) / 2) + (o.beacon_supply_width / 2);
    let supply_up = ((PUMPJACK_HEIGHT - 1) / 2) + (o.beacon_supply_height / 2);
    let supply_right = (PUMPJACK_WIDTH / 2) + ((o.beacon_supply_width - 1) / 2);
    let supply_down = (PUMPJACK_HEIGHT / 2) + ((o.beacon_supply_height - 1) / 2);

    let beacon_left = (o.beacon_width - 1) / 2;
    let beacon_up = (o.beacon_height - 1) / 2;
    let beacon_right = o.beacon_width / 2;
    let beacon_down = o.beacon_height / 2;

    let area_size = (o.beacon_width * o.beacon_height) as usize;

    for center in &context.centers {
        let supply_min_x = grid_min_x.max(center.x - supply_left);
        let supply_min_y = grid_min_y.max(center.y - supply_up);
        let supply_max_x = grid_max_x.min(center.x + supply_right);
        let supply_max_y = grid_max_y.min(center.y + supply_down);

        for center_x in supply_min_x..=supply_max_x {
            for center_y in supply_min_y..=supply_max_y {
                let beacon_center = Location::new(center_x, center_y);
                if !valid_beacon_centers.add(beacon_center) {
                    continue;
                }

                let min_x = beacon_center.x - beacon_left;
                let min_y = beacon_center.y - beacon_up;
                let max_x = beacon_center.x + beacon_right;
                let max_y = beacon_center.y + beacon_down;

                // Row by row (y outer, x inner), stopping at the first occupied cell.
                let mut area = Vec::with_capacity(area_size);
                let mut fits = true;
                'rows: for y in min_y..=max_y {
                    for x in min_x..=max_x {
                        let location = Location::new(x, y);
                        if occupied_positions.contains(location) {
                            fits = false;
                            break 'rows;
                        }
                        area.push(location);
                    }
                }

                if fits {
                    possible_beacon_areas.push(area);
                }
            }
        }
    }

    possible_beacon_areas
}

/// `GetPointToBeaconCount`: how many possible beacon areas cover each cell.
fn get_point_to_beacon_count(
    context: &Context,
    possible_beacon_areas: &[Vec<Location>],
) -> LocationDictionary<i32> {
    let mut point_to_beacon_count = context.get_location_dictionary();
    for areas in possible_beacon_areas {
        for &point in areas {
            match point_to_beacon_count.get_mut(point) {
                None => point_to_beacon_count.add(point, 1),
                Some(sum) => *sum += 1,
            }
        }
    }

    point_to_beacon_count
}

/// `GetPointToEntityArea`: each cell of an effect area, mapped to that area's position in
/// `effect_entity_areas`.
fn get_point_to_entity_area(
    context: &Context,
    effect_entity_areas: &[Area],
) -> LocationDictionary<usize> {
    let mut point_to_entity_area = context.get_location_dictionary();
    for (i, area) in effect_entity_areas.iter().enumerate() {
        for &location in &area.locations {
            point_to_entity_area.add(location, i);
        }
    }

    point_to_entity_area
}
