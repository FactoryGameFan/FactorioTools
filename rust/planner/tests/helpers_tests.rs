//! Ports of `test/FactorioTools.Test/OilField/Steps/HelpersTest.cs` and
//! `test/FactorioTools.Test/OilField/HelpersGeometryTest.cs`.

use planner::Location;
use planner::containers::CountedBitArray;
use planner::context::Context;
use planner::grid::{EntityKind, GridEntity};
use planner::helpers::{
    self, CandidateToCovered, PUMPJACK_HEIGHT, PUMPJACK_WIDTH, add_provider_to_grid,
    are_locations_collinear, count_turns, get_electric_pole_candidate_to_covered,
    get_provider_center_to_covered_centers, make_straight_line,
};
use planner::models::{CandidateFactory, CandidateInfo, ProviderRecipient};
use planner::options::OilFieldOptions;
use planner::steps::initialize_context::get_empty;

fn l(x: i32, y: i32) -> Location {
    Location::new(x, y)
}

/// `BaseTest.AddElectricPole`.
fn add_electric_pole(context: &mut Context, center: Location) {
    let entity = GridEntity::electric_pole_center(context.grid.get_id());
    let (width, height) = (
        context.options.electric_pole_width,
        context.options.electric_pole_height,
    );
    add_provider_to_grid(
        &mut context.grid,
        center,
        entity,
        |grid, c| GridEntity::new(grid.get_id(), EntityKind::ElectricPoleSide { center: c }),
        width,
        height,
    )
    .unwrap();
}

/// `BaseTest.AddBeacon`.
fn add_beacon(context: &mut Context, center: Location) {
    let entity = GridEntity::new(context.grid.get_id(), EntityKind::BeaconCenter);
    let (width, height) = (context.options.beacon_width, context.options.beacon_height);
    add_provider_to_grid(
        &mut context.grid,
        center,
        entity,
        |grid, c| GridEntity::new(grid.get_id(), EntityKind::BeaconSide { center: c }),
        width,
        height,
    )
    .unwrap();
}

/// `BaseTest.AddPumpjack`, for the grid only. The C# helper also re-derives and picks
/// terminals, which these tests never look at and which places no entity on an empty grid.
fn add_pumpjack(context: &mut Context, center: Location) {
    helpers::add_pumpjack(&mut context.grid, center).unwrap();
}

fn covered_centers_of(
    options: OilFieldOptions,
    size: i32,
    center: Location,
    pole: bool,
    others: &[(i32, i32)],
) -> Vec<Location> {
    let mut context = get_empty(options, size, size).unwrap();
    if pole {
        add_electric_pole(&mut context, center);
        for &(x, y) in others {
            add_beacon(&mut context, l(x, y));
        }
    } else {
        add_beacon(&mut context, center);
        for &(x, y) in others {
            add_pumpjack(&mut context, l(x, y));
        }
    }

    let o = &context.options;
    let (w, h, sw, sh) = if pole {
        (
            o.electric_pole_width,
            o.electric_pole_height,
            o.electric_pole_supply_width,
            o.electric_pole_supply_height,
        )
    } else {
        (
            o.beacon_width,
            o.beacon_height,
            o.beacon_supply_width,
            o.beacon_supply_height,
        )
    };
    let result =
        get_provider_center_to_covered_centers(&context, w, h, sw, sh, [center], true, pole);
    assert_eq!(result.len(), 1);
    let (provider_center, covered) = result.iter().next().unwrap();
    assert_eq!(provider_center, center);
    covered.to_vec()
}

fn assert_covers(covered: &[Location], expected: &[(i32, i32)]) {
    assert_eq!(covered.len(), expected.len(), "{covered:?}");
    for &(x, y) in expected {
        assert!(
            covered.contains(&l(x, y)),
            "missing ({x}, {y}) in {covered:?}"
        );
    }
}

#[test]
fn provider_coverage_small_electric_pole() {
    let covered = covered_centers_of(
        OilFieldOptions::for_small_electric_pole(),
        20,
        l(10, 10),
        true,
        &[
            (11, 12),
            (12, 6),
            (12, 9),
            (14, 12),
            (16, 7),
            (6, 9),
            (7, 13),
            (9, 8),
        ],
    );
    assert_covers(&covered, &[(11, 12), (12, 9), (7, 13), (9, 8)]);
}

