//! `AddElectricPoles`, from `src/FactorioTools/OilField/Steps/AddElectricPoles.cs`.
//!
//! Places electric poles so every pumpjack and beacon has power and the poles form one wired
//! network: a greedy cover over pole candidates (with retry strategies when the cover fails),
//! then bridging poles between disconnected groups, then removing poles the cover no longer
//! needs.
//!
//! The C# holds `ElectricPoleCenter` objects; here poles are their entity ids, resolved
//! through the grid (stage 1's design). Candidate infos are shared `Rc<RefCell<..>>`s, as in
//! the stage 4 provider helpers, and the batch a candidate is picked from is reached through
//! [`Scoped`] the way the C# passes `coveredCountBatches`' own batch as
//! `scopedCandidateToInfo`. Every collection the C# enumerates keeps .NET's order.

use std::cmp::Ordering;
use std::collections::VecDeque;

use dotnet_compat::{NetCompare, NetHashSet};

use crate::Location;
use crate::algorithms::bresenhams_line;
use crate::collection_ext;
use crate::containers::{CountedBitArray, LocationDictionary, LocationSet, SortedBatches};
use crate::context::Context;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity, SquareGrid};
use crate::helpers::{
    self, Scoped, SharedInfo, add_provider_and_allow_multiple_providers, add_provider_to_grid,
    does_provider_fit, get_covered_to_candidates, get_electric_pole_candidate_to_covered,
    get_electric_pole_coverage, get_entity_distance, get_powered_entities, is_provider_in_bounds,
    remove_entity,
};
use crate::models::{CandidateFactory, ElectricPoleCandidateInfo, Endpoints, ProviderRecipient};
use crate::steps::validate;

/// `RetryStrategy`. The numbers matter: the loop counts down through them.
const RETRY_NONE: i32 = 0;
const RETRY_REMOVE_UNPOWERED_ENTITIES: i32 = 1;
const RETRY_REMOVE_UNPOWERED_BEACONS: i32 = 2;
const RETRY_PREFER_UNCOVERED_ENTITIES: i32 = 3;

/// `Execute(context, avoid, allowRetries)`: places the poles and returns their centers, or
/// `None` when no placement covers everything (which the planner answers by retrying with
/// poles first).
///
/// `avoid` cells that are empty are blocked with temporary entities while placing, and freed
/// again on success (not on failure, as in the C#). The returned set is
/// `electricPoles.Keys.ToReadOnlySet(context)`: a bit set, or a single or empty set.
pub fn execute(
    context: &mut Context,
    avoid: &LocationSet,
    allow_retries: bool,
) -> FtResult<Option<LocationSet>> {
    let mut avoid_entities: Option<LocationSet> = None;
    if !avoid.is_empty() {
        let mut entities = context.get_location_set_enumerable(true);
        for location in avoid.iter() {
            if context.grid.is_empty(location) && entities.add(location) {
                let id = context.grid.get_id();
                context
                    .grid
                    .add_entity(location, GridEntity::new(id, EntityKind::TemporaryEntity))?;
            }
        }
        avoid_entities = Some(entities);
    }

    let (electric_pole_list, powered_entities) =
        add_electric_poles_around_entities_with_retries(context, allow_retries)?;
    let Some(electric_pole_list) = electric_pole_list else {
        return Ok(None);
    };

    // Center to pole entity id.
    let mut electric_poles: LocationDictionary<i32> = context.get_location_dictionary();
    for center in electric_pole_list {
        let existing = context
            .grid
            .get(center)
            .and_then(|entity| match entity.kind {
                EntityKind::ElectricPoleCenter { .. } => Some(entity.id),
                _ => None,
            });
        match existing {
            None => {
                add_electric_pole(context, &mut electric_poles, center)?;
            }
            Some(center_id) => {
                connect_existing_electric_poles(context, &electric_poles, center, center_id);
                electric_poles.add(center, center_id);
            }
        }
    }

    connect_electric_poles(context, &mut electric_poles)?;

    remove_extra_electric_poles(context, &powered_entities, &mut electric_poles)?;

    if let Some(avoid_entities) = avoid_entities
        && !avoid_entities.is_empty()
    {
        for terminal in avoid_entities.iter() {
            context.grid.remove_entity(terminal);
        }
    }

    Ok(Some(context.get_read_only_location_set(
        electric_poles.keys().collect::<Vec<_>>(),
        false,
    )))
}

