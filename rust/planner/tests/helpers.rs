//! Replays `fixtures/helpers.jsonl` (recorded from the C# by `oracle/HelpersRecorder.cs`,
//! removed in stage 13b; format in `FIXTURES.md`): every `Helpers` and `Validate` case, on
//! a context rebuilt from its recorded snapshot the same way the recorder rebuilt it, must
//! give the recorded result.

use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::PathBuf;
use std::rc::Rc;

use delaunator_net::DelaunatorError;
use dotnet_compat::NetHashSet;
use planner::Location;
use planner::containers::{CountedBitArray, LocationDictionary, LocationSet, SortedBatches};
use planner::context::Context;
use planner::data::{Blueprint, Direction};
use planner::grid::{EntityKind, GridEntity, SquareGrid, TerminalLocation};
use planner::helpers::{
    self, CandidateToCovered, PUMPJACK_HEIGHT, PUMPJACK_WIDTH, Scoped, SharedInfo,
};
use planner::models::{
    AsCandidateInfo, BeaconSolution, BeaconStrategy, CandidateFactory, CandidateInfo,
    ElectricPoleCandidateInfo, ProviderRecipient,
};
use planner::options::OilFieldOptions;
use planner::steps::validate;
use serde_json::Value;

// -------------------------------------------------------------------------------------------
// JSON

fn int(value: &Value) -> i32 {
    value.as_i64().expect("an integer") as i32
}

fn boolean(value: &Value) -> bool {
    value.as_bool().expect("a bool")
}

fn string(value: &Value) -> &str {
    value.as_str().expect("a string")
}

fn arr(value: &Value) -> &Vec<Value> {
    value.as_array().expect("an array")
}

fn point(value: &Value) -> Location {
    let a = arr(value);
    Location::new(int(&a[0]), int(&a[1]))
}

/// A flat `[x0, y0, x1, y1, ...]` array.
fn points(value: &Value) -> Vec<Location> {
    arr(value)
        .chunks(2)
        .map(|p| Location::new(int(&p[0]), int(&p[1])))
        .collect()
}

fn flat(locations: impl IntoIterator<Item = Location>) -> Vec<i32> {
    locations.into_iter().flat_map(|l| [l.x, l.y]).collect()
}

fn ints(value: &Value) -> Vec<i32> {
    arr(value).iter().map(int).collect()
}

// -------------------------------------------------------------------------------------------
// Rebuilding a context

fn direction(value: i32) -> Direction {
    Direction::from_value(value).expect("a Direction value")
}

fn entity(fields: &[Value]) -> (Location, GridEntity) {
    let location = Location::new(int(&fields[0]), int(&fields[1]));
    let id = int(&fields[3]);
    let kind = match string(&fields[2]) {
        "PumpjackCenter" => EntityKind::PumpjackCenter {
            direction: direction(int(&fields[4])),
        },
        "PumpjackSide" => EntityKind::PumpjackSide {
            center: int(&fields[4]),
        },
        "BeaconCenter" => EntityKind::BeaconCenter,
        "BeaconSide" => EntityKind::BeaconSide {
            center: int(&fields[4]),
        },
        "ElectricPoleCenter" => {
            let mut neighbors = NetHashSet::new();
            for n in arr(&fields[4]) {
                neighbors.add(int(n));
            }
            EntityKind::ElectricPoleCenter { neighbors }
        }
        "ElectricPoleSide" => EntityKind::ElectricPoleSide {
            center: int(&fields[4]),
        },
        "Pipe" => EntityKind::Pipe,
        "Terminal" => EntityKind::Terminal,
        "UndergroundPipe" => EntityKind::UndergroundPipe {
            direction: direction(int(&fields[4])),
        },
        "HeatPipe" => EntityKind::HeatPipe,
        "TemporaryEntity" => EntityKind::TemporaryEntity,
        "AvoidEntity" => EntityKind::AvoidEntity,
        other => panic!("unknown entity kind {other}"),
    };
    (location, GridEntity::new(id, kind))
}

fn terminals(value: &Value) -> LocationDictionary<Vec<TerminalLocation>> {
    let mut dictionary = LocationDictionary::new();
    for pair in arr(value) {
        let pair = arr(pair);
        let key = Location::new(int(&pair[0]), int(&pair[1]));
        let list = arr(&pair[2])
            .chunks(5)
            .map(|t| {
                TerminalLocation::new(
                    Location::new(int(&t[0]), int(&t[1])),
                    Location::new(int(&t[2]), int(&t[3])),
                    direction(int(&t[4])),
                )
            })
            .collect();
        dictionary.add(key, list);
    }
    dictionary
}

/// In C#, `LocationToTerminals` holds the same terminal objects as `CenterToTerminals`,
/// and `EliminateOtherTerminals` compares them by reference. The snapshot records both
/// maps as values, so rebuild the second from the first's objects.
fn share_terminals(
    center_to_terminals: &LocationDictionary<Vec<TerminalLocation>>,
    mut location_to_terminals: LocationDictionary<Vec<TerminalLocation>>,
) -> LocationDictionary<Vec<TerminalLocation>> {
    let keys: Vec<Location> = location_to_terminals.iter().map(|(key, _)| key).collect();
    for key in keys {
        for terminal in location_to_terminals.value_mut(key).iter_mut() {
            *terminal = *center_to_terminals
                .value(terminal.center)
                .iter()
                .find(|t| *t == terminal)
                .expect("every terminal in LocationToTerminals is one of its center's");
        }
    }
    location_to_terminals
}

