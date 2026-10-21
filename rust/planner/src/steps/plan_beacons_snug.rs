//! `PlanBeaconsSnug`, from `src/FactorioTools/OilField/Steps/PlanBeacons.2.Snug.cs`: places
//! beacons as close to each other as possible, after starting where they cover the most
//! pumpjacks.
//!
//! Two places depend on .NET's order. The start of each run is a `MinBy` over the candidate
//! dictionary, which keeps the first of several equal keys in enumeration order. And each
//! beacon's new neighbors are sorted with `List.Sort(index, count, comparer)`, whose
//! comparer can tie, so that sort is .NET's own introsort ([`net_sort_range`]).

use std::cmp::Ordering;
use std::rc::Rc;

use dotnet_compat::{NetCompare, net_sort_range};

use crate::Location;
use crate::collection_ext::{min, min_by};
use crate::containers::{CountedBitArray, LocationDictionary};
use crate::context::Context;
use crate::error::FtResult;
use crate::helpers::{PUMPJACK_HEIGHT, PUMPJACK_WIDTH};
use crate::models::{AsCandidateInfo, CandidateFactory, CandidateInfo, ProviderRecipient};
use crate::providers::{
    SharedInfo, add_provider_and_prevent_multiple_providers, get_beacon_candidate_to_covered,
    get_covered_to_candidates, get_entity_distance, get_provider_overlap_bounds,
    remove_overlapping_candidates,
};
use crate::steps::plan_beacons::BeaconPlannerResult;
use crate::steps::validate;

/// `BeaconCandidateInfo`, a `CandidateInfo` subclass private to `PlanBeaconsSnug`.
#[derive(Clone, Debug)]
pub struct BeaconCandidateInfo {
    pub base: CandidateInfo,
    pub middle_distance: i32,
    pub covered_count: i32,
}

impl AsCandidateInfo for BeaconCandidateInfo {
    fn info(&self) -> &CandidateInfo {
        &self.base
    }

    fn info_mut(&mut self) -> &mut CandidateInfo {
        &mut self.base
    }
}

/// `PlanBeaconsSnug.CandidateFactory`.
struct Factory;

impl CandidateFactory<BeaconCandidateInfo> for Factory {
    fn create(&self, covered: CountedBitArray) -> BeaconCandidateInfo {
        BeaconCandidateInfo {
            base: CandidateInfo::new(covered),
            middle_distance: 0,
            covered_count: 0,
        }
    }
}

type Shared = SharedInfo<BeaconCandidateInfo>;

/// The `MinBy` key, a `Tuple<int, int, double, int>`, which compares item by item with
/// `Comparer<T>.Default`.
struct StartKey(i32, i32, f64, i32);

impl NetCompare for StartKey {
    fn net_cmp(&self, other: &Self) -> Ordering {
        self.0
            .cmp(&other.0)
            .then_with(|| self.1.cmp(&other.1))
            .then_with(|| self.2.net_cmp(&other.2))
            .then_with(|| self.3.cmp(&other.3))
    }
}

/// `PlanBeaconsSnug.Execute(context)`.
pub fn execute(context: &mut Context) -> FtResult<BeaconPlannerResult> {
    let mut powered_entities = Vec::with_capacity(context.center_to_terminals.len());
    for &center in &context.centers {
        powered_entities.push(ProviderRecipient::new(
            center,
            PUMPJACK_WIDTH,
            PUMPJACK_HEIGHT,
        ));
    }

    // Unused beacons are not removed here, because there should be no existing beacons yet.
    let candidates = get_beacon_candidate_to_covered(context, &powered_entities, &Factory, false);
    let mut candidate_to_info = candidates.candidate_to_info;
    let mut covered_entities = candidates.covered_entities;

    validate::no_existing_beacons(context, &candidates.providers)?;

    populate_candidate_to_info(context, &candidate_to_info, &powered_entities);

    let mut scoped_candidates: Vec<(Location, Shared)> = Vec::new();
    let mut scoped_candidates_set: LocationDictionary<Shared> = context.get_location_dictionary();

    let mut covered_to_candidates = if context.options.overlap_beacons {
        None
    } else {
        Some(get_covered_to_candidates(
            context,
            &candidate_to_info,
            &covered_entities,
        ))
    };

    let mut beacons: Vec<Location> = Vec::new();
    let mut effects_given_counts = Vec::new();
    let mut effects = 0;

    let (beacon_width, beacon_height) =
        (context.options.beacon_width, context.options.beacon_height);
    let overlap_beacons = context.options.overlap_beacons;

    while !candidate_to_info.is_empty() {
        let pair = min_by(
            candidate_to_info
                .iter()
                .map(|(location, info)| (location, Rc::clone(info))),
            |(location, info)| {
                let info = info.borrow();
                StartKey(
                    if !beacons.is_empty() && overlap_beacons {
                        min(beacons.iter().map(|b| b.manhattan_distance(*location)))
                            .expect("beacons is not empty")
                    } else {
                        0
                    },
                    -info.base.covered.true_count(),
                    -info.base.entity_distance,
                    info.middle_distance,
                )
            },
        )
        .expect("candidate_to_info is not empty");

        scoped_candidates.clear();
        scoped_candidates_set.clear();
        scoped_candidates_set.add(pair.0, Rc::clone(&pair.1));
        scoped_candidates.push(pair);

        while let Some((candidate, info)) = scoped_candidates.pop() {
            if !candidate_to_info.contains_key(candidate) {
                continue;
            }

            if overlap_beacons {
                remove_overlapping_candidates(
                    &context.grid,
                    candidate,
                    beacon_width,
                    beacon_height,
                    &mut candidate_to_info,
                );
            } else {
                add_provider_and_prevent_multiple_providers(
                    context,
                    candidate,
                    &info,
                    beacon_width,
                    beacon_height,
                    &powered_entities,
                    &mut covered_entities,
                    covered_to_candidates
                        .as_mut()
                        .expect("coveredToCandidates is set when beacons do not overlap"),
                    &mut candidate_to_info,
                )?;
            }

            let covered_count = info.borrow().covered_count;
            beacons.push(candidate);
            effects += covered_count;
            effects_given_counts.push(covered_count);

            add_neighbors_and_sort(
                context,
                &candidate_to_info,
                &mut scoped_candidates,
                &mut scoped_candidates_set,
                candidate,
            );
        }
    }

    Ok(BeaconPlannerResult {
        beacons,
        effects,
        effects_given_counts,
    })
}

