//! Runs the Rust planner on exactly the inputs of the C# Verify snapshot tests in
//! `test/FactorioTools.Test/OilField` and compares its output with the committed
//! `*.verified.txt` files, byte for byte, with no .NET involved (#138).
//!
//! It lives in its own crate because it needs both the planner and the blueprint parser,
//! and because the full set is slow enough to want a release build of its own.
//!
//! [`normalize`] and the `normalize-lists` binary keep the two blueprint lists those tests
//! read normalized, in place of the C# CLI's `oil-field normalize`.
//!
//! With `UPDATE_SNAPSHOTS=1` the tests write the snapshots instead of comparing them: they
//! create missing files, rewrite changed ones and delete files no case produces any more.
//! The bytes are what Verify writes: a UTF-8 byte order mark, then the text, LF line ends.
//! See [`UPDATE_COMMAND`].

pub mod issue_cases;
pub mod normalize;

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use planner::data::{
    Blueprint, Entity, Icon, Position, SignalId, entity_names, item_names, signal_types,
};
use planner::error::FtResult;
use planner::models::{AvoidLocation, BeaconStrategy, OilFieldPlan, PipeStrategy};
use planner::options::OilFieldOptions;
use planner::planner::{PlannerResult, execute, execute_sample, execute_with_avoid};

/// The repository root, two levels above this crate.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository root exists")
}

/// `test/FactorioTools.Test/OilField`.
pub fn oil_field_test_dir() -> PathBuf {
    repo_root().join("test/FactorioTools.Test/OilField")
}

/// Set to `1` to write the snapshots rather than compare against them.
pub const UPDATE_ENV: &str = "UPDATE_SNAPSHOTS";

/// The command that rewrites every snapshot, run from `rust/`.
pub const UPDATE_COMMAND: &str =
    "UPDATE_SNAPSHOTS=1 cargo test --release -p snapshots -- --include-ignored";

/// The scoreboard snapshot `tests/score.rs` checks, relative to [`oil_field_test_dir`].
pub const SCORE_SNAPSHOT: &str = "Score.HasExpectedScore.verified.txt";

/// Whether [`UPDATE_ENV`] is `1`. Any other value, or none, compares.
pub fn update_mode() -> bool {
    std::env::var(UPDATE_ENV).is_ok_and(|value| value == "1")
}

/// What a failure message ends with: how to accept the Rust output instead.
pub fn update_hint() -> String {
    format!(
        "If the new output is right, rewrite the snapshots from it, then review and commit the \
         diff. From rust/:\n  {UPDATE_COMMAND}"
    )
}

/// `BasePlannerTest.SmallListBlueprintStrings`.
pub fn small_list() -> Vec<String> {
    blueprint::read_blueprint_file(oil_field_test_dir().join("small-list.txt"))
        .expect("small-list.txt reads")
}

/// `BasePlannerTest.BigListBlueprintStrings`.
pub fn big_list() -> Vec<String> {
    blueprint::read_blueprint_file(oil_field_test_dir().join("big-list.txt"))
        .expect("big-list.txt reads")
}

/// An options preset, such as `OilFieldOptions::for_medium_electric_pole`.
pub type Preset = fn() -> OilFieldOptions;