fn rebuild(s: &Value) -> Context {
    let o = &s["options"];
    let options = OilFieldOptions {
        beacon_width: int(&o["beacon_width"]),
        beacon_height: int(&o["beacon_height"]),
        beacon_supply_width: int(&o["beacon_supply_width"]),
        beacon_supply_height: int(&o["beacon_supply_height"]),
        electric_pole_width: int(&o["electric_pole_width"]),
        electric_pole_height: int(&o["electric_pole_height"]),
        overlap_beacons: boolean(&o["overlap_beacons"]),
        validate_solution: boolean(&o["validate_solution"]),
        add_heat_pipes: boolean(&o["add_heat_pipes"]),
        add_beacons: boolean(&o["add_beacons"]),
        ..OilFieldOptions::default()
    };

    let width = int(&s["width"]);
    let height = int(&s["height"]);
    let mut grid = SquareGrid::new(width, height);
    for fields in arr(&s["entities"]) {
        let (location, entity) = entity(arr(fields));
        grid.add_entity(location, entity).unwrap();
    }

    // `GetId()` until the next id is the recorded one.
    let next_id = int(&s["next_id"]);
    while grid.get_id() + 1 < next_id {}

    let heat_pipes = match &s["heat_pipes"] {
        Value::Null => None,
        value => {
            let mut set = LocationSet::new_hash();
            for l in points(value) {
                set.add(l);
            }
            Some(set)
        }
    };

    let supply = arr(&s["pole_supply"]);
    let center_to_terminals = terminals(&s["center_to_terminals"]);
    Context {
        options,
        input_blueprint: Blueprint::default(),
        delta_x: 0.0,
        delta_y: 0.0,
        grid,
        centers: points(&s["centers"]),
        center_to_original_direction: LocationDictionary::new(),
        location_to_terminals: share_terminals(
            &center_to_terminals,
            terminals(&s["location_to_terminals"]),
        ),
        center_to_terminals,
        location_to_adjacent_count: vec![0; (width * height) as usize],
        electric_pole_supply_width_with_quality: int(&supply[0]),
        electric_pole_supply_height_with_quality: int(&supply[1]),
        electric_pole_wire_reach_with_quality: 0.0,
        electric_pole_wire_reach_squared_with_quality: 0.0,
        heat_pipes,
        heat_dropped_pumpjacks: 0,
    }
}

fn add_pole(context: &mut Context, center: Location) -> planner::FtResult<()> {
    let (width, height) = (
        context.options.electric_pole_width,
        context.options.electric_pole_height,
    );
    let entity = GridEntity::electric_pole_center(context.grid.get_id());
    helpers::add_provider_to_grid(
        &mut context.grid,
        center,
        entity,
        |grid, c| GridEntity::new(grid.get_id(), EntityKind::ElectricPoleSide { center: c }),
        width,
        height,
    )
}

/// The recorder's `Mutation.Apply`.
fn apply_mutation(case: &Value, context: &mut Context) {
    let Some(m) = case.get("mut") else {
        return;
    };
    if let Some(overlap) = m.get("overlap_beacons") {
        context.options.overlap_beacons = boolean(overlap);
    }
    for l in points(&m["remove_cells"]) {
        context.grid.remove_entity(l);
    }
    for l in points(&m["add_poles"]) {
        add_pole(context, l).unwrap();
    }
    for l in points(&m["remove_heat_pipes"]) {
        context.heat_pipes.as_mut().unwrap().remove(l);
    }
}

// -------------------------------------------------------------------------------------------
// Hashes, the same as the recorder's

struct Fnv(u64);

impl Fnv {
    fn new() -> Self {
        Fnv(1469598103934665603)
    }

    fn mix(&mut self, value: i32) -> &mut Self {
        for b in (value as u32).to_le_bytes() {
            self.0 ^= u64::from(b);
            self.0 = self.0.wrapping_mul(1099511628211);
        }
        self
    }

    fn mix_double(&mut self, value: f64) -> &mut Self {
        let bits = value.to_bits();
        self.mix(bits as u32 as i32);
        self.mix((bits >> 32) as u32 as i32)
    }

    fn mix_location(&mut self, l: Location) -> &mut Self {
        self.mix(l.x).mix(l.y)
    }

    fn hex(&self) -> String {
        format!("{:016x}", self.0)
    }
}

fn kind_code(kind: &EntityKind) -> i32 {
    match kind {
        EntityKind::PumpjackCenter { .. } => 0,
        EntityKind::PumpjackSide { .. } => 1,
        EntityKind::BeaconCenter => 2,
        EntityKind::BeaconSide { .. } => 3,
        EntityKind::ElectricPoleCenter { .. } => 4,
        EntityKind::ElectricPoleSide { .. } => 5,
        EntityKind::Pipe => 6,
        EntityKind::Terminal => 7,
        EntityKind::UndergroundPipe { .. } => 8,
        EntityKind::HeatPipe => 9,
        EntityKind::TemporaryEntity => 10,
        EntityKind::AvoidEntity => 11,
    }
}

