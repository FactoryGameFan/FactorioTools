//! Replays the operations recorded from real .NET (`fixtures/*.jsonl`, format in
//! `FIXTURES.md`) and checks every result the recording holds.

use std::cmp::Ordering;
use std::path::PathBuf;

use dotnet_compat::{
    NetCompare, NetDictionary, NetHashSet, NetPriorityQueue, net_sort, net_sort_range,
    round_half_even, round_to_i32, truncate_to_i32,
};
use serde_json::Value;

struct Scenario {
    name: String,
    ops: Vec<Value>,
}

fn load(file: &str) -> Vec<Scenario> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join(file);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let scenarios: Vec<Scenario> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let v: Value = serde_json::from_str(line).expect("valid JSON line");
            Scenario {
                name: v["name"].as_str().expect("name").to_owned(),
                ops: v["ops"].as_array().expect("ops").clone(),
            }
        })
        .collect();
    assert!(!scenarios.is_empty(), "{file} has no scenarios");
    scenarios
}

fn op_name(op: &Value) -> &str {
    op[0].as_str().expect("op name")
}

fn int(v: &Value) -> i64 {
    v.as_i64()
        .unwrap_or_else(|| panic!("expected an integer, got {v}"))
}

fn i32_of(v: &Value) -> i32 {
    i32::try_from(int(v)).expect("fits in i32")
}

fn boolean(v: &Value) -> bool {
    v.as_bool()
        .unwrap_or_else(|| panic!("expected a bool, got {v}"))
}

/// A double written with .NET's "R" format. It may come back as a JSON number or string.
fn float(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_f64().expect("number"),
        Value::String(s) => s.parse().expect("float string"),
        _ => panic!("expected a float, got {v}"),
    }
}

fn ints(v: &Value) -> Vec<i32> {
    v.as_array().expect("array").iter().map(i32_of).collect()
}

/// Asserts with the scenario name, op index and op, so a failure points at one line.
macro_rules! check {
    ($scenario:expr, $i:expr, $op:expr, $actual:expr, $expected:expr) => {
        assert_eq!(
            $actual, $expected,
            "scenario {:?}, op {} {}",
            $scenario.name, $i, $op
        )
    };
}

fn passed(file: &str, scenarios: usize, ops: usize) {
    println!("{file}: {scenarios} scenarios, {ops} ops passed");
}

#[test]
fn hashset() {
    let scenarios = load("hashset.jsonl");
    let mut total = 0;
    for s in &scenarios {
        let mut set = NetHashSet::<i32>::new();
        for (i, op) in s.ops.iter().enumerate() {
            total += 1;
            match op_name(op) {
                "new" => {
                    let capacity = int(&op[1]);
                    set = if capacity < 0 {
                        NetHashSet::new()
                    } else {
                        NetHashSet::with_capacity(capacity as usize)
                    };
                }
                "add" => check!(s, i, op, set.add(i32_of(&op[1])), boolean(&op[2])),
                "remove" => check!(s, i, op, set.remove(&i32_of(&op[1])), boolean(&op[2])),
                "contains" => check!(s, i, op, set.contains(&i32_of(&op[1])), boolean(&op[2])),
                "clear" => set.clear(),
                "union" => set.union_with(ints(&op[1])),
                "union_set" => {
                    let other = NetHashSet::from_collection(&ints(&op[1]));
                    set.union_with(other.iter().collect::<Vec<_>>());
                }
                "except" => set.except_with(ints(&op[1])),
                "intersect" => set.intersect_with_collection(&ints(&op[1])),
                "intersect_set" => {
                    let other = NetHashSet::from_collection(&ints(&op[1]));
                    set.intersect_with_set(&other);
                }
                "overlaps" => check!(s, i, op, set.overlaps(ints(&op[1])), boolean(&op[2])),
                "set_equals" => check!(s, i, op, set.set_equals(ints(&op[1])), boolean(&op[2])),
                "copy" => set = NetHashSet::copy_from(&set),
                "copy_list" => set = NetHashSet::from_collection(&set.to_vec()),
                "ensure_capacity" => {
                    let result = set.ensure_capacity(int(&op[1]) as usize);
                    check!(s, i, op, result as i64, int(&op[2]));
                }
                "trim" => set.trim_excess(),
                "order" => check!(s, i, op, set.to_vec(), ints(&op[1])),
                other => panic!("scenario {:?}: unknown op {other}", s.name),
            }
        }
    }
    passed("hashset.jsonl", scenarios.len(), total);
}

