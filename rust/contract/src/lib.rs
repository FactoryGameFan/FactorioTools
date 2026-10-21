//! The planner's JSON contract, from `src/FactorioTools.Contract` (#138): what the site
//! sends the planner and gets back.
//!
//! [`plan_json`] and [`normalize_json`] port `PlanOrchestrator.PlanJson` and `NormalizeJson`.
//! A `FactorioToolsException` becomes the error envelope (`ErrorEnvelope`), returned like
//! a normal response. Anything else the C# would throw out of the call, such as a
//! `JsonException` for malformed JSON, is returned as [`Thrown`], which the WASM module
//! turns into a JavaScript exception, as the .NET module's `[JSExport]` does.
//!
//! Request JSON is read with the C#'s rules ([`request`]); responses are written with
//! `System.Text.Json`'s text formats ([`blueprint::json_out`]).

pub mod request;

use blueprint::json_out::JsonWriter;
use planner::error::FactorioToolsError;
use planner::models::{BeaconStrategy, OilFieldPlan, OilFieldPlanSummary, PipeStrategy};
use planner::options::OilFieldOptions;
use planner::quality::Quality;

use crate::request::{Property, Root, read_root};

/// An exception the C# lets escape `PlanJson` or `NormalizeJson`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Thrown {
    /// The .NET exception type: `JsonException`, `NullReferenceException`, or
    /// `NotSupportedException` for a request this port declines (see [`plan_json`]).
    pub exception: &'static str,
    pub message: String,
}

impl Thrown {
    pub fn json(message: impl Into<String>) -> Self {
        Thrown {
            exception: "JsonException",
            message: message.into(),
        }
    }

    fn null_reference() -> Self {
        Thrown {
            exception: "NullReferenceException",
            message: "Object reference not set to an instance of an object.".to_string(),
        }
    }

    fn not_supported(message: impl Into<String>) -> Self {
        Thrown {
            exception: "NotSupportedException",
            message: message.into(),
        }
    }
}

impl std::fmt::Display for Thrown {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.exception, self.message)
    }
}

impl std::error::Error for Thrown {}

const PIPE_STRATEGY_NAMES: [(&str, i32); 5] = [
    ("FbeOriginal", 0),
    ("Fbe", 1),
    ("ConnectedCentersDelaunay", 2),
    ("ConnectedCentersDelaunayMst", 3),
    ("ConnectedCentersFlute", 4),
];
const BEACON_STRATEGY_NAMES: [(&str, i32); 3] = [("FbeOriginal", 0), ("Fbe", 1), ("Snug", 2)];
const QUALITY_NAMES: [(&str, i32); 5] = [
    ("Normal", 0),
    ("Uncommon", 1),
    ("Rare", 2),
    ("Epic", 3),
    ("Legendary", 5),
];

/// `OilFieldPlanRequest` as read: the options with nulls and undefined enum values kept,
/// because the response echoes the request exactly as given.
#[derive(Clone, Debug)]
pub struct PlanRequest {
    pub blueprint: Option<String>,
    pub add_fbe_offset: bool,
    pub use_underground_pipes: bool,
    pub add_beacons: bool,
    pub optimize_pipes: bool,
    pub overlap_beacons: bool,
    pub add_electric_poles: bool,
    pub add_heat_pipes: bool,
    pub heat_pipe_entity_name: Option<String>,
    pub pipe_strategies: Option<Vec<i32>>,
    pub beacon_strategies: Option<Vec<i32>>,
    pub electric_pole_entity_name: Option<String>,
    pub electric_pole_supply_width: i32,
    pub electric_pole_supply_height: i32,
    pub electric_pole_wire_reach: f64,
    pub electric_pole_width: i32,
    pub electric_pole_height: i32,
    pub beacon_entity_name: Option<String>,
    pub beacon_supply_width: i32,
    pub beacon_supply_height: i32,
    pub beacon_width: i32,
    pub beacon_height: i32,
    pub validate_solution: bool,
    pub pumpjack_modules: Option<Vec<(String, i32)>>,
    pub beacon_modules: Option<Vec<(String, i32)>>,
    pub pumpjack_quality: i32,
    pub beacon_quality: i32,
    pub electric_pole_quality: i32,
    pub pumpjack_module_quality: i32,
    pub beacon_module_quality: i32,
}