/// `RemoveExtraElectricPoles`: drops poles whose recipients other poles also cover, as long
/// as the network stays connected without them.
fn remove_extra_electric_poles(
    context: &mut Context,
    powered_entities: &[ProviderRecipient],
    electric_poles: &mut LocationDictionary<i32>,
) -> FtResult<()> {
    let (mut pole_center_to_covered_centers, mut covered_center_to_pole_centers) =
        get_electric_pole_coverage(context, powered_entities, electric_poles.keys())?;

    let mut remove_candidates = context.get_location_set_enumerable(true);

    for (_, poles) in covered_center_to_pole_centers.iter() {
        // Poles covering a recipient that at least two other poles also cover.
        if poles.len() > 2 {
            remove_candidates.union_with(poles);
        }
    }

    for (center, covered) in pole_center_to_covered_centers.iter() {
        if covered.is_empty() {
            remove_candidates.add(center);
        }
    }

    for (_, poles) in covered_center_to_pole_centers.iter() {
        // A recipient with one pole keeps that pole.
        if poles.len() == 1 {
            remove_candidates.except_with(poles);
        }
    }

    while !remove_candidates.is_empty() {
        let center = collection_ext::first(remove_candidates.iter())?;
        let center_id = *electric_poles.value(center);
        if are_poles_connected_without(&context.grid, electric_poles, center_id) {
            electric_poles.remove(center);
            remove_entity(
                &mut context.grid,
                center,
                context.options.electric_pole_width,
                context.options.electric_pole_height,
            );

            for covered_center in pole_center_to_covered_centers.value(center).iter() {
                let pole_centers = covered_center_to_pole_centers.value_mut(covered_center);
                pole_centers.remove(center);
                if pole_centers.len() == 1 {
                    remove_candidates.except_with(pole_centers);
                }
            }

            pole_center_to_covered_centers.remove(center);
        }

        remove_candidates.remove(center);
    }

    Ok(())
}

/// `ArePolesConnectedWithout`: whether the other poles still form one network without
/// `except`.
fn are_poles_connected_without(
    grid: &SquareGrid,
    electric_poles: &LocationDictionary<i32>,
    except: i32,
) -> bool {
    let mut queue = VecDeque::new();
    if let Some(&center) = electric_poles.values().find(|&&id| id != except) {
        queue.push_back(center);
    }
    let mut discovered = NetHashSet::new();

    while let Some(current) = queue.pop_front() {
        if current == except {
            continue;
        }

        if discovered.add(current) {
            for id in grid.neighbors(current).iter() {
                queue.push_back(id);
            }
        }
    }

    discovered.len() == electric_poles.len().wrapping_sub(1)
}

/// `ConnectExistingElectricPoles`: wires a pole to every pole already placed within reach, in
/// placement order.
fn connect_existing_electric_poles(
    context: &mut Context,
    electric_poles: &LocationDictionary<i32>,
    center: Location,
    center_id: i32,
) {
    for (other, other_id) in electric_poles.iter() {
        if center == other {
            continue;
        }

        if are_electric_poles_connected(center, other, context) {
            context.grid.add_neighbor(center_id, *other_id);
        }
    }
}

/// `AreElectricPolesConnected`: whether two pole centers are within wire reach.
pub fn are_electric_poles_connected(a: Location, b: Location, context: &Context) -> bool {
    f64::from(get_electric_pole_distance_squared(a, b, context))
        <= context.electric_pole_wire_reach_squared_with_quality
}

/// `GetElectricPoleDistanceSquared`.
fn get_electric_pole_distance_squared(a: Location, b: Location, context: &Context) -> i32 {
    let offset_x = (context.options.electric_pole_width - 1) / 2;
    let offset_y = (context.options.electric_pole_height - 1) / 2;

    b.euclidean_distance_squared_to_int(a.x + offset_x, a.y + offset_y)
}