#[test]
fn provider_coverage_medium_electric_pole() {
    let covered = covered_centers_of(
        OilFieldOptions::for_medium_electric_pole(),
        20,
        l(10, 10),
        true,
        &[
            (12, 9),
            (13, 6),
            (14, 14),
            (15, 10),
            (16, 7),
            (5, 9),
            (6, 14),
            (6, 6),
            (9, 12),
            (9, 7),
        ],
    );
    assert_covers(
        &covered,
        &[(12, 9), (13, 6), (14, 14), (6, 14), (6, 6), (9, 12), (9, 7)],
    );
}

#[test]
fn provider_coverage_substation() {
    let covered = covered_centers_of(
        OilFieldOptions::for_substation(),
        28,
        l(12, 12),
        true,
        &[
            (11, 15),
            (11, 18),
            (11, 21),
            (11, 24),
            (15, 3),
            (19, 16),
            (2, 13),
            (2, 2),
            (2, 5),
            (20, 13),
            (21, 10),
            (22, 22),
            (22, 7),
            (23, 4),
            (3, 22),
            (4, 8),
            (6, 17),
            (9, 9),
        ],
    );
    assert_covers(
        &covered,
        &[
            (11, 15),
            (11, 18),
            (11, 21),
            (15, 3),
            (19, 16),
            (20, 13),
            (21, 10),
            (22, 22),
            (22, 7),
            (3, 22),
            (4, 8),
            (6, 17),
            (9, 9),
        ],
    );
}

#[test]
fn provider_coverage_big_electric_pole() {
    let covered = covered_centers_of(
        OilFieldOptions::for_big_electric_pole(),
        20,
        l(10, 10),
        true,
        &[(7, 11), (10, 8), (10, 13), (13, 12), (14, 9)],
    );
    assert_covers(&covered, &[(10, 13), (10, 8), (13, 12)]);
}

#[test]
fn provider_coverage_beacon() {
    let covered = covered_centers_of(
        OilFieldOptions::for_small_electric_pole(),
        20,
        l(10, 10),
        false,
        &[
            (10, 13),
            (10, 16),
            (12, 4),
            (13, 11),
            (15, 15),
            (15, 6),
            (4, 14),
            (5, 11),
            (5, 5),
            (6, 8),
            (9, 7),
        ],
    );
    assert_covers(
        &covered,
        &[
            (15, 15),
            (15, 6),
            (10, 13),
            (13, 11),
            (5, 11),
            (5, 5),
            (6, 8),
            (9, 7),
        ],
    );
}

struct Factory;

impl CandidateFactory<CandidateInfo> for Factory {
    fn create(&self, covered: CountedBitArray) -> CandidateInfo {
        CandidateInfo::new(covered)
    }
}

fn pumpjacks(centers: &[(i32, i32)]) -> Vec<ProviderRecipient> {
    centers
        .iter()
        .map(|&(x, y)| ProviderRecipient::new(l(x, y), PUMPJACK_WIDTH, PUMPJACK_HEIGHT))
        .collect()
}

#[test]
fn small_electric_pole_remove_unused() {
    let mut context = get_empty(OilFieldOptions::for_small_electric_pole(), 20, 7).unwrap();
    add_electric_pole(&mut context, l(1, 3));
    add_electric_pole(&mut context, l(5, 3));
    add_electric_pole(&mut context, l(8, 3));
    let recipients = pumpjacks(&[(3, 2), (13, 2)]);
    for r in &recipients {
        add_pumpjack(&mut context, r.center);
    }

    let CandidateToCovered {
        candidate_to_info,
        covered_entities,
        providers,
    } = get_electric_pole_candidate_to_covered(&mut context, &recipients, &Factory, true);

    let is_pole = |c: &Context, x, y| {
        matches!(
            c.grid.get(l(x, y)).map(|e| &e.kind),
            Some(EntityKind::ElectricPoleCenter { .. })
        )
    };
    assert!(is_pole(&context, 1, 3));
    assert!(is_pole(&context, 5, 3));
    assert!(context.grid.is_empty(l(8, 3)));

    assert_eq!(providers.len(), 2);
    assert!(providers.contains_key(l(1, 3)));
    assert!(providers.contains_key(l(5, 3)));

    assert_eq!(covered_entities.count(), 2);
    assert!(covered_entities.get(0));
    assert!(!covered_entities.get(1));

    // Columns to the left and right of the pumpjack.
    for x in [10, 11, 15, 16] {
        for y in 0..6 {
            assert!(candidate_to_info.contains_key(l(x, y)), "({x}, {y})");
        }
    }

    // Columns broken up by the pumpjack itself. The C# test checks (12, 0) for every column,
    // then rows 4 and 5 of each.
    assert!(candidate_to_info.contains_key(l(12, 0)));
    for x in [12, 13, 14] {
        for y in 4..6 {
            assert!(candidate_to_info.contains_key(l(x, y)), "({x}, {y})");
        }
    }

    assert_eq!(candidate_to_info.len(), 33);
    for info in candidate_to_info.values() {
        let info = info.borrow();
        assert_eq!(info.covered.count(), 2);
        assert!(!info.covered.get(0));
        assert!(info.covered.get(1));
    }
}