/// `BasePlannerTest.ElectricPoleToOptions`, keyed by the pole's entity name, in the C#
/// array's order.
pub fn electric_pole_options() -> [(&'static str, Preset); 4] {
    [
        (
            entity_names::vanilla::SMALL_ELECTRIC_POLE,
            OilFieldOptions::for_small_electric_pole,
        ),
        (
            entity_names::vanilla::MEDIUM_ELECTRIC_POLE,
            OilFieldOptions::for_medium_electric_pole,
        ),
        (
            entity_names::vanilla::BIG_ELECTRIC_POLE,
            OilFieldOptions::for_big_electric_pole,
        ),
        (
            entity_names::vanilla::SUBSTATION,
            OilFieldOptions::for_substation,
        ),
    ]
}

fn preset(name: &str) -> OilFieldOptions {
    match name {
        "ForSmallElectricPole" => OilFieldOptions::for_small_electric_pole(),
        "ForMediumElectricPole" => OilFieldOptions::for_medium_electric_pole(),
        "ForBigElectricPole" => OilFieldOptions::for_big_electric_pole(),
        "ForSubstation" => OilFieldOptions::for_substation(),
        other => panic!("unknown preset {other}"),
    }
}

/// Where a case's blueprint comes from.
#[derive(Clone, Debug)]
pub enum Input {
    SmallList(usize),
    BigList(usize),
    String(&'static str),
    Blueprint(Blueprint),
    /// `Planner.ExecuteSample()`.
    Sample,
}

/// One snapshot test case: the inputs, and the snapshot file it must match.
#[derive(Clone, Debug)]
pub struct SnapshotCase {
    /// The snapshot file, relative to `test/FactorioTools.Test/OilField`.
    pub snapshot: String,
    pub input: Input,
    pub options: OilFieldOptions,
    pub avoid: Vec<AvoidLocation>,
}

/// The blueprint `AllowsBlueprintWithNonBlockingIsolatedArea` plans.
const NON_BLOCKING_ISOLATED_AREA: &str = "0eNqV1UluhDAQBdC71NoLXLYBc5UoatG0FTlpDGKIghB3D9MiAxKfJWAehVXfNdL92bu68aGjbCRfVKGl7GWk1r+F/LncC3npKKO6L+v3vPggQd1QL3d850qaBPnwcF+UyelVkAud77zbjPViuIW+vLtmXiAOrLpq5xeqsHxpRlgLGualanYfvnHF9iyaxD+OAU4xzCmES2BOI1y0cfFvTh9w5kJ1ABcDnNy55PxnE4DT8cal51yKNMpenT3nLLJ3CuZkhGze5nEEeBdywRLwGG89yLuQDGbAQ6LBe31A0qTB24U14CHhUBr3oHREuIfEQ1+oD8kH7/0MHAaM5EPZQ+/orGIoH2b1FNDPfGFwQB6Uj+30U+avN8/MdY5mPwaxoE/XtOsCE7PV1hpjZKpjPU3fXdaDKQ==";

fn pumpjack(x: f32, y: f32) -> Entity {
    Entity {
        name: entity_names::vanilla::PUMPJACK.to_string(),
        position: Position { x, y },
        ..Entity::default()
    }
}

/// Every Verify snapshot test that plans, in the C# tests' order. `big_list_step` keeps
/// only every n-th big-list blueprint (1 keeps them all).
pub fn all_cases(big_list_step: usize) -> Vec<SnapshotCase> {
    let small_count = small_list().len();
    let big_count = big_list().len();
    let mut cases = Vec::new();

    // PlannerWithAllOptions/Theory.cs: `BasePlannerTest.BlueprintsAndOptions`.
    for index in 0..small_count {
        for (pole, get_options) in electric_pole_options() {
            for add_beacons in [false, true] {
                for overlap_beacons in [false, true] {
                    if !add_beacons && overlap_beacons {
                        continue;
                    }

                    for use_underground_pipes in [false, true] {
                        let mut options = get_options();
                        options.add_beacons = add_beacons;
                        options.use_underground_pipes = use_underground_pipes;
                        options.overlap_beacons = overlap_beacons;
                        options.validate_solution = true;
                        cases.push(SnapshotCase {
                            snapshot: format!(
                                "PlannerWithAllOptions/Theory.E_{index:04}_{pole}{}{}{}.verified.txt",
                                if add_beacons { "_b" } else { "" },
                                if overlap_beacons { "_o" } else { "" },
                                if use_underground_pipes { "_u" } else { "" },
                            ),
                            input: Input::SmallList(index),
                            options,
                            avoid: Vec::new(),
                        });
                    }
                }
            }
        }
    }

    // AllStrategiesOnSmallList and AllStrategiesOnBigList: `ExecuteAllStrategies`.
    let all_strategies = || {
        let mut options = OilFieldOptions::for_medium_electric_pole();
        options.validate_solution = true;
        options.pipe_strategies = PipeStrategy::ALL.to_vec();
        options.beacon_strategies = BeaconStrategy::ALL.to_vec();
        options
    };
    for index in 0..small_count {
        cases.push(SnapshotCase {
            snapshot: format!("AllStrategiesOnSmallList/Theory.E_{index:04}.verified.txt"),
            input: Input::SmallList(index),
            options: all_strategies(),
            avoid: Vec::new(),
        });
    }
    for index in (0..big_count).step_by(big_list_step.max(1)) {
        cases.push(SnapshotCase {
            snapshot: format!("AllStrategiesOnBigList/Theory.E_{index:04}.verified.txt"),
            input: Input::BigList(index),
            options: all_strategies(),
            avoid: Vec::new(),
        });
    }

    // NonStandardBeacon/Theory.cs.
    for index in 0..small_count {
        let mut options = OilFieldOptions::for_medium_electric_pole();
        options.add_beacons = true;
        options.use_underground_pipes = true;
        options.overlap_beacons = true;
        options.validate_solution = true;
        options.beacon_width = 2;
        options.beacon_height = 4;
        options.beacon_supply_width = 4;
        options.beacon_supply_height = 6;
        options.beacon_strategies = BeaconStrategy::ALL.to_vec();
        cases.push(SnapshotCase {
            snapshot: format!("NonStandardBeacon/Theory.E_{index:04}.verified.txt"),
            input: Input::SmallList(index),
            options,
            avoid: Vec::new(),
        });
    }

    // AllowsBlueprintWithNonBlockingIsolatedArea/Theory.cs. No ValidateSolution here.
    for strategy in PipeStrategy::ALL {
        let mut options = OilFieldOptions::for_medium_electric_pole();
        options.pipe_strategies = vec![strategy];
        cases.push(SnapshotCase {
            snapshot: format!(
                "AllowsBlueprintWithNonBlockingIsolatedArea/Theory.E_{}.verified.txt",
                strategy.name()
            ),
            input: Input::String(NON_BLOCKING_ISOLATED_AREA),
            options,
            avoid: Vec::new(),
        });
    }

    cases.extend(planner_test_cases());
    cases
}

/// The snapshot methods of `PlannerTest.cs` and `PlannerIssueTest.cs`.
pub fn planner_test_cases() -> Vec<SnapshotCase> {
    let mut cases = Vec::new();

    // PlannerTest.AllowsLocationsToBeAvoided.
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_beacons = true;
    cases.push(SnapshotCase {
        snapshot: "PlannerTest.AllowsLocationsToBeAvoided.verified.txt".to_string(),
        input: Input::Blueprint(Blueprint {
            entities: vec![pumpjack(-3.0, -5.0), pumpjack(4.0, 5.0)],
            icons: vec![Icon {
                index: 1,
                signal: SignalId {
                    name: entity_names::vanilla::PUMPJACK.to_string(),
                    signal_type: signal_types::vanilla::ITEM.to_string(),
                },
            }],
            item: item_names::vanilla::BLUEPRINT.to_string(),
            version: 0,
        }),
        options,
        avoid: (-7..-7 + 16)
            .map(|x| AvoidLocation::new(x as f32, 0.0))
            .collect(),
    });

    // PlannerTest.AddsHeatPipesForAquilo.
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_heat_pipes = true;
    options.add_beacons = false;
    cases.push(SnapshotCase {
        snapshot: "PlannerTest.AddsHeatPipesForAquilo.verified.txt".to_string(),
        input: Input::SmallList(0),
        options,
        avoid: Vec::new(),
    });

    // PlannerTest.AddsHeatPipesAndBeaconsTogetherForAquilo.
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_heat_pipes = true;
    options.add_beacons = true;
    cases.push(SnapshotCase {
        snapshot: "PlannerTest.AddsHeatPipesAndBeaconsTogetherForAquilo.verified.txt".to_string(),
        input: Input::SmallList(0),
        options,
        avoid: Vec::new(),
    });

    // PlannerTest.ExecuteSample.
    cases.push(SnapshotCase {
        snapshot: "PlannerTest.ExecuteSample.verified.txt".to_string(),
        input: Input::Sample,
        options: OilFieldOptions::default(),
        avoid: Vec::new(),
    });

    // PlannerTest.DropsPumpjacksToFullyHeatBoxedInField (BoxedInHeatIndex = 55).
    let mut options = OilFieldOptions::for_medium_electric_pole();
    options.validate_solution = true;
    options.add_heat_pipes = true;
    options.add_beacons = false;
    cases.push(SnapshotCase {
        snapshot: "PlannerTest.DropsPumpjacksToFullyHeatBoxedInField.verified.txt".to_string(),
        input: Input::SmallList(55),
        options,
        avoid: Vec::new(),
    });

    // PlannerIssueTest's four snapshot methods.
    for case in issue_cases::ISSUE_CASES {
        let mut options = preset(case.preset);
        options.validate_solution = true;
        if !case.add_beacons {
            options.add_beacons = false;
        }
        cases.push(SnapshotCase {
            snapshot: format!("PlannerIssueTest.{}.verified.txt", case.name),
            input: Input::Blueprint(Blueprint {
                entities: case
                    .pumpjacks
                    .iter()
                    .map(|&(x, y)| pumpjack(x, y))
                    .collect(),
                ..Blueprint::default()
            }),
            options,
            avoid: case
                .avoid
                .iter()
                .map(|&(x, y)| AvoidLocation::new(x, y))
                .collect(),
        });
    }

    cases
}

/// The blueprint a case plans.
pub fn blueprint_for(input: &Input, small: &[String], big: &[String]) -> FtResult<Blueprint> {
    match input {
        Input::SmallList(i) => blueprint::parse_blueprint(&small[*i]),
        Input::BigList(i) => blueprint::parse_blueprint(&big[*i]),
        Input::String(s) => blueprint::parse_blueprint(s),
        Input::Blueprint(b) => Ok(b.clone()),
        Input::Sample => Ok(planner::planner::get_sample_blueprint()),
    }
}

/// Plans a case the way its C# test does.
pub fn run_case(case: &SnapshotCase, small: &[String], big: &[String]) -> FtResult<PlannerResult> {
    if let Input::Sample = case.input {
        return execute_sample();
    }

    let blueprint = blueprint_for(&case.input, small, big)?;
    if case.avoid.is_empty() {
        execute(case.options.clone(), blueprint)
    } else {
        execute_with_avoid(case.options.clone(), blueprint, &case.avoid)
    }
}

/// `BasePlannerTest.GetGridString`: the plan lines (selected `S`, alternate `A`, unused
/// space), stably sorted by pipe strategy, optimized flag and beacon strategy; a blank line;
/// then the grid.
pub fn grid_string(result: &PlannerResult) -> String {
    let summary = &result.summary;
    let mut plans: Vec<(char, &OilFieldPlan)> = summary
        .selected_plans
        .iter()
        .map(|p| ('S', p))
        .chain(summary.alternate_plans.iter().map(|p| ('A', p)))
        .chain(summary.unused_plans.iter().map(|p| (' ', p)))
        .collect();
    // LINQ's OrderBy/ThenBy is stable, as `sort_by` is.
    plans.sort_by(|a, b| {
        a.1.pipe_strategy
            .cmp(&b.1.pipe_strategy)
            .then(a.1.optimize_pipes.cmp(&b.1.optimize_pipes))
            .then(a.1.beacon_strategy.cmp(&b.1.beacon_strategy))
    });

    let mut output = String::new();
    for (prefix, plan) in plans {
        output.push(prefix);
        output.push(' ');
        output.push_str(&plan.to_string_with_counts(true));
        output.push('\n');
    }

    output.push('\n');
    output.push_str(&result.context.grid.to_string_with_spacing(1));
    output
}

/// A committed snapshot's text: the file without its UTF-8 byte order mark.
pub fn read_snapshot(relative: &str) -> std::io::Result<String> {
    let text = std::fs::read_to_string(oil_field_test_dir().join(relative))?;
    Ok(text.strip_prefix('\u{feff}').unwrap_or(&text).to_string())
}

/// What [`write_snapshot`] did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Written {
    /// The file already held exactly these bytes, so it was left alone.
    Unchanged,
    Created,
    Updated,
}

/// The bytes Verify writes for a snapshot: a UTF-8 byte order mark, then the text as it
/// is. The text's own `\n` line ends are kept, which is what `*.verified.txt eol=lf` checks
/// out on every platform.
pub fn snapshot_bytes(text: &str) -> Vec<u8> {
    let mut bytes = "\u{feff}".as_bytes().to_vec();
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

/// Writes a snapshot, relative to [`oil_field_test_dir`], unless it already holds exactly
/// [`snapshot_bytes`]. A file with the same text but no byte order mark is rewritten.
pub fn write_snapshot(relative: &str, text: &str) -> std::io::Result<Written> {
    let path = oil_field_test_dir().join(relative);
    let bytes = snapshot_bytes(text);
    let written = match std::fs::read(&path) {
        Ok(existing) if existing == bytes => return Ok(Written::Unchanged),
        Ok(_) => Written::Updated,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Written::Created,
        Err(error) => return Err(error),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, bytes)?;
    Ok(written)
}

/// Compares a case's output with its snapshot, or in [`update_mode`] writes it. Two tests
/// run the `PlannerTest` cases at once, so both can write one of those files, but always
/// with the same bytes, so the file ends up whole either way.
fn compare_or_write(relative: &str, actual: &str) -> Outcome {
    if update_mode() {
        return match write_snapshot(relative, actual) {
            Ok(Written::Unchanged) => Outcome::Matched,
            Ok(written) => Outcome::Written(written),
            Err(error) => Outcome::Error(format!("snapshot unwritable: {error}")),
        };
    }

    match read_snapshot(relative) {
        Err(error) => Outcome::Error(format!("snapshot unreadable: {error}")),
        Ok(expected) if expected == actual => Outcome::Matched,
        Ok(expected) => Outcome::Mismatch(first_difference(&expected, actual)),
    }
}

/// The first lines where two texts differ, for a failure message.
pub fn first_difference(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<&str> = expected.split('\n').collect();
    let actual_lines: Vec<&str> = actual.split('\n').collect();
    for i in 0..expected_lines.len().max(actual_lines.len()) {
        let e = expected_lines.get(i).copied();
        let a = actual_lines.get(i).copied();
        if e != a {
            let mut message = format!("first difference at line {}:\n", i + 1);
            for j in i..(i + 3).min(expected_lines.len().max(actual_lines.len())) {
                message.push_str(&format!(
                    "  expected: {:?}\n  actual:   {:?}\n",
                    expected_lines.get(j).unwrap_or(&"<end>"),
                    actual_lines.get(j).unwrap_or(&"<end>"),
                ));
            }
            return message;
        }
    }
    "no line differs (line endings or a trailing newline differ)".to_string()
}

/// What happened to one case.
pub enum Outcome {
    Matched,
    Mismatch(String),
    Error(String),
    /// In [`update_mode`], the snapshot was created or rewritten.
    Written(Written),
}

/// Runs cases on all cores, returning each case's outcome in case order. Each case is
/// planned on its own, so the order the threads finish in cannot change any output.
pub fn run_cases(cases: &[SnapshotCase]) -> Vec<Outcome> {
    let small = small_list();
    let big = big_list();
    let next = AtomicUsize::new(0);
    let outcomes: Mutex<Vec<Option<Outcome>>> =
        Mutex::new((0..cases.len()).map(|_| None).collect());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());

    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= cases.len() {
                        break;
                    }

                    let case = &cases[i];
                    let outcome = match std::panic::catch_unwind(|| run_case(case, &small, &big)) {
                        Err(panic) => Outcome::Error(format!(
                            "panicked: {}",
                            panic
                                .downcast_ref::<String>()
                                .cloned()
                                .or_else(|| panic.downcast_ref::<&str>().map(|s| s.to_string()))
                                .unwrap_or_default()
                        )),
                        Ok(Err(error)) => {
                            Outcome::Error(format!("planner error: {}", error.message))
                        }
                        Ok(Ok(result)) => compare_or_write(&case.snapshot, &grid_string(&result)),
                    };
                    outcomes.lock().unwrap()[i] = Some(outcome);
                }
            });
        }
    });

    outcomes
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|o| o.expect("every case ran"))
        .collect()
}

