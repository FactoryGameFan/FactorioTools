//! The provider half of `Helpers` (`src/FactorioTools/OilField/Helpers.cs`): finding spots for
//! providers (beacons and electric poles) that cover recipients (pumpjacks and beacons), and
//! the bookkeeping the beacon and pole passes do as they place them.
//!
//! **Candidate infos are shared.** In the C# a `CandidateInfo` is an object, and the same
//! object sits in several dictionaries at once: `candidateToInfo`, every
//! `coveredToCandidates[i]` it covers, and the `SortedBatches` batches. Changing
//! `info.Covered` through one is seen through all of them, and
//! [`add_provider_and_allow_multiple_providers`] relies on that. So the dictionaries here
//! hold a [`SharedInfo`], an `Rc<RefCell<T>>`, and cloning one clones the reference.
//!
//! **One dictionary can be reached two ways.** `AddElectricPoles` passes a batch that
//! `coveredCountBatches` also owns as `scopedCandidateToInfo`, so a removal through either
//! name is a removal from the same dictionary. [`Scoped`] says which it is.

use std::cell::RefCell;
use std::rc::Rc;

use dotnet_compat::net_sort;

use crate::Location;
use crate::containers::{CountedBitArray, LocationDictionary, LocationSet, SortedBatches};
use crate::context::Context;
use crate::error::{FactorioToolsError, FtResult};
use crate::grid::{EntityKind, GridEntity, SquareGrid};
use crate::helpers::{PUMPJACK_HEIGHT, PUMPJACK_WIDTH};
use crate::models::{AsCandidateInfo, CandidateFactory, ProviderRecipient};
use crate::options::OilFieldOptions;

/// A candidate info shared between dictionaries, as a C# object reference is.
pub type SharedInfo<T> = Rc<RefCell<T>>;

/// Wraps a new candidate info for sharing.
pub fn share<T>(info: T) -> SharedInfo<T> {
    Rc::new(RefCell::new(info))
}

/// The kind of provider a candidate search looks for: the C# `TProvider` type argument.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderKind {
    /// `BeaconCenter`.
    Beacon,
    /// `ElectricPoleCenter`.
    ElectricPole,
}

impl ProviderKind {
    /// `grid[location] as TProvider` is not null.
    fn matches(self, kind: &EntityKind) -> bool {
        match self {
            ProviderKind::Beacon => matches!(kind, EntityKind::BeaconCenter),
            ProviderKind::ElectricPole => matches!(kind, EntityKind::ElectricPoleCenter { .. }),
        }
    }
}

/// What `GetCandidateToCovered` returns.
pub struct CandidateToCovered<T> {
    /// `CandidateToInfo`: each spot a provider fits, with what it would cover.
    pub candidate_to_info: LocationDictionary<SharedInfo<T>>,
    /// `CoveredEntities`: recipients an existing provider already covers.
    pub covered_entities: CountedBitArray,
    /// `Providers`: the existing providers still on the grid, by center, with their entity
    /// ids (the C# holds the entity objects).
    pub providers: LocationDictionary<i32>,
}

/// `GetBeaconCandidateToCovered`.
pub fn get_beacon_candidate_to_covered<T: AsCandidateInfo>(
    context: &mut Context,
    recipients: &[ProviderRecipient],
    candidate_factory: &impl CandidateFactory<T>,
    remove_unused: bool,
) -> CandidateToCovered<T> {
    let o = &context.options;
    let (width, height, supply_width, supply_height) = (
        o.beacon_width,
        o.beacon_height,
        o.beacon_supply_width,
        o.beacon_supply_height,
    );
    get_candidate_to_covered(
        context,
        ProviderKind::Beacon,
        recipients,
        candidate_factory,
        width,
        height,
        supply_width,
        supply_height,
        remove_unused,
        true,
        false,
    )
}