fn hash_grid(grid: &mut SquareGrid) -> String {
    let mut h = Fnv::new();
    for location in grid.entity_locations().iter() {
        let e = grid.get(location).unwrap();
        h.mix_location(location).mix(kind_code(&e.kind)).mix(e.id);
        match &e.kind {
            EntityKind::PumpjackCenter { direction } => {
                h.mix(direction.value());
            }
            EntityKind::PumpjackSide { center }
            | EntityKind::BeaconSide { center }
            | EntityKind::ElectricPoleSide { center } => {
                h.mix(*center);
            }
            EntityKind::UndergroundPipe { direction } => {
                h.mix(direction.value());
            }
            EntityKind::ElectricPoleCenter { neighbors } => {
                h.mix(neighbors.len() as i32);
                for n in neighbors.iter() {
                    h.mix(n);
                }
            }
            _ => {}
        }
    }
    h.mix(grid.get_id());
    h.hex()
}

fn hash_candidates<T: AsCandidateInfo>(d: &LocationDictionary<SharedInfo<T>>) -> String {
    let mut h = Fnv::new();
    h.mix(d.len() as i32);
    for (location, info) in d.iter() {
        let info = info.borrow();
        let info = info.info();
        h.mix_location(location)
            .mix(info.covered.count())
            .mix(info.covered.true_count());
        for i in 0..info.covered.count() {
            h.mix(i32::from(info.covered.get(i)));
        }
        h.mix_double(info.entity_distance);
    }
    h.hex()
}

fn hash_keys<T>(d: &LocationDictionary<T>) -> String {
    let mut h = Fnv::new();
    h.mix(d.len() as i32);
    for location in d.keys() {
        h.mix_location(location);
    }
    h.hex()
}

fn hash_covered_to_candidates<T>(c2c: &[LocationDictionary<T>]) -> String {
    let mut h = Fnv::new();
    h.mix(c2c.len() as i32);
    for (i, d) in c2c.iter().enumerate() {
        h.mix(i as i32).mix(d.len() as i32);
        for location in d.keys() {
            h.mix_location(location);
        }
    }
    h.hex()
}

fn hash_batches<T>(batches: &SortedBatches<T>) -> String {
    let mut h = Fnv::new();
    let keys: Vec<i32> = batches.lookup_keys().collect();
    h.mix(batches.queue_len() as i32).mix(keys.len() as i32);
    for key in keys {
        let d = batches.lookup(key).unwrap();
        h.mix(key).mix(d.len() as i32);
        for location in d.keys() {
            h.mix_location(location);
        }
    }
    h.hex()
}

fn hash_terminals(d: &LocationDictionary<Vec<TerminalLocation>>) -> String {
    let mut h = Fnv::new();
    h.mix(d.len() as i32);
    for (key, list) in d.iter() {
        h.mix_location(key).mix(list.len() as i32);
        for t in list {
            h.mix_location(t.center)
                .mix_location(t.terminal)
                .mix(t.direction.value());
        }
    }
    h.hex()
}

fn bits(b: &CountedBitArray) -> String {
    (0..b.count())
        .map(|i| if b.get(i) { '1' } else { '0' })
        .collect()
}

// -------------------------------------------------------------------------------------------
// Shared pieces of the cases

fn cells_of(grid: &SquareGrid, matches: impl Fn(&EntityKind) -> bool) -> Vec<Location> {
    grid.entity_locations()
        .iter()
        .filter(|&l| grid.get(l).is_some_and(|e| matches(&e.kind)))
        .collect()
}

fn is_pole(kind: &EntityKind) -> bool {
    matches!(kind, EntityKind::ElectricPoleCenter { .. })
}

fn is_beacon(kind: &EntityKind) -> bool {
    matches!(kind, EntityKind::BeaconCenter)
}

fn recipients_flat(recipients: &[ProviderRecipient]) -> Vec<i32> {
    recipients
        .iter()
        .flat_map(|r| [r.center.x, r.center.y, r.width, r.height])
        .collect()
}

fn set_dictionary(d: &LocationDictionary<LocationSet>) -> Value {
    Value::Array(
        d.iter()
            .map(|(key, set)| serde_json::json!([key.x, key.y, flat(set.iter())]))
            .collect(),
    )
}

/// An outcome as the recorder writes it: `{"ok": true}` or `{"error": message}`.
fn result_json(result: planner::FtResult<()>) -> Value {
    match result {
        Ok(()) => serde_json::json!({ "ok": true }),
        Err(e) => serde_json::json!({ "error": e.message }),
    }
}

/// Runs `f`, turning a panic into its message, the way the recorder catches
/// `ArgumentException`.
fn catch<R>(f: impl FnOnce() -> R) -> Result<R, String> {
    // Keep the expected panic quiet, then put the normal hook back so real test failures
    // still print.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = catch_unwind(AssertUnwindSafe(f));
    std::panic::set_hook(default_hook);
    result.map_err(|payload| {
        payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_default()
    })
}

struct PlainFactory;

impl CandidateFactory<CandidateInfo> for PlainFactory {
    fn create(&self, covered: CountedBitArray) -> CandidateInfo {
        CandidateInfo::new(covered)
    }
}

struct PoleFactory;

impl CandidateFactory<ElectricPoleCandidateInfo> for PoleFactory {
    fn create(&self, covered: CountedBitArray) -> ElectricPoleCandidateInfo {
        ElectricPoleCandidateInfo::new(covered)
    }
}

