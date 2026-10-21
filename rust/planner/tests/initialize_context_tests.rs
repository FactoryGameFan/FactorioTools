//! Ports of `test/FactorioTools.Test/OilField/Steps/InitializeContextTest.cs` and
//! `Steps/ElectricPoleQualityScalingTest.cs`.

use planner::Location;
use planner::context::Context;
use planner::data::{Blueprint, Entity, Position};
use planner::models::AvoidLocation;
use planner::options::OilFieldOptions;
use planner::quality::Quality;
use planner::steps::initialize_context::{execute, execute_with_min, get_empty};

fn small_power_no_beacon() -> OilFieldOptions {
    OilFieldOptions {
        add_beacons: false,
        ..OilFieldOptions::for_medium_electric_pole()
    }
}

fn big_power_no_beacon() -> OilFieldOptions {
    OilFieldOptions {
        add_beacons: false,
        ..OilFieldOptions::for_substation()
    }
}

fn small_power_with_beacon() -> OilFieldOptions {
    OilFieldOptions {
        add_beacons: true,
        ..OilFieldOptions::for_medium_electric_pole()
    }
}

fn big_power_with_beacon() -> OilFieldOptions {
    OilFieldOptions {
        add_beacons: true,
        ..OilFieldOptions::for_substation()
    }
}

fn non_standard_beacon(options: OilFieldOptions) -> OilFieldOptions {
    OilFieldOptions {
        add_beacons: true,
        beacon_width: 2,
        beacon_height: 4,
        beacon_supply_width: 4,
        beacon_supply_height: 6,
        ..options
    }
}

/// `BlueprintWithCenters`: pumpjacks at the given positions and nothing else.
fn blueprint_with_centers(centers: &[(f32, f32)]) -> Blueprint {
    Blueprint {
        entities: centers
            .iter()
            .map(|&(x, y)| Entity {
                name: "pumpjack".to_string(),
                position: Position { x, y },
                ..Entity::default()
            })
            .collect(),
        ..Blueprint::default()
    }
}

fn assert_shape(context: &Context, width: i32, height: i32, delta_x: f32, delta_y: f32) {
    assert_eq!(context.grid.width(), width);
    assert_eq!(context.grid.height(), height);
    assert_eq!(context.delta_x, delta_x);
    assert_eq!(context.delta_y, delta_y);
}

const ONE: &[(f32, f32)] = &[(25.5, -14.5)];
const TWO: &[(f32, f32)] = &[(25.5, -14.5), (33.5, -12.5)];

#[test]
fn empty() {
    for (width, height) in [(0, 0), (0, 1), (1, 0), (1, 1), (5, 3)] {
        let context = get_empty(small_power_no_beacon(), width, height).unwrap();
        assert_shape(&context, width, height, 0.0, 0.0);
    }
}

#[test]
fn one_pumpjack() {
    let cases = [
        (
            small_power_no_beacon(),
            7,
            7,
            -22.5,
            17.5,
            Location::new(3, 3),
        ),
        (
            big_power_no_beacon(),
            9,
            9,
            -21.5,
            18.5,
            Location::new(4, 4),
        ),
        (
            small_power_with_beacon(),
            17,
            17,
            -17.5,
            22.5,
            Location::new(8, 8),
        ),
        (
            big_power_with_beacon(),
            19,
            19,
            -16.5,
            23.5,
            Location::new(9, 9),
        ),
        (
            non_standard_beacon(OilFieldOptions::for_medium_electric_pole()),
            11,
            15,
            -20.5,
            21.5,
            Location::new(5, 7),
        ),
        (
            non_standard_beacon(OilFieldOptions::for_substation()),
            13,
            17,
            -19.5,
            22.5,
            Location::new(6, 8),
        ),
    ];

    for (options, width, height, dx, dy, center) in cases {
        let context = execute(options, blueprint_with_centers(ONE), &[]).unwrap();
        assert_shape(&context, width, height, dx, dy);
        assert_eq!(context.centers, vec![center]);
    }
}