/// `GetElectricPoleCandidateToCovered`.
pub fn get_electric_pole_candidate_to_covered<T: AsCandidateInfo>(
    context: &mut Context,
    recipients: &[ProviderRecipient],
    candidate_factory: &impl CandidateFactory<T>,
    remove_unused: bool,
) -> CandidateToCovered<T> {
    let (width, height, supply_width, supply_height) = (
        context.options.electric_pole_width,
        context.options.electric_pole_height,
        context.electric_pole_supply_width_with_quality,
        context.electric_pole_supply_height_with_quality,
    );
    get_candidate_to_covered(
        context,
        ProviderKind::ElectricPole,
        recipients,
        candidate_factory,
        width,
        height,
        supply_width,
        supply_height,
        remove_unused,
        true,
        true,
    )
}

/// `GetCandidateToCovered<TProvider, TInfo>`. With `remove_unused`, an existing provider that
/// covers no recipient is taken off the grid, and the spots around it become candidates.
#[allow(clippy::too_many_arguments)]
fn get_candidate_to_covered<T: AsCandidateInfo>(
    context: &mut Context,
    provider_kind: ProviderKind,
    recipients: &[ProviderRecipient],
    candidate_factory: &impl CandidateFactory<T>,
    provider_width: i32,
    provider_height: i32,
    supply_width: i32,
    supply_height: i32,
    remove_unused: bool,
    include_pumpjacks: bool,
    include_beacons: bool,
) -> CandidateToCovered<T> {
    let recipient_count = recipients.len() as i32;
    let mut candidate_to_info: LocationDictionary<SharedInfo<T>> =
        context.get_location_dictionary();
    let mut covered_entities = CountedBitArray::new(recipient_count);

    let mut providers: LocationDictionary<i32> = context.get_location_dictionary();
    for location in context.grid.entity_locations().iter() {
        if let Some(entity) = context.grid.get(location)
            && provider_kind.matches(&entity.kind)
        {
            providers.add(location, entity.id);
        }
    }
    let mut unused_providers =
        context.get_read_only_location_set(providers.keys().collect::<Vec<_>>(), remove_unused);

    let grid_width = context.grid.width();
    let grid_height = context.grid.height();

    for (i, entity) in recipients.iter().enumerate() {
        let i = i as i32;
        let min_x = ((provider_width - 1) / 2)
            .max(entity.center.x - ((entity.width - 1) / 2) - (supply_width / 2));
        let min_y = ((provider_height - 1) / 2)
            .max(entity.center.y - ((entity.height - 1) / 2) - (supply_height / 2));
        let max_x = (grid_width - (provider_width / 2) - 1)
            .min(entity.center.x + (entity.width / 2) + ((supply_width - 1) / 2));
        let max_y = (grid_height - (provider_height / 2) - 1)
            .min(entity.center.y + (entity.height / 2) + ((supply_height - 1) / 2));

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let candidate = Location::new(x, y);
                if context.grid.get(candidate).is_some() {
                    if providers.contains_key(candidate) {
                        unused_providers.remove(candidate);
                        covered_entities.set(i, true);
                    }
                } else {
                    if !does_provider_fit(&context.grid, provider_width, provider_height, candidate)
                    {
                        continue;
                    }

                    match candidate_to_info.get(candidate) {
                        None => {
                            let mut covered = CountedBitArray::new(recipient_count);
                            covered.set(i, true);
                            candidate_to_info
                                .add(candidate, share(candidate_factory.create(covered)));
                        }
                        Some(info) => {
                            info.borrow_mut().info_mut().covered.set(i, true);
                        }
                    }
                }
            }
        }
    }

    if remove_unused && !unused_providers.is_empty() {
        let mut covered_centers = context.get_location_set();
        for center in unused_providers.to_vec() {
            let entity_min_x = center.x - ((provider_width - 1) / 2);
            let entity_min_y = center.y - ((provider_height - 1) / 2);
            let entity_max_x = center.x + (provider_width / 2);
            let entity_max_y = center.y + (provider_height / 2);

            // Expand the loop bounds beyond the entity bounds so candidates that are no
            // longer valid next to the removed provider are refreshed too.
            let min_x = ((provider_width - 1) / 2).max(entity_min_x - (provider_width / 2));
            let min_y = ((provider_height - 1) / 2).max(entity_min_y - (provider_height / 2));
            let max_x = (grid_width - (provider_width / 2) - 1)
                .min(entity_max_x + ((provider_width - 1) / 2));
            let max_y = (grid_height - (provider_height / 2) - 1)
                .min(entity_max_y + ((provider_height - 1) / 2));

            for x in min_x..=max_x {
                for y in min_y..=max_y {
                    let candidate = Location::new(x, y);

                    if x >= entity_min_x
                        && x <= entity_max_x
                        && y >= entity_min_y
                        && y <= entity_max_y
                    {
                        context.grid.remove_entity(candidate);
                    } else {
                        add_covered_centers(
                            &mut covered_centers,
                            &context.grid,
                            candidate,
                            provider_width,
                            provider_height,
                            supply_width,
                            supply_height,
                            include_pumpjacks,
                            include_beacons,
                        );

                        let info = match candidate_to_info.get(candidate) {
                            Some(info) => Rc::clone(info),
                            None => {
                                let info = share(
                                    candidate_factory.create(CountedBitArray::new(recipient_count)),
                                );
                                candidate_to_info.add(candidate, Rc::clone(&info));
                                info
                            }
                        };

                        for (i, recipient) in recipients.iter().enumerate() {
                            if covered_centers.contains(recipient.center) {
                                info.borrow_mut().info_mut().covered.set(i as i32, true);
                            }
                        }

                        covered_centers.clear();
                    }
                }
            }

            providers.remove(center);
        }
    }

    if !providers.is_empty() || !unused_providers.is_empty() {
        // Remove candidates that only cover recipients that are already covered.
        let mut to_remove = Vec::new();
        for (candidate, info) in candidate_to_info.iter() {
            let mut subset = info.borrow().info().covered.clone();
            subset.not();
            subset.or(&covered_entities);
            if subset.all(true) {
                to_remove.push(candidate);
            }
        }

        for candidate in to_remove {
            candidate_to_info.remove(candidate);
        }
    }

    CandidateToCovered {
        candidate_to_info,
        covered_entities,
        providers,
    }
}