/// `PopulateCandidateToInfo`.
fn populate_candidate_to_info(
    context: &Context,
    candidate_to_info: &LocationDictionary<Shared>,
    powered_entities: &[ProviderRecipient],
) {
    let middle = context.grid.middle();
    for (candidate, info) in candidate_to_info.iter() {
        let mut info = info.borrow_mut();
        info.base.entity_distance =
            get_entity_distance(powered_entities, candidate, &info.base.covered);
        info.middle_distance = candidate.euclidean_distance_squared(middle);
        info.covered_count = info.base.covered.true_count();
    }
}

/// `AddNeighborsAndSort`: queues the remaining candidates on the ring just outside the
/// cells a beacon at `candidate` blocks, then sorts the newly queued ones so the best is
/// popped first.
fn add_neighbors_and_sort(
    context: &Context,
    candidate_to_info: &LocationDictionary<Shared>,
    scoped_candidates: &mut Vec<(Location, Shared)>,
    scoped_candidates_set: &mut LocationDictionary<Shared>,
    candidate: Location,
) {
    let (beacon_width, beacon_height) =
        (context.options.beacon_width, context.options.beacon_height);
    let (overlap_min_x, overlap_min_y, overlap_max_x, overlap_max_y) =
        get_provider_overlap_bounds(&context.grid, candidate, beacon_width, beacon_height);

    let min_x = ((beacon_width - 1) / 2).max(overlap_min_x - 1);
    let min_y = ((beacon_height - 1) / 2).max(overlap_min_y - 1);
    let max_x = (context.grid.width() - (beacon_width / 2) - 1).min(overlap_max_x + 1);
    let max_y = (context.grid.height() - (beacon_height / 2) - 1).min(overlap_max_y + 1);

    let initial_count = scoped_candidates.len();

    let mut consider = |location: Location| {
        if let Some(info) = candidate_to_info.get(location)
            && scoped_candidates_set.try_add(location, Rc::clone(info))
        {
            scoped_candidates.push((location, Rc::clone(info)));
        }
    };

    // The top bound of the neighbor rectangle.
    if overlap_min_y - 1 == min_y {
        for x in min_x..=max_x {
            consider(Location::new(x, min_y));
        }
    }

    // The bottom bound.
    if overlap_max_y + 1 == max_y {
        for x in min_x..=max_x {
            consider(Location::new(x, max_y));
        }
    }

    // The left bound, avoiding the corners.
    if overlap_min_x - 1 == min_x {
        for y in (min_y + 1)..=(max_y - 1) {
            consider(Location::new(min_x, y));
        }
    }

    // The right bound, avoiding the corners.
    if overlap_max_x + 1 == max_x {
        for y in (min_y + 1)..=(max_y - 1) {
            consider(Location::new(max_x, y));
        }
    }

    if initial_count < scoped_candidates.len() {
        let count = scoped_candidates.len() - initial_count;
        net_sort_range(scoped_candidates, initial_count, count, |x, y| {
            snug_candidate_sorter(&x.1, &y.1)
        });
    }
}

/// `SnugCandidateSorter.Compare`: fewer covered first, then smaller entity distance, then
/// farther from the middle, so the best candidate ends up last. Equal candidates tie.
fn snug_candidate_sorter(x: &Shared, y: &Shared) -> Ordering {
    let x = x.borrow();
    let y = y.borrow();

    let c = x
        .base
        .covered
        .true_count()
        .cmp(&y.base.covered.true_count());
    if c != Ordering::Equal {
        return c;
    }

    let c = x.base.entity_distance.net_cmp(&y.base.entity_distance);
    if c != Ordering::Equal {
        return c;
    }

    y.middle_distance.cmp(&x.middle_distance)
}