/// Runs cases and returns a report grouped by snapshot folder, plus how many failed. In
/// [`update_mode`] a written snapshot is not a failure, and the report lists it.
pub fn check(cases: &[SnapshotCase]) -> (String, usize) {
    let outcomes = run_cases(cases);
    let mut groups: Vec<(String, usize, usize)> = Vec::new();
    let mut failures = Vec::new();
    let mut written = Vec::new();
    for (case, outcome) in cases.iter().zip(&outcomes) {
        let group = case.snapshot.split_once('/').map_or_else(
            || case.snapshot.split('.').next().unwrap().to_string(),
            |(g, _)| g.to_string(),
        );
        let entry = match groups.iter_mut().find(|(g, _, _)| *g == group) {
            Some(entry) => entry,
            None => {
                groups.push((group, 0, 0));
                groups.last_mut().unwrap()
            }
        };
        entry.2 += 1;
        match outcome {
            Outcome::Matched => entry.1 += 1,
            Outcome::Mismatch(diff) => {
                failures.push(format!("{}: mismatch\n{diff}", case.snapshot))
            }
            Outcome::Error(error) => failures.push(format!("{}: {error}", case.snapshot)),
            Outcome::Written(how) => written.push(format!("{how:?}: {}", case.snapshot)),
        }
    }

    let mut report = String::new();
    for (group, matched, total) in &groups {
        report.push_str(&format!("{group}: {matched}/{total} matched\n"));
    }
    if update_mode() {
        report.push_str(&format!(
            "{UPDATE_ENV}=1: wrote {} snapshots\n",
            written.len()
        ));
        for line in written.iter().take(20) {
            report.push_str(line);
            report.push('\n');
        }
        if written.len() > 20 {
            report.push_str(&format!("... and {} more\n", written.len() - 20));
        }
    }
    for failure in failures.iter().take(10) {
        report.push_str(failure);
        report.push('\n');
    }
    if failures.len() > 10 {
        report.push_str(&format!("... and {} more failures\n", failures.len() - 10));
    }
    if !failures.is_empty() && !update_mode() {
        report.push_str(&update_hint());
        report.push('\n');
    }
    (report, failures.len())
}