/// `GetProviderCenterToCoveredCenters`: for each provider center, in the order given, the
/// recipient centers it covers.
#[allow(clippy::too_many_arguments)]
pub fn get_provider_center_to_covered_centers(
    context: &Context,
    provider_width: i32,
    provider_height: i32,
    supply_width: i32,
    supply_height: i32,
    provider_centers: impl IntoIterator<Item = Location>,
    include_pumpjacks: bool,
    include_beacons: bool,
) -> LocationDictionary<LocationSet> {
    let mut pole_center_to_covered_centers = context.get_location_dictionary();

    for center in provider_centers {
        let mut covered_centers = context.get_location_set_enumerable(true);
        add_covered_centers(
            &mut covered_centers,
            &context.grid,
            center,
            provider_width,
            provider_height,
            supply_width,
            supply_height,
            include_pumpjacks,
            include_beacons,
        );

        pole_center_to_covered_centers.add(center, covered_centers);
    }

    pole_center_to_covered_centers
}

/// `GetCoveredCenterToProviderCenters`: the inverse of
/// [`get_provider_center_to_covered_centers`].
pub fn get_covered_center_to_provider_centers(
    context: &Context,
    provider_center_to_covered_centers: &LocationDictionary<LocationSet>,
) -> LocationDictionary<LocationSet> {
    let mut output: LocationDictionary<LocationSet> = context.get_location_dictionary();

    for (center, covered) in provider_center_to_covered_centers.iter() {
        for other_center in covered.iter() {
            if !output.contains_key(other_center) {
                output.add(other_center, context.get_location_set_enumerable(true));
            }

            output.value_mut(other_center).add(center);
        }
    }

    output
}

