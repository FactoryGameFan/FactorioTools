//! The asserts in `test/FactorioTools.Test/OilField/PlannerTest.cs` and
//! `PlannerIssueTest.cs` that need only the planner. The tests that check the emitted
//! blueprint string (`EmitsHeatPipesInTwoPointZeroBlueprint`,
//! `EmitsTwoPointZeroAndEntityQualityWithoutHeat`, `OmitsQualityFieldWhenNormal`,
//! `EmitsModuleQualityInItemsArray`) wait for `GridToBlueprintString` in stage 12.
//!
//! The tests that plan every small-list blueprint are `#[ignore]`d for debug builds; run them
//! with `cargo test --release -p snapshots --test planner_tests -- --include-ignored`.

use blueprint::parse_blueprint;
use planner::Location;
use planner::data::Direction;
use planner::grid::{EntityKind, SquareGrid};
use planner::models::PipeStrategy;
use planner::options::OilFieldOptions;
use planner::planner::{PlannerResult, execute, execute_sample};
use planner::quality::Quality;
use snapshots::small_list;

const BOXED_IN_HEAT_INDEX: usize = 55;
const BEACONS_COST_A_PUMPJACK_INDEX: usize = 35;

fn plan(options: OilFieldOptions, blueprint_string: &str) -> PlannerResult {
    execute(options, parse_blueprint(blueprint_string).unwrap()).unwrap()
}

fn count(grid: &SquareGrid, matches: impl Fn(&EntityKind) -> bool) -> usize {
    grid.entity_locations()
        .iter()
        .filter(|&l| grid.get(l).is_some_and(|e| matches(&e.kind)))
        .count()
}

fn poles(grid: &SquareGrid) -> usize {
    count(grid, |k| matches!(k, EntityKind::ElectricPoleCenter { .. }))
}

fn heat_pipes(grid: &SquareGrid) -> usize {
    count(grid, |k| matches!(k, EntityKind::HeatPipe))
}

fn beacon_centers(grid: &SquareGrid) -> usize {
    count(grid, |k| matches!(k, EntityKind::BeaconCenter))
}

const BLUEPRINTS_WITH_ISOLATED_AREAS: [&str; 2] = [
    "0eNqN1V1vgyAUBuD/cq65kCNQ8K8sS9NasrBVavxY1hj/+1R6sbUkvpciPh4+XpjofB1924U4UDVRqG+xp+ptoj58xNN1bYunxlNF7di0n6f6iwQN93ZtCYNvaBYU4sX/UCXnd0E+DmEIPhnbw/0Yx+bsu6WDyFjtrV8+uMX1TwtSsqD70rVc3EvofJ3eFbN44RjgpEmc2edKgOND4uw+pxBOJc7tcxoZrNs4LvY5gyxFqo7lPndABptWlvk/ZzOcRTid5XLVOYRzMCcLZC1s8p72scp5SC7kY7gaqI/xrffsZeuDklEkDwiaVHjSIA/JRimTdwA8JBzrpKyeBeYPSsfDA44CCcXD4p7D918JHAZc4OMtgdOAkXywyXq59WDGPfVyHix33HbvVX8uTkHfvuu3DtqwU85praVVRs3zLzJCaTc=",
    "0eNqVl91ugzAMRt8l11yQ2A6EV5mmqT/RlG2kqNBpVcW7jxIu1hWJL5el6WmwfezkpvZfF9+dQxxUc1PhcIq9al5uqg/vcfd1fxZ3rVeN6i5t97E7fKpCDdfu/iQMvlVjoUI8+h/V6PG1UD4OYQg+MeYP17d4aff+PC0oVljdqZ9+cIr3f5ogZAt1nZbSxD2Gsz+k78qxeMIZHGfKbRwhOJ1wehvHAM4sOLONEwTHCQfEziK4KuF4G1dlpALA1Rmxk0dcvYJzGZmV7d3pEuDp5W0twNMZyageebzGQ8QwlHg1sD/EDONmHgGiaUQNIpwneLmQBuIHySGJB7imETuMSTykXmq8niEe5EeqP6qAvlzi9UcO4GXMDS6382sy/GBkDhFeL4y8L+P5hXiC51cA34zF+58Ak9JUuL8QD/GDFx6SX4fPXgHmB0F+1DgP8iNjf4gfetkf0F8o42QlQH+hjKOVAPONBJ+//3lr/YUQP8jh+6sy6g/oB4T4oVO/ssjBGZof5SpvLX5c4vWM7I8zzldWbx8nOWN+WKC/MOKHXnjA6ZkhP1J/toC/LBn1AvjLNiN+T35Md8z53tn8ubgW6tuf+3mBWOPYORHRNU/RGn8BgZrbLA==",
];