impl Default for PlanRequest {
    /// `new OilFieldPlanRequest()`: the options' defaults, no blueprint, no FBE offset.
    fn default() -> Self {
        let o = OilFieldOptions::default();
        PlanRequest {
            blueprint: None,
            add_fbe_offset: false,
            use_underground_pipes: o.use_underground_pipes,
            add_beacons: o.add_beacons,
            optimize_pipes: o.optimize_pipes,
            overlap_beacons: o.overlap_beacons,
            add_electric_poles: o.add_electric_poles,
            add_heat_pipes: o.add_heat_pipes,
            heat_pipe_entity_name: Some(o.heat_pipe_entity_name),
            pipe_strategies: Some(o.pipe_strategies.iter().map(|s| s.value()).collect()),
            beacon_strategies: Some(o.beacon_strategies.iter().map(|s| s.value()).collect()),
            electric_pole_entity_name: Some(o.electric_pole_entity_name),
            electric_pole_supply_width: o.electric_pole_supply_width,
            electric_pole_supply_height: o.electric_pole_supply_height,
            electric_pole_wire_reach: o.electric_pole_wire_reach,
            electric_pole_width: o.electric_pole_width,
            electric_pole_height: o.electric_pole_height,
            beacon_entity_name: Some(o.beacon_entity_name),
            beacon_supply_width: o.beacon_supply_width,
            beacon_supply_height: o.beacon_supply_height,
            beacon_width: o.beacon_width,
            beacon_height: o.beacon_height,
            validate_solution: o.validate_solution,
            pumpjack_modules: Some(o.pumpjack_modules),
            beacon_modules: Some(o.beacon_modules),
            pumpjack_quality: o.pumpjack_quality.level(),
            beacon_quality: o.beacon_quality.level(),
            electric_pole_quality: o.electric_pole_quality.level(),
            pumpjack_module_quality: o.pumpjack_module_quality.level(),
            beacon_module_quality: o.beacon_module_quality.level(),
        }
    }
}

/// Reads an `OilFieldPlanRequest`.
pub fn read_plan_request(json: &str) -> Result<PlanRequest, Thrown> {
    let Root::Object(properties) =
        read_root(json, "Knapcode.FactorioTools.Contract.OilFieldPlanRequest")?
    else {
        // `Deserialize` returns null and `Plan(null)` dereferences it.
        return Err(Thrown::null_reference());
    };

    let mut r = PlanRequest::default();
    for (name, raw) in &properties {
        let p = Property {
            name,
            raw: raw.get(),
        };
        // `PropertyNameCaseInsensitive`; the names are ASCII.
        match name.to_ascii_lowercase().as_str() {
            "blueprint" => r.blueprint = p.string()?,
            "addfbeoffset" => r.add_fbe_offset = p.bool()?,
            "useundergroundpipes" => r.use_underground_pipes = p.bool()?,
            "addbeacons" => r.add_beacons = p.bool()?,
            "optimizepipes" => r.optimize_pipes = p.bool()?,
            "overlapbeacons" => r.overlap_beacons = p.bool()?,
            "addelectricpoles" => r.add_electric_poles = p.bool()?,
            "addheatpipes" => r.add_heat_pipes = p.bool()?,
            "heatpipeentityname" => r.heat_pipe_entity_name = p.string()?,
            "pipestrategies" => {
                r.pipe_strategies = p.enum_list(
                    &PIPE_STRATEGY_NAMES,
                    "Knapcode.FactorioTools.OilField.PipeStrategy",
                )?
            }
            "beaconstrategies" => {
                r.beacon_strategies = p.enum_list(
                    &BEACON_STRATEGY_NAMES,
                    "Knapcode.FactorioTools.OilField.BeaconStrategy",
                )?
            }
            "electricpoleentityname" => r.electric_pole_entity_name = p.string()?,
            "electricpolesupplywidth" => r.electric_pole_supply_width = p.int()?,
            "electricpolesupplyheight" => r.electric_pole_supply_height = p.int()?,
            "electricpolewirereach" => r.electric_pole_wire_reach = p.double()?,
            "electricpolewidth" => r.electric_pole_width = p.int()?,
            "electricpoleheight" => r.electric_pole_height = p.int()?,
            "beaconentityname" => r.beacon_entity_name = p.string()?,
            "beaconsupplywidth" => r.beacon_supply_width = p.int()?,
            "beaconsupplyheight" => r.beacon_supply_height = p.int()?,
            "beaconwidth" => r.beacon_width = p.int()?,
            "beaconheight" => r.beacon_height = p.int()?,
            "validatesolution" => r.validate_solution = p.bool()?,
            "pumpjackmodules" => r.pumpjack_modules = p.int_dictionary()?,
            "beaconmodules" => r.beacon_modules = p.int_dictionary()?,
            "pumpjackquality" => r.pumpjack_quality = quality(&p)?,
            "beaconquality" => r.beacon_quality = quality(&p)?,
            "electricpolequality" => r.electric_pole_quality = quality(&p)?,
            "pumpjackmodulequality" => r.pumpjack_module_quality = quality(&p)?,
            "beaconmodulequality" => r.beacon_module_quality = quality(&p)?,
            _ => {}
        }
    }

    Ok(r)
}