/// `AddCoveredCenters`: adds to `covered_centers` the center of every pumpjack (and beacon,
/// if asked) that a provider at `center` would cover.
#[allow(clippy::too_many_arguments)]
fn add_covered_centers(
    covered_centers: &mut LocationSet,
    grid: &SquareGrid,
    center: Location,
    provider_width: i32,
    provider_height: i32,
    supply_width: i32,
    supply_height: i32,
    include_pumpjacks: bool,
    include_beacons: bool,
) {
    const MIN_POWERED_ENTITY_WIDTH: i32 = 3;
    const MIN_POWERED_ENTITY_HEIGHT: i32 = 3;

    let min_x = (MIN_POWERED_ENTITY_WIDTH - 1)
        .max(center.x - (supply_width / 2) + ((provider_width - 1) % 2));
    let min_y = (MIN_POWERED_ENTITY_HEIGHT - 1)
        .max(center.y - (supply_height / 2) + ((provider_height - 1) % 2));
    let max_x = (grid.width() - MIN_POWERED_ENTITY_WIDTH + 1).min(center.x + (supply_width / 2));
    let max_y = (grid.height() - MIN_POWERED_ENTITY_HEIGHT + 1).min(center.y + (supply_height / 2));

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let location = Location::new(x, y);
            let Some(entity) = grid.get(location) else {
                continue;
            };

            match entity.kind {
                EntityKind::PumpjackCenter { .. } if include_pumpjacks => {
                    covered_centers.add(location);
                }
                EntityKind::PumpjackSide { center } if include_pumpjacks => {
                    covered_centers.add(grid.location_of(center));
                }
                EntityKind::BeaconCenter if include_beacons => {
                    covered_centers.add(location);
                }
                EntityKind::BeaconSide { center } if include_beacons => {
                    covered_centers.add(grid.location_of(center));
                }
                _ => {}
            }
        }
    }
}

/// `DoesProviderFit`: whether every cell of a provider centered at `center` is empty. This
/// does NOT account for grid bounds, as in the C#.
pub fn does_provider_fit(
    grid: &SquareGrid,
    provider_width: i32,
    provider_height: i32,
    center: Location,
) -> bool {
    let min_x = center.x - ((provider_width - 1) / 2);
    let max_x = center.x + (provider_width / 2);
    let min_y = center.y - ((provider_height - 1) / 2);
    let max_y = center.y + (provider_height / 2);

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            if !grid.is_empty(Location::new(x, y)) {
                return false;
            }
        }
    }

    true
}

/// `GetEntityDistance`: the summed Euclidean distance from `candidate` to each covered
/// recipient, added in recipient order.
pub fn get_entity_distance(
    powered_entities: &[ProviderRecipient],
    candidate: Location,
    covered: &CountedBitArray,
) -> f64 {
    let mut sum = 0.0;
    for (i, entity) in powered_entities.iter().enumerate() {
        if covered.get(i as i32) {
            sum += candidate.euclidean_distance(entity.center);
        }
    }

    sum
}

/// `scopedCandidateToInfo` for [`add_provider_and_allow_multiple_providers`] and
/// [`remove_overlapping_candidates_scoped`].
pub enum Scoped<'a, T> {
    /// A dictionary of its own.
    Separate(&'a mut LocationDictionary<SharedInfo<T>>),
    /// The batch at this index in the `covered_count_batches` passed alongside (see
    /// [`SortedBatches::peek_index`]). `AddElectricPoles` passes `coveredCountBatches`'s own
    /// top batch, so the C# reaches one dictionary under both names.
    Batch(usize),
}