/// The outer `AddElectricPolesAroundEntities(context, allowRetries)`: tries the cover, and
/// when it fails and retries are allowed, first prefers the entities left uncovered, then
/// drops unpowered beacons, then drops every unpowered entity.
fn add_electric_poles_around_entities_with_retries(
    context: &mut Context,
    allow_retries: bool,
) -> FtResult<(Option<Vec<Location>>, Vec<ProviderRecipient>)> {
    let mut retry_strategy = if allow_retries {
        RETRY_PREFER_UNCOVERED_ENTITIES
    } else {
        RETRY_NONE
    };
    let mut entities_to_power_first: Option<CountedBitArray> = None;

    loop {
        let (mut powered_entities, has_beacons) = get_powered_entities(context);
        let (electric_pole_list, mut covered_entities) = add_electric_poles_around_entities(
            context,
            &powered_entities,
            entities_to_power_first.as_ref(),
        )?;

        if retry_strategy == RETRY_NONE || electric_pole_list.is_some() {
            return Ok((electric_pole_list, powered_entities));
        }

        if retry_strategy == RETRY_PREFER_UNCOVERED_ENTITIES {
            covered_entities.not();
            entities_to_power_first = Some(covered_entities);
        } else if retry_strategy == RETRY_REMOVE_UNPOWERED_BEACONS {
            let mut centers_to_power_first = context.get_location_set();
            for i in (0..powered_entities.len()).rev() {
                let entity = powered_entities[i];
                if !covered_entities.get(i as i32) {
                    if context
                        .grid
                        .get(entity.center)
                        .is_some_and(|e| matches!(e.kind, EntityKind::BeaconCenter))
                    {
                        powered_entities.remove(i);
                        remove_entity(
                            &mut context.grid,
                            entity.center,
                            entity.width,
                            entity.height,
                        );
                    } else {
                        centers_to_power_first.add(entity.center);
                    }
                }
            }

            if centers_to_power_first.is_empty() {
                entities_to_power_first = None;
            } else {
                let mut first = CountedBitArray::new(powered_entities.len() as i32);
                let mut i = 0;
                while !centers_to_power_first.is_empty() && i < powered_entities.len() {
                    if centers_to_power_first.remove(powered_entities[i].center) {
                        first.set(i as i32, true);
                    }
                    i += 1;
                }
                entities_to_power_first = Some(first);
            }
        } else if retry_strategy == RETRY_REMOVE_UNPOWERED_ENTITIES {
            for i in (0..powered_entities.len()).rev() {
                let entity = powered_entities[i];
                if !covered_entities.get(i as i32) {
                    // The C# also checks for a beacon under RemoveUnpoweredBeacons here, which
                    // cannot be the strategy in this branch.
                    powered_entities.remove(i);
                    remove_entity(
                        &mut context.grid,
                        entity.center,
                        entity.width,
                        entity.height,
                    );
                }
            }

            entities_to_power_first = None;
        }

        retry_strategy -= 1;
        if retry_strategy == RETRY_REMOVE_UNPOWERED_BEACONS && !has_beacons {
            retry_strategy -= 1;
        }
    }
}

/// `CandidateFactory`: makes `ElectricPoleCandidateInfo`s.
struct PoleCandidateFactory;

impl CandidateFactory<ElectricPoleCandidateInfo> for PoleCandidateFactory {
    fn create(&self, covered: CountedBitArray) -> ElectricPoleCandidateInfo {
        ElectricPoleCandidateInfo::new(covered)
    }
}

/// Which `SortedBatches` in `allSubsets` a scoped dictionary belongs to.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Subsets {
    /// The batches by `PriorityPowered`, used on the prefer-uncovered retry.
    Priority,
    /// `coveredCountBatches`, the batches by how many entities a candidate covers.
    CoveredCount,
}

/// The two candidate orders. `entitiesToPowerFirst` picks which.
#[derive(Clone, Copy)]
enum Sorter {
    /// `CandidateComparerForSameCoveredCount`.
    SameCoveredCount,
    /// `CandidateComparerForSamePriorityPowered`.
    SamePriorityPowered,
}

impl Sorter {
    fn compare(self, x: &ElectricPoleCandidateInfo, y: &ElectricPoleCandidateInfo) -> Ordering {
        match self {
            Sorter::SameCoveredCount => compare_without_priority_powered(x, y),
            Sorter::SamePriorityPowered => {
                let c = y
                    .base
                    .covered
                    .true_count()
                    .cmp(&x.base.covered.true_count());
                if c != Ordering::Equal {
                    return c;
                }

                compare_without_priority_powered(x, y)
            }
        }
    }
}