#[test]
fn dictionary() {
    let scenarios = load("dictionary.jsonl");
    let mut total = 0;
    for s in &scenarios {
        let mut dict = NetDictionary::<i32, i32>::new();
        for (i, op) in s.ops.iter().enumerate() {
            total += 1;
            match op_name(op) {
                "new" => {
                    let capacity = int(&op[1]);
                    dict = if capacity < 0 {
                        NetDictionary::new()
                    } else {
                        NetDictionary::with_capacity(capacity as usize)
                    };
                }
                "set" => dict.set(i32_of(&op[1]), i32_of(&op[2])),
                "add" => dict.add(i32_of(&op[1]), i32_of(&op[2])),
                "try_add" => {
                    let added = dict.try_add(i32_of(&op[1]), i32_of(&op[2]));
                    check!(s, i, op, added, boolean(&op[3]));
                }
                "get" => {
                    let got = dict.get(&i32_of(&op[1])).copied();
                    check!(s, i, op, got.is_some(), boolean(&op[2]));
                    check!(s, i, op, got.unwrap_or(0), i32_of(&op[3]));
                }
                "remove" => {
                    let removed = dict.remove(&i32_of(&op[1])).is_some();
                    check!(s, i, op, removed, boolean(&op[2]));
                }
                "contains" => {
                    check!(
                        s,
                        i,
                        op,
                        dict.contains_key(&i32_of(&op[1])),
                        boolean(&op[2])
                    )
                }
                "clear" => dict.clear(),
                "copy" => dict = NetDictionary::copy_from(&dict),
                "ensure_capacity" => {
                    let result = dict.ensure_capacity(int(&op[1]) as usize);
                    check!(s, i, op, result as i64, int(&op[2]));
                }
                "trim" => dict.trim_excess(),
                "order" => check!(s, i, op, dict.keys().collect::<Vec<_>>(), ints(&op[1])),
                "pairs" => {
                    let expected: Vec<(i32, i32)> = op[1]
                        .as_array()
                        .expect("pairs")
                        .iter()
                        .map(|p| (i32_of(&p[0]), i32_of(&p[1])))
                        .collect();
                    let actual: Vec<(i32, i32)> = dict.iter().map(|(k, v)| (*k, *v)).collect();
                    check!(s, i, op, actual, expected);
                }
                other => panic!("scenario {:?}: unknown op {other}", s.name),
            }
        }
    }
    passed("dictionary.jsonl", scenarios.len(), total);
}

/// One recorded priority-queue scenario, for either priority type.
fn replay_queue<P: NetCompare + Copy + PartialEq + std::fmt::Debug>(
    s: &Scenario,
    priority: impl Fn(&Value) -> P,
) -> usize {
    let mut queue = NetPriorityQueue::<i32, P>::new();
    for (i, op) in s.ops.iter().enumerate().skip(1) {
        match op_name(op) {
            "enqueue" => queue.enqueue(i32_of(&op[1]), priority(&op[2])),
            "dequeue" => check!(s, i, op, queue.dequeue(), Some(i32_of(&op[1]))),
            "peek" => check!(s, i, op, queue.peek().copied(), Some(i32_of(&op[1]))),
            "count" => check!(s, i, op, queue.len() as i64, int(&op[1])),
            "clear" => queue.clear(),
            "remove" => {
                let removed = queue.remove(&i32_of(&op[1]));
                check!(s, i, op, removed.is_some(), boolean(&op[2]));
                if let Some((e, p)) = removed {
                    check!(s, i, op, e, i32_of(&op[3]));
                    check!(s, i, op, p, priority(&op[4]));
                }
            }
            "drain" => {
                let expected: Vec<(i32, P)> = op[1]
                    .as_array()
                    .expect("drain")
                    .iter()
                    .map(|p| (i32_of(&p[0]), priority(&p[1])))
                    .collect();
                let mut actual = Vec::new();
                while let Some(item) = queue.try_dequeue() {
                    actual.push(item);
                }
                check!(s, i, op, actual, expected);
            }
            other => panic!("scenario {:?}: unknown op {other}", s.name),
        }
    }
    s.ops.len()
}

#[test]
fn priority_queue() {
    let scenarios = load("priority_queue.jsonl");
    let mut total = 0;
    for s in &scenarios {
        let first = &s.ops[0];
        assert_eq!(op_name(first), "new", "scenario {:?}", s.name);
        total += match first[1].as_str() {
            Some("double") => replay_queue(s, float),
            Some("int") => replay_queue(s, i32_of),
            _ => panic!("scenario {:?}: unknown priority type {}", s.name, first[1]),
        };
    }
    passed("priority_queue.jsonl", scenarios.len(), total);
}

#[test]
fn sort() {
    let scenarios = load("sort.jsonl");
    let mut total = 0;
    for s in &scenarios {
        for (i, op) in s.ops.iter().enumerate() {
            total += 1;
            // Keys may be ints or doubles; comparing as double.CompareTo covers both.
            let mut items: Vec<(f64, i32)> = op[1]
                .as_array()
                .expect("items")
                .iter()
                .map(|p| (float(&p[0]), i32_of(&p[1])))
                .collect();
            let by_key = |a: &(f64, i32), b: &(f64, i32)| -> Ordering { a.0.net_cmp(&b.0) };
            let expected = match op_name(op) {
                "list_sort" | "array_sort" | "span_sort" => {
                    net_sort(&mut items, by_key);
                    ints(&op[2])
                }
                "list_sort_range" => {
                    let index = int(&op[2]) as usize;
                    let count = int(&op[3]) as usize;
                    net_sort_range(&mut items, index, count, by_key);
                    ints(&op[4])
                }
                other => panic!("scenario {:?}: unknown op {other}", s.name),
            };
            let actual: Vec<i32> = items.iter().map(|&(_, id)| id).collect();
            check!(s, i, op_name(op), actual, expected);
        }
    }
    passed("sort.jsonl", scenarios.len(), total);
}

#[test]
fn math() {
    let scenarios = load("math.jsonl");
    let mut total = 0;
    for s in &scenarios {
        for (i, op) in s.ops.iter().enumerate() {
            total += 1;
            let x = float(&op[1]);
            match op_name(op) {
                "round" => check!(
                    s,
                    i,
                    op,
                    round_half_even(x).to_bits(),
                    float(&op[2]).to_bits()
                ),
                "round_int" => check!(s, i, op, round_to_i32(x), i32_of(&op[2])),
                "to_int" => check!(s, i, op, truncate_to_i32(x), i32_of(&op[2])),
                "ceiling" => check!(s, i, op, x.ceil().to_bits(), float(&op[2]).to_bits()),
                "floor" => check!(s, i, op, x.floor().to_bits(), float(&op[2]).to_bits()),
                other => panic!("scenario {:?}: unknown op {other}", s.name),
            }
        }
    }
    passed("math.jsonl", scenarios.len(), total);
}