#[test]
fn two_pumpjacks() {
    let context = execute(small_power_with_beacon(), blueprint_with_centers(TWO), &[]).unwrap();
    assert_shape(&context, 25, 19, -17.5, 22.5);
    assert_eq!(
        context.centers,
        vec![Location::new(8, 8), Location::new(16, 10)]
    );

    let context = execute(big_power_with_beacon(), blueprint_with_centers(TWO), &[]).unwrap();
    assert_shape(&context, 27, 21, -16.5, 23.5);
    assert_eq!(
        context.centers,
        vec![Location::new(9, 9), Location::new(17, 11)]
    );
}

/// (avoid, width, height, deltaX, deltaY, centers)
type AvoidCase = (Vec<AvoidLocation>, i32, i32, f32, f32, [Location; 2]);

#[test]
fn avoid_locations_grow_the_grid() {
    let cases: [AvoidCase; 6] = [
        // AvoidAtMaxXMaxYBeaconBound
        (
            vec![AvoidLocation::new(39.5, -6.5)],
            27,
            21,
            -16.5,
            23.5,
            [Location::new(9, 9), Location::new(17, 11)],
        ),
        // AvoidLessThanMinY
        (
            vec![AvoidLocation::new(29.5, -21.5)],
            27,
            22,
            -16.5,
            24.5,
            [Location::new(9, 10), Location::new(17, 12)],
        ),
        // AvoidGreaterThanMaxY
        (
            vec![AvoidLocation::new(29.5, -5.5)],
            27,
            22,
            -16.5,
            23.5,
            [Location::new(9, 9), Location::new(17, 11)],
        ),
        // AvoidLessThanMinX
        (
            vec![AvoidLocation::new(18.5, -13.5)],
            28,
            21,
            -15.5,
            23.5,
            [Location::new(10, 9), Location::new(18, 11)],
        ),
        // AvoidGreaterThanMaxX
        (
            vec![AvoidLocation::new(40.5, -13.5)],
            28,
            21,
            -16.5,
            23.5,
            [Location::new(9, 9), Location::new(17, 11)],
        ),
        // AvoidGreaterThanAllBeaconBounds
        (
            all_sides_avoid(),
            35,
            29,
            -12.5,
            27.5,
            [Location::new(13, 13), Location::new(21, 15)],
        ),
    ];

    for (avoid, width, height, dx, dy, centers) in cases {
        let context =
            execute(big_power_with_beacon(), blueprint_with_centers(TWO), &avoid).unwrap();
        assert_shape(&context, width, height, dx, dy);
        assert_eq!(context.centers, centers.to_vec());
    }
}

fn all_sides_avoid() -> Vec<AvoidLocation> {
    vec![
        AvoidLocation::new(29.5, -24.5),
        AvoidLocation::new(43.5, -13.5),
        AvoidLocation::new(29.5, -2.5),
        AvoidLocation::new(15.5, -13.5),
    ]
}

#[test]
fn larger_min_with_no_avoid() {
    let context = execute_with_min(
        big_power_with_beacon(),
        blueprint_with_centers(TWO),
        &[],
        45,
        0,
    )
    .unwrap();
    assert_shape(&context, 45, 21, -7.5, 23.5);
    assert_eq!(
        context.centers,
        vec![Location::new(18, 9), Location::new(26, 11)]
    );
}

#[test]
fn larger_min_width_and_min_height_with_avoid() {
    let context = execute_with_min(
        big_power_with_beacon(),
        blueprint_with_centers(TWO),
        &all_sides_avoid(),
        50,
        40,
    )
    .unwrap();
    assert_shape(&context, 50, 40, -5.5, 32.5);
    assert_eq!(
        context.centers,
        vec![Location::new(20, 18), Location::new(28, 20)]
    );
}

fn assert_pole_quality(context: &Context, supply_width: i32, supply_height: i32, reach: f64) {
    assert_eq!(
        context.electric_pole_supply_width_with_quality,
        supply_width
    );
    assert_eq!(
        context.electric_pole_supply_height_with_quality,
        supply_height
    );
    assert_eq!(context.electric_pole_wire_reach_with_quality, reach);
    assert_eq!(
        context.electric_pole_wire_reach_squared_with_quality,
        reach * reach
    );
}