/// `CandidateComparerForSameCoveredCount.CompareWithoutPriorityPowered`: prefer candidates
/// wired to the fewest (but some) other poles, then the nearest pole, then the shortest total
/// distance to what they cover, then the nearest to the grid's middle.
fn compare_without_priority_powered(
    x: &ElectricPoleCandidateInfo,
    y: &ElectricPoleCandidateInfo,
) -> Ordering {
    let xi = if x.others_connected > 0 {
        x.others_connected
    } else {
        i32::MAX
    };
    let yi = if y.others_connected > 0 {
        y.others_connected
    } else {
        i32::MAX
    };
    let c = xi.cmp(&yi);
    if c != Ordering::Equal {
        return c;
    }

    let xi = if x.others_connected > 0 {
        0
    } else {
        x.pole_distance
    };
    let yi = if y.others_connected > 0 {
        0
    } else {
        y.pole_distance
    };
    let c = xi.cmp(&yi);
    if c != Ordering::Equal {
        return c;
    }

    let c = x.base.entity_distance.net_cmp(&y.base.entity_distance);
    if c != Ordering::Equal {
        return c;
    }

    x.middle_distance.cmp(&y.middle_distance)
}

/// Groups candidates into batches keyed by `key`, in first-seen key order, as the C# fills a
/// `Dictionary<int, ...>` that only ever gains keys.
fn batches_by(
    all_candidate_to_info: &LocationDictionary<SharedInfo<ElectricPoleCandidateInfo>>,
    key: impl Fn(&ElectricPoleCandidateInfo) -> i32,
) -> Vec<(
    i32,
    LocationDictionary<SharedInfo<ElectricPoleCandidateInfo>>,
)> {
    let mut batches: Vec<(
        i32,
        LocationDictionary<SharedInfo<ElectricPoleCandidateInfo>>,
    )> = Vec::new();
    for (candidate, info) in all_candidate_to_info.iter() {
        let k = key(&info.borrow());
        let batch = match batches.iter().position(|(existing, _)| *existing == k) {
            Some(index) => &mut batches[index].1,
            None => {
                batches.push((k, LocationDictionary::new()));
                &mut batches.last_mut().expect("just pushed").1
            }
        };
        batch.add(candidate, SharedInfo::clone(info));
    }
    batches
}

