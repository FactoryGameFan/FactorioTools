//! `PlannerDefaultsTest`, from `test/FactorioTools.Test/OilField/PlannerDefaultsTest.cs`.
//!
//! Emits the planner constants the Vue app needs, so they are not typed out a second time in
//! TypeScript. The Vue app imports the generated file directly (issue #54).
//!
//! This test is the generator. It compares the committed file byte for byte and fails on any
//! difference. Run it with `UPDATE_PLANNER_DEFAULTS=1` to rewrite the file instead, then
//! commit the result. CI never sets the variable, so a stale file fails there.
//!
//! The bytes match what the C# test wrote through Verify's `VerifyJson(...).UseStrictJson()`:
//! a UTF-8 byte order mark, two-space indent, `"key": value`, one array item per line, LF
//! line ends and no newline at the end.

use std::path::PathBuf;

use blueprint::json_out::format_f64;
use planner::models::{BeaconStrategy, PipeStrategy};
use planner::options::{ALL_BEACON_STRATEGIES, ALL_PIPE_STRATEGIES, OilFieldOptions};
use planner::quality::Quality;

/// Set to `1` to rewrite the committed file rather than compare against it.
const UPDATE_ENV: &str = "UPDATE_PLANNER_DEFAULTS";

/// The generated file the Vue app imports.
fn planner_defaults_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../src/vue/src/lib/plannerDefaults.verified.json")
}

#[test]
fn planner_defaults() {
    let generated = render_file(&build_payload());
    let path = planner_defaults_path();

    if std::env::var(UPDATE_ENV).is_ok_and(|value| value == "1") {
        std::fs::write(&path, &generated)
            .unwrap_or_else(|e| panic!("could not write {}: {e}", path.display()));
        return;
    }

    let committed =
        std::fs::read(&path).unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    if committed != generated {
        let committed = String::from_utf8_lossy(&committed);
        let generated = String::from_utf8_lossy(&generated);
        let first_difference = committed
            .lines()
            .zip(generated.lines())
            .enumerate()
            .find(|(_, (old, new))| old != new)
            .map(|(index, (old, new))| {
                format!(
                    "first different line is {}:\n  committed: {old}\n  generated: {new}",
                    index + 1
                )
            })
            .unwrap_or_else(|| "the files differ in length or in their last bytes".to_string());
        panic!(
            "{} is stale: it no longer matches OilFieldOptions.\n{first_difference}\n\
             Rerun with {UPDATE_ENV}=1 to rewrite it from the Rust, then commit the file. \
             From rust/:\n  {UPDATE_ENV}=1 cargo test -p contract --test planner_defaults",
            path.canonicalize().unwrap_or(path).display()
        );
    }
}

/// `BuildOptions`. Every field of `OilFieldOptions` is taken apart by name with no `..`, so a
/// new field that is not emitted here fails to compile. That is the C#'s
/// `EmitsEveryOilFieldOptionsProperty` test, done by the compiler. To leave a field out on
/// purpose, bind it as `field: _` with a comment saying why.
fn build_options() -> Json {
    let OilFieldOptions {
        use_underground_pipes,
        add_beacons,
        optimize_pipes,
        overlap_beacons,
        add_electric_poles,
        add_heat_pipes,
        heat_pipe_entity_name,
        pipe_strategies,
        beacon_strategies,
        electric_pole_entity_name,
        electric_pole_supply_width,
        electric_pole_supply_height,
        electric_pole_wire_reach,
        electric_pole_width,
        electric_pole_height,
        beacon_entity_name,
        beacon_supply_width,
        beacon_supply_height,
        beacon_width,
        beacon_height,
        validate_solution,
        pumpjack_modules,
        beacon_modules,
        pumpjack_quality,
        beacon_quality,
        electric_pole_quality,
        pumpjack_module_quality,
        beacon_module_quality,
    } = OilFieldOptions::default();

    Json::object([
        ("useUndergroundPipes", Json::Bool(use_underground_pipes)),
        ("addBeacons", Json::Bool(add_beacons)),
        ("optimizePipes", Json::Bool(optimize_pipes)),
        ("overlapBeacons", Json::Bool(overlap_beacons)),
        ("addElectricPoles", Json::Bool(add_electric_poles)),
        ("addHeatPipes", Json::Bool(add_heat_pipes)),
        ("heatPipeEntityName", Json::Str(heat_pipe_entity_name)),
        ("pipeStrategies", pipe_strategy_names(&pipe_strategies)),
        (
            "beaconStrategies",
            beacon_strategy_names(&beacon_strategies),
        ),
        (
            "electricPoleEntityName",
            Json::Str(electric_pole_entity_name),
        ),
        (
            "electricPoleSupplyWidth",
            Json::Int(electric_pole_supply_width),
        ),
        (
            "electricPoleSupplyHeight",
            Json::Int(electric_pole_supply_height),
        ),
        (
            "electricPoleWireReach",
            Json::Double(electric_pole_wire_reach),
        ),
        ("electricPoleWidth", Json::Int(electric_pole_width)),
        ("electricPoleHeight", Json::Int(electric_pole_height)),
        ("beaconEntityName", Json::Str(beacon_entity_name)),
        ("beaconSupplyWidth", Json::Int(beacon_supply_width)),
        ("beaconSupplyHeight", Json::Int(beacon_supply_height)),
        ("beaconWidth", Json::Int(beacon_width)),
        ("beaconHeight", Json::Int(beacon_height)),
        ("validateSolution", Json::Bool(validate_solution)),
        ("pumpjackModules", modules(pumpjack_modules)),
        ("beaconModules", modules(beacon_modules)),
        ("pumpjackQuality", quality_name(pumpjack_quality)),
        ("beaconQuality", quality_name(beacon_quality)),
        ("electricPoleQuality", quality_name(electric_pole_quality)),
        (
            "pumpjackModuleQuality",
            quality_name(pumpjack_module_quality),
        ),
        ("beaconModuleQuality", quality_name(beacon_module_quality)),
    ])
}

