//! `test/FactorioTools.Test/OilField/Steps/PlanUndergroundPipesTest.cs`, ported: every
//! theory and fact, with the same inline data.

use planner::Location;
use planner::containers::LocationDictionary;
use planner::context::Context;
use planner::data::Direction;
use planner::grid::{EntityKind, GridEntity, TerminalLocation};
use planner::helpers::{
    add_pumpjack, eliminate_other_terminals, populate_center_to_terminals,
    populate_location_to_terminals,
};
use planner::options::OilFieldOptions;
use planner::steps::{add_pipe_entities, initialize_context, plan_underground_pipes};

fn add_pipes(pipes: &mut Vec<Location>, min_x: i32, max_x: i32, min_y: i32, max_y: i32) {
    for x in min_x..=max_x {
        for y in min_y..=max_y {
            pipes.push(Location::new(x, y));
        }
    }
}

fn get_context(pipes: &[Location], width: Option<i32>, height: Option<i32>) -> Context {
    let width = width.unwrap_or_else(|| pipes.iter().map(|l| l.x).max().unwrap() + 2);
    let height = height.unwrap_or_else(|| pipes.iter().map(|l| l.y).max().unwrap() + 2);
    initialize_context::get_empty(OilFieldOptions::for_medium_electric_pole(), width, height)
        .unwrap()
}

/// `BaseTest.AddPumpjack`: places a pumpjack, recomputes every center's terminals, and keeps
/// one per pumpjack (this one's facing `direction`, the others' as before).
fn add_pumpjack_facing(context: &mut Context, center: Location, direction: Direction) {
    add_pumpjack(&mut context.grid, center).unwrap();

    let previous: Vec<(Location, Direction)> = context
        .center_to_terminals
        .iter()
        .map(|(c, t)| (c, t[0].direction))
        .collect();

    let mut centers: Vec<Location> = context.center_to_terminals.keys().collect();
    if !centers.contains(&center) {
        centers.push(center);
    }

    let mut center_to_terminals = LocationDictionary::new();
    populate_center_to_terminals(&mut center_to_terminals, &context.grid, &centers).unwrap();
    let mut location_to_terminals = LocationDictionary::new();
    populate_location_to_terminals(&mut location_to_terminals, &center_to_terminals);
    context.center_to_terminals = center_to_terminals;
    context.location_to_terminals = location_to_terminals;

    let pairs: Vec<(Location, Vec<TerminalLocation>)> = context
        .center_to_terminals
        .iter()
        .map(|(c, t)| (c, t.clone()))
        .collect();
    for (other_center, terminals) in pairs {
        let selected_direction = if other_center == center {
            direction
        } else {
            previous.iter().find(|(c, _)| *c == other_center).unwrap().1
        };
        // OrderByDescending(t => t.Direction == selectedDirection).First(): stable, so the
        // first match, or the first terminal when none match.
        let selected = *terminals
            .iter()
            .find(|t| t.direction == selected_direction)
            .unwrap_or(&terminals[0]);
        eliminate_other_terminals(context, selected);

        if context
            .grid
            .get(selected.terminal)
            .is_some_and(|e| e.kind.is_pipe())
        {
            context.grid.remove_entity(selected.terminal);
            let id = context.grid.get_id();
            context
                .grid
                .add_entity(selected.terminal, GridEntity::new(id, EntityKind::Terminal))
                .unwrap();
        }
    }
}

/// `Run`.
fn run(context: &mut Context, pipes: &[Location]) {
    let mut set = context.get_read_only_location_set(pipes.iter().copied(), true);
    let underground = plan_underground_pipes::execute(context, &mut set).unwrap();

    assert_eq!(pipe_count(context), 0);
    assert!(underground.keys().all(|p| pipes.contains(&p)));

    add_pipe_entities::execute(context, &set, Some(&underground)).unwrap();
}

fn pipe_entities(context: &Context) -> Vec<&GridEntity> {
    context
        .grid
        .entity_locations()
        .iter()
        .map(|l| context.grid.get(l).unwrap())
        .collect()
}

/// `GetEntities().Count(e => e is Pipe)`: Pipe and its subclasses.
fn pipe_count(context: &Context) -> usize {
    pipe_entities(context)
        .iter()
        .filter(|e| e.kind.is_pipe())
        .count()
}