/// The inner `AddElectricPolesAroundEntities(context, poweredEntities, entitiesToPowerFirst)`:
/// the greedy cover. Returns the pole centers (existing poles first) or `None` when no
/// candidate is left, and which entities ended up covered.
fn add_electric_poles_around_entities(
    context: &mut Context,
    powered_entities: &[ProviderRecipient],
    entities_to_power_first: Option<&CountedBitArray>,
) -> FtResult<(Option<Vec<Location>>, CountedBitArray)> {
    let candidates = get_electric_pole_candidate_to_covered(
        context,
        powered_entities,
        &PoleCandidateFactory,
        true,
    );
    let mut all_candidate_to_info = candidates.candidate_to_info;
    let mut covered_entities = candidates.covered_entities;

    let mut electric_pole_list: Vec<Location> = candidates.providers.keys().collect();

    populate_candidate_to_info(
        context,
        &all_candidate_to_info,
        entities_to_power_first,
        powered_entities,
        &electric_pole_list,
    );

    let mut covered_to_candidates =
        get_covered_to_candidates(context, &all_candidate_to_info, &covered_entities);

    let mut all_subsets: VecDeque<Subsets> = VecDeque::new();

    let sorter;
    let mut priority_batches = None;
    if entities_to_power_first.is_none() {
        sorter = Sorter::SameCoveredCount;
    } else {
        sorter = Sorter::SamePriorityPowered;
        let pairs = batches_by(&all_candidate_to_info, |info| info.priority_powered);
        priority_batches = Some(SortedBatches::new(pairs, false));
        all_subsets.push_back(Subsets::Priority);
    }

    let pairs = batches_by(&all_candidate_to_info, |info| {
        info.base.covered.true_count()
    });
    let mut covered_count_batches = SortedBatches::new(pairs, false);

    all_subsets.push_back(Subsets::CoveredCount);

    let rounded_reach = context.electric_pole_wire_reach_with_quality.ceil() as i32;

    // `candidateToInfo`: the batch candidates are being picked from, as which subsets it
    // came from and its index there.
    let mut scoped: Option<(Subsets, usize)> = None;

    while covered_entities.any(false) {
        let (which, index) = match scoped {
            Some(current) => current,
            None => {
                let Some(&which) = all_subsets.front() else {
                    // No candidates are left, or none fit. No solution exists on this grid
                    // (existing pipes may have taken every pole spot, say).
                    return Ok((None, covered_entities));
                };

                let subsets = match which {
                    Subsets::Priority => priority_batches.as_mut().expect("enqueued with it"),
                    Subsets::CoveredCount => &mut covered_count_batches,
                };

                if subsets.queue_len() == 0 || (all_subsets.len() > 1 && subsets.queue_len() == 1) {
                    all_subsets.pop_front();
                    continue;
                }

                let index = subsets.peek_index().expect("the queue is not empty");
                if subsets.batch(index).is_empty() {
                    subsets.dequeue();
                    continue;
                }

                scoped = Some((which, index));
                (which, index)
            }
        };

        let batch = match which {
            Subsets::Priority => priority_batches
                .as_ref()
                .expect("enqueued with it")
                .batch(index),
            Subsets::CoveredCount => covered_count_batches.batch(index),
        };

        let mut best: Option<(Location, SharedInfo<ElectricPoleCandidateInfo>)> = None;
        for (location, info) in batch.iter() {
            match &best {
                None => best = Some((location, SharedInfo::clone(info))),
                Some((_, best_info)) => {
                    if sorter.compare(&info.borrow(), &best_info.borrow()) == Ordering::Less {
                        best = Some((location, SharedInfo::clone(info)));
                    }
                }
            }
        }

        let Some((candidate, candidate_info)) = best else {
            return Err(FactorioToolsError::new(
                "A candidate should have been found.",
            ));
        };

        if !all_candidate_to_info.contains_key(candidate) {
            let batch = scoped_batch_mut(
                which,
                index,
                &mut priority_batches,
                &mut covered_count_batches,
            );
            batch.remove(candidate);

            if batch.is_empty() {
                scoped = None;
            }

            continue;
        }

        validate::candidate_covers_more_entities(
            context,
            powered_entities,
            &covered_entities,
            candidate,
            &candidate_info.borrow(),
        )?;

        let scoped_argument = match which {
            Subsets::CoveredCount => Scoped::Batch(index),
            Subsets::Priority => Scoped::Separate(
                priority_batches
                    .as_mut()
                    .expect("enqueued with it")
                    .batch_mut(index),
            ),
        };

        add_provider_and_allow_multiple_providers(
            context,
            candidate,
            &candidate_info,
            context.options.electric_pole_width,
            context.options.electric_pole_height,
            powered_entities,
            &mut covered_entities,
            &mut covered_to_candidates,
            &mut all_candidate_to_info,
            scoped_argument,
            &mut covered_count_batches,
        )?;

        electric_pole_list.push(candidate);

        update_candidate_info(context, &all_candidate_to_info, rounded_reach, candidate);

        let batch = scoped_batch_mut(
            which,
            index,
            &mut priority_batches,
            &mut covered_count_batches,
        );
        if batch.is_empty() {
            scoped = None;
        }
    }

    Ok((Some(electric_pole_list), covered_entities))
}

fn scoped_batch_mut<'a>(
    which: Subsets,
    index: usize,
    priority_batches: &'a mut Option<SortedBatches<SharedInfo<ElectricPoleCandidateInfo>>>,
    covered_count_batches: &'a mut SortedBatches<SharedInfo<ElectricPoleCandidateInfo>>,
) -> &'a mut LocationDictionary<SharedInfo<ElectricPoleCandidateInfo>> {
    match which {
        Subsets::Priority => priority_batches
            .as_mut()
            .expect("enqueued with it")
            .batch_mut(index),
        Subsets::CoveredCount => covered_count_batches.batch_mut(index),
    }
}