// InitializeContextTest.ElectricPoleQualityScalesSupplyAndReach.
#[test]
fn electric_pole_quality_scales_supply_and_reach() {
    let cases = [
        (7, 9.0, Quality::NORMAL, 7, 9.0),
        (7, 9.0, Quality::UNCOMMON, 9, 11.0),
        (7, 9.0, Quality::RARE, 11, 13.0),
        (7, 9.0, Quality::EPIC, 13, 15.0),
        (7, 9.0, Quality::LEGENDARY, 17, 19.0),
        (18, 18.0, Quality::LEGENDARY, 28, 28.0),
    ];

    for (base_supply, base_reach, quality, supply, reach) in cases {
        let options = OilFieldOptions {
            electric_pole_supply_width: base_supply,
            electric_pole_supply_height: base_supply,
            electric_pole_wire_reach: base_reach,
            electric_pole_quality: quality,
            ..OilFieldOptions::default()
        };
        let context = get_empty(options, 10, 10).unwrap();
        assert_pole_quality(&context, supply, supply, reach);
    }
}

// ElectricPoleQualityScalingTest.BigElectricPole_ScalesGeometryByQuality and
// MediumElectricPole_ScalesGeometryByQuality.
#[test]
fn presets_scale_geometry_by_quality() {
    let cases = [
        (
            OilFieldOptions::for_big_electric_pole as fn() -> OilFieldOptions,
            Quality::NORMAL,
            4,
            32.0,
            1024.0,
        ),
        (
            OilFieldOptions::for_big_electric_pole,
            Quality::UNCOMMON,
            6,
            34.0,
            1156.0,
        ),
        (
            OilFieldOptions::for_big_electric_pole,
            Quality::RARE,
            8,
            36.0,
            1296.0,
        ),
        (
            OilFieldOptions::for_big_electric_pole,
            Quality::EPIC,
            10,
            38.0,
            1444.0,
        ),
        (
            OilFieldOptions::for_big_electric_pole,
            Quality::LEGENDARY,
            14,
            42.0,
            1764.0,
        ),
        (
            OilFieldOptions::for_medium_electric_pole,
            Quality::NORMAL,
            7,
            9.0,
            81.0,
        ),
        (
            OilFieldOptions::for_medium_electric_pole,
            Quality::RARE,
            11,
            13.0,
            169.0,
        ),
        (
            OilFieldOptions::for_medium_electric_pole,
            Quality::LEGENDARY,
            17,
            19.0,
            361.0,
        ),
    ];

    for (preset, quality, supply, reach, reach_squared) in cases {
        let options = OilFieldOptions {
            electric_pole_quality: quality,
            ..preset()
        };
        let context = get_empty(options, 20, 20).unwrap();
        assert_pole_quality(&context, supply, supply, reach);
        assert_eq!(
            context.electric_pole_wire_reach_squared_with_quality,
            reach_squared
        );
    }
}

// ElectricPoleQualityScalingTest.SupplyWidthAndHeight_ScaleIndependently.
#[test]
fn supply_width_and_height_scale_independently() {
    let options = OilFieldOptions {
        electric_pole_supply_width: 4,
        electric_pole_supply_height: 6,
        electric_pole_quality: Quality::RARE,
        ..OilFieldOptions::for_big_electric_pole()
    };
    let context = get_empty(options, 20, 20).unwrap();
    assert_eq!(context.electric_pole_supply_width_with_quality, 8);
    assert_eq!(context.electric_pole_supply_height_with_quality, 10);
}

// ElectricPoleQualityScalingTest.NormalQuality_LeavesBaseGeometryUnchanged.
#[test]
fn normal_quality_leaves_base_geometry_unchanged() {
    let options = OilFieldOptions {
        electric_pole_quality: Quality::NORMAL,
        ..OilFieldOptions::for_big_electric_pole()
    };
    let (width, height, reach) = (
        options.electric_pole_supply_width,
        options.electric_pole_supply_height,
        options.electric_pole_wire_reach,
    );
    let context = get_empty(options, 20, 20).unwrap();
    assert_pole_quality(&context, width, height, reach);
}