#[test]
fn rejects_blueprint_with_blocking_isolated_area() {
    for blueprint_string in BLUEPRINTS_WITH_ISOLATED_AREAS {
        let error = execute(
            OilFieldOptions::for_medium_electric_pole(),
            parse_blueprint(blueprint_string).unwrap(),
        )
        .unwrap_err();
        assert!(error.is_no_path_between_terminals(), "{}", error.message);
    }
}

#[test]
fn allows_pumpjack_with_default_direction() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    let result = plan(
        options,
        "0eNqV1OtqgzAUAOB3Ob9DMTHHJL7KKMN2YWSrqXgZE8m7Tx1mgxp6/KmYz3PNBJfbYJvW+R7KCdz17jsoXybo3Luvbsu7fmwslOB6WwMDX9XLUzPUzUd1/YTAwPk3+w0lD2cG1veud/bXWB/GVz/UF9vOHzyeZtDcu/nA3S9/mhGp+QkZjFCKPDthCOyBERTG5M+YnMSojRFmn5EEBjOM0Yh9BknR/DHFPlOQShwZmYhGkZLKIpPvM5qUlIhMIilzrMQphmckJ3YcE4PD+bFeIU84lEFGHvcBE1XmlElGEbuFKuFIUl5FdBIbwQ/OciETDmmYjd4cpROOOlZnlaqPJtUZn8ZjSEtqNkcn+i5o8xzXS6/xzHf0epOX/y5+Bl+27dZDQnOpzBy7yDmiCOEHgSfwqQ==",
    );
    assert_eq!(result.summary.rotated_pumpjacks, 17);
}

#[test]
fn legendary_electric_poles_reduce_or_match_pole_count() {
    let small = small_list();
    let mut normal = OilFieldOptions::for_medium_electric_pole();
    normal.validate_solution = true;
    let normal_poles = poles(&plan(normal, &small[0]).context.grid);

    let mut legendary = OilFieldOptions::for_medium_electric_pole();
    legendary.validate_solution = true;
    legendary.electric_pole_quality = Quality::LEGENDARY;
    let legendary_poles = poles(&plan(legendary, &small[0]).context.grid);

    assert!(
        legendary_poles <= normal_poles,
        "legendary {legendary_poles} vs normal {normal_poles}"
    );
}

#[test]
fn allows_electric_poles_to_not_be_planned() {
    let mut options = OilFieldOptions::for_big_electric_pole();
    options.validate_solution = true;
    options.add_electric_poles = false;
    let grid = plan(options, &small_list()[0]).context.grid;
    assert_eq!(poles(&grid), 0);
    assert_eq!(
        count(&grid, |k| matches!(k, EntityKind::ElectricPoleSide { .. })),
        0
    );
}

#[test]
fn allows_beacons_to_not_be_planned() {
    let mut options = OilFieldOptions::for_big_electric_pole();
    options.validate_solution = true;
    options.add_beacons = false;
    let grid = plan(options, &small_list()[0]).context.grid;
    assert_eq!(beacon_centers(&grid), 0);
    assert_eq!(
        count(&grid, |k| matches!(k, EntityKind::BeaconSide { .. })),
        0
    );
}

#[test]
fn adds_heat_pipes_for_aquilo() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_heat_pipes = true;
    options.add_beacons = false;
    let result = plan(options, &small_list()[0]);
    assert_eq!(beacon_centers(&result.context.grid), 0);
    assert!(result.context.heat_pipes.is_some());
    assert!(heat_pipes(&result.context.grid) > 0);
}

#[test]
fn adds_heat_pipes_and_beacons_together_for_aquilo() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_heat_pipes = true;
    options.add_beacons = true;
    let result = plan(options, &small_list()[0]);
    assert!(result.context.heat_pipes.is_some());
    assert!(heat_pipes(&result.context.grid) > 0);
    assert!(beacon_centers(&result.context.grid) > 0);
}