/// One scoreboard run in `Score.HasExpectedScore`.
struct ScoreRow {
    add_beacons: bool,
    overlap_beacons: bool,
    options_index: usize,
    pole_name: String,
    selected: Vec<OilFieldPlan>,
    alternate: Vec<OilFieldPlan>,
    beacon_effect_count: i32,
    beacon_count: i32,
    pipe_count_with_underground: i32,
    pipe_count_without_underground: i32,
    pole_count: i32,
}

/// `Enumerable.Average` over `int`s: the sum as a `long`, divided as a `double`.
fn average(values: impl Iterator<Item = i32>) -> f64 {
    let (sum, count) = values.fold((0i64, 0i64), |(s, c), v| (s + i64::from(v), c + 1));
    sum as f64 / count as f64
}

/// `double.ToString()`: the shortest text that reads back to the same value. Rust's
/// `Display` gives the same digits for the values a scoreboard holds.
fn format_double(value: f64) -> String {
    format!("{value}")
}

/// `Score.HasExpectedScore`: plans the 61 small-list blueprints with every pole, with and
/// without beacons (and beacon overlap), using every strategy, and renders the scoreboard
/// text that `Score.HasExpectedScore.verified.txt` holds.
pub fn score_text() -> String {
    let small = small_list();
    let get_options_all: [fn() -> OilFieldOptions; 4] = [
        OilFieldOptions::for_small_electric_pole,
        OilFieldOptions::for_medium_electric_pole,
        OilFieldOptions::for_substation,
        OilFieldOptions::for_big_electric_pole,
    ];

    let mut inputs = Vec::new();
    for add_beacons in [true, false] {
        for overlap_beacons in [true, false] {
            for (options_index, _) in get_options_all.iter().enumerate() {
                for blueprint_index in 0..small.len() {
                    if add_beacons || !overlap_beacons {
                        inputs.push((add_beacons, overlap_beacons, options_index, blueprint_index));
                    }
                }
            }
        }
    }

    // `AsParallel()`: each run is independent, and the grouping below sorts explicitly.
    let next = AtomicUsize::new(0);
    let rows: Mutex<Vec<Option<ScoreRow>>> = Mutex::new((0..inputs.len()).map(|_| None).collect());
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    std::thread::scope(|scope| {
        for _ in 0..threads {
            scope.spawn(|| {
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= inputs.len() {
                        break;
                    }

                    let (add_beacons, overlap_beacons, options_index, blueprint_index) = inputs[i];
                    let blueprint = blueprint::parse_blueprint(&small[blueprint_index])
                        .expect("small-list blueprints parse");
                    let mut options = get_options_all[options_index]();
                    options.add_beacons = add_beacons;
                    options.use_underground_pipes = true;
                    options.optimize_pipes = true;
                    options.validate_solution = false;
                    options.overlap_beacons = overlap_beacons;
                    options.pipe_strategies = PipeStrategy::ALL.to_vec();
                    options.beacon_strategies = BeaconStrategy::ALL.to_vec();
                    let pole_name = options.electric_pole_entity_name.clone();

                    let result = execute(options, blueprint).expect("score plans succeed");
                    let first = result.summary.selected_plans[0];
                    let grid = &result.context.grid;
                    let pole_count = grid
                        .entity_locations()
                        .iter()
                        .filter(|&l| {
                            matches!(
                                grid.get(l).map(|e| &e.kind),
                                Some(planner::grid::EntityKind::ElectricPoleCenter { .. })
                            )
                        })
                        .count() as i32;

                    rows.lock().unwrap()[i] = Some(ScoreRow {
                        add_beacons,
                        overlap_beacons,
                        options_index,
                        pole_name,
                        selected: result.summary.selected_plans.clone(),
                        alternate: result.summary.alternate_plans.clone(),
                        beacon_effect_count: first.beacon_effect_count,
                        beacon_count: first.beacon_count,
                        pipe_count_with_underground: first.pipe_count,
                        pipe_count_without_underground: first.pipe_count_without_underground,
                        pole_count,
                    });
                }
            });
        }
    });
    let rows: Vec<ScoreRow> = rows
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|r| r.unwrap())
        .collect();

    let mut output = String::new();
    let columns = [
        "Electric pole",
        "Add beacons",
        "Overlap beacons",
        "Effects",
        "Beacons",
        "Pipes (UG)",
        "Pipes (no UG)",
        "Poles",
    ];
    let mut table: Vec<Vec<String>> = Vec::new();

    for add_beacons in [true, false] {
        let group: Vec<&ScoreRow> = rows
            .iter()
            .filter(|r| r.add_beacons == add_beacons)
            .collect();
        for overlap_beacons in [true, false] {
            let overlap_group: Vec<&ScoreRow> = group
                .iter()
                .copied()
                .filter(|r| r.overlap_beacons == overlap_beacons)
                .collect();
            if overlap_group.is_empty() {
                continue;
            }

            for options_index in 0..get_options_all.len() {
                let pole_group: Vec<&ScoreRow> = overlap_group
                    .iter()
                    .copied()
                    .filter(|r| r.options_index == options_index)
                    .collect();
                table.push(vec![
                    pole_group[0].pole_name.clone(),
                    if add_beacons { "yes" } else { "no" }.to_string(),
                    if add_beacons {
                        if overlap_beacons { "yes" } else { "no" }
                    } else {
                        "N/A"
                    }
                    .to_string(),
                    format_double(average(pole_group.iter().map(|r| r.beacon_effect_count))),
                    format_double(average(pole_group.iter().map(|r| r.beacon_count))),
                    format_double(average(
                        pole_group.iter().map(|r| r.pipe_count_with_underground),
                    )),
                    format_double(average(
                        pole_group.iter().map(|r| r.pipe_count_without_underground),
                    )),
                    format_double(average(pole_group.iter().map(|r| r.pole_count))),
                ]);
            }
        }

        let heading = format!(
            "Wins {} beacons",
            if add_beacons { "with" } else { "without" }
        );
        output.push_str(&heading);
        output.push('\n');
        output.push_str(&"-".repeat(heading.len()));
        output.push('\n');

        // `GroupBy(ToString(false)).ToDictionary(Count)`, then by count descending and name.
        let mut plan_to_wins: Vec<(String, usize)> = Vec::new();
        for row in &group {
            for plan in row.selected.iter().chain(&row.alternate) {
                let key = plan.to_string_with_counts(false);
                match plan_to_wins.iter_mut().find(|(k, _)| *k == key) {
                    Some(entry) => entry.1 += 1,
                    None => plan_to_wins.push((key, 1)),
                }
            }
        }
        plan_to_wins.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let max_width = plan_to_wins.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
        for (plan, wins) in &plan_to_wins {
            output.push_str(&format!("{plan:>max_width$} : {wins}\n"));
        }
        output.push('\n');
    }

    // `TableBuilder.Build`.
    let max_widths: Vec<usize> = (0..columns.len())
        .map(|i| {
            table
                .iter()
                .map(|row| row[i].len())
                .chain([columns[i].len()])
                .max()
                .unwrap()
        })
        .collect();
    let append_row = |output: &mut String, values: &dyn Fn(usize) -> String| {
        for (i, width) in max_widths.iter().enumerate() {
            output.push_str(if i == 0 { "| " } else { " | " });
            output.push_str(&format!("{:<width$}", values(i)));
            if i == columns.len() - 1 {
                output.push_str(" |");
            }
        }
        output.push('\n');
    };
    append_row(&mut output, &|i| columns[i].to_string());
    append_row(&mut output, &|i| "-".repeat(max_widths[i]));
    for row in &table {
        append_row(&mut output, &|i| row[i].clone());
    }

    output
}