/// `AddProviderAndPreventMultipleProviders`: records a provider at `center` and drops every
/// candidate that would also cover a recipient it newly covers.
#[allow(clippy::too_many_arguments)]
pub fn add_provider_and_prevent_multiple_providers<T: AsCandidateInfo>(
    context: &Context,
    center: Location,
    center_info: &SharedInfo<T>,
    provider_width: i32,
    provider_height: i32,
    recipients: &[ProviderRecipient],
    covered_entities: &mut CountedBitArray,
    covered_to_candidates: &mut [LocationDictionary<SharedInfo<T>>],
    candidate_to_info: &mut LocationDictionary<SharedInfo<T>>,
) -> FtResult<()> {
    let center_covered = center_info.borrow().info().covered.clone();
    let mut newly_covered = covered_entities.clone();
    newly_covered.not();
    newly_covered.and(&center_covered);

    if newly_covered.true_count() == 0 {
        return Err(FactorioToolsError::new(
            "At least one recipient should should have been newly covered.",
        ));
    }

    covered_entities.or(&center_covered);

    remove_overlapping_candidates_covered(
        &context.grid,
        center,
        provider_width,
        provider_height,
        candidate_to_info,
        covered_to_candidates,
    );

    let mut to_remove = Vec::new();
    let mut updated = context.get_location_set();
    for (i, candidates) in covered_to_candidates
        .iter()
        .enumerate()
        .take(recipients.len())
    {
        if !newly_covered.get(i as i32) {
            continue;
        }

        for other_candidate in candidates.keys() {
            if !updated.add(other_candidate) {
                continue;
            }

            to_remove.push(other_candidate);
        }

        for &candidate in &to_remove {
            candidate_to_info.remove(candidate);
        }

        to_remove.clear();
    }

    Ok(())
}

/// `AddProviderAndAllowMultipleProviders`: records a provider at `center`, then discounts
/// every other candidate by the recipients that no longer need covering, moving it between
/// `covered_count_batches` and dropping it once it covers nothing new.
#[allow(clippy::too_many_arguments)]
pub fn add_provider_and_allow_multiple_providers<T: AsCandidateInfo>(
    context: &Context,
    center: Location,
    center_info: &SharedInfo<T>,
    provider_width: i32,
    provider_height: i32,
    recipients: &[ProviderRecipient],
    covered_entities: &mut CountedBitArray,
    covered_to_candidates: &mut [LocationDictionary<SharedInfo<T>>],
    candidate_to_info: &mut LocationDictionary<SharedInfo<T>>,
    mut scoped: Scoped<'_, T>,
    covered_count_batches: &mut SortedBatches<SharedInfo<T>>,
) -> FtResult<()> {
    let center_covered = center_info.borrow().info().covered.clone();
    let mut newly_covered = covered_entities.clone();
    newly_covered.not();
    newly_covered.and(&center_covered);

    if newly_covered.true_count() == 0 {
        return Err(FactorioToolsError::new(
            "At least one recipient should should have been newly covered.",
        ));
    }

    covered_entities.or(&center_covered);

    remove_overlapping_candidates_scoped(
        &context.grid,
        center,
        provider_width,
        provider_height,
        candidate_to_info,
        &mut scoped,
        covered_count_batches,
        Some(covered_to_candidates),
    );

    if covered_entities.all(true) {
        return Ok(());
    }

    let mut to_remove = Vec::new();
    let mut updated = context.get_location_set();

    // Remove the covered entities from the candidate data, so that the next candidates are
    // discounted by the entities that no longer need to be covered.
    for i in 0..recipients.len() {
        if !newly_covered.get(i as i32) {
            continue;
        }

        // The C# enumerates `coveredToCandidates[i]` while changing only the other lists, so
        // a snapshot of it sees the same pairs in the same order.
        let current: Vec<(Location, SharedInfo<T>)> = covered_to_candidates[i]
            .iter()
            .map(|(location, info)| (location, Rc::clone(info)))
            .collect();
        for (other_candidate, other_info) in current {
            if !updated.add(other_candidate) {
                continue;
            }

            let mut modified = false;
            let old_covered_count = other_info.borrow().info().covered.true_count();
            let mut j = 0;
            while j < recipients.len() && other_info.borrow().info().covered.true_count() > 0 {
                let j32 = j as i32;
                let covered_here = other_info.borrow().info().covered.get(j32);
                if covered_entities.get(j32) && covered_here {
                    other_info.borrow_mut().info_mut().covered.set(j32, false);
                    modified = true;

                    // Avoid modifying the collection being enumerated.
                    if i != j {
                        covered_to_candidates[j].remove(other_candidate);
                    }
                }
                j += 1;
            }

            let new_covered_count = other_info.borrow().info().covered.true_count();
            if new_covered_count == 0 {
                to_remove.push(other_candidate);
                covered_count_batches.remove_candidate(other_candidate, old_covered_count);
            } else if modified {
                covered_count_batches.move_candidate(
                    other_candidate,
                    Rc::clone(&other_info),
                    old_covered_count,
                    new_covered_count,
                );

                let mut entity_distance = 0.0;
                for recipient in recipients {
                    entity_distance += other_candidate.euclidean_distance(recipient.center);
                }

                other_info.borrow_mut().info_mut().entity_distance = entity_distance;
            }
        }

        // Now that the enumeration is done, the list can be cleared.
        covered_to_candidates[i].clear();

        for &candidate in &to_remove {
            if candidate_to_info.remove(candidate) {
                scoped_remove(&mut scoped, covered_count_batches, candidate);
            }
        }

        to_remove.clear();
    }

    Ok(())
}