fn quality(p: &Property) -> Result<i32, Thrown> {
    p.enum_value(&QUALITY_NAMES, "Knapcode.FactorioTools.OilField.Quality")
}

impl PlanRequest {
    /// The planner's options. The C# planner reads the request object itself; a few values
    /// it can hold do not fit the Rust types, and this port declines those requests:
    ///
    /// - a `null` strategy list, entity name or module dictionary (the C# throws a
    ///   `NullReferenceException` where it reads one, and writes an entity with no name
    ///   for a `null` name);
    /// - a strategy integer the enum does not name (the C# planner throws where it reads one).
    ///
    /// The site always sends every option with a real value, so neither comes up in use.
    pub fn to_options(&self) -> Result<OilFieldOptions, Thrown> {
        fn required<T: Clone>(value: &Option<T>, name: &str) -> Result<T, Thrown> {
            value.clone().ok_or_else(|| {
                Thrown::not_supported(format!(
                    "The request's {name} is null, which the Rust port does not support."
                ))
            })
        }

        let pipe_strategies = required(&self.pipe_strategies, "pipeStrategies")?
            .into_iter()
            .map(|v| {
                PipeStrategy::ALL
                    .into_iter()
                    .find(|s| s.value() == v)
                    .ok_or_else(|| {
                        Thrown::not_supported(format!("Pipe strategy {v} is not defined."))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let beacon_strategies = required(&self.beacon_strategies, "beaconStrategies")?
            .into_iter()
            .map(|v| {
                BeaconStrategy::ALL
                    .into_iter()
                    .find(|s| s.value() == v)
                    .ok_or_else(|| {
                        Thrown::not_supported(format!("Beacon strategy {v} is not defined."))
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(OilFieldOptions {
            use_underground_pipes: self.use_underground_pipes,
            add_beacons: self.add_beacons,
            optimize_pipes: self.optimize_pipes,
            overlap_beacons: self.overlap_beacons,
            add_electric_poles: self.add_electric_poles,
            add_heat_pipes: self.add_heat_pipes,
            heat_pipe_entity_name: required(&self.heat_pipe_entity_name, "heatPipeEntityName")?,
            pipe_strategies,
            beacon_strategies,
            electric_pole_entity_name: required(
                &self.electric_pole_entity_name,
                "electricPoleEntityName",
            )?,
            electric_pole_supply_width: self.electric_pole_supply_width,
            electric_pole_supply_height: self.electric_pole_supply_height,
            electric_pole_wire_reach: self.electric_pole_wire_reach,
            electric_pole_width: self.electric_pole_width,
            electric_pole_height: self.electric_pole_height,
            beacon_entity_name: required(&self.beacon_entity_name, "beaconEntityName")?,
            beacon_supply_width: self.beacon_supply_width,
            beacon_supply_height: self.beacon_supply_height,
            beacon_width: self.beacon_width,
            beacon_height: self.beacon_height,
            validate_solution: self.validate_solution,
            pumpjack_modules: required(&self.pumpjack_modules, "pumpjackModules")?,
            beacon_modules: required(&self.beacon_modules, "beaconModules")?,
            pumpjack_quality: Quality(self.pumpjack_quality),
            beacon_quality: Quality(self.beacon_quality),
            electric_pole_quality: Quality(self.electric_pole_quality),
            pumpjack_module_quality: Quality(self.pumpjack_module_quality),
            beacon_module_quality: Quality(self.beacon_module_quality),
        })
    }

    /// The request as `JsonSerializer.Serialize(request, ContractJson.Options)` writes it.
    pub fn to_json(&self) -> String {
        let mut w = JsonWriter::new();
        self.write(&mut w);
        w.finish()
    }

    /// Writes the request as the response echoes it: the request type's own properties
    /// first, then `OilFieldOptions`' in declaration order, as `System.Text.Json` orders a
    /// derived type.
    fn write(&self, w: &mut JsonWriter) {
        w.begin_object();
        w.name("blueprint")
            .string_or_null(self.blueprint.as_deref());
        w.name("addFbeOffset").bool(self.add_fbe_offset);
        w.name("useUndergroundPipes")
            .bool(self.use_underground_pipes);
        w.name("addBeacons").bool(self.add_beacons);
        w.name("optimizePipes").bool(self.optimize_pipes);
        w.name("overlapBeacons").bool(self.overlap_beacons);
        w.name("addElectricPoles").bool(self.add_electric_poles);
        w.name("addHeatPipes").bool(self.add_heat_pipes);
        w.name("heatPipeEntityName")
            .string_or_null(self.heat_pipe_entity_name.as_deref());
        w.name("pipeStrategies");
        write_enum_list(w, self.pipe_strategies.as_deref(), &PIPE_STRATEGY_NAMES);
        w.name("beaconStrategies");
        write_enum_list(w, self.beacon_strategies.as_deref(), &BEACON_STRATEGY_NAMES);
        w.name("electricPoleEntityName")
            .string_or_null(self.electric_pole_entity_name.as_deref());
        w.name("electricPoleSupplyWidth")
            .int(self.electric_pole_supply_width.into());
        w.name("electricPoleSupplyHeight")
            .int(self.electric_pole_supply_height.into());
        w.name("electricPoleWireReach")
            .f64(self.electric_pole_wire_reach);
        w.name("electricPoleWidth")
            .int(self.electric_pole_width.into());
        w.name("electricPoleHeight")
            .int(self.electric_pole_height.into());
        w.name("beaconEntityName")
            .string_or_null(self.beacon_entity_name.as_deref());
        w.name("beaconSupplyWidth")
            .int(self.beacon_supply_width.into());
        w.name("beaconSupplyHeight")
            .int(self.beacon_supply_height.into());
        w.name("beaconWidth").int(self.beacon_width.into());
        w.name("beaconHeight").int(self.beacon_height.into());
        w.name("validateSolution").bool(self.validate_solution);
        w.name("pumpjackModules");
        write_int_dictionary(w, self.pumpjack_modules.as_deref());
        w.name("beaconModules");
        write_int_dictionary(w, self.beacon_modules.as_deref());
        w.name("pumpjackQuality");
        write_enum(w, self.pumpjack_quality, &QUALITY_NAMES);
        w.name("beaconQuality");
        write_enum(w, self.beacon_quality, &QUALITY_NAMES);
        w.name("electricPoleQuality");
        write_enum(w, self.electric_pole_quality, &QUALITY_NAMES);
        w.name("pumpjackModuleQuality");
        write_enum(w, self.pumpjack_module_quality, &QUALITY_NAMES);
        w.name("beaconModuleQuality");
        write_enum(w, self.beacon_module_quality, &QUALITY_NAMES);
        w.end_object();
    }
}

/// `JsonStringEnumConverter`: the member name, or the integer for an undefined value.
fn write_enum(w: &mut JsonWriter, value: i32, names: &[(&str, i32)]) {
    match names.iter().find(|(_, v)| *v == value) {
        Some((name, _)) => w.string(name),
        None => w.int(value.into()),
    };
}

fn write_enum_list(w: &mut JsonWriter, values: Option<&[i32]>, names: &[(&str, i32)]) {
    match values {
        None => {
            w.null();
        }
        Some(values) => {
            w.begin_array();
            for value in values {
                write_enum(w, *value, names);
            }
            w.end_array();
        }
    }
}

fn write_int_dictionary(w: &mut JsonWriter, dictionary: Option<&[(String, i32)]>) {
    match dictionary {
        None => {
            w.null();
        }
        Some(entries) => {
            w.begin_object();
            for (key, value) in entries {
                w.name(key).int((*value).into());
            }
            w.end_object();
        }
    }
}

fn write_plan(w: &mut JsonWriter, plan: &OilFieldPlan) {
    w.begin_object();
    w.name("pipeStrategy").string(plan.pipe_strategy.name());
    w.name("optimizePipes").bool(plan.optimize_pipes);
    w.name("beaconStrategy")
        .string_or_null(plan.beacon_strategy.map(|s| s.name()));
    w.name("beaconEffectCount")
        .int(plan.beacon_effect_count.into());
    w.name("beaconCount").int(plan.beacon_count.into());
    w.name("pipeCount").int(plan.pipe_count.into());
    w.name("pipeCountWithoutUnderground")
        .int(plan.pipe_count_without_underground.into());
    w.end_object();
}

fn write_summary(w: &mut JsonWriter, summary: &OilFieldPlanSummary) {
    w.begin_object();
    w.name("missingPumpjacks")
        .int(summary.missing_pumpjacks.into());
    w.name("rotatedPumpjacks")
        .int(summary.rotated_pumpjacks.into());
    w.name("heatDroppedPumpjacks")
        .int(summary.heat_dropped_pumpjacks.into());
    w.name("unheatedPumpjacks")
        .int(summary.unheated_pumpjacks.into());
    w.name("unheatedPipes").int(summary.unheated_pipes.into());
    for (name, plans) in [
        ("selectedPlans", &summary.selected_plans),
        ("alternatePlans", &summary.alternate_plans),
        ("unusedPlans", &summary.unused_plans),
    ] {
        w.name(name).begin_array();
        for plan in plans {
            write_plan(w, plan);
        }
        w.end_array();
    }
    w.end_object();
}

/// `ToEnvelope`: the error envelope for a `FactorioToolsException`.
pub fn error_envelope(error: &FactorioToolsError) -> String {
    let mut w = JsonWriter::new();
    w.begin_object();
    w.name("title").string(if error.bad_input {
        "Bad input was provided."
    } else {
        "A FactorioTools exception occurred."
    });
    w.name("status")
        .int(if error.bad_input { 400 } else { 500 });
    w.name("errors").begin_object();
    w.name("FactorioToolsException").begin_array();
    w.string(&error.message);
    for inner in &error.inner_messages {
        w.string(inner);
    }
    w.end_array();
    w.end_object();
    w.end_object();
    w.finish()
}

/// `PlanOrchestrator.PlanJson`.
pub fn plan_json(request_json: &str) -> Result<String, Thrown> {
    let request = read_plan_request(request_json)?;
    let options = request.to_options()?;

    let result = blueprint::parse_blueprint(request.blueprint.as_deref().unwrap_or(""))
        .and_then(|parsed| planner::planner::execute(options, parsed));
    let result = match result {
        Ok(result) => result,
        Err(error) => return Ok(error_envelope(&error)),
    };

    let output =
        blueprint::grid_to_blueprint_string(&result.context, request.add_fbe_offset, false);

    let mut w = JsonWriter::new();
    w.begin_object();
    w.name("request");
    request.write(&mut w);
    w.name("blueprint").string(&output);
    w.name("summary");
    write_summary(&mut w, &result.summary);
    w.end_object();
    Ok(w.finish())
}

/// `PlanOrchestrator.NormalizeJson`. `OilFieldNormalizeRequest` has only `blueprint`, so
/// every other property is skipped unread.
pub fn normalize_json(request_json: &str) -> Result<String, Thrown> {
    let Root::Object(properties) = read_root(
        request_json,
        "Knapcode.FactorioTools.Contract.OilFieldNormalizeRequest",
    )?
    else {
        return Err(Thrown::null_reference());
    };

    let mut blueprint_string: Option<String> = None;
    for (name, raw) in &properties {
        if name.eq_ignore_ascii_case("blueprint") {
            blueprint_string = Property {
                name,
                raw: raw.get(),
            }
            .string()?;
        }
    }

    let result = blueprint::parse_blueprint(blueprint_string.as_deref().unwrap_or(""))
        .and_then(planner::steps::clean_blueprint::execute);
    let clean = match result {
        Ok(clean) => clean,
        Err(error) => return Ok(error_envelope(&error)),
    };
    let output = blueprint::serialize_blueprint(&clean, false);

    let mut w = JsonWriter::new();
    w.begin_object();
    w.name("request").begin_object();
    w.name("blueprint")
        .string_or_null(blueprint_string.as_deref());
    w.end_object();
    w.name("blueprint").string(&output);
    w.end_object();
    Ok(w.finish())
}