fn recipients(context: &Context, provider: &str) -> Vec<ProviderRecipient> {
    if provider == "pole" {
        helpers::get_powered_entities(context).0
    } else {
        context
            .centers
            .iter()
            .map(|&c| ProviderRecipient::new(c, PUMPJACK_WIDTH, PUMPJACK_HEIGHT))
            .collect()
    }
}

struct Run<T> {
    context: Context,
    recipients: Vec<ProviderRecipient>,
    result: CandidateToCovered<T>,
}

fn run_candidates<T: AsCandidateInfo>(
    snapshot: &Value,
    case: &Value,
    provider: &str,
    factory: &impl CandidateFactory<T>,
    remove_unused: bool,
) -> Run<T> {
    let mut context = rebuild(snapshot);
    apply_mutation(case, &mut context);
    let recipients = recipients(&context, provider);
    let result = if provider == "pole" {
        helpers::get_electric_pole_candidate_to_covered(
            &mut context,
            &recipients,
            factory,
            remove_unused,
        )
    } else {
        helpers::get_beacon_candidate_to_covered(&mut context, &recipients, factory, remove_unused)
    };
    Run {
        context,
        recipients,
        result,
    }
}

fn copy_dictionary<T>(d: &LocationDictionary<SharedInfo<T>>) -> LocationDictionary<SharedInfo<T>> {
    let mut copy = LocationDictionary::new();
    for (location, info) in d.iter() {
        copy.add(location, Rc::clone(info));
    }
    copy
}

// -------------------------------------------------------------------------------------------
// The cases

fn check_candidates<T: AsCandidateInfo>(
    name: &str,
    snapshot: &Value,
    case: &Value,
    factory: &impl CandidateFactory<T>,
) {
    let out = &case["out"];
    let provider = string(&case["provider"]);
    let remove_unused = boolean(&case["remove_unused"]);
    let Run {
        mut context,
        recipients,
        result,
    } = run_candidates(snapshot, case, provider, factory, remove_unused);

    assert_eq!(
        recipients_flat(&recipients),
        ints(&out["recipients"]),
        "{name}: recipients"
    );
    if let Some(list) = out.get("list") {
        let actual: Vec<Value> = result
            .candidate_to_info
            .iter()
            .map(|(l, info)| serde_json::json!([l.x, l.y, bits(&info.borrow().info().covered)]))
            .collect();
        assert_eq!(&Value::Array(actual), list, "{name}: list");
    }
    assert_eq!(
        result.candidate_to_info.len() as i64,
        out["count"].as_i64().unwrap(),
        "{name}: count"
    );
    assert_eq!(
        hash_candidates(&result.candidate_to_info),
        string(&out["hash"]),
        "{name}: candidates"
    );
    assert_eq!(
        bits(&result.covered_entities),
        string(&out["covered"]),
        "{name}: covered"
    );
    let providers: Vec<i32> = result
        .providers
        .iter()
        .flat_map(|(l, &id)| [l.x, l.y, id])
        .collect();
    assert_eq!(providers, ints(&out["providers"]), "{name}: providers");
    assert_eq!(
        hash_grid(&mut context.grid),
        string(&out["grid"]),
        "{name}: grid"
    );

    let mut distances = Fnv::new();
    for (location, info) in result.candidate_to_info.iter() {
        distances.mix_double(helpers::get_entity_distance(
            &recipients,
            location,
            &info.borrow().info().covered,
        ));
    }
    assert_eq!(
        distances.hex(),
        string(&out["entity_distance"]),
        "{name}: entity distance"
    );
    let c2c = helpers::get_covered_to_candidates(
        &context,
        &result.candidate_to_info,
        &result.covered_entities,
    );
    assert_eq!(
        hash_covered_to_candidates(&c2c),
        string(&out["covered_to_candidates"]),
        "{name}: covered to candidates"
    );
}

fn check_covers_more(name: &str, snapshot: &Value, case: &Value) {
    let Some(expected) = case["out"].get("covers_more") else {
        return;
    };
    let run = run_candidates(snapshot, case, "pole", &PoleFactory, true);
    let (first, info) = run.result.candidate_to_info.iter().next().unwrap();
    let info = info.borrow();
    let mut actual = Vec::new();
    for covered in [
        CountedBitArray::new(run.recipients.len() as i32),
        info.base.covered.clone(),
    ] {
        actual.push(
            match validate::candidate_covers_more_entities(
                &run.context,
                &run.recipients,
                &covered,
                first,
                &info,
            ) {
                Ok(()) => "ok".to_string(),
                Err(e) => e.message,
            },
        );
    }
    let expected: Vec<String> = arr(expected)
        .iter()
        .map(|v| string(v).to_string())
        .collect();
    assert_eq!(actual, expected, "{name}: covers more");
}