/// `PopulateCandidateToInfo`: fills in each candidate's ranking fields.
fn populate_candidate_to_info(
    context: &Context,
    candidate_to_info: &LocationDictionary<SharedInfo<ElectricPoleCandidateInfo>>,
    entities_to_power_first: Option<&CountedBitArray>,
    powered_entities: &[ProviderRecipient],
    electric_pole_list: &[Location],
) {
    let middle = context.grid.middle();
    for (candidate, info) in candidate_to_info.iter() {
        let mut info = info.borrow_mut();
        if let Some(first) = entities_to_power_first {
            let mut priority = first.clone();
            priority.and(&info.base.covered);
            info.priority_powered = priority.true_count();
        }

        let mut others_connected = 0;
        let mut min = i32::MAX;
        for &pole in electric_pole_list {
            if are_electric_poles_connected(candidate, pole, context) {
                others_connected += 1;
            }

            let val = pole.euclidean_distance_squared(candidate);
            if val < min {
                min = val;
            }
        }

        info.others_connected = others_connected;
        info.pole_distance = min;

        info.base.entity_distance =
            get_entity_distance(powered_entities, candidate, &info.base.covered);
        info.middle_distance = candidate.euclidean_distance_squared(middle);
    }
}

/// `UpdateCandidateInfo`: after placing a pole, bumps the wired-pole count and nearest-pole
/// distance of the candidates around it.
fn update_candidate_info(
    context: &Context,
    candidate_to_info: &LocationDictionary<SharedInfo<ElectricPoleCandidateInfo>>,
    rounded_reach: i32,
    candidate: Location,
) {
    let min_x = 0.max(candidate.x - rounded_reach);
    let max_x = (context.grid.width() - 1).min(candidate.x + rounded_reach);
    let min_y = 0.max(candidate.y - rounded_reach);
    let max_y = (context.grid.height() - 1).min(candidate.y + rounded_reach);

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let other = Location::new(x, y);
            let distance_squared = get_electric_pole_distance_squared(candidate, other, context);

            if let Some(info) = candidate_to_info.get(other) {
                let mut info = info.borrow_mut();
                if f64::from(distance_squared)
                    <= context.electric_pole_wire_reach_squared_with_quality
                {
                    info.others_connected += 1;
                }

                if distance_squared < info.pole_distance {
                    info.pole_distance = distance_squared;
                }
            }
        }
    }
}

/// `AddElectricPole`: places a pole and wires it to the poles within reach. Returns its id.
fn add_electric_pole(
    context: &mut Context,
    electric_poles: &mut LocationDictionary<i32>,
    center: Location,
) -> FtResult<i32> {
    let center_id = context.grid.get_id();
    let center_entity = GridEntity::electric_pole_center(center_id);

    add_provider_to_grid(
        &mut context.grid,
        center,
        center_entity,
        |grid, center| GridEntity::new(grid.get_id(), EntityKind::ElectricPoleSide { center }),
        context.options.electric_pole_width,
        context.options.electric_pole_height,
    )?;

    electric_poles.add(center, center_id);
    connect_existing_electric_poles(context, electric_poles, center, center_id);

    Ok(center_id)
}

/// `ConnectElectricPoles`: while the poles form more than one group, bridges the closest pair
/// of groups (along the Delaunay lines between pole centers) with a new pole.
fn connect_electric_poles(
    context: &mut Context,
    electric_poles: &mut LocationDictionary<i32>,
) -> FtResult<()> {
    let mut groups = get_electric_pole_groups(context, electric_poles)?;

    while groups.len() > 1 {
        let mut closest: Option<Endpoints> = None;
        let mut closest_distance_squared = 0;
        let keys: Vec<Location> = electric_poles.keys().collect();
        let lines = helpers::points_to_lines(&keys)
            .map_err(|error| FactorioToolsError::new(error.to_string()))?;
        for endpoint in lines {
            let group_a =
                collection_ext::single_where(0..groups.len(), |&g| groups[g].contains(endpoint.a))?;
            let group_b =
                collection_ext::single_where(0..groups.len(), |&g| groups[g].contains(endpoint.b))?;
            if group_a == group_b {
                continue;
            }

            let distance_squared =
                get_electric_pole_distance_squared(endpoint.a, endpoint.b, context);
            if f64::from(distance_squared) <= context.electric_pole_wire_reach_squared_with_quality
            {
                continue;
            }

            if closest.is_none() || distance_squared < closest_distance_squared {
                closest = Some(endpoint);
                closest_distance_squared = distance_squared;
            }
        }

        let Some(closest) = closest else {
            return Err(FactorioToolsError::new(
                "No closest electric pole could be found.",
            ));
        };

        add_single_pole_for_connection(
            context,
            electric_poles,
            &mut groups,
            f64::from(closest_distance_squared).sqrt(),
            closest,
        )?;
    }

    Ok(())
}