fn underground_count(context: &Context) -> usize {
    pipe_entities(context)
        .iter()
        .filter(|e| matches!(e.kind, EntityKind::UndergroundPipe { .. }))
        .count()
}

fn all_are_pipes(context: &Context) -> bool {
    pipe_entities(context).iter().all(|e| e.kind.is_pipe())
}

fn all_are_underground(context: &Context) -> bool {
    pipe_entities(context)
        .iter()
        .all(|e| matches!(e.kind, EntityKind::UndergroundPipe { .. }))
}

fn underground_at(context: &Context, x: i32, y: i32) -> Direction {
    match context.grid.get(Location::new(x, y)).map(|e| &e.kind) {
        Some(EntityKind::UndergroundPipe { direction }) => *direction,
        other => panic!("expected an UndergroundPipe at ({x}, {y}), found {other:?}"),
    }
}

fn is_plain_pipe(context: &Context, x: i32, y: i32) -> bool {
    matches!(
        context.grid.get(Location::new(x, y)).map(|e| &e.kind),
        Some(EntityKind::Pipe)
    )
}

fn is_terminal(context: &Context, x: i32, y: i32) -> bool {
    matches!(
        context.grid.get(Location::new(x, y)).map(|e| &e.kind),
        Some(EntityKind::Terminal)
    )
}

fn too_short(horizontal: bool, length: i32) {
    let mut pipes = Vec::new();
    if horizontal {
        add_pipes(&mut pipes, 1, length, 1, 1);
    } else {
        add_pipes(&mut pipes, 1, 1, 1, length);
    }
    let mut context = get_context(&pipes, None, None);
    let original = context.get_read_only_location_set(pipes.iter().copied(), true);

    run(&mut context, &pipes);

    assert!(original.iter().all(|p| is_plain_pipe(&context, p.x, p.y)));
    let after = context.get_read_only_location_set(pipes.iter().copied(), true);
    assert!(original.set_equals(&after));
}

#[test]
fn horizontal_too_short() {
    for length in [1, 2] {
        too_short(true, length);
    }
}

#[test]
fn vertical_too_short() {
    for length in [1, 2] {
        too_short(false, length);
    }
}

#[test]
fn horizontal_one_run() {
    for length in 3..=11 {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, length, 1, 1);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        assert_eq!(context.grid.entity_locations().len(), 2);
        assert!(all_are_underground(&context));
        assert_eq!(underground_at(&context, 1, 1), Direction::Left);
        assert_eq!(underground_at(&context, length, 1), Direction::Right);
    }
}

#[test]
fn vertical_one_run() {
    for length in 3..=11 {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, 1, 1, length);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        assert_eq!(context.grid.entity_locations().len(), 2);
        assert!(all_are_underground(&context));
        assert_eq!(underground_at(&context, 1, 1), Direction::Up);
        assert_eq!(underground_at(&context, 1, length), Direction::Down);
    }
}

#[test]
fn horizontal_one_run_with_extra() {
    for length in [12, 13] {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, length, 1, 1);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        let expected = 2 + (length - 11) as usize;
        assert_eq!(context.grid.entity_locations().len(), expected);
        assert!(all_are_pipes(&context));
        assert_eq!(underground_count(&context), 2);
        assert_eq!(pipe_count(&context), expected);
        assert_eq!(underground_at(&context, 1, 1), Direction::Left);
        assert_eq!(underground_at(&context, 11, 1), Direction::Right);
        assert!((12..=length).all(|x| is_plain_pipe(&context, x, 1)));
    }
}

#[test]
fn vertical_one_run_with_extra() {
    for length in [12, 13] {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, 1, 1, length);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        let expected = 2 + (length - 11) as usize;
        assert_eq!(context.grid.entity_locations().len(), expected);
        assert!(all_are_pipes(&context));
        assert_eq!(underground_count(&context), 2);
        assert_eq!(pipe_count(&context), expected);
        assert_eq!(underground_at(&context, 1, 1), Direction::Up);
        assert_eq!(underground_at(&context, 1, 11), Direction::Down);
        assert!((12..=length).all(|y| is_plain_pipe(&context, 1, y)));
    }
}