/// The recorder's `SimulateAllow`.
fn check_allow(name: &str, snapshot: &Value, case: &Value, key: &str, separate: bool) {
    let Some(expected) = case["out"].get(key) else {
        return;
    };
    let Run {
        context,
        recipients,
        result,
    } = run_candidates(snapshot, case, "pole", &PoleFactory, true);
    let CandidateToCovered {
        candidate_to_info: mut all,
        covered_entities: mut covered,
        ..
    } = result;

    for (location, info) in all.iter() {
        let distance =
            helpers::get_entity_distance(&recipients, location, &info.borrow().base.covered);
        info.borrow_mut().base.entity_distance = distance;
    }

    let mut c2c = helpers::get_covered_to_candidates(&context, &all, &covered);

    // `Dictionary<int, ...>` in insertion order; nothing is removed from it.
    let mut groups: Vec<(
        i32,
        LocationDictionary<SharedInfo<ElectricPoleCandidateInfo>>,
    )> = Vec::new();
    for (location, info) in all.iter() {
        let count = info.borrow().base.covered.true_count();
        match groups.iter_mut().find(|(k, _)| *k == count) {
            Some((_, d)) => d.add(location, Rc::clone(info)),
            None => {
                let mut d = LocationDictionary::new();
                d.add(location, Rc::clone(info));
                groups.push((count, d));
            }
        }
    }
    let mut batches = SortedBatches::new(groups, false);

    enum State {
        Unset,
        Batch(usize),
        Separate,
    }
    let mut separate_dictionary = separate.then(|| copy_dictionary(&all));
    let mut state = if separate {
        State::Separate
    } else {
        State::Unset
    };

    let width = context.options.electric_pole_width;
    let height = context.options.electric_pole_height;
    let mut steps = Vec::new();
    while covered.any(false) && steps.len() < 60 {
        if let State::Unset = state {
            if separate || batches.queue_len() == 0 {
                break;
            }
            let index = batches.peek_index().unwrap();
            if batches.batch(index).is_empty() {
                batches.dequeue();
                continue;
            }
            state = State::Batch(index);
        }

        let scoped_first = match state {
            State::Batch(index) => batches.batch(index).iter().next(),
            State::Separate => separate_dictionary.as_ref().unwrap().iter().next(),
            State::Unset => unreachable!(),
        }
        .map(|(l, info)| (l, Rc::clone(info)))
        .unwrap();
        let (candidate, info) = scoped_first;

        if !all.contains_key(candidate) {
            let now_empty = match state {
                State::Batch(index) => {
                    let d = batches.batch_mut(index);
                    d.remove(candidate);
                    d.is_empty()
                }
                _ => {
                    let d = separate_dictionary.as_mut().unwrap();
                    d.remove(candidate);
                    d.is_empty()
                }
            };
            if now_empty {
                state = State::Unset;
            }
            continue;
        }

        let scoped = match state {
            State::Batch(index) => Scoped::Batch(index),
            _ => Scoped::Separate(separate_dictionary.as_mut().unwrap()),
        };
        let outcome = helpers::add_provider_and_allow_multiple_providers(
            &context,
            candidate,
            &info,
            width,
            height,
            &recipients,
            &mut covered,
            &mut c2c,
            &mut all,
            scoped,
            &mut batches,
        );

        let scoped_len = match state {
            State::Batch(index) => batches.batch(index).len(),
            _ => separate_dictionary.as_ref().unwrap().len(),
        };
        let error = outcome.as_ref().err().map(|e| e.message.clone());
        steps.push(serde_json::json!([
            candidate.x,
            candidate.y,
            error.clone().unwrap_or_else(|| bits(&covered)),
            hash_candidates(&all),
            hash_covered_to_candidates(&c2c),
            hash_batches(&batches),
            scoped_len,
        ]));

        if error.is_some() {
            break;
        }
        if scoped_len == 0 {
            state = State::Unset;
        }
    }

    assert_eq!(&Value::Array(steps), expected, "{name}: {key}");
}

/// The recorder's `SimulatePrevent`.
fn check_prevent(name: &str, snapshot: &Value, case: &Value) {
    let Some(expected) = case["out"].get("prevent") else {
        return;
    };
    let Run {
        context,
        recipients,
        result,
    } = run_candidates(snapshot, case, "beacon", &PlainFactory, false);
    let CandidateToCovered {
        mut candidate_to_info,
        covered_entities: mut covered,
        ..
    } = result;
    let mut c2c = helpers::get_covered_to_candidates(&context, &candidate_to_info, &covered);
    let (width, height) = (context.options.beacon_width, context.options.beacon_height);

    let mut steps = Vec::new();
    while steps.len() < 60 {
        let pick = candidate_to_info
            .iter()
            .find(|(_, info)| {
                let mut newly = covered.clone();
                newly.not();
                newly.and(&info.borrow().covered);
                newly.true_count() > 0
            })
            .map(|(l, info)| (l, Rc::clone(info)));
        let Some((pick, info)) = pick else {
            break;
        };

        helpers::add_provider_and_prevent_multiple_providers(
            &context,
            pick,
            &info,
            width,
            height,
            &recipients,
            &mut covered,
            &mut c2c,
            &mut candidate_to_info,
        )
        .unwrap();
        steps.push(serde_json::json!([
            pick.x,
            pick.y,
            bits(&covered),
            hash_keys(&candidate_to_info),
            hash_covered_to_candidates(&c2c),
        ]));
    }
    assert_eq!(&Value::Array(steps), expected, "{name}: prevent");

    let rest = candidate_to_info
        .iter()
        .next()
        .map(|(l, info)| (l, Rc::clone(info)));
    if let Some((rest, info)) = rest {
        let outcome = helpers::add_provider_and_prevent_multiple_providers(
            &context,
            rest,
            &info,
            width,
            height,
            &recipients,
            &mut covered,
            &mut c2c,
            &mut candidate_to_info,
        );
        let actual = match outcome {
            Ok(()) => "ok".to_string(),
            Err(e) => e.message,
        };
        assert_eq!(
            actual,
            string(&case["out"]["prevent_last"]),
            "{name}: prevent last"
        );
    } else {
        assert!(case["out"].get("prevent_last").is_none(), "{name}");
    }
}