#[test]
fn substation_remove_unused() {
    let mut context = get_empty(OilFieldOptions::for_substation(), 16, 16).unwrap();
    add_electric_pole(&mut context, l(10, 12));
    add_electric_pole(&mut context, l(12, 10));
    add_electric_pole(&mut context, l(12, 12));
    let recipients = pumpjacks(&[(2, 2)]);
    for r in &recipients {
        add_pumpjack(&mut context, r.center);
    }

    let CandidateToCovered {
        candidate_to_info,
        providers,
        ..
    } = get_electric_pole_candidate_to_covered(&mut context, &recipients, &Factory, true);

    for location in context.grid.entity_locations().iter() {
        assert!(matches!(
            context.grid.get(location).unwrap().kind,
            EntityKind::PumpjackSide { .. } | EntityKind::PumpjackCenter { .. }
        ));
    }
    assert_eq!(providers.len(), 0);

    // Spots that become candidates once the unused substations are removed.
    for (x, y) in [(9, 11), (10, 11), (11, 9), (11, 10), (11, 11)] {
        assert!(candidate_to_info.contains_key(l(x, y)), "({x}, {y})");
    }

    assert_eq!(candidate_to_info.len(), 128);
    for info in candidate_to_info.values() {
        let info = info.borrow();
        assert_eq!(info.covered.count(), 1);
        assert!(info.covered.get(0));
    }

    // Columns blocked at the top by the pumpjack itself.
    for x in 0..4 {
        for y in 4..12 {
            assert!(candidate_to_info.contains_key(l(x, y)), "({x}, {y})");
        }
    }

    // Columns not blocked by the pumpjack.
    for x in 4..12 {
        for y in 0..12 {
            assert!(candidate_to_info.contains_key(l(x, y)), "({x}, {y})");
        }
    }
}

#[test]
fn straight_line_horizontal_left_to_right() {
    assert_eq!(
        make_straight_line(l(0, 0), l(3, 0)),
        vec![l(0, 0), l(1, 0), l(2, 0), l(3, 0)]
    );
}

#[test]
fn straight_line_vertical_top_to_bottom() {
    assert_eq!(
        make_straight_line(l(0, 0), l(0, 3)),
        vec![l(0, 0), l(0, 1), l(0, 2), l(0, 3)]
    );
}

#[test]
fn straight_line_reversed_endpoints_still_ascend() {
    assert_eq!(
        make_straight_line(l(0, 3), l(0, 0)),
        vec![l(0, 0), l(0, 1), l(0, 2), l(0, 3)]
    );
}

#[test]
fn straight_line_same_point() {
    assert_eq!(make_straight_line(l(2, 2), l(2, 2)), vec![l(2, 2)]);
}

#[test]
#[should_panic(expected = "ArgumentException")]
fn straight_line_diagonal_throws() {
    make_straight_line(l(0, 0), l(1, 1));
}

#[test]
fn count_turns_straight_line() {
    assert_eq!(count_turns(&[l(0, 0), l(1, 0), l(2, 0)]), 0);
}

#[test]
fn count_turns_single_corner() {
    assert_eq!(count_turns(&[l(0, 0), l(1, 0), l(1, 1)]), 1);
}

#[test]
fn count_turns_staircase() {
    assert_eq!(count_turns(&[l(0, 0), l(1, 0), l(1, 1), l(2, 1)]), 2);
}

#[test]
fn count_turns_single_point() {
    assert_eq!(count_turns(&[l(0, 0)]), 0);
}

#[test]
fn collinear_diagonal() {
    assert!(are_locations_collinear(&[l(0, 0), l(1, 1), l(2, 2)]));
}

#[test]
fn collinear_horizontal() {
    assert!(are_locations_collinear(&[l(0, 0), l(1, 0), l(2, 0)]));
}

#[test]
fn collinear_vertical() {
    assert!(are_locations_collinear(&[l(0, 0), l(0, 1), l(0, 2)]));
}

#[test]
fn not_collinear_bent_path() {
    assert!(!are_locations_collinear(&[l(0, 0), l(1, 0), l(1, 1)]));
}

#[test]
fn collinear_single_point() {
    assert!(are_locations_collinear(&[l(5, 5)]));
}