#[test]
fn horizontal_two_runs() {
    for length in 14..=22 {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, length, 1, 1);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        assert_eq!(context.grid.entity_locations().len(), 4);
        assert!(all_are_pipes(&context));
        assert_eq!(underground_count(&context), 4);
        assert_eq!(underground_at(&context, 1, 1), Direction::Left);
        assert_eq!(underground_at(&context, 11, 1), Direction::Right);
        assert_eq!(underground_at(&context, 12, 1), Direction::Left);
        assert_eq!(underground_at(&context, length, 1), Direction::Right);
    }
}

#[test]
fn vertical_two_runs() {
    for length in 14..=22 {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, 1, 1, length);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        assert_eq!(context.grid.entity_locations().len(), 4);
        assert!(all_are_pipes(&context));
        assert_eq!(underground_count(&context), 4);
        assert_eq!(underground_at(&context, 1, 1), Direction::Up);
        assert_eq!(underground_at(&context, 1, 11), Direction::Down);
        assert_eq!(underground_at(&context, 1, 12), Direction::Up);
        assert_eq!(underground_at(&context, 1, length), Direction::Down);
    }
}

#[test]
fn horizontal_two_runs_with_extra() {
    for length in [23, 24] {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, length, 1, 1);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        let expected = 4 + (length - 22) as usize;
        assert_eq!(context.grid.entity_locations().len(), expected);
        assert!(all_are_pipes(&context));
        assert_eq!(underground_count(&context), 4);
        assert_eq!(pipe_count(&context), expected);
        assert_eq!(underground_at(&context, 1, 1), Direction::Left);
        assert_eq!(underground_at(&context, 11, 1), Direction::Right);
        assert_eq!(underground_at(&context, 12, 1), Direction::Left);
        assert_eq!(underground_at(&context, 22, 1), Direction::Right);
        assert!((23..=length).all(|x| is_plain_pipe(&context, x, 1)));
    }
}

#[test]
fn vertical_two_runs_with_extra() {
    for length in [23, 24] {
        let mut pipes = Vec::new();
        add_pipes(&mut pipes, 1, 1, 1, length);
        let mut context = get_context(&pipes, None, None);
        run(&mut context, &pipes);

        let expected = 4 + (length - 22) as usize;
        assert_eq!(context.grid.entity_locations().len(), expected);
        assert!(all_are_pipes(&context));
        assert_eq!(underground_count(&context), 4);
        assert_eq!(pipe_count(&context), expected);
        assert_eq!(underground_at(&context, 1, 1), Direction::Up);
        assert_eq!(underground_at(&context, 1, 11), Direction::Down);
        assert_eq!(underground_at(&context, 1, 12), Direction::Up);
        assert_eq!(underground_at(&context, 1, 22), Direction::Down);
        assert!((23..=length).all(|y| is_plain_pipe(&context, 1, y)));
    }
}

#[test]
fn intersection_plus() {
    let mut pipes = Vec::new();
    add_pipes(&mut pipes, 1, 11, 6, 6);
    add_pipes(&mut pipes, 6, 6, 1, 11);
    let mut context = get_context(&pipes, None, None);
    run(&mut context, &pipes);

    assert_eq!(context.grid.entity_locations().len(), 9);
    assert!(all_are_pipes(&context));
    assert_eq!(underground_at(&context, 1, 6), Direction::Left);
    assert_eq!(underground_at(&context, 5, 6), Direction::Right);
    assert_eq!(underground_at(&context, 7, 6), Direction::Left);
    assert_eq!(underground_at(&context, 11, 6), Direction::Right);
    assert_eq!(underground_at(&context, 6, 1), Direction::Up);
    assert_eq!(underground_at(&context, 6, 5), Direction::Down);
    assert_eq!(underground_at(&context, 6, 7), Direction::Up);
    assert_eq!(underground_at(&context, 6, 11), Direction::Down);
    assert!(is_plain_pipe(&context, 6, 6));
}