fn scoped_remove<T>(
    scoped: &mut Scoped<'_, T>,
    covered_count_batches: &mut SortedBatches<SharedInfo<T>>,
    location: Location,
) {
    match scoped {
        Scoped::Separate(dictionary) => {
            dictionary.remove(location);
        }
        Scoped::Batch(index) => {
            covered_count_batches.batch_mut(*index).remove(location);
        }
    }
}

/// `GetElectricPoleCoverage`: which recipients each pole covers, and the reverse. Fails if
/// any powered entity is not covered.
pub fn get_electric_pole_coverage(
    context: &Context,
    powered_entities: &[ProviderRecipient],
    electric_pole_centers: impl IntoIterator<Item = Location>,
) -> FtResult<(
    LocationDictionary<LocationSet>,
    LocationDictionary<LocationSet>,
)> {
    let pole_center_to_covered_centers = get_provider_center_to_covered_centers(
        context,
        context.options.electric_pole_width,
        context.options.electric_pole_height,
        context.electric_pole_supply_width_with_quality,
        context.electric_pole_supply_height_with_quality,
        electric_pole_centers,
        true,
        true,
    );

    let covered_center_to_pole_centers =
        get_covered_center_to_provider_centers(context, &pole_center_to_covered_centers);

    if covered_center_to_pole_centers.len() != powered_entities.len() {
        return Err(FactorioToolsError::new(
            "Not all powered entities are covered by an electric pole.",
        ));
    }

    Ok((
        pole_center_to_covered_centers,
        covered_center_to_pole_centers,
    ))
}

/// `GetPoweredEntities`: every pumpjack and beacon center as a recipient, sorted by Y then X,
/// and whether any beacon was found.
pub fn get_powered_entities(context: &Context) -> (Vec<ProviderRecipient>, bool) {
    let mut powered_entities = Vec::new();
    let mut has_beacons = false;

    for location in context.grid.entity_locations().iter() {
        match context.grid.get(location).map(|e| &e.kind) {
            Some(EntityKind::PumpjackCenter { .. }) => {
                powered_entities.push(ProviderRecipient::new(
                    location,
                    PUMPJACK_WIDTH,
                    PUMPJACK_HEIGHT,
                ));
            }
            Some(EntityKind::BeaconCenter) => {
                powered_entities.push(ProviderRecipient::new(
                    location,
                    context.options.beacon_width,
                    context.options.beacon_height,
                ));
                has_beacons = true;
            }
            _ => {}
        }
    }

    // Sort the result so the enumeration order above does not affect the output.
    net_sort(&mut powered_entities, |a, b| {
        a.center
            .y
            .cmp(&b.center.y)
            .then(a.center.x.cmp(&b.center.x))
    });

    (powered_entities, has_beacons)
}