fn heat_and_beacons(index: usize) -> PlannerResult {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_heat_pipes = true;
    options.add_beacons = true;
    plan(options, &small_list()[index])
}

#[test]
#[ignore]
fn heats_every_kept_beacon_when_beacons_and_heat_are_on() {
    for index in 0..small_list().len() {
        let result = heat_and_beacons(index);
        let grid = &result.context.grid;
        let width = result.context.options.beacon_width;
        let height = result.context.options.beacon_height;
        for location in grid.entity_locations().iter() {
            if !matches!(
                grid.get(location).map(|e| &e.kind),
                Some(EntityKind::BeaconCenter)
            ) {
                continue;
            }

            let min_x = location.x - ((width - 1) / 2);
            let max_x = location.x + (width / 2);
            let min_y = location.y - ((height - 1) / 2);
            let max_y = location.y + (height / 2);
            let mut heated = false;
            for x in min_x..=max_x {
                for y in min_y..=max_y {
                    for n in [
                        Location::new(x - 1, y),
                        Location::new(x + 1, y),
                        Location::new(x, y - 1),
                        Location::new(x, y + 1),
                    ] {
                        let inside = n.x >= min_x && n.x <= max_x && n.y >= min_y && n.y <= max_y;
                        if !inside
                            && grid.is_in_bounds(n)
                            && matches!(grid.get(n).map(|e| &e.kind), Some(EntityKind::HeatPipe))
                        {
                            heated = true;
                        }
                    }
                }
            }

            assert!(
                heated,
                "beacon at {location} (index {index}) is not heat-adjacent"
            );
        }
    }
}

/// `ValidatesBeaconsAreHeatedWhenValidationIsOn`: every small-list field plans with heat,
/// beacons and validation on.
#[test]
#[ignore]
fn validates_beacons_are_heated_when_validation_is_on() {
    for index in 0..small_list().len() {
        heat_and_beacons(index);
    }
}

#[test]
#[ignore]
fn enabling_beacons_never_forces_more_heat_drops() {
    for (index, blueprint_string) in small_list().iter().enumerate() {
        let mut heat_only = OilFieldOptions::for_medium_electric_pole();
        heat_only.validate_solution = true;
        heat_only.add_heat_pipes = true;
        heat_only.add_beacons = false;
        let heat_only = plan(heat_only, blueprint_string).summary;
        let both_on = heat_and_beacons(index).summary;

        if index == BEACONS_COST_A_PUMPJACK_INDEX {
            assert_eq!(heat_only.heat_dropped_pumpjacks, 0);
            assert_eq!(both_on.heat_dropped_pumpjacks, 1);
        } else {
            assert!(
                both_on.heat_dropped_pumpjacks <= heat_only.heat_dropped_pumpjacks,
                "index {index}: beacons on dropped {} vs heat-only {}",
                both_on.heat_dropped_pumpjacks,
                heat_only.heat_dropped_pumpjacks
            );
        }

        assert_eq!(both_on.unheated_pumpjacks, 0);
        assert_eq!(both_on.unheated_pipes, 0);
    }
}

#[test]
fn sets_pumpjack_center_direction() {
    let context = execute_sample().unwrap().context;
    let mut centers: Vec<(Location, Direction)> = context
        .grid
        .entity_locations()
        .iter()
        .filter_map(|l| match context.grid.get(l).map(|e| &e.kind) {
            Some(EntityKind::PumpjackCenter { direction }) => Some((l, *direction)),
            _ => None,
        })
        .collect();
    centers.sort_by_key(|(l, _)| (l.y, l.x));
    let directions: Vec<_> = centers.iter().map(|(_, d)| *d).collect();
    assert_eq!(
        directions,
        [
            Direction::Down,
            Direction::Left,
            Direction::Up,
            Direction::Up
        ]
    );
}

#[test]
fn sets_deltas_from_original_positions() {
    let context = execute_sample().unwrap().context;
    assert_eq!(context.delta_x, 16.0);
    assert_eq!(context.delta_y, 13.0);
}