/// `BuildPayload`.
fn build_payload() -> Json {
    let presets = [
        OilFieldOptions::for_small_iron_electric_pole(),
        OilFieldOptions::for_small_electric_pole(),
        OilFieldOptions::for_medium_electric_pole(),
        OilFieldOptions::for_big_electric_pole(),
        OilFieldOptions::for_substation(),
    ];

    Json::object([
        ("options", build_options()),
        (
            "electricPolePresets",
            Json::Object(
                presets
                    .iter()
                    .map(|options| (options.electric_pole_entity_name.clone(), preset(options)))
                    .collect(),
            ),
        ),
        (
            "qualityLevels",
            Json::Object(
                Quality::ALL
                    .iter()
                    .map(|quality| (quality_text(*quality), Json::Int(quality.level())))
                    .collect(),
            ),
        ),
        (
            "allPipeStrategies",
            pipe_strategy_names(&ALL_PIPE_STRATEGIES),
        ),
        (
            "allBeaconStrategies",
            beacon_strategy_names(&ALL_BEACON_STRATEGIES),
        ),
    ])
}

/// `Preset`.
fn preset(options: &OilFieldOptions) -> Json {
    Json::object([
        ("width", Json::Int(options.electric_pole_width)),
        ("height", Json::Int(options.electric_pole_height)),
        ("supplyWidth", Json::Int(options.electric_pole_supply_width)),
        (
            "supplyHeight",
            Json::Int(options.electric_pole_supply_height),
        ),
        ("wireReach", Json::Double(options.electric_pole_wire_reach)),
    ])
}

fn pipe_strategy_names(strategies: &[PipeStrategy]) -> Json {
    Json::Array(
        strategies
            .iter()
            .map(|s| Json::Str(s.name().to_string()))
            .collect(),
    )
}

fn beacon_strategy_names(strategies: &[BeaconStrategy]) -> Json {
    Json::Array(
        strategies
            .iter()
            .map(|s| Json::Str(s.name().to_string()))
            .collect(),
    )
}

/// A `Dictionary<string, int>` of modules serializes as an object, in insertion order.
fn modules(modules: Vec<(String, i32)>) -> Json {
    Json::Object(
        modules
            .into_iter()
            .map(|(name, count)| (name, Json::Int(count)))
            .collect(),
    )
}

/// `Enum.ToString()`: the member name, or the number for a value the enum does not name.
fn quality_text(quality: Quality) -> String {
    quality
        .name()
        .map_or_else(|| quality.level().to_string(), str::to_string)
}

fn quality_name(quality: Quality) -> Json {
    Json::Str(quality_text(quality))
}

/// Just enough of a JSON tree to write the file in Verify's layout. Objects keep their keys
/// in insertion order, as the C#'s anonymous types and dictionaries do.
enum Json {
    Bool(bool),
    Int(i32),
    /// A C# `double`, written as .NET writes it: 9.0 is `9`, not `9.0`.
    Double(f64),
    Str(String),
    Array(Vec<Json>),
    Object(Vec<(String, Json)>),
}

impl Json {
    fn object<const N: usize>(entries: [(&str, Json); N]) -> Json {
        Json::Object(
            entries
                .into_iter()
                .map(|(key, value)| (key.to_string(), value))
                .collect(),
        )
    }
}

/// The whole file: a UTF-8 byte order mark, then the document with no newline at the end.
fn render_file(root: &Json) -> Vec<u8> {
    let mut text = String::new();
    render(root, 0, &mut text);
    let mut bytes = vec![0xEF, 0xBB, 0xBF];
    bytes.extend_from_slice(text.as_bytes());
    bytes
}

fn render(value: &Json, depth: usize, out: &mut String) {
    match value {
        Json::Bool(value) => out.push_str(if *value { "true" } else { "false" }),
        Json::Int(value) => out.push_str(&value.to_string()),
        Json::Double(value) => out.push_str(&format_f64(*value)),
        Json::Str(value) => push_string(value, out),
        Json::Array(items) => render_container('[', ']', items, depth, out, |item, out| {
            render(item, depth + 1, out)
        }),
        Json::Object(entries) => {
            render_container('{', '}', entries, depth, out, |(key, value), out| {
                push_string(key, out);
                out.push_str(": ");
                render(value, depth + 1, out);
            })
        }
    }
}

/// Writes each item on its own line, indented one level past `depth`. An empty container
/// stays on one line (`[]`, `{}`).
fn render_container<T>(
    open: char,
    close: char,
    items: &[T],
    depth: usize,
    out: &mut String,
    mut render_item: impl FnMut(&T, &mut String),
) {
    out.push(open);
    for (index, item) in items.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        out.push('\n');
        push_indent(depth + 1, out);
        render_item(item, out);
    }
    if !items.is_empty() {
        out.push('\n');
        push_indent(depth, out);
    }
    out.push(close);
}

fn push_indent(depth: usize, out: &mut String) {
    out.push_str(&"  ".repeat(depth));
}

/// Every string here is a plain ASCII name, so standard JSON escaping gives the same text as
/// Verify's writer.
fn push_string(value: &str, out: &mut String) {
    out.push_str(&serde_json::to_string(value).expect("a string always serializes"));
}