/// `GetCoveredToCandidates`: for each recipient index, the candidates that cover it, in
/// `all_candidate_to_info`'s order. The C# `Dictionary<int, ...>` has the keys
/// `0..coveredEntities.Count` and is only ever indexed, so it is a `Vec` here.
pub fn get_covered_to_candidates<T>(
    context: &Context,
    all_candidate_to_info: &LocationDictionary<SharedInfo<T>>,
    covered_entities: &CountedBitArray,
) -> Vec<LocationDictionary<SharedInfo<T>>>
where
    T: AsCandidateInfo,
{
    let mut covered_to_candidates = Vec::with_capacity(covered_entities.count().max(0) as usize);
    for i in 0..covered_entities.count() {
        let mut candidates = context.get_location_dictionary();
        for (candidate, info) in all_candidate_to_info.iter() {
            if info.borrow().info().covered.get(i) {
                candidates.add(candidate, Rc::clone(info));
            }
        }

        covered_to_candidates.push(candidates);
    }

    covered_to_candidates
}

/// `GetProviderOverlapBounds`: the range of provider centers that would overlap a provider
/// at `center`, clamped to where a provider fits on the grid.
pub fn get_provider_overlap_bounds(
    grid: &SquareGrid,
    center: Location,
    provider_width: i32,
    provider_height: i32,
) -> (i32, i32, i32, i32) {
    let entity_min_x = center.x - ((provider_width - 1) / 2);
    let entity_min_y = center.y - ((provider_height - 1) / 2);
    let entity_max_x = center.x + (provider_width / 2);
    let entity_max_y = center.y + (provider_height / 2);

    let overlap_min_x = ((provider_width - 1) / 2).max(entity_min_x - (provider_width / 2));
    let overlap_min_y = ((provider_height - 1) / 2).max(entity_min_y - (provider_height / 2));
    let overlap_max_x =
        (grid.width() - (provider_width / 2) - 1).min(entity_max_x + ((provider_width - 1) / 2));
    let overlap_max_y =
        (grid.height() - (provider_height / 2) - 1).min(entity_max_y + ((provider_height - 1) / 2));

    (overlap_min_x, overlap_min_y, overlap_max_x, overlap_max_y)
}

/// `RemoveOverlappingCandidates(grid, center, w, h, candidateToInfo)`.
pub fn remove_overlapping_candidates<T>(
    grid: &SquareGrid,
    center: Location,
    provider_width: i32,
    provider_height: i32,
    candidate_to_info: &mut LocationDictionary<T>,
) {
    let (min_x, min_y, max_x, max_y) =
        get_provider_overlap_bounds(grid, center, provider_width, provider_height);

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            candidate_to_info.remove(Location::new(x, y));
        }
    }
}

/// `RemoveOverlappingCandidates(grid, center, w, h, candidateToInfo, coveredToCandidates)`.
pub fn remove_overlapping_candidates_covered<T: AsCandidateInfo>(
    grid: &SquareGrid,
    center: Location,
    provider_width: i32,
    provider_height: i32,
    candidate_to_info: &mut LocationDictionary<SharedInfo<T>>,
    covered_to_candidates: &mut [LocationDictionary<SharedInfo<T>>],
) {
    let (min_x, min_y, max_x, max_y) =
        get_provider_overlap_bounds(grid, center, provider_width, provider_height);

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let location = Location::new(x, y);
            if let Some(info) = candidate_to_info.take(location) {
                let info = info.borrow();
                let covered = &info.info().covered;
                for i in 0..covered.count() {
                    if covered.get(i) {
                        covered_to_candidates[i as usize].remove(location);
                    }
                }
            }
        }
    }
}