#[test]
fn counts_rotated_pumpjacks() {
    for (blueprint_string, expected) in [
        (
            "0eJyNkMsOgjAQRf/lrisJBcR26W8YY3hMTBVKU4qRkP67BaIxsnE3jztn7syEshnIWKUd5ARVdbqHPE3o1VUXzVxzoyFIKEctGHTRzpkZWnMrqjs8g9I1PSFjf2Yg7ZRTtDKWZLzooS3JBsF2msF0fRjo9LwpQHZJlDGMIciiLLBrZala+3vPNkj+B/JNTH+BfDa8nCW/vsDwINuvgkOc5oLnKRciEcF+U5QUfoLjR+39C6d7aOc=",
            2,
        ),
        (
            "0eJyNkE0OgjAQhe/y1pWECkG69BrGmAITU6WlocVISO9ugWiMbNzNz5vvzcyEqh3I9sp4iAmq7oyDOE1w6mpkO9f8aAkCypMGg5F6zuyg7U3WdwQGZRp6QqThzEDGK69oZSzJeDGDrqiPgu00g+1cHOjM7BQhu32SM4wxyJM8shvVU732eWAbJP8D+SZmv8BsXng5S3x9geFBvVsdD2lWlLzIeFnuy7h+KyuKP8Hxow7hBaWraOU=",
            1,
        ),
        (
            "0eJyMkNsKwjAMht8l13Ww2Q7XVxGRHYJE16ysnThG3912QxD1wrvkz58vhwWafkI7EnvQC1A7sAN9XMDRhes+aVwbBA12MvZatzcQ4GebFPJoIAgg7vABOg8nAciePOHGWJP5zJNpcIwG8YNlBxcbBk6TImS3z5SAOQYqU5Hd0YjtVpdBfCGLP5AvovwE5kXaeL1Cv71BwB1HtzpUWVSyqpRS+UGWMoQnAAAA//8DAB7EYng=",
            0,
        ),
    ] {
        let summary = plan(
            OilFieldOptions::for_medium_electric_pole(),
            blueprint_string,
        )
        .summary;
        assert_eq!(summary.rotated_pumpjacks, expected);
    }
}

#[test]
fn yields_alternate_solutions() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    let summary = plan(options, "0eJyM0ksKwyAQBuC7zNpFNZqHVyml5DEU28RIYkpD8O5NdFNIQZeO4yfM/Bs0/YJmUtqC3EC1o55BXjeY1UPX/VHT9YAgwSyDedbtCwjY1RwVZXEAR0DpDj8gqbsRQG2VVRgMf1jvehkanPYG8scy47w/GPXx04FwAivIcmc7NWEbri6OnDSWoJUeoyyuZQlaRgOXxTmewHEWOB7nRPrkaB7n8hTuErgizhUJHOPJXJnCFZ5jp83uEfSxlD+5JvDGafYNImcVryohBC15zp37AgAA//8DABZt+/A=").summary;
    assert_eq!(summary.selected_plans.len(), 1);
    assert_eq!(summary.alternate_plans.len(), 1);
    assert!(!summary.unused_plans.is_empty());
}

#[test]
fn heat_router_does_not_strand_reachable_pipes_behind_enclosed_seed() {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_heat_pipes = true;
    options.add_beacons = false;
    let result = plan(options, &small_list()[6]);
    assert_eq!(result.summary.heat_dropped_pumpjacks, 0);
    assert!(result.context.heat_pipes.as_ref().unwrap().len() > 1);
}

fn heat_only(index: usize, validate: bool) -> PlannerResult {
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = validate;
    options.add_heat_pipes = true;
    options.add_beacons = false;
    plan(options, &small_list()[index])
}

#[test]
fn drops_pumpjacks_to_fully_heat_boxed_in_field() {
    let result = heat_only(BOXED_IN_HEAT_INDEX, true);
    assert!(result.summary.heat_dropped_pumpjacks > 0);
    assert_eq!(result.summary.unheated_pumpjacks, 0);
    assert_eq!(result.summary.unheated_pipes, 0);
    assert!(heat_pipes(&result.context.grid) > 0);
}

#[test]
#[ignore]
fn heat_only_fully_heats_every_field_and_rarely_drops() {
    let mut zero_drop = 0;
    for index in 0..small_list().len() {
        let summary = heat_only(index, true).summary;
        assert_eq!(summary.unheated_pumpjacks, 0);
        assert_eq!(summary.unheated_pipes, 0);
        if summary.heat_dropped_pumpjacks == 0 {
            zero_drop += 1;
        }
    }
    assert!(zero_drop >= 34, "{zero_drop}");
}