/// The recorder's `SimulateOverlap`.
fn check_overlap(name: &str, snapshot: &Value, case: &Value) {
    let Some(expected) = case["out"].get("overlap") else {
        return;
    };
    let Run {
        context, result, ..
    } = run_candidates(snapshot, case, "beacon", &PlainFactory, false);
    let candidates = &result.candidate_to_info;
    let covered = &result.covered_entities;
    let grid = &context.grid;
    let (width, height) = (context.options.beacon_width, context.options.beacon_height);
    let centers: Vec<Location> = candidates.keys().take(4).collect();

    let mut one = copy_dictionary(candidates);
    let mut two = copy_dictionary(candidates);
    let mut two_covered = helpers::get_covered_to_candidates(&context, &two, covered);
    let mut three = copy_dictionary(candidates);
    let mut three_scoped = copy_dictionary(candidates);
    let mut four = copy_dictionary(candidates);
    let mut four_scoped = copy_dictionary(candidates);
    let mut four_covered = helpers::get_covered_to_candidates(&context, &four, covered);
    let mut unused_batches: SortedBatches<SharedInfo<CandidateInfo>> =
        SortedBatches::new(Vec::new(), false);

    let mut actual = Vec::new();
    for center in centers {
        helpers::remove_overlapping_candidates(grid, center, width, height, &mut one);
        helpers::remove_overlapping_candidates_covered(
            grid,
            center,
            width,
            height,
            &mut two,
            &mut two_covered,
        );
        helpers::remove_overlapping_candidates_scoped(
            grid,
            center,
            width,
            height,
            &mut three,
            &mut Scoped::Separate(&mut three_scoped),
            &mut unused_batches,
            None,
        );
        helpers::remove_overlapping_candidates_scoped(
            grid,
            center,
            width,
            height,
            &mut four,
            &mut Scoped::Separate(&mut four_scoped),
            &mut unused_batches,
            Some(&mut four_covered),
        );
        actual.push(serde_json::json!([
            center.x,
            center.y,
            hash_keys(&one),
            hash_keys(&two),
            hash_covered_to_candidates(&two_covered),
            hash_keys(&three),
            hash_keys(&three_scoped),
            hash_keys(&four),
            hash_keys(&four_scoped),
            hash_covered_to_candidates(&four_covered),
        ]));
    }
    assert_eq!(&Value::Array(actual), expected, "{name}: overlap");
}

fn check_validate(name: &str, snapshot: &Value, case: &Value) {
    let mut context = rebuild(snapshot);
    apply_mutation(case, &mut context);
    let out = &case["out"];
    let grid = &context.grid;
    let pipes = cells_of(grid, EntityKind::is_pipe);
    let check = string(&case["check"]);
    let actual = match check {
        "pipes_connected" => {
            let drop = int(&case["drop"]);
            let mut set = context.get_location_set_enumerable(true);
            for (i, &p) in pipes.iter().enumerate() {
                if i as i32 != drop {
                    set.add(p);
                }
            }
            result_json(validate::pipes_are_connected(&context, &set))
        }
        "underground_are_pipes" => {
            let drop = int(&case["drop"]);
            let undergrounds = cells_of(grid, |k| matches!(k, EntityKind::UndergroundPipe { .. }));
            let mut directions = LocationDictionary::new();
            for &u in &undergrounds {
                let EntityKind::UndergroundPipe { direction } = grid.get(u).unwrap().kind else {
                    unreachable!()
                };
                directions.add(u, direction);
            }
            let mut set = context.get_location_set_enumerable(true);
            for &p in &pipes {
                if drop < 0 || p != undergrounds[drop as usize] {
                    set.add(p);
                }
            }
            result_json(validate::underground_pipes_are_pipes(
                &context,
                &set,
                &directions,
            ))
        }
        "pipes_do_not_match" => {
            let variant = string(&case["variant"]);
            let enumerable = variant != "bit_same";
            let mut one = context.get_location_set_enumerable(enumerable);
            let mut two = context.get_location_set_enumerable(enumerable);
            for &p in &pipes {
                one.add(p);
                two.add(p);
            }
            if variant == "minus_one" && !pipes.is_empty() {
                two.remove(pipes[0]);
            }
            result_json(validate::pipes_do_not_match(&context, &one, &two))
        }
        "beacons_do_not_overlap" => {
            let solutions = vec![BeaconSolution {
                strategy: BeaconStrategy::Snug,
                beacons: cells_of(grid, is_beacon),
                effects: 0,
                effects_given_counts: Vec::new(),
            }];
            result_json(validate::beacons_do_not_overlap(&context, &solutions))
        }
        "no_existing_beacons" => {
            let mut existing = LocationDictionary::new();
            if string(&case["variant"]) == "grid" {
                for b in cells_of(grid, is_beacon) {
                    existing.add(b, grid.get(b).unwrap().id);
                }
            }
            result_json(validate::no_existing_beacons(&context, &existing))
        }
        "unheated" => {
            let (pumpjacks, unheated_pipes) = validate::count_unheated_targets(&context);
            serde_json::json!({ "counts": [pumpjacks, unheated_pipes] })
        }
        "no_unheated_beacons" => result_json(validate::no_unheated_beacons(&context)),
        "heat_connected" => result_json(validate::heat_pipes_are_connected(&context)),
        "all_power" => result_json(validate::all_entities_have_power(&context)),
        other => panic!("unknown check {other}"),
    };
    assert_eq!(&actual, out, "{name}: {check}");
}