/// `RemoveOverlappingCandidates(grid, center, w, h, candidateToInfo, scopedCandidateToInfo,
/// coveredToCandidates)`. `covered_to_candidates` is nullable in the C#.
#[allow(clippy::too_many_arguments)]
pub fn remove_overlapping_candidates_scoped<T: AsCandidateInfo>(
    grid: &SquareGrid,
    center: Location,
    provider_width: i32,
    provider_height: i32,
    candidate_to_info: &mut LocationDictionary<SharedInfo<T>>,
    scoped: &mut Scoped<'_, T>,
    covered_count_batches: &mut SortedBatches<SharedInfo<T>>,
    mut covered_to_candidates: Option<&mut [LocationDictionary<SharedInfo<T>>]>,
) {
    let (min_x, min_y, max_x, max_y) =
        get_provider_overlap_bounds(grid, center, provider_width, provider_height);

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let location = Location::new(x, y);
            if let Some(covered_to_candidates) = covered_to_candidates.as_deref_mut() {
                if let Some(info) = candidate_to_info.take(location) {
                    scoped_remove(scoped, covered_count_batches, location);
                    let info = info.borrow();
                    let covered = &info.info().covered;
                    for i in 0..covered.count() {
                        if covered.get(i) {
                            covered_to_candidates[i as usize].remove(location);
                        }
                    }
                }
            } else if candidate_to_info.remove(location) {
                scoped_remove(scoped, covered_count_batches, location);
            }
        }
    }
}

/// `RemoveEntity(grid, center, width, height)`: clears every cell of an entity.
pub fn remove_entity(grid: &mut SquareGrid, center: Location, width: i32, height: i32) {
    let min_x = center.x - ((width - 1) / 2);
    let max_x = center.x + (width / 2);
    let min_y = center.y - ((height - 1) / 2);
    let max_y = center.y + (height / 2);

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            grid.remove_entity(Location::new(x, y));
        }
    }
}

/// `AddProviderToGrid<TCenter, TSide>`: places `center_entity` at `center` and a new side,
/// from `new_side(grid, center id)`, on each other cell. Cells go column by column (x outer,
/// y inner), and each side is made as its cell is reached, so ids interleave the way the C#
/// hands them out. Stops at the first occupied cell, as the C# throws there.
pub fn add_provider_to_grid(
    grid: &mut SquareGrid,
    center: Location,
    center_entity: GridEntity,
    mut new_side: impl FnMut(&mut SquareGrid, i32) -> GridEntity,
    provider_width: i32,
    provider_height: i32,
) -> FtResult<()> {
    let center_id = center_entity.id;
    let mut center_entity = Some(center_entity);
    let min_x = center.x - ((provider_width - 1) / 2);
    let max_x = center.x + (provider_width / 2);
    let min_y = center.y - ((provider_height - 1) / 2);
    let max_y = center.y + (provider_height / 2);

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            let location = Location::new(x, y);
            let entity = if location == center {
                center_entity
                    .take()
                    .expect("the center cell is reached once")
            } else {
                new_side(grid, center_id)
            };
            grid.add_entity(location, entity)?;
        }
    }

    Ok(())
}

/// `AddBeaconsToGrid`: a beacon at each center, in order.
pub fn add_beacons_to_grid(
    grid: &mut SquareGrid,
    options: &OilFieldOptions,
    centers: impl IntoIterator<Item = Location>,
) -> FtResult<()> {
    for center in centers {
        let center_entity = GridEntity::new(grid.get_id(), EntityKind::BeaconCenter);
        add_provider_to_grid(
            grid,
            center,
            center_entity,
            |grid, center_id| {
                GridEntity::new(grid.get_id(), EntityKind::BeaconSide { center: center_id })
            },
            options.beacon_width,
            options.beacon_height,
        )?;
    }

    Ok(())
}

/// `IsProviderInBounds`: whether a provider at `center` stays inside the grid, with a
/// one-cell margin on the top and left (the C# uses `> 0` there and `<` on the other sides).
pub fn is_provider_in_bounds(
    grid: &SquareGrid,
    provider_width: i32,
    provider_height: i32,
    center: Location,
) -> bool {
    center.x - ((provider_width - 1) / 2) > 0
        && center.y - ((provider_height - 1) / 2) > 0
        && center.x + (provider_width / 2) < grid.width()
        && center.y + (provider_height / 2) < grid.height()
}