#[test]
fn heat_on_boxed_in_field_drops_pumpjacks_and_fully_heats() {
    let summary = heat_only(BOXED_IN_HEAT_INDEX, false).summary;
    assert!(summary.heat_dropped_pumpjacks > 0);
    assert_eq!(summary.unheated_pumpjacks, 0);
    assert_eq!(summary.unheated_pipes, 0);
}

#[test]
fn plans_electric_poles() {
    let mut options = OilFieldOptions::for_substation();
    options.validate_solution = true;
    let grid = plan(options, "0eJyM00sOgyAQANC7zJqFgp+WqzRN42fS0CoSwabGePeidNHEJsySYXjM8Fmg7iY0o9IO5AKqGbQFeVnAqruuui2mqx5Bgpl686iaJzBws9kiymEPKwOlW3yDTNcrA9ROOYXB2AfzTU99jaNPYH8sM1i/YNDbTh4RGYPZpwrvtmrEJswlKztwnMDxPHBZnBMELi0CV8S5jFJdErgyzuWU6sTO8TTOFQQuS8hcSWk2nB0n3OyJ0my4WZ7HuTO9WXGozr/p/Z3Ln4/C4IWj/SasHwAAAP//AwCxgxNI").context.grid;
    assert!(poles(&grid) > 0);
    assert!(count(&grid, |k| matches!(k, EntityKind::ElectricPoleSide { .. })) > 0);
}

#[test]
fn fbe_original_falls_back_to_fbe() {
    for blueprint_string in [
        // FbeOriginalFallsBackToFbeWhenLeftoverPumpsCannotConnect (big-list index 827).
        "0eJyU1MFuwyAMANB/8ZlDIEADvzJNU5qiia4hKCHTooh/H4HLqrbCOwLmAcL2DufbavxsXQC9gx0mt4B+22Gxn66/HXOuHw1o8Ovor/3wBQTC5o8ZG8wIkYB1F/MDmsZ3AsYFG6wpRh5sH24dz2ZOAeSJ5aclbZjccVJCGCOwge4Se7GzGcpSE8mDxhAaFVlTda3FaAqrcYx2yhpl9xx/wgkMxwvX1m8nEVx5KuV17YT51RbNdRiuKZyscwrBdUVD5Bxt/vGxGA9VEuW1rEF4qKKQxaP1xKOYsiipwhCJR1F10b7wUnfJHUf/aVkEvs285AAhmeJKCSFoxyWP8RcAAP//AwC8p5dm",
        // FbeOriginalFallsBackToFbeWhenAloneGroupRemains.
        "0eNqVlsluhDAQRP+lzxywu22WX4miaBYrcjJ4EEuU0Yh/D4M5RBNHFEeMebTbVWXf6XgZXdv5MFB9J3+6hp7qlzv1/j0cLo+xcGgc1dSOTftxOH1SRsOtfYz4wTU0ZeTD2X1TrabXjFwY/OBdZCwPt7cwNkfXzROyBKu99vMH1/D40wzhMqPbPJVn7tl37hTf5VP2B6cBnMkjzmzjGKnORly5jRMAZ9fqqm2cAXCiFpzOt3EW6Z2OOLWNKxBcEXF6G1ciW2EiDhBKhfSugHEqR8qrIk8AHuILxZEHCFkhxtBrfRbgIc4Qk+RJiodYg9f1FkB9kDdynLfHHAgPcQev+gOSRSH2kLV/FbAfkD/KJC+Zyzv8wUC4aIXHAQPpojWeB8+8VP80dHJE/TFyriH+EI3zDH5QQjy7Y3+f8q9M8Yod67UAr8TzioH804g/WCIPyAPOd+gFyANWO/QM+Jc1ns8C+JcZ14sA+mPB80CQ9ULnh/zDm++8yz24/nWRzujLdf0ywVhdSVUZY1QpVqbpB1ASvGc=",
    ] {
        let mut options = OilFieldOptions::for_medium_electric_pole();
        options.validate_solution = true;
        options.pipe_strategies = vec![PipeStrategy::FbeOriginal];
        let summary = plan(options, blueprint_string).summary;
        assert_eq!(summary.selected_plans.len(), 1);
        assert_eq!(summary.selected_plans[0].pipe_strategy, PipeStrategy::Fbe);
    }
}