fn check_mutate_grid(name: &str, snapshot: &Value, case: &Value) {
    let mut context = rebuild(snapshot);
    let center = point(&case["center"]);
    let size = ints(&case["size"]);
    let action = string(&case["action"]);
    let mut error = None;
    match action {
        "remove_entity" => helpers::remove_entity(&mut context.grid, center, size[0], size[1]),
        "remove_pumpjack" => helpers::remove_pumpjack(&mut context.grid, center),
        "add_pole" => error = add_pole(&mut context, center).err(),
        "add_beacons" => {
            error = helpers::add_beacons_to_grid(
                &mut context.grid,
                &context.options,
                points(&case["centers"]),
            )
            .err();
        }
        other => panic!("unknown action {other}"),
    }

    let out = &case["out"];
    assert_eq!(
        error.map(|e| e.message),
        out.get("error").map(|e| string(e).to_string()),
        "{name}: {action} error"
    );
    assert_eq!(
        context.grid.entity_locations().len() as i64,
        out["count"].as_i64().unwrap(),
        "{name}: {action} count"
    );
    assert_eq!(
        hash_grid(&mut context.grid),
        string(&out["grid"]),
        "{name}: {action} grid"
    );
}

fn check_fit(name: &str, snapshot: &Value, case: &Value) {
    let context = rebuild(snapshot);
    let grid = &context.grid;
    let size = ints(&case["size"]);
    let (width, height) = (size[0], size[1]);
    let mut fit = String::new();
    let mut bounds = String::new();
    for y in 0..grid.height() {
        for x in 0..grid.width() {
            let l = Location::new(x, y);
            let inside = x - ((width - 1) / 2) >= 0
                && y - ((height - 1) / 2) >= 0
                && x + (width / 2) < grid.width()
                && y + (height / 2) < grid.height();
            fit.push(if !inside {
                '-'
            } else if helpers::does_provider_fit(grid, width, height, l) {
                '1'
            } else {
                '0'
            });
            bounds.push(if helpers::is_provider_in_bounds(grid, width, height, l) {
                '1'
            } else {
                '0'
            });
        }
    }
    assert_eq!(fit, string(&case["out"]["fit"]), "{name}: fit");
    assert_eq!(bounds, string(&case["out"]["bounds"]), "{name}: bounds");
}

fn check_terminals(name: &str, snapshot: &Value, case: &Value) {
    let context = rebuild(snapshot);
    let out = &case["out"];
    let c2t = helpers::get_center_to_terminals(&context, &context.grid, &context.centers).unwrap();
    let l2t = helpers::get_location_to_terminals(&context, &c2t);
    assert_eq!(hash_terminals(&c2t), string(&out["c2t"]), "{name}: c2t");
    assert_eq!(hash_terminals(&l2t), string(&out["l2t"]), "{name}: l2t");

    let mut actual = Vec::new();
    for &center in context.centers.iter().take(6) {
        let mut fresh = rebuild(snapshot);
        let selected = *fresh
            .center_to_terminals
            .value(center)
            .last()
            .expect("a terminal option");
        helpers::eliminate_other_terminals(&mut fresh, selected);
        actual.push(serde_json::json!([
            center.x,
            center.y,
            hash_terminals(&fresh.center_to_terminals),
            hash_terminals(&fresh.location_to_terminals),
        ]));
    }
    assert_eq!(
        &Value::Array(actual),
        &out["eliminate"],
        "{name}: eliminate"
    );
}

fn lines_json(result: Result<Vec<planner::models::Endpoints>, DelaunatorError>) -> Value {
    match result {
        Ok(lines) => serde_json::json!({
            "lines": lines.iter().flat_map(|e| [e.a.x, e.a.y, e.b.x, e.b.y]).collect::<Vec<_>>()
        }),
        Err(DelaunatorError::TooFewPoints) => {
            serde_json::json!({ "error_type": "ArgumentOutOfRangeException" })
        }
        Err(DelaunatorError::NoTriangulation) => serde_json::json!({
            "error_type": "Exception",
            "error": "No Delaunay triangulation exists for this input."
        }),
    }
}