/// `AddSinglePoleForConnection`: places one pole on the line between two groups, as far along
/// as one wire reaches, searching outward from the ideal spot for one that fits, and merges
/// the groups it connects.
fn add_single_pole_for_connection(
    context: &mut Context,
    electric_poles: &mut LocationDictionary<i32>,
    groups: &mut Vec<LocationSet>,
    distance: f64,
    endpoints: Endpoints,
) -> FtResult<()> {
    let segments = (distance / context.electric_pole_wire_reach_with_quality).ceil() as i32;
    let ideal_line = bresenhams_line(endpoints.a, endpoints.b);
    let mut ideal_index = ideal_line.len() as i32 / segments;
    if !are_electric_poles_connected(ideal_line[0], ideal_line[ideal_index as usize], context) {
        ideal_index -= 1;
    }
    let ideal_point = ideal_line[ideal_index as usize];

    let mut selected_point = Location::INVALID;
    let mut match_found = false;

    let mut candidates = VecDeque::new();
    let mut attempted = context.get_location_set();
    candidates.push_back(ideal_point);
    attempted.add(ideal_point);

    let (width, height) = (
        context.options.electric_pole_width,
        context.options.electric_pole_height,
    );
    while let Some(candidate) = candidates.pop_front() {
        if does_provider_fit(&context.grid, width, height, candidate)
            && is_provider_in_bounds(&context.grid, width, height, candidate)
        {
            selected_point = candidate;
            match_found = true;
            break;
        }

        for neighbor in context.grid.get_adjacent(candidate) {
            if neighbor.is_valid()
                && are_electric_poles_connected(ideal_line[0], neighbor, context)
                && attempted.add(neighbor)
            {
                candidates.push_back(neighbor);
            }
        }
    }

    if !match_found {
        return Err(FactorioToolsError::new(
            "Could not find a pole that can be connected.",
        ));
    }

    let center_id = add_electric_pole(context, electric_poles, selected_point)?;
    let neighbor_locations: Vec<Location> = context
        .grid
        .neighbors(center_id)
        .iter()
        .map(|id| context.grid.location_of(id))
        .collect();
    let connected_groups: Vec<usize> = (0..groups.len())
        .filter(|&g| neighbor_locations.iter().any(|&l| groups[g].contains(l)))
        .collect();

    if connected_groups.is_empty() {
        return Err(FactorioToolsError::new(
            "Could not find the group containing the selected electric pole.",
        ));
    }

    let first = connected_groups[0];
    groups[first].add(selected_point);
    for &other in &connected_groups[1..] {
        let other_group = std::mem::replace(&mut groups[other], LocationSet::Empty);
        groups[first].union_with(&other_group);
    }
    for &other in connected_groups[1..].iter().rev() {
        groups.remove(other);
    }

    Ok(())
}

/// `GetElectricPoleGroups`: the poles split into wired networks.
fn get_electric_pole_groups(
    context: &Context,
    electric_poles: &LocationDictionary<i32>,
) -> FtResult<Vec<LocationSet>> {
    let mut groups = Vec::new();
    let mut remaining = context.get_location_set_from(electric_poles.keys(), true);
    while !remaining.is_empty() {
        let current = collection_ext::first(remaining.iter())?;
        remaining.remove(current);

        let mut entity_ids = NetHashSet::new();
        let mut explore = VecDeque::new();
        explore.push_back(*electric_poles.value(current));

        while let Some(entity) = explore.pop_front() {
            if entity_ids.add(entity) {
                for id in context.grid.neighbors(entity).iter() {
                    explore.push_back(id);
                }
            }
        }

        let mut group = context.get_location_set_enumerable(true);
        for entity_id in entity_ids.iter() {
            group.add(context.grid.location_of(entity_id));
        }

        remaining.except_with(&group);
        groups.push(group);
    }

    Ok(groups)
}