#[test]
fn intersection_t() {
    let mut pipes = Vec::new();
    add_pipes(&mut pipes, 1, 11, 6, 6);
    add_pipes(&mut pipes, 11, 11, 1, 11);
    let mut context = get_context(&pipes, None, None);
    run(&mut context, &pipes);

    assert_eq!(context.grid.entity_locations().len(), 7);
    assert!(all_are_pipes(&context));
    assert_eq!(underground_at(&context, 1, 6), Direction::Left);
    assert_eq!(underground_at(&context, 10, 6), Direction::Right);
    assert_eq!(underground_at(&context, 11, 1), Direction::Up);
    assert_eq!(underground_at(&context, 11, 5), Direction::Down);
    assert!(is_plain_pipe(&context, 11, 6));
    assert_eq!(underground_at(&context, 11, 7), Direction::Up);
    assert_eq!(underground_at(&context, 11, 11), Direction::Down);
}

#[test]
fn run_is_interrupted_by_terminal() {
    let mut pipes = Vec::new();
    add_pipes(&mut pipes, 1, 11, 4, 4);
    let mut context = get_context(&pipes, None, None);
    add_pumpjack_facing(&mut context, Location::new(7, 2), Direction::Down);
    run(&mut context, &pipes);

    assert_eq!(pipe_count(&context), 5);
    assert_eq!(underground_at(&context, 1, 4), Direction::Left);
    assert_eq!(underground_at(&context, 5, 4), Direction::Right);
    assert!(is_terminal(&context, 6, 4));
    assert_eq!(underground_at(&context, 7, 4), Direction::Left);
    assert_eq!(underground_at(&context, 11, 4), Direction::Right);
}

#[test]
fn run_is_adjacent_to_terminal() {
    let mut pipes = Vec::new();
    add_pipes(&mut pipes, 1, 11, 4, 4);
    pipes.push(Location::new(5, 3));
    let mut context = get_context(&pipes, None, None);
    // The terminal must land on (5, 3), beside the run rather than on it.
    add_pumpjack_facing(&mut context, Location::new(3, 2), Direction::Right);
    run(&mut context, &pipes);

    assert_eq!(pipe_count(&context), 6);
    assert_eq!(underground_at(&context, 1, 4), Direction::Left);
    assert_eq!(underground_at(&context, 4, 4), Direction::Right);
    assert!(is_terminal(&context, 5, 3));
    assert!(is_plain_pipe(&context, 5, 4));
    assert_eq!(underground_at(&context, 6, 4), Direction::Left);
    assert_eq!(underground_at(&context, 11, 4), Direction::Right);
}

#[test]
fn run_ends_with_terminal() {
    let mut pipes = Vec::new();
    add_pipes(&mut pipes, 1, 5, 3, 3);
    let mut context = get_context(&pipes, Some(10), Some(7));
    add_pumpjack_facing(&mut context, Location::new(7, 4), Direction::Left);
    run(&mut context, &pipes);

    assert_eq!(pipe_count(&context), 2);
    assert_eq!(underground_at(&context, 1, 3), Direction::Left);
    assert_eq!(underground_at(&context, 5, 3), Direction::Right);
}

#[test]
fn run_starts_with_terminal() {
    let mut pipes = Vec::new();
    add_pipes(&mut pipes, 4, 8, 3, 3);
    let mut context = get_context(&pipes, None, Some(5));
    add_pumpjack_facing(&mut context, Location::new(2, 2), Direction::Right);
    run(&mut context, &pipes);

    assert_eq!(pipe_count(&context), 2);
    assert_eq!(underground_at(&context, 4, 3), Direction::Left);
    assert_eq!(underground_at(&context, 8, 3), Direction::Right);
}

#[test]
fn run_starts_and_ends_with_terminal() {
    let mut pipes = Vec::new();
    add_pipes(&mut pipes, 4, 8, 3, 3);
    let mut context = get_context(&pipes, Some(13), Some(7));
    add_pumpjack_facing(&mut context, Location::new(2, 2), Direction::Right);
    add_pumpjack_facing(&mut context, Location::new(10, 4), Direction::Left);
    run(&mut context, &pipes);

    assert_eq!(pipe_count(&context), 2);
    assert_eq!(underground_at(&context, 4, 3), Direction::Left);
    assert_eq!(underground_at(&context, 8, 3), Direction::Right);
}