#[test]
fn helpers_match_the_csharp() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/helpers.jsonl");
    let text = std::fs::read_to_string(&path).expect("read helpers.jsonl");

    let mut contexts: HashMap<i64, Value> = HashMap::new();
    let mut ops: HashMap<String, usize> = HashMap::new();
    for (line_number, line) in text.lines().enumerate() {
        let case: Value = serde_json::from_str(line).expect("valid JSON");
        if string(&case["kind"]) == "context" {
            contexts.insert(case["id"].as_i64().unwrap(), case);
            continue;
        }

        let op = string(&case["op"]);
        *ops.entry(op.to_string()).or_default() += 1;
        let snapshot = case.get("ctx").map(|id| &contexts[&id.as_i64().unwrap()]);
        let name = format!(
            "line {} ({op} on {})",
            line_number + 1,
            snapshot.map_or("no context", |s| string(&s["name"]))
        );
        let out = &case["out"];

        match op {
            "powered" => {
                let context = rebuild(snapshot.unwrap());
                let (recipients, has_beacons) = helpers::get_powered_entities(&context);
                assert_eq!(
                    recipients_flat(&recipients),
                    ints(&out["recipients"]),
                    "{name}"
                );
                assert_eq!(has_beacons, boolean(&out["has_beacons"]), "{name}");
            }
            "pole_coverage" => {
                let mut context = rebuild(snapshot.unwrap());
                apply_mutation(&case, &mut context);
                let (powered, _) = helpers::get_powered_entities(&context);
                let poles = cells_of(&context.grid, is_pole);
                let actual = match helpers::get_electric_pole_coverage(&context, &powered, poles) {
                    Ok((p2c, c2p)) => {
                        serde_json::json!({ "p2c": set_dictionary(&p2c), "c2p": set_dictionary(&c2p) })
                    }
                    Err(e) => serde_json::json!({ "error": e.message }),
                };
                assert_eq!(&actual, out, "{name}");
            }
            "beacon_coverage" => {
                let mut context = rebuild(snapshot.unwrap());
                apply_mutation(&case, &mut context);
                let o = &context.options;
                let p2c = helpers::get_provider_center_to_covered_centers(
                    &context,
                    o.beacon_width,
                    o.beacon_height,
                    o.beacon_supply_width,
                    o.beacon_supply_height,
                    cells_of(&context.grid, is_beacon),
                    true,
                    false,
                );
                let c2p = helpers::get_covered_center_to_provider_centers(&context, &p2c);
                let actual =
                    serde_json::json!({ "p2c": set_dictionary(&p2c), "c2p": set_dictionary(&c2p) });
                assert_eq!(&actual, out, "{name}");
            }
            "candidates" => {
                let snapshot = snapshot.unwrap();
                if string(&case["info"]) == "pole" {
                    check_candidates(&name, snapshot, &case, &PoleFactory);
                } else {
                    check_candidates(&name, snapshot, &case, &PlainFactory);
                }
                check_covers_more(&name, snapshot, &case);
                check_allow(&name, snapshot, &case, "allow", false);
                check_allow(&name, snapshot, &case, "allow_separate", true);
                check_prevent(&name, snapshot, &case);
                check_overlap(&name, snapshot, &case);
            }
            "mutate_grid" => check_mutate_grid(&name, snapshot.unwrap(), &case),
            "fit" => check_fit(&name, snapshot.unwrap(), &case),
            "validate" => check_validate(&name, snapshot.unwrap(), &case),
            "terminals" => check_terminals(&name, snapshot.unwrap(), &case),
            "path" => {
                let mut came_from = LocationDictionary::new();
                for c in arr(&case["came_from"]).chunks(4) {
                    came_from.add(
                        Location::new(int(&c[0]), int(&c[1])),
                        Location::new(int(&c[2]), int(&c[3])),
                    );
                }
                let path =
                    helpers::get_path(&came_from, point(&case["start"]), point(&case["goal"]));
                assert_eq!(flat(path), ints(&out["path"]), "{name}");
            }
            "line" => {
                let actual = match catch(|| {
                    helpers::make_straight_line(point(&case["a"]), point(&case["b"]))
                }) {
                    Ok(line) => serde_json::json!({ "points": flat(line) }),
                    Err(message) => serde_json::json!({ "error": message }),
                };
                assert_eq!(&actual, out, "{name}");
            }
            "line_on_empty" => {
                let context = rebuild(snapshot.unwrap());
                let actual = match catch(|| {
                    helpers::make_straight_line_on_empty(
                        &context.grid,
                        point(&case["a"]),
                        point(&case["b"]),
                    )
                }) {
                    Ok(Some(line)) => serde_json::json!({ "points": flat(line) }),
                    Ok(None) => serde_json::json!({ "none": true }),
                    Err(message) => serde_json::json!({ "error": message }),
                };
                assert_eq!(&actual, out, "{name}");
            }
            "lines" => {
                let actual = lines_json(helpers::points_to_lines_sorted(
                    &points(&case["points"]),
                    boolean(&case["sort"]),
                ));
                // The C#'s ArgumentOutOfRangeException message names a parameter; only the
                // exception type is compared for it.
                let mut expected = out.clone();
                if expected["error_type"] == "ArgumentOutOfRangeException" {
                    expected.as_object_mut().unwrap().remove("error");
                }
                assert_eq!(actual, expected, "{name}");
            }
            "turns" => {
                let result = helpers::count_turns(&points(&case["points"]));
                assert_eq!(i64::from(result), out["result"].as_i64().unwrap(), "{name}");
            }
            "collinear" => {
                let result = helpers::are_locations_collinear(&points(&case["points"]));
                assert_eq!(result, boolean(&out["result"]), "{name}");
            }
            other => panic!("unknown op {other}"),
        }
    }

    let mut summary: Vec<_> = ops.into_iter().collect();
    summary.sort();
    println!("{} contexts; cases: {summary:?}", contexts.len());
    assert!(!contexts.is_empty());
}
