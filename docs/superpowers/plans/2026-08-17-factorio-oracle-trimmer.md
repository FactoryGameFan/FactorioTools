# factorio-oracle Trimmer and Acceptance Test Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Trim a full `data.raw` dump down to a caller-chosen slice, merge in renames and `defines`, and write it as canonical JSON that reproduces FactorioTools' committed `factorio-oracle.json` byte for byte.

**Architecture:** A `trim` subcommand that takes the JSON a `run` produced, plus a caller-supplied trim spec, and emits one canonical JSON document. Every stage is a pure function over `serde_json::Value` and is unit-tested with no Factorio present. The allowlists live in the caller's spec file, never in this crate, because the ten entity names FactorioTools wants are exactly its planner's list and no other consumer wants them.

**Tech Stack:** Rust (edition 2021), the existing `serde` / `serde_json` / `clap` / `anyhow` dependencies. One feature is added to `serde_json`: `arbitrary_precision`, for the reason in Task 4.

**Spec:** `/Users/ericjohnson/GitHub/FactorioTools/docs/superpowers/specs/2026-08-16-shared-factorio-oracle-design.md`

**Plan 1 (already built):** `/Users/ericjohnson/GitHub/FactorioTools/docs/superpowers/plans/2026-08-16-factorio-oracle-runner-core.md`. Read its "Corrections found while executing this plan" section before starting. Two of its tasks describe behaviour that measurement disproved.

**Repo:** `https://github.com/FactoryGameFan/factorio-oracle`, cloned at `~/GitHub/factorio-oracle`.

## What is already measured

Do not re-derive any of this. It was measured on 2026-08-17 against Factorio 2.1.14, and it is why this plan is shaped the way it is.

**The port is already known to be achievable.** A sixty-line Rust spike read the real 28 MB dump, applied the same allowlists, and produced `entities` and `modules` blocks byte identical to the committed fixture. The float formatting the spec called "the trap that would make `--check` permanently red" is not a problem for this fixture: `serde_json`'s printer emits shortest round-trip, the same as Python's `repr`, and every value in the trimmed slice matched.

**The dump itself is already correct.** `factorio-oracle run` with a `dump-data` probe produced a 28 MB `data-raw-dump.json` in 2.9 seconds, and feeding that file to the existing `tools/trim-factorio-oracle.py` reproduced the committed fixture byte for byte. So the capture half of `capture-factorio-oracle.sh` is already replaced; only the trim half is left.

**`serde_json` parses long decimal literals one ULP wrong.** This is the one real hazard, and it is latent rather than live. Factorio writes floats in full exact expansion, for example `0.394500000000000028421709430404007434844970703125`. Round-tripping the whole 25 MB dump through Python and through Rust produced 9,744 differing lines. Every one of them was in a graphics field the fixture discards, which is why the spike still matched.

The cause is precise: Rust's own `f64::from_str` is correctly rounded and agrees with Python bit for bit, and `serde_json`'s printer is correct, but `serde_json`'s *number parser* is off by one ULP on long literals. Measured on the two literals above, `std` gave bits `0x3fd93f7ced916873` and `0x3fdfc01a36e2eb1d` while `serde_json` gave `...6874` and `...eb1c`. Python agrees with `std`. Task 4 fixes this, and Task 4 exists because a wrong number in a fixture is exactly the silent failure the fixture is built to prevent.

**`captureInfo.loadedMods` cannot come from the active-mods prelude.** The committed fixture lists `core`, and `script.active_mods` does not report `core`. It comes from grepping the game's stdout for `Loading mod <name>`, which is what the shell script does. `dump-data` runs no mod at all, so there is no prelude to read. Task 7 handles this.

**Every wanted name exists in three or four prototype types.** `pumpjack` is in `item`, `recipe` and `mining-drill`; `stone-wall` is also in `technology`. So `find_prototype`'s collision-box disambiguation is load-bearing on all ten, not a defensive nicety.

**`--dump-data` honours the isolated `write-data`.** The dump lands in the work directory's own `script-output`, so there is no shared user data directory to search and no mtime check to write.

## Global Constraints

- **House writing style:** hyphens only. Never em dashes or en dashes, in code comments, docs, or commit messages.
- **CI must pass with no Factorio installed.** Every test in this plan except Task 10's second half runs without the game. Tests that need an install skip themselves, following `tests/real_game.rs`.
- **Determinism is the product.** `--check` is a `diff` against a committed file. Every map that reaches output is a `BTreeMap` or is explicitly sorted, output is `indent=2`, and the file ends with exactly one trailing newline. `serde_json::Map` is already a `BTreeMap` because the `preserve_order` feature is deliberately not enabled. Do not enable it.
- **Raw values only, never derived.** Never compute a covered tile area from `supply_area_distance`, or anything like it. Factorio's rule is not one formula: poles come out as `2*distance`, a beacon as `2*distance` plus its own footprint, and substation fits neither. A guessed formula in a fixture is confidently wrong and drifts invisibly.
- **The allowlists belong to the caller.** No entity name, field name, or prototype type from any consumer appears in this crate's source. They arrive in the trim spec JSON.
- **The toolchain is pinned** at 1.97.1 in `rust-toolchain.toml`.
- **Nothing automerges** and Renovate stays as configured.

## File Structure

```
src/trim/mod.rs          assembly and the public entry point
src/trim/spec.rs         the caller's trim spec, deserialised
src/trim/prototypes.rs   find_prototype and entity trimming
src/trim/canonical.rs    number normalisation and canonical writing
src/trim/renames.rs      migrations to a rename table
src/trim/defines.rs      a defines table out of runtime-api.json
tests/fixtures/          a committed data.raw slice and the expected output
tests/acceptance.rs      the byte-for-byte test, offline and install-gated
```

`src/trim/` is a new module tree. `src/lib.rs` gains `pub mod trim;`, and `src/run.rs` gains one field. Nothing else in plan 1's code changes.

---

### Task 1: The trim spec

The caller's document. It carries every name this crate must not know.

**Files:**
- Create: `src/trim/mod.rs`
- Create: `src/trim/spec.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub struct TrimSpec { pub comment: Option<String>, pub entities: Vec<String>, pub entity_fields: Vec<String>, pub connection_fields: Vec<String>, pub fluid_boxes: Vec<String>, pub name_lists: BTreeMap<String, String>, pub defines: BTreeMap<String, String>, pub include_renames: bool }`.

- [ ] **Step 1: Write the failing test**

Create `src/trim/spec.rs`:

```rust
//! The document a consumer hands in to say which slice of the game it wants.

use serde::Deserialize;
use std::collections::BTreeMap;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialises_the_factoriotools_shape() {
        let json = r#"{
            "comment": "Generated by a tool. Do not hand-edit.",
            "entities": ["pumpjack", "beacon"],
            "entity_fields": ["collision_box", "module_slots"],
            "connection_fields": ["position", "flow_direction"],
            "fluid_boxes": ["fluid_box", "output_fluid_box"],
            "name_lists": { "modules": "module" },
            "defines": { "directions": "direction" },
            "include_renames": true
        }"#;
        let spec: TrimSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.comment.as_deref(), Some("Generated by a tool. Do not hand-edit."));
        assert_eq!(spec.entities, vec!["pumpjack", "beacon"]);
        assert_eq!(spec.fluid_boxes.len(), 2);
        assert_eq!(spec.name_lists.get("modules").unwrap(), "module");
        assert_eq!(spec.defines.get("directions").unwrap(), "direction");
        assert!(spec.include_renames);
    }

    #[test]
    fn everything_except_entities_has_a_default() {
        // A caller that wants only entity geometry should not have to write six
        // empty lists to say so.
        let spec: TrimSpec = serde_json::from_str(r#"{ "entities": ["pipe"] }"#).unwrap();
        assert!(spec.comment.is_none());
        assert!(spec.entity_fields.is_empty());
        assert!(spec.name_lists.is_empty());
        assert!(!spec.include_renames);
    }

    #[test]
    fn an_unknown_key_is_rejected_rather_than_ignored() {
        // A typo in an allowlist name would otherwise silently produce a fixture
        // missing the field the caller asked for, which is the failure this
        // whole tool exists to prevent.
        let json = r#"{ "entities": ["pipe"], "entity_feilds": ["collision_box"] }"#;
        assert!(serde_json::from_str::<TrimSpec>(json).is_err());
    }

    #[test]
    fn the_output_key_is_the_callers_choice_not_the_games() {
        // The game calls it `direction`; FactorioTools' fixture calls it
        // `directions`. Neither name belongs to this crate.
        let spec: TrimSpec =
            serde_json::from_str(r#"{ "entities": [], "defines": { "whatever": "direction" } }"#)
                .unwrap();
        assert_eq!(spec.defines.get("whatever").unwrap(), "direction");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --lib trim::spec`
Expected: FAIL, `cannot find type TrimSpec in this scope`.

- [ ] **Step 3: Write the type**

Add above the test module in `src/trim/spec.rs`:

```rust
/// Which slice of the game a consumer wants, and what to call it.
///
/// Every name in here belongs to the caller. FactorioTools wants ten entity
/// names that are exactly its planner's list; a blueprint editor wants
/// hundreds; a map tool wants none of them. Baking any of them into this crate
/// would make it one consumer's tool with extra steps.
///
/// Unknown keys are rejected. A misspelled allowlist would otherwise produce a
/// fixture quietly missing whatever the caller asked for.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrimSpec {
    /// Written to the output as `_comment`. Usually "do not hand-edit".
    #[serde(default)]
    pub comment: Option<String>,
    /// Prototype names to look up, by name rather than by type.
    pub entities: Vec<String>,
    /// Prototype fields worth pinning. A field absent on a given prototype is
    /// skipped, so one flat list covers every type.
    #[serde(default)]
    pub entity_fields: Vec<String>,
    /// Fluid box connection keys worth keeping. Everything else on a connection
    /// is graphics: `pipe_covers` alone is several hundred lines of sprite
    /// definitions per entity.
    #[serde(default)]
    pub connection_fields: Vec<String>,
    /// Which fluid boxes to look inside, for example `output_fluid_box`.
    #[serde(default)]
    pub fluid_boxes: Vec<String>,
    /// Output key to prototype type. Emits the sorted names of every prototype
    /// of that type, which is how FactorioTools pins the module list.
    #[serde(default)]
    pub name_lists: BTreeMap<String, String>,
    /// Output key to `defines` table name. The game calls it `direction`;
    /// FactorioTools' fixture calls the result `directions`.
    #[serde(default)]
    pub defines: BTreeMap<String, String>,
    /// Whether to read the game's migration files into a rename table.
    #[serde(default)]
    pub include_renames: bool,
}
```

Create `src/trim/mod.rs`:

```rust
//! Turning a full `data.raw` dump into the small slice a consumer asked for.
//!
//! Every stage here is a pure function over `serde_json::Value`, so the whole
//! module is testable with no Factorio present. The allowlists arrive from the
//! caller: see [`spec::TrimSpec`].

pub mod spec;
```

Add to `src/lib.rs`, keeping the existing modules in place:

```rust
pub mod trim;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib trim::spec`
Expected: PASS, 4 tests.

- [ ] **Step 5: Check formatting and lints**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Take the allowlists from the caller, not from this crate

The ten entity names FactorioTools wants are exactly its planner's list.
A blueprint editor wants hundreds and a map tool wants none, so a shared
tool that hardcoded any of them would be one consumer's script with extra
steps.

Unknown keys are rejected rather than ignored, because a misspelled
allowlist would otherwise write a fixture quietly missing the field the
caller asked for."
```

---

### Task 2: Find a prototype by name, across every type

**Files:**
- Create: `src/trim/prototypes.rs`
- Modify: `src/trim/mod.rs`

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `pub fn find_prototype<'a>(raw: &'a Map<String, Value>, name: &str) -> Option<(String, &'a Value)>`, returning the prototype type and the prototype.

- [ ] **Step 1: Write the failing test**

Create `src/trim/prototypes.rs`:

```rust
//! Locating prototypes in a `data.raw` dump, and cutting them down to size.

use serde_json::{Map, Value};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn dump() -> Map<String, Value> {
        // The shape that matters: the same name in several types, only one of
        // which is the placeable entity. Measured on 2.1.14, every one of
        // FactorioTools' ten names appears in three or four types.
        json!({
            "item": {
                "pumpjack": { "stack_size": 20 },
                "stone-wall": { "stack_size": 100 }
            },
            "recipe": {
                "pumpjack": { "ingredients": [] },
                "stone-wall": { "ingredients": [] }
            },
            "mining-drill": {
                "pumpjack": { "collision_box": [[-1.2, -1.2], [1.2, 1.2]] }
            },
            "wall": {
                "stone-wall": { "collision_box": [[-0.29, -0.29], [0.29, 0.29]] }
            },
            "technology": {
                "stone-wall": { "unit": {} }
            },
            "not-an-object": 42
        })
        .as_object()
        .unwrap()
        .clone()
    }

    #[test]
    fn prefers_the_candidate_that_has_a_collision_box() {
        // data.raw["item"]["pumpjack"] is a real prototype. It is just the wrong
        // one, and it has none of the geometry. Preferring the candidate with a
        // collision_box picks the entity without a name to type table that
        // silently rots when Factorio reclassifies something.
        let (kind, proto) = find_prototype(&dump(), "pumpjack").unwrap();
        assert_eq!(kind, "mining-drill");
        assert!(proto.get("collision_box").is_some());
    }

    #[test]
    fn picks_the_entity_even_when_four_types_share_the_name() {
        let (kind, _) = find_prototype(&dump(), "stone-wall").unwrap();
        assert_eq!(kind, "wall");
    }

    #[test]
    fn returns_none_for_a_name_no_type_has() {
        assert!(find_prototype(&dump(), "quantum-pumpjack").is_none());
    }

    #[test]
    fn a_type_that_is_not_an_object_is_skipped_rather_than_panicking() {
        // data.raw is not uniformly a map of maps.
        assert!(find_prototype(&dump(), "not-an-object").is_none());
    }

    #[test]
    fn the_fallback_is_alphabetical_so_it_is_deterministic() {
        // When nothing has a collision_box there is no right answer, only a
        // stable one. The Python script took whichever type came first in the
        // document; sorted order is the same idea without depending on how the
        // game happened to serialise the file.
        let raw = json!({
            "zebra": { "ghost": { "a": 1 } },
            "alpha": { "ghost": { "b": 2 } }
        })
        .as_object()
        .unwrap()
        .clone();
        let (kind, _) = find_prototype(&raw, "ghost").unwrap();
        assert_eq!(kind, "alpha");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --lib trim::prototypes`
Expected: FAIL, `cannot find function find_prototype`.

- [ ] **Step 3: Write the implementation**

Add above the test module in `src/trim/prototypes.rs`:

```rust
/// Finds a prototype by name, searching every prototype type.
///
/// `data.raw` is keyed by prototype TYPE, not by name, and the names a consumer
/// cares about are scattered across types nobody would guess: a pumpjack is a
/// `mining-drill`, a stone wall is a `wall`. Searching every type is cheaper
/// than maintaining a name to type table that silently rots when Factorio
/// reclassifies something.
///
/// The catch is that most names exist more than once. Measured on 2.1.14, all
/// ten of FactorioTools' names appear in three types and `stone-wall` appears
/// in four. `data.raw["item"]["pumpjack"]` is a real prototype; it is simply
/// the item you carry, and it has none of the geometry. Preferring the
/// candidate that has a `collision_box` picks the placeable entity with no
/// hardcoded table.
///
/// When nothing has one there is no right answer, only a stable one, so the
/// first in sorted order wins. `serde_json::Map` is a `BTreeMap` here, so
/// iteration is already sorted.
pub fn find_prototype<'a>(raw: &'a Map<String, Value>, name: &str) -> Option<(String, &'a Value)> {
    let candidates: Vec<(String, &Value)> = raw
        .iter()
        .filter_map(|(kind, protos)| {
            protos
                .as_object()
                .and_then(|o| o.get(name))
                .map(|p| (kind.clone(), p))
        })
        .collect();

    candidates
        .iter()
        .find(|(_, p)| p.get("collision_box").is_some())
        .cloned()
        .or_else(|| candidates.into_iter().next())
}
```

Add to `src/trim/mod.rs`:

```rust
pub mod prototypes;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib trim::prototypes`
Expected: PASS, 5 tests.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Find a prototype by name across every type

data.raw is keyed by type, and the names consumers care about are
scattered across types nobody would guess: a pumpjack is a mining-drill.
Searching every type beats a name to type table that rots silently when
Factorio reclassifies something.

Measured on 2.1.14: all ten of FactorioTools' names exist in three types
and stone-wall exists in four, so preferring the candidate that has a
collision_box is load-bearing on every one of them rather than defensive."
```

---

### Task 3: Trim a prototype to the fields asked for

**Files:**
- Modify: `src/trim/prototypes.rs`

**Interfaces:**
- Consumes: `find_prototype` from Task 2, `TrimSpec` from Task 1.
- Produces: `pub fn trim_entity(kind: &str, proto: &Value, spec: &TrimSpec) -> Value`.

- [ ] **Step 1: Write the failing test**

Add to the test module in `src/trim/prototypes.rs`:

```rust
    fn spec_for_tests() -> crate::trim::spec::TrimSpec {
        serde_json::from_value(json!({
            "entities": ["pumpjack"],
            "entity_fields": ["collision_box", "module_slots", "energy_usage"],
            "connection_fields": ["position", "positions", "flow_direction",
                                  "max_underground_distance"],
            "fluid_boxes": ["fluid_box", "output_fluid_box", "input_fluid_box"]
        }))
        .unwrap()
    }

    #[test]
    fn keeps_the_asked_for_fields_and_records_the_type() {
        let proto = json!({
            "collision_box": [[-1.2, -1.2], [1.2, 1.2]],
            "module_slots": 2,
            "unwanted_graphics": { "layers": [1, 2, 3] }
        });
        let trimmed = trim_entity("mining-drill", &proto, &spec_for_tests());
        assert_eq!(trimmed["prototypeType"], "mining-drill");
        assert_eq!(trimmed["module_slots"], 2);
        assert!(trimmed.get("unwanted_graphics").is_none());
    }

    #[test]
    fn a_field_the_prototype_does_not_have_is_skipped_not_nulled() {
        // One flat allowlist covers every type, so most fields are absent on
        // most prototypes. A null would be a claim the game never made.
        let proto = json!({ "collision_box": [[0, 0], [1, 1]] });
        let trimmed = trim_entity("pipe", &proto, &spec_for_tests());
        assert!(trimmed.get("module_slots").is_none());
        assert!(!trimmed.as_object().unwrap().contains_key("energy_usage"));
    }

    #[test]
    fn keeps_only_the_asked_for_keys_inside_a_pipe_connection() {
        // pipe_covers alone is several hundred lines of sprite definitions per
        // entity, and none of it is a fact about geometry.
        let proto = json!({
            "output_fluid_box": {
                "pipe_connections": [
                    {
                        "positions": [[1, -1], [1, 1], [-1, 1], [-1, -1]],
                        "flow_direction": "output",
                        "pipe_covers": { "sheets": "lots of sprites" }
                    }
                ],
                "volume": 1000
            }
        });
        let trimmed = trim_entity("mining-drill", &proto, &spec_for_tests());
        let conn = &trimmed["output_fluid_box"]["pipe_connections"][0];
        assert_eq!(conn["flow_direction"], "output");
        assert_eq!(conn["positions"].as_array().unwrap().len(), 4);
        assert!(conn.get("pipe_covers").is_none());
        // Only pipe_connections survives from the box itself.
        assert!(trimmed["output_fluid_box"].get("volume").is_none());
    }

    #[test]
    fn a_fluid_box_with_no_connections_is_left_out_entirely() {
        // An empty pipe_connections list says nothing, and emitting it would
        // churn the diff whenever a box gains or loses one.
        let proto = json!({ "fluid_box": { "volume": 100 } });
        let trimmed = trim_entity("pipe", &proto, &spec_for_tests());
        assert!(trimmed.get("fluid_box").is_none());
    }

    #[test]
    fn the_four_position_output_box_two_point_one_introduced_survives() {
        // Factorio 2.1 changed the pumpjack's output fluid box from 2 distinct
        // corners to 4, one per rotation. That is the exact kind of change this
        // fixture exists to make visible, so it must come through intact.
        let proto = json!({
            "output_fluid_box": {
                "pipe_connections": [{
                    "direction": 0,
                    "positions": [[1, -1], [1, 1], [-1, 1], [-1, -1]],
                    "flow_direction": "output"
                }]
            }
        });
        let mut spec = spec_for_tests();
        spec.connection_fields.push("direction".to_string());
        let trimmed = trim_entity("mining-drill", &proto, &spec);
        let conn = &trimmed["output_fluid_box"]["pipe_connections"][0];
        assert_eq!(conn["positions"], json!([[1, -1], [1, 1], [-1, 1], [-1, -1]]));
        assert_eq!(conn["direction"], 0);
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --lib trim::prototypes`
Expected: FAIL, `cannot find function trim_entity`.

- [ ] **Step 3: Write the implementation**

Add to `src/trim/prototypes.rs`, and add `use crate::trim::spec::TrimSpec;` to the imports at the top:

```rust
/// Keeps only the connection keys the caller asked for.
fn trim_connections(fluid_box: &Value, spec: &TrimSpec) -> Vec<Value> {
    let Some(connections) = fluid_box.get("pipe_connections").and_then(|v| v.as_array()) else {
        return vec![];
    };
    connections
        .iter()
        .map(|connection| {
            let mut kept = Map::new();
            for key in &spec.connection_fields {
                if let Some(value) = connection.get(key) {
                    kept.insert(key.clone(), value.clone());
                }
            }
            Value::Object(kept)
        })
        .collect()
}

/// Cuts one prototype down to the fields the caller asked for.
///
/// A field the prototype does not have is skipped rather than written as null.
/// One flat allowlist covers every prototype type, so most fields are absent
/// from most prototypes, and a null would be a claim the game never made.
///
/// `prototypeType` is recorded because it is the thing a name lookup had to
/// discover, and because a reclassification is worth seeing in the diff.
pub fn trim_entity(kind: &str, proto: &Value, spec: &TrimSpec) -> Value {
    let mut trimmed = Map::new();
    trimmed.insert("prototypeType".to_string(), Value::String(kind.to_string()));

    for field in &spec.entity_fields {
        if let Some(value) = proto.get(field) {
            trimmed.insert(field.clone(), value.clone());
        }
    }

    for box_name in &spec.fluid_boxes {
        let Some(fluid_box) = proto.get(box_name) else {
            continue;
        };
        let connections = trim_connections(fluid_box, spec);
        // An empty list says nothing and would churn the diff whenever a box
        // gains or loses a connection, so the box is left out entirely.
        if connections.is_empty() {
            continue;
        }
        let mut kept = Map::new();
        kept.insert(
            "pipe_connections".to_string(),
            Value::Array(connections),
        );
        trimmed.insert(box_name.clone(), Value::Object(kept));
    }

    Value::Object(trimmed)
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib trim::prototypes`
Expected: PASS, 10 tests.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Trim a prototype to the fields the caller asked for

An absent field is skipped rather than written as null, because one flat
allowlist covers every prototype type and a null would be a claim the game
never made.

Inside a fluid box only pipe_connections survives, and inside a connection
only the asked-for keys. pipe_covers alone is several hundred lines of
sprite definitions per entity and none of it is a fact about geometry."
```

---

### Task 4: Parse numbers the way Python does, not the way serde_json does

This is the task that makes byte-for-byte possible in the long run. It fixes a latent one-ULP defect rather than a visible one, so the test carries the measurement that proves it is real.

**Files:**
- Create: `src/trim/canonical.rs`
- Modify: `src/trim/mod.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn normalise_numbers(value: &Value) -> Value` and `pub fn to_canonical_json(value: &Value) -> String`.

- [ ] **Step 1: Add the `arbitrary_precision` feature**

In `Cargo.toml`, replace the `serde_json` line with:

```toml
# arbitrary_precision keeps every number as the literal text the game wrote,
# so `trim` can parse it with std's correctly-rounded f64::from_str instead of
# serde_json's own parser. Measured 2026-08-17 on 2.1.14: serde_json is one ULP
# out on Factorio's long decimal expansions, in both directions. See
# src/trim/canonical.rs. preserve_order stays OFF: Map must remain a BTreeMap so
# output is sorted, which is what makes --check a usable diff.
serde_json = { version = "1", features = ["arbitrary_precision"] }
```

- [ ] **Step 2: Write the failing test**

Create `src/trim/canonical.rs`:

```rust
//! Canonical output, and the number handling that makes it reproducible.

use serde_json::{Map, Value};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The two literals the defect was measured on. Factorio writes floats in
    /// full exact expansion, and these are real values out of a 2.1.14 dump.
    const LONG_A: &str = "0.394500000000000028421709430404007434844970703125";
    const LONG_B: &str = "0.49610000000000002984279490192420780658721923828125";

    #[test]
    fn a_long_literal_parses_the_way_python_does() {
        // Measured 2026-08-17 on 2.1.14. std::f64::from_str is correctly
        // rounded and agrees with CPython bit for bit; serde_json's own number
        // parser is one ULP out, in both directions:
        //
        //   LONG_A: std 0x3fd93f7ced916873, serde_json 0x3fd93f7ced916874
        //   LONG_B: std 0x3fdfc01a36e2eb1d, serde_json 0x3fdfc01a36e2eb1c
        //
        // Python prints 0.3945 and 0.49610000000000004 respectively, so those
        // are the bytes a faithful port has to produce.
        let value: Value = serde_json::from_str(&format!("[{LONG_A}, {LONG_B}]")).unwrap();
        let text = to_canonical_json(&normalise_numbers(&value));
        assert_eq!(text, "[\n  0.3945,\n  0.49610000000000004\n]\n");
    }

    #[test]
    fn an_integer_stays_an_integer() {
        // JSON does not distinguish them but both Python and this tool do, and
        // turning 0 into 0.0 would rewrite every integer in the fixture.
        let value: Value = serde_json::from_str(r#"{"a": 0, "b": -7, "c": 32}"#).unwrap();
        let text = to_canonical_json(&normalise_numbers(&value));
        assert_eq!(text, "{\n  \"a\": 0,\n  \"b\": -7,\n  \"c\": 32\n}\n");
    }

    #[test]
    fn exponent_notation_becomes_a_float_as_python_reads_it() {
        // Python parses 1e2 as a float and prints 100.0. The rule is textual:
        // a literal containing '.', 'e' or 'E' is a float.
        let value: Value = serde_json::from_str(r#"[1e2, 1E2, 1.5]"#).unwrap();
        let text = to_canonical_json(&normalise_numbers(&value));
        assert_eq!(text, "[\n  100.0,\n  100.0,\n  1.5\n]\n");
    }

    #[test]
    fn short_literals_are_untouched() {
        // The values actually in FactorioTools' fixture. These already survived
        // both parsers identically; the test pins that they still do.
        let value: Value =
            serde_json::from_str("[-1.2, 0.29, 2.5, 1.5, 0.2, 2.1, 3.5, 7.5, -0.15]").unwrap();
        let text = to_canonical_json(&normalise_numbers(&value));
        assert_eq!(
            text,
            "[\n  -1.2,\n  0.29,\n  2.5,\n  1.5,\n  0.2,\n  2.1,\n  3.5,\n  7.5,\n  -0.15\n]\n"
        );
    }

    #[test]
    fn normalisation_reaches_all_the_way_down() {
        let value: Value =
            serde_json::from_str(&format!(r#"{{"box": [[{LONG_A}]], "n": 3}}"#)).unwrap();
        let out = normalise_numbers(&value);
        let text = to_canonical_json(&out);
        assert!(text.contains("0.3945"), "got {text}");
        assert!(!text.contains("0.39450000000000002"), "got {text}");
        assert!(text.contains("\"n\": 3"));
    }

    #[test]
    fn keys_come_out_sorted_and_the_file_ends_with_one_newline() {
        let value = json!({ "zebra": 1, "alpha": 2 });
        let text = to_canonical_json(&value);
        assert_eq!(text, "{\n  \"alpha\": 2,\n  \"zebra\": 1\n}\n");
        assert!(text.ends_with('\n'));
        assert!(!text.ends_with("\n\n"));
    }
}
```

- [ ] **Step 3: Run it to see it fail**

Run: `cargo test --lib trim::canonical`
Expected: FAIL, `cannot find function normalise_numbers`.

- [ ] **Step 4: Write the implementation**

Add above the test module in `src/trim/canonical.rs`:

```rust
/// Re-parses every number through `std`, which is correctly rounded.
///
/// The crate enables `serde_json/arbitrary_precision`, so a parsed number keeps
/// the literal text the game wrote rather than a `f64` somebody else rounded.
/// That matters because Factorio writes floats in full exact expansion, for
/// example `0.394500000000000028421709430404007434844970703125`, and measured
/// 2026-08-17 on 2.1.14 `serde_json`'s own parser is one ULP out on those, in
/// both directions. Rust's `f64::from_str` agrees with CPython bit for bit, and
/// `serde_json`'s printer already emits shortest round-trip, so parsing through
/// `std` and printing through `serde_json` reproduces Python's bytes.
///
/// Round-tripping the whole 25 MB dump found 9,744 lines where the two
/// disagreed. None were in a field FactorioTools keeps, so this is a latent
/// defect rather than a live one - which is exactly when it is cheap to fix. A
/// wrong number in a fixture is the silent pass the fixture exists to prevent.
///
/// Integers keep their literal. JSON does not distinguish them from floats but
/// Python does, and turning `0` into `0.0` would rewrite every integer in the
/// fixture. The test is textual, matching how Python decides: a literal holding
/// `.`, `e` or `E` is a float.
pub fn normalise_numbers(value: &Value) -> Value {
    match value {
        Value::Number(number) => {
            let literal = number.as_str();
            let is_float = literal.contains('.') || literal.contains('e') || literal.contains('E');
            if !is_float {
                return value.clone();
            }
            match literal.parse::<f64>() {
                Ok(parsed) => serde_json::Number::from_f64(parsed)
                    .map(Value::Number)
                    // Not finite, so there is no f64 to write. Keeping the
                    // literal is better than inventing null.
                    .unwrap_or_else(|| value.clone()),
                Err(_) => value.clone(),
            }
        }
        Value::Array(items) => Value::Array(items.iter().map(normalise_numbers).collect()),
        Value::Object(map) => {
            let mut out = Map::new();
            for (key, item) in map {
                out.insert(key.clone(), normalise_numbers(item));
            }
            Value::Object(out)
        }
        other => other.clone(),
    }
}

/// Two-space indent, sorted keys, exactly one trailing newline.
///
/// Sorting is free: `preserve_order` is deliberately off, so `serde_json::Map`
/// is a `BTreeMap`. Do not turn that feature on. `--check` is a diff against a
/// committed file, and an output that reshuffled on every run would make it
/// permanently red.
pub fn to_canonical_json(value: &Value) -> String {
    serde_json::to_string_pretty(value).expect("a Value always serialises") + "\n"
}
```

Add to `src/trim/mod.rs`:

```rust
pub mod canonical;
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --lib trim::canonical`
Expected: PASS, 6 tests.

- [ ] **Step 6: Run everything, since the feature change is crate-wide**

Run: `cargo test --all-targets`
Expected: PASS. `arbitrary_precision` changes how every number deserialises, so plan 1's tests are the check that nothing else broke. If a `run` test fails on a number comparison, fix it there rather than dropping the feature.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Parse numbers through std, which rounds correctly

Factorio writes floats in full exact expansion, like
0.394500000000000028421709430404007434844970703125. Measured 2026-08-17 on
2.1.14: serde_json's number parser lands one ULP away on literals like that,
in both directions, while Rust's own f64::from_str agrees with CPython bit
for bit. serde_json's printer is fine.

So the crate now enables arbitrary_precision, keeps the literal text, and
re-parses through std. Round-tripping the whole 25 MB dump showed 9,744
lines where the two parsers disagreed. Not one was in a field FactorioTools
keeps, which is why this is worth doing now: it is latent, cheap, and a
wrong number in a fixture is the exact silent pass the fixture exists to
prevent.

Integers keep their literal, decided textually the way Python decides it,
because turning 0 into 0.0 would rewrite every integer in the fixture."
```

---

### Task 5: Read renames out of the game's migration files

**Files:**
- Create: `src/trim/renames.rs`
- Modify: `src/trim/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn collect_renames(data_dir: &Path) -> Value`.

- [ ] **Step 1: Write the failing test**

Create `src/trim/renames.rs`:

```rust
//! Every rename the game knows about, taken from its own migration files.

use serde_json::{Map, Value};
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write(dir: &Path, mod_name: &str, file: &str, body: &str) {
        let migrations = dir.join(mod_name).join("migrations");
        fs::create_dir_all(&migrations).unwrap();
        fs::write(migrations.join(file), body).unwrap();
    }

    #[test]
    fn reads_pairs_out_of_every_mods_migrations() {
        let dir = tempdir().unwrap();
        write(dir.path(), "base", "2.0.0.json",
              r#"{"item": [["effectivity-module", "efficiency-module"]]}"#);
        write(dir.path(), "space-age", "2.0.0.json",
              r#"{"entity": [["bio-chemical-plant", "biochamber"]]}"#);

        let renames = collect_renames(dir.path());
        assert_eq!(renames["item"]["effectivity-module"], "efficiency-module");
        assert_eq!(renames["entity"]["bio-chemical-plant"], "biochamber");
    }

    #[test]
    fn a_later_migration_wins() {
        // Two migrations can rename the same name in sequence. Reading them in
        // sorted path order and letting the last write win matches the order
        // the game applies them in.
        let dir = tempdir().unwrap();
        write(dir.path(), "base", "1.1.0.json", r#"{"item": [["a", "b"]]}"#);
        write(dir.path(), "base", "2.0.0.json", r#"{"item": [["a", "c"]]}"#);
        assert_eq!(collect_renames(dir.path())["item"]["a"], "c");
    }

    #[test]
    fn lua_migrations_are_skipped_because_they_are_code() {
        let dir = tempdir().unwrap();
        write(dir.path(), "base", "2.0.0.json", r#"{"item": [["a", "b"]]}"#);
        write(dir.path(), "base", "2.0.0.lua", "error('not data')");
        let renames = collect_renames(dir.path());
        assert_eq!(renames["item"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn unreadable_and_odd_shaped_files_are_skipped_rather_than_fatal() {
        // A migration this tool cannot read is not a reason to refuse to
        // produce a fixture, and the game ships shapes beyond name pairs.
        let dir = tempdir().unwrap();
        write(dir.path(), "base", "0-broken.json", "{not json");
        write(dir.path(), "base", "1-list.json", "[1, 2, 3]");
        write(dir.path(), "base", "2-odd.json",
              r#"{"item": [["only-one"], ["a", "b", "c"], [1, 2], ["a", "b"]]}"#);
        let renames = collect_renames(dir.path());
        assert_eq!(renames["item"].as_object().unwrap().len(), 1);
        assert_eq!(renames["item"]["a"], "b");
    }

    #[test]
    fn a_missing_data_directory_gives_an_empty_table() {
        assert_eq!(collect_renames(Path::new("/no/such/place")), serde_json::json!({}));
    }

    #[test]
    fn categories_and_names_come_out_sorted() {
        let dir = tempdir().unwrap();
        write(dir.path(), "base", "1.json",
              r#"{"tile": [["z", "1"], ["a", "2"]], "item": [["m", "3"]]}"#);
        let renames = collect_renames(dir.path());
        let text = crate::trim::canonical::to_canonical_json(&renames);
        assert!(text.find("\"item\"").unwrap() < text.find("\"tile\"").unwrap());
        assert!(text.find("\"a\"").unwrap() < text.find("\"z\"").unwrap());
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --lib trim::renames`
Expected: FAIL, `cannot find function collect_renames`.

- [ ] **Step 3: Write the implementation**

Add above the test module in `src/trim/renames.rs`:

```rust
/// Every rename the game knows about, read from `<data>/*/migrations/*.json`.
///
/// This is the difference between "I think `effectivity-module` was renamed"
/// and knowing it, along with every other rename shipped in the same window.
/// Factorio 2.0 did exactly that rename and nothing noticed for a long time.
///
/// `.lua` migrations are skipped: they are arbitrary code, not data.
///
/// Files are read in sorted order, by mod directory and then by file name, and
/// a later file overwrites an earlier one for the same name. That matches the
/// order the game applies migrations in, so a name renamed twice ends up at its
/// final value rather than its intermediate one.
///
/// A file that will not parse is skipped rather than fatal. A migration this
/// tool cannot read is not a reason to refuse to produce a fixture, and the
/// game ships shapes beyond name pairs.
pub fn collect_renames(data_dir: &Path) -> Value {
    let mut paths: Vec<(String, String, std::path::PathBuf)> = Vec::new();

    let Ok(mods) = std::fs::read_dir(data_dir) else {
        return Value::Object(Map::new());
    };
    for mod_entry in mods.flatten() {
        let mod_name = mod_entry.file_name().to_string_lossy().into_owned();
        let Ok(files) = std::fs::read_dir(mod_entry.path().join("migrations")) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".json") {
                continue;
            }
            paths.push((mod_name.clone(), name, file.path()));
        }
    }
    paths.sort();

    let mut renames: Map<String, Value> = Map::new();
    for (_, _, path) in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(Value::Object(content)) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        for (category, pairs) in content {
            let Some(pairs) = pairs.as_array() else {
                continue;
            };
            for pair in pairs {
                let Some(pair) = pair.as_array() else {
                    continue;
                };
                if pair.len() != 2 {
                    continue;
                }
                let (Some(from), Some(to)) = (pair[0].as_str(), pair[1].as_str()) else {
                    continue;
                };
                let table = renames
                    .entry(category.clone())
                    .or_insert_with(|| Value::Object(Map::new()));
                if let Some(table) = table.as_object_mut() {
                    table.insert(from.to_string(), Value::String(to.to_string()));
                }
            }
        }
    }

    // Sorting is free: serde_json::Map is a BTreeMap here.
    Value::Object(renames)
}
```

Add to `src/trim/mod.rs`:

```rust
pub mod renames;
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib trim::renames`
Expected: PASS, 6 tests.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Read renames out of the game's own migration files

This is the difference between thinking effectivity-module was renamed and
knowing it, together with every other rename shipped in the same window.
Factorio 2.0 did that rename and nothing here noticed for a long time.

Files are read in sorted order and a later one wins, matching the order the
game applies migrations, so a name renamed twice lands on its final value.
Lua migrations are skipped because they are code rather than data, and a
file that will not parse is skipped rather than fatal."
```

---

### Task 6: Pull a defines table out of runtime-api.json

This task deliberately reproduces a known defect. Read the whole task before starting.

**Files:**
- Create: `src/trim/defines.rs`
- Modify: `src/trim/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `pub fn collect_define(doc_dir: &Path, table: &str) -> anyhow::Result<Value>`.

- [ ] **Step 1: Understand what is being ported**

`tools/trim-factorio-oracle.py:150` reads:

```python
return {v["name"]: v["order"] for v in define.get("values", [])}
```

`order` is a documentation index, not the runtime value. FactorioTools#83 has the evidence: across all 1,554 define entries in the installed 2.1.14 `runtime-api.json` the only keys are `name`, `order` and `description`, there is no value field at all, and `order` is a dense `0..n-1` index across all 137 define tables, so it cannot express a gap, a duplicate, or a non-zero start.

It is right today only because Factorio declares directions clockwise from `north = 0` with no gaps.

**Port the defect first anyway.** The acceptance test in Task 10 compares byte for byte against a fixture produced by that code. Fixing the bug in the same change would make a port error and a deliberate fix indistinguishable in the diff. Task 11 fixes it as its own reviewable change.

- [ ] **Step 2: Write the failing test**

Create `src/trim/defines.rs`:

```rust
//! Reading a `defines` table out of the shipped API documentation.

use serde_json::{Map, Value};
use std::path::Path;

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn doc_dir_with(body: &str) -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("runtime-api.json"), body).unwrap();
        dir
    }

    #[test]
    fn reads_the_named_table() {
        let dir = doc_dir_with(
            r#"{"defines": [
                 {"name": "direction", "values": [
                    {"name": "north", "order": 0},
                    {"name": "east", "order": 4}]},
                 {"name": "inventory", "values": [{"name": "fuel", "order": 0}]}
               ]}"#,
        );
        let table = collect_define(dir.path(), "direction").unwrap();
        assert_eq!(table["north"], 0);
        assert_eq!(table["east"], 4);
        assert!(table.get("fuel").is_none());
    }

    #[test]
    fn uses_order_which_is_a_documentation_index_not_the_value() {
        // Deliberate, and wrong. See FactorioTools#83. Ported unchanged so the
        // acceptance test can prove the port before the fix changes anything.
        // Across all 1,554 entries in 2.1.14's runtime-api.json there is no
        // value field at all, and `order` is a dense 0..n-1 index, so it cannot
        // express a gap, a duplicate, or a non-zero start.
        let dir = doc_dir_with(
            r#"{"defines": [{"name": "gappy", "values": [
                 {"name": "first", "order": 0},
                 {"name": "second", "order": 1}]}]}"#,
        );
        let table = collect_define(dir.path(), "gappy").unwrap();
        assert_eq!(table["second"], 1);
    }

    #[test]
    fn a_missing_table_is_an_error_rather_than_an_empty_object() {
        let dir = doc_dir_with(r#"{"defines": []}"#);
        let err = collect_define(dir.path(), "direction").unwrap_err().to_string();
        assert!(err.contains("direction"), "got {err}");
    }

    #[test]
    fn a_missing_file_names_the_path_it_wanted() {
        let dir = tempdir().unwrap();
        let err = collect_define(dir.path(), "direction").unwrap_err().to_string();
        assert!(err.contains("runtime-api.json"), "got {err}");
    }
}
```

- [ ] **Step 3: Run it to see it fail**

Run: `cargo test --lib trim::defines`
Expected: FAIL, `cannot find function collect_define`.

- [ ] **Step 4: Write the implementation**

Add above the test module in `src/trim/defines.rs`:

```rust
/// Reads one `defines` table out of the install's `runtime-api.json`.
///
/// # This reads `order`, and `order` is not the value
///
/// Ported unchanged from `tools/trim-factorio-oracle.py:150` so that the
/// acceptance test can prove the port before any behaviour changes. It is
/// wrong, deliberately, and tracked as FactorioTools#83.
///
/// `runtime-api.json` does not contain the values of `defines`. Across all
/// 1,554 entries in the installed 2.1.14 file the only keys are `name`, `order`
/// and `description`. `order` is a dense `0..n-1` index across all 137 tables,
/// so it cannot express a gap, a duplicate, or a non-zero start, and the values
/// are stored alphabetically. It is right today only because Factorio declares
/// directions clockwise from `north = 0` with no gaps.
///
/// The irony is worth keeping in the source: direction encoding is the exact
/// constant that silently broke in 2.0, and it is the one thing here that is
/// inferred rather than read. Only the running game knows that
/// `defines.direction.east` is 4. Reading it properly needs a probe mod, which
/// this crate now has.
pub fn collect_define(doc_dir: &Path, table: &str) -> anyhow::Result<Value> {
    let path = doc_dir.join("runtime-api.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
    let api: Value = serde_json::from_str(&text)
        .map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))?;

    let defines = api
        .get("defines")
        .and_then(|d| d.as_array())
        .ok_or_else(|| anyhow::anyhow!("{} has no defines array", path.display()))?;

    for define in defines {
        if define.get("name").and_then(|n| n.as_str()) != Some(table) {
            continue;
        }
        let mut out = Map::new();
        for value in define.get("values").and_then(|v| v.as_array()).unwrap_or(&vec![]) {
            let (Some(name), Some(order)) = (
                value.get("name").and_then(|n| n.as_str()),
                value.get("order"),
            ) else {
                continue;
            };
            out.insert(name.to_string(), order.clone());
        }
        return Ok(Value::Object(out));
    }

    Err(anyhow::anyhow!(
        "could not find defines.{table} in {}",
        path.display()
    ))
}
```

Add to `src/trim/mod.rs`:

```rust
pub mod defines;
```

- [ ] **Step 5: Run the tests**

Run: `cargo test --lib trim::defines`
Expected: PASS, 4 tests.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Port the defines reader, order bug and all

runtime-api.json does not contain the values of defines. Across all 1,554
entries in 2.1.14 the only keys are name, order and description, and order
is a dense 0..n-1 index across all 137 tables, so it cannot express a gap,
a duplicate, or a non-zero start. It is right today only because Factorio
declares directions clockwise from north = 0.

Ported unchanged on purpose. The acceptance test compares byte for byte
against a fixture the buggy code produced, so fixing this in the same change
would make a port error and a deliberate fix look identical in the diff.
Tracked as FactorioTools#83 and fixed in its own commit."
```

---

### Task 7: Report which mods actually loaded

`captureInfo.loadedMods` cannot come from the active-mods prelude. The committed fixture lists `core`, and `script.active_mods` does not report `core`. `dump-data` runs no mod at all, so there is no prelude. It has to come from the game's stdout.

**Files:**
- Modify: `src/run.rs`

**Interfaces:**
- Consumes: `SpawnResult` from plan 1.
- Produces: `pub fn loaded_mods(stdout: &str) -> Vec<String>`, and a `loadedMods` key in the `run` result JSON.

- [ ] **Step 1: Write the failing test**

Add to the test module in `src/run.rs`:

```rust
    #[test]
    fn loaded_mods_are_read_off_the_games_own_output() {
        // Real lines from a 2.1.14 --dump-data run.
        let stdout = "\
   0.043 Loading mod core 0.0.0 (data.lua)
   0.053 Loading mod base 2.1.14 (data.lua)
   0.165 Loading mod recycler 2.1.14 (data.lua)
   0.173 Loading mod base 2.1.14 (data-updates.lua)
   0.177 Loading mod recycler 2.1.14 (data-updates.lua)
   0.674 Prototype list checksum: 3041708406
";
        // Sorted and deduplicated: base loads three times across the stages.
        assert_eq!(loaded_mods(stdout), vec!["base", "core", "recycler"]);
    }

    #[test]
    fn loaded_mods_includes_core_which_active_mods_does_not() {
        // This is why the report cannot come from the script.active_mods
        // prelude. FactorioTools' committed fixture lists core, and dump-data
        // runs no mod at all so there is no prelude to ask.
        let stdout = "   0.043 Loading mod core 0.0.0 (data.lua)\n";
        assert_eq!(loaded_mods(stdout), vec!["core"]);
    }

    #[test]
    fn a_mod_name_with_a_hyphen_or_underscore_survives() {
        let stdout = "Loading mod elevated-rails 2.1.14 (data.lua)\n\
                      Loading mod oracle_probe 0.0.1 (data.lua)\n";
        assert_eq!(loaded_mods(stdout), vec!["elevated-rails", "oracle_probe"]);
    }

    #[test]
    fn output_with_no_such_lines_gives_an_empty_list() {
        assert!(loaded_mods("nothing to see here").is_empty());
    }
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --lib run::tests::loaded_mods`
Expected: FAIL, `cannot find function loaded_mods`.

- [ ] **Step 3: Write the implementation**

Add to `src/run.rs`, above `run_probe`:

```rust
/// The mods the game reported loading, sorted and deduplicated.
///
/// Read from stdout rather than from the `script.active_mods` prelude, for two
/// reasons. `dump-data` runs no mod at all, so there is no control script to
/// host a prelude. And the prelude cannot see `core`: measured on 2.1.14, a
/// create run reported base and the DLC but never `core`, while the game's
/// output names it first. FactorioTools' committed fixture lists it.
///
/// Hand-rolled rather than a regex, to keep the dependency surface small. The
/// line shape is `Loading mod <name> <version> (<stage>.lua)`.
pub fn loaded_mods(stdout: &str) -> Vec<String> {
    const MARKER: &str = "Loading mod ";
    let mut names: Vec<String> = stdout
        .lines()
        .filter_map(|line| {
            let start = line.find(MARKER)? + MARKER.len();
            let rest = &line[start..];
            let name = rest.split_whitespace().next()?;
            if name.is_empty() {
                None
            } else {
                Some(name.to_string())
            }
        })
        .collect();
    names.sort();
    names.dedup();
    names
}
```

In `run_probe`, add the key to the result object. Change the `json!` block that builds `out` so it also contains:

```rust
        "loadedMods": loaded_mods(&result.stdout),
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --all-targets`
Expected: PASS.

- [ ] **Step 5: Prove it against the real game**

Add to `tests/real_game.rs`:

```rust
#[test]
fn a_real_dump_data_run_reports_the_bundled_mod_set() {
    let Some(found) = find_install() else {
        eprintln!("skipping: no Factorio install found. Set FACTORIO_BIN to run this.");
        return;
    };

    let work = tempfile::Builder::new()
        .prefix("factorio-oracle-it-")
        .tempdir()
        .unwrap();

    let spec: ProbeSpec = serde_json::from_value(serde_json::json!({
        "mode": "dump-data",
        "timeout_seconds": 300,
    }))
    .unwrap();

    let request = RunRequest {
        map_gen_settings: spec.resolved_map_gen_settings(),
        spec,
        layout: found.layout,
        version: found.version.unwrap(),
        work_dir: work.path().to_path_buf(),
    };

    let result = run_probe(&request, &RealSpawner).unwrap();
    assert_eq!(
        result["ok"],
        true,
        "the run failed: {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    let mods: Vec<String> =
        serde_json::from_value(result["loadedMods"].clone()).unwrap();
    // core is the one the active-mods prelude cannot see.
    assert!(mods.contains(&"core".to_string()), "got {mods:?}");
    assert!(mods.contains(&"base".to_string()), "got {mods:?}");

    // The dump landed in the isolated write directory, not a shared one.
    assert!(work
        .path()
        .join("write/script-output/data-raw-dump.json")
        .is_file());
}
```

Run: `cargo test --test real_game`
Expected: PASS, 3 tests. On 2.1.14 the reported set is base, core, elevated-rails, quality, recycler and space-age.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Report the loaded mods from the game's own output

captureInfo.loadedMods cannot come from the active-mods prelude. dump-data
runs no mod at all, so there is no control script to host one, and measured
on 2.1.14 the prelude never reports core while the game's output names it
first. FactorioTools' committed fixture lists core, so the fixture could not
be reproduced from script.active_mods.

Hand-rolled rather than pulling in a regex crate: the line shape is
Loading mod <name> <version> (<stage>.lua)."
```

---

### Task 8: Assemble the fixture

**Files:**
- Modify: `src/trim/mod.rs`

**Interfaces:**
- Consumes: everything from Tasks 1 to 6.
- Produces: `pub struct TrimInputs<'a> { pub dump: &'a Value, pub spec: &'a TrimSpec, pub data_dir: &'a Path, pub doc_dir: &'a Path, pub factorio_version: &'a str, pub loaded_mods: &'a [String] }` and `pub fn build_fixture(inputs: &TrimInputs) -> anyhow::Result<Value>`.

- [ ] **Step 1: Write the failing test**

Add to `src/trim/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn spec() -> spec::TrimSpec {
        serde_json::from_value(json!({
            "comment": "Do not hand-edit.",
            "entities": ["pumpjack"],
            "entity_fields": ["collision_box"],
            "connection_fields": ["positions"],
            "fluid_boxes": ["output_fluid_box"],
            "name_lists": { "modules": "module" },
            "defines": { "directions": "direction" },
            "include_renames": true
        }))
        .unwrap()
    }

    fn dump() -> Value {
        json!({
            "item": { "pumpjack": { "stack_size": 20 } },
            "mining-drill": { "pumpjack": {
                "collision_box": [[-1.2, -1.2], [1.2, 1.2]],
                "output_fluid_box": { "pipe_connections": [{ "positions": [[1, -1]] }] }
            }},
            "module": { "speed-module": {}, "efficiency-module": {} }
        })
    }

    fn game_dirs() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("data/base/migrations")).unwrap();
        std::fs::write(
            dir.path().join("data/base/migrations/2.0.0.json"),
            r#"{"item": [["effectivity-module", "efficiency-module"]]}"#,
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("doc-html")).unwrap();
        std::fs::write(
            dir.path().join("doc-html/runtime-api.json"),
            r#"{"defines": [{"name": "direction", "values": [
                 {"name": "north", "order": 0}, {"name": "east", "order": 4}]}]}"#,
        )
        .unwrap();
        dir
    }

    #[test]
    fn assembles_every_section_the_caller_asked_for() {
        let dirs = game_dirs();
        let dump = dump();
        let spec = spec();
        let mods = vec!["base".to_string(), "core".to_string()];
        let fixture = build_fixture(&TrimInputs {
            dump: &dump,
            spec: &spec,
            data_dir: &dirs.path().join("data"),
            doc_dir: &dirs.path().join("doc-html"),
            factorio_version: "2.1.14",
            loaded_mods: &mods,
        })
        .unwrap();

        assert_eq!(fixture["_comment"], "Do not hand-edit.");
        assert_eq!(fixture["captureInfo"]["factorioVersion"], "2.1.14");
        assert_eq!(fixture["captureInfo"]["loadedMods"], json!(["base", "core"]));
        assert_eq!(fixture["directions"]["east"], 4);
        assert_eq!(fixture["entities"]["pumpjack"]["prototypeType"], "mining-drill");
        assert_eq!(fixture["modules"], json!(["efficiency-module", "speed-module"]));
        assert_eq!(fixture["renames"]["item"]["effectivity-module"], "efficiency-module");
    }

    #[test]
    fn a_named_entity_that_no_longer_exists_is_a_loud_failure() {
        // An entity the consumer names but the game does not have is exactly
        // the drift this tool is built to catch. Writing a quietly incomplete
        // fixture would be the silent pass it exists to prevent.
        let dirs = game_dirs();
        let dump = dump();
        let mut spec = spec();
        spec.entities.push("quantum-pumpjack".to_string());
        let err = build_fixture(&TrimInputs {
            dump: &dump,
            spec: &spec,
            data_dir: &dirs.path().join("data"),
            doc_dir: &dirs.path().join("doc-html"),
            factorio_version: "2.1.14",
            loaded_mods: &[],
        })
        .unwrap_err()
        .to_string();
        assert!(err.contains("quantum-pumpjack"), "got {err}");
        assert!(err.contains("do not delete them"), "got {err}");
    }

    #[test]
    fn sections_the_caller_did_not_ask_for_are_absent() {
        let dirs = game_dirs();
        let dump = dump();
        let spec: spec::TrimSpec =
            serde_json::from_value(json!({ "entities": [] })).unwrap();
        let fixture = build_fixture(&TrimInputs {
            dump: &dump,
            spec: &spec,
            data_dir: &dirs.path().join("data"),
            doc_dir: &dirs.path().join("doc-html"),
            factorio_version: "2.1.14",
            loaded_mods: &[],
        })
        .unwrap();
        assert!(fixture.get("_comment").is_none());
        assert!(fixture.get("renames").is_none());
        assert!(fixture.get("directions").is_none());
        assert!(fixture.get("modules").is_none());
    }

    #[test]
    fn numbers_are_normalised_on_the_way_out() {
        let dirs = game_dirs();
        let long = "0.394500000000000028421709430404007434844970703125";
        let dump: Value = serde_json::from_str(&format!(
            r#"{{"mining-drill": {{"pumpjack": {{"collision_box": [{long}]}}}}}}"#
        ))
        .unwrap();
        let spec: spec::TrimSpec = serde_json::from_value(json!({
            "entities": ["pumpjack"], "entity_fields": ["collision_box"]
        }))
        .unwrap();
        let fixture = build_fixture(&TrimInputs {
            dump: &dump,
            spec: &spec,
            data_dir: &dirs.path().join("data"),
            doc_dir: &dirs.path().join("doc-html"),
            factorio_version: "2.1.14",
            loaded_mods: &[],
        })
        .unwrap();
        let text = canonical::to_canonical_json(&fixture);
        assert!(text.contains("0.3945"), "got {text}");
        assert!(!text.contains("0.39450000000000002"), "got {text}");
    }
}
```

- [ ] **Step 2: Run it to see it fail**

Run: `cargo test --lib trim::tests`
Expected: FAIL, `cannot find function build_fixture`.

- [ ] **Step 3: Write the implementation**

Add to `src/trim/mod.rs`, above the test module:

```rust
use crate::trim::spec::TrimSpec;
use serde_json::{Map, Value};
use std::path::Path;

/// Everything `build_fixture` needs. The caller resolves the install and runs
/// the game, so nothing here launches anything.
pub struct TrimInputs<'a> {
    /// A parsed `data-raw-dump.json`.
    pub dump: &'a Value,
    pub spec: &'a TrimSpec,
    /// The install's `data` directory, for migrations.
    pub data_dir: &'a Path,
    /// The install's `doc-html` directory, for `runtime-api.json`.
    pub doc_dir: &'a Path,
    pub factorio_version: &'a str,
    pub loaded_mods: &'a [String],
}

/// Builds the fixture document.
///
/// Only the sections the caller asked for appear. A consumer that wants entity
/// geometry and nothing else gets a file with `captureInfo` and `entities`, not
/// four empty objects.
pub fn build_fixture(inputs: &TrimInputs) -> anyhow::Result<Value> {
    let raw = inputs
        .dump
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("the dump is not a JSON object"))?;

    let mut entities = Map::new();
    let mut missing: Vec<&str> = Vec::new();
    for name in &inputs.spec.entities {
        match prototypes::find_prototype(raw, name) {
            Some((kind, proto)) => {
                entities.insert(
                    name.clone(),
                    prototypes::trim_entity(&kind, proto, inputs.spec),
                );
            }
            None => missing.push(name),
        }
    }
    if !missing.is_empty() {
        // A named entity that no longer exists is exactly the failure this tool
        // is built to catch, so it is loud rather than a quietly incomplete
        // file.
        anyhow::bail!(
            "these entities are named by the caller but do not exist in this Factorio \
             version: {}. That is a real finding - fix the consumer, do not delete them \
             from the trim spec.",
            missing.join(", ")
        );
    }

    let mut fixture = Map::new();
    if let Some(comment) = inputs.spec.comment.as_ref() {
        fixture.insert("_comment".to_string(), Value::String(comment.clone()));
    }

    let mut capture = Map::new();
    capture.insert(
        "factorioVersion".to_string(),
        Value::String(inputs.factorio_version.to_string()),
    );
    let mut mods: Vec<String> = inputs.loaded_mods.to_vec();
    mods.sort();
    capture.insert(
        "loadedMods".to_string(),
        Value::Array(mods.into_iter().map(Value::String).collect()),
    );
    fixture.insert("captureInfo".to_string(), Value::Object(capture));

    for (output_key, table) in &inputs.spec.defines {
        fixture.insert(
            output_key.clone(),
            defines::collect_define(inputs.doc_dir, table)?,
        );
    }

    if !entities.is_empty() {
        fixture.insert("entities".to_string(), Value::Object(entities));
    }

    for (output_key, prototype_type) in &inputs.spec.name_lists {
        let mut names: Vec<String> = raw
            .get(prototype_type)
            .and_then(|v| v.as_object())
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        names.sort();
        fixture.insert(
            output_key.clone(),
            Value::Array(names.into_iter().map(Value::String).collect()),
        );
    }

    if inputs.spec.include_renames {
        fixture.insert(
            "renames".to_string(),
            renames::collect_renames(inputs.data_dir),
        );
    }

    Ok(canonical::normalise_numbers(&Value::Object(fixture)))
}
```

- [ ] **Step 4: Run the tests**

Run: `cargo test --lib trim`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Assemble the fixture from the pieces

Only the sections the caller asked for appear, so a consumer wanting entity
geometry alone gets captureInfo and entities rather than four empty objects.

A named entity the game does not have is a loud failure. That is exactly the
drift this tool is built to catch, and writing a quietly incomplete file
would be the silent pass it exists to prevent."
```

---

### Task 9: The `trim` subcommand, with `--check`

**Files:**
- Modify: `src/main.rs`

**Interfaces:**
- Consumes: `build_fixture`, `to_canonical_json`, `install::resolve`.
- Produces: `factorio-oracle trim --run <run.json> --spec <trim.json> --out <fixture.json> [--check]`.

- [ ] **Step 1: Add the subcommand**

In `src/main.rs`, add to the `Command` enum:

```rust
    /// Trim a dump into a consumer's fixture
    Trim {
        /// The JSON a `run` produced. Names the dump, the install and the mods.
        #[arg(long)]
        run: PathBuf,
        /// The caller's trim spec
        #[arg(long)]
        spec: PathBuf,
        /// Where to write the fixture
        #[arg(long)]
        out: PathBuf,
        /// Report drift against `--out` and change nothing. Exits 1 on a
        /// mismatch.
        #[arg(long)]
        check: bool,
    },
```

- [ ] **Step 2: Handle it**

Add the match arm:

```rust
        Command::Trim {
            run,
            spec,
            out,
            check,
        } => {
            let run_result: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&run)?)?;
            let trim_spec: factorio_oracle::trim::spec::TrimSpec =
                serde_json::from_str(&std::fs::read_to_string(&spec)?)?;

            let script_output = run_result["scriptOutput"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("the run result has no scriptOutput"))?;
            let dump_path = PathBuf::from(script_output).join("data-raw-dump.json");
            let dump: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&dump_path)?)?;

            // The install is re-derived from the binary the run recorded, so
            // the fixture cannot describe a different install than the dump.
            let binary = run_result["provenance"]["binaryPath"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("the run result has no binaryPath"))?;
            let layout = install::resolve(Path::new(binary))
                .ok_or_else(|| anyhow::anyhow!("could not resolve the install at {binary}"))?;

            let version = run_result["provenance"]["factorioVersion"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("the run result has no factorioVersion"))?;
            let loaded_mods: Vec<String> =
                serde_json::from_value(run_result["loadedMods"].clone()).unwrap_or_default();

            let fixture = factorio_oracle::trim::build_fixture(
                &factorio_oracle::trim::TrimInputs {
                    dump: &dump,
                    spec: &trim_spec,
                    data_dir: &layout.data_dir,
                    doc_dir: &layout.doc_dir,
                    factorio_version: version,
                    loaded_mods: &loaded_mods,
                },
            )?;
            let text = factorio_oracle::trim::canonical::to_canonical_json(&fixture);

            if check {
                let committed = std::fs::read_to_string(&out).unwrap_or_default();
                if committed == text {
                    println!("Up to date: {} matches Factorio {version}.", out.display());
                } else {
                    eprintln!(
                        "DRIFT: {} does not match Factorio {version}.",
                        out.display()
                    );
                    eprintln!("Re-run without --check to update it, then review what moved.");
                    std::process::exit(1);
                }
            } else {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&out, &text)?;
                println!("Wrote {}", out.display());
            }
        }
```

Add `use std::path::Path;` to the imports if it is not already there.

- [ ] **Step 3: Confirm `install::resolve` has the signature this uses**

Run: `grep -n "pub fn resolve" src/install.rs`
Expected: a function taking a path and returning `Option<InstallLayout>`. If it takes something else, adapt the call rather than changing `install.rs`, which plan 1's tests cover.

- [ ] **Step 4: Build and check the help text**

Run: `cargo run --quiet -- trim --help`
Expected: the four flags, with `--check` described as reporting drift.

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "Add the trim subcommand, taking a run result as its input

One JSON in, one JSON out. The run result already names the dump, the
binary, the version and the loaded mods, so trim needs no environment
variables and cannot be pointed at a dump from a different install than the
provenance it stamps on the output. That was the shape of the shell script
this replaces.

--check reports drift and changes nothing, so 'has the game moved past what
we committed?' is answerable without a dirty tree."
```

---

### Task 10: The acceptance test, byte for byte

Two halves. The first runs in CI with no game. The second runs only where the matching Factorio version is installed.

**Files:**
- Create: `tests/fixtures/data-raw-slice.json`
- Create: `tests/fixtures/factoriotools-trim-spec.json`
- Create: `tests/fixtures/expected-factorio-oracle-2.1.14.json`
- Create: `tests/acceptance.rs`

**Interfaces:**
- Consumes: `build_fixture`, `to_canonical_json`.
- Produces: nothing. This is the gate.

- [ ] **Step 1: Build the committed input slice**

The slice holds the full prototypes of every wanted name from every type that has one, plus the `module` keys. Measured at 163 KB, which is small enough to commit and large enough to exercise the disambiguation: all ten names appear in three types and `stone-wall` appears in four.

With a real install present, run this from `~/GitHub/factorio-oracle`:

```bash
cargo run --quiet -- run --probe /tmp/dump-data.json --work-dir /tmp/oracle-dump
python3 - <<'EOF'
import json
d = json.load(open('/tmp/oracle-dump/write/script-output/data-raw-dump.json'))
want = ["pumpjack", "pipe", "pipe-to-ground", "small-electric-pole",
        "medium-electric-pole", "big-electric-pole", "substation", "beacon",
        "heat-pipe", "stone-wall"]
out = {}
for kind, protos in d.items():
    if not isinstance(protos, dict):
        continue
    for name in want:
        if name in protos:
            out.setdefault(kind, {})[name] = protos[name]
out["module"] = {k: {} for k in d.get("module", {})}
json.dump(out, open("tests/fixtures/data-raw-slice.json", "w"), indent=2, sort_keys=True)
EOF
```

where `/tmp/dump-data.json` is `{ "mode": "dump-data", "timeout_seconds": 300 }`.

If no install is available, the slice can be produced from any 2.1.14 `data-raw-dump.json` with the same script.

- [ ] **Step 2: Write the trim spec, which is FactorioTools' allowlists verbatim**

Create `tests/fixtures/factoriotools-trim-spec.json`:

```json
{
  "comment": "Generated by tools/capture-factorio-oracle.sh. Do not hand-edit. Raw Factorio prototype values that the oil field planner depends on. Re-capture after a Factorio update and commit the diff.",
  "entities": [
    "pumpjack",
    "pipe",
    "pipe-to-ground",
    "small-electric-pole",
    "medium-electric-pole",
    "big-electric-pole",
    "substation",
    "beacon",
    "heat-pipe",
    "stone-wall"
  ],
  "entity_fields": [
    "collision_box",
    "selection_box",
    "tile_width",
    "tile_height",
    "supply_area_distance",
    "maximum_wire_distance",
    "distribution_effectivity",
    "distribution_effectivity_bonus_per_quality_level",
    "module_slots",
    "beacon_counter",
    "allowed_effects",
    "energy_usage"
  ],
  "connection_fields": [
    "connection_type",
    "direction",
    "position",
    "positions",
    "flow_direction",
    "max_underground_distance"
  ],
  "fluid_boxes": ["fluid_box", "output_fluid_box", "input_fluid_box"],
  "name_lists": { "modules": "module" },
  "defines": { "directions": "direction" },
  "include_renames": true
}
```

- [ ] **Step 3: Copy the expected output**

```bash
cp /Users/ericjohnson/GitHub/FactorioTools/test/FactorioTools.Test/OilField/factorio-oracle.json \
   tests/fixtures/expected-factorio-oracle-2.1.14.json
```

- [ ] **Step 4: Write the offline acceptance test**

Create `tests/acceptance.rs`:

```rust
//! Reproducing FactorioTools' committed fixture, byte for byte.
//!
//! This is the gate on whether this tool can replace
//! `tools/capture-factorio-oracle.sh`. Semantic equality is not enough: the
//! shell script's `--check` mode is a `diff` against a committed file, so an
//! output differing by a float's last digit or a key's position would make
//! every future check permanently red for no real reason.
//!
//! The offline half uses a committed 163 KB slice of `data.raw`, so it runs in
//! CI with no game. The install-gated half runs the real thing.

use factorio_oracle::trim::canonical::to_canonical_json;
use factorio_oracle::trim::{build_fixture, spec::TrimSpec, TrimInputs};
use std::path::{Path, PathBuf};

const EXPECTED_VERSION: &str = "2.1.14";

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(name: &str) -> String {
    std::fs::read_to_string(fixtures().join(name))
        .unwrap_or_else(|e| panic!("reading {name}: {e}"))
}

/// The six mods a default 2.1.14 install loads, as the game reports them.
fn loaded_mods() -> Vec<String> {
    ["base", "core", "elevated-rails", "quality", "recycler", "space-age"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

#[test]
fn the_committed_fixture_is_reproduced_byte_for_byte() {
    let dump: serde_json::Value = serde_json::from_str(&read("data-raw-slice.json")).unwrap();
    let spec: TrimSpec = serde_json::from_str(&read("factoriotools-trim-spec.json")).unwrap();
    let expected = read("expected-factorio-oracle-2.1.14.json");

    // Renames and defines come off the install's own directories, so the
    // offline test needs a stand-in. These two are committed alongside the
    // slice by Step 6.
    let data_dir = fixtures().join("data");
    let doc_dir = fixtures().join("doc-html");

    let mods = loaded_mods();
    let fixture = build_fixture(&TrimInputs {
        dump: &dump,
        spec: &spec,
        data_dir: &data_dir,
        doc_dir: &doc_dir,
        factorio_version: EXPECTED_VERSION,
        loaded_mods: &mods,
    })
    .unwrap();

    let actual = to_canonical_json(&fixture);
    if actual != expected {
        // A unified diff of the first difference is far more useful than
        // "assertion failed", because the whole point is which byte moved.
        let mut line = 0;
        for (a, b) in actual.lines().zip(expected.lines()) {
            line += 1;
            if a != b {
                panic!("first difference at line {line}\n  ours:     {a}\n  expected: {b}");
            }
        }
        panic!(
            "same prefix, different length: ours {} lines, expected {} lines",
            actual.lines().count(),
            expected.lines().count()
        );
    }
}

#[test]
fn the_real_install_reproduces_it_too() {
    use factorio_oracle::install;
    use factorio_oracle::probe::ProbeSpec;
    use factorio_oracle::run::{run_probe, RunRequest};
    use factorio_oracle::spawn::RealSpawner;

    let home = PathBuf::from(std::env::var("HOME").unwrap_or_default());
    let env_bin = std::env::var_os("FACTORIO_BIN").map(PathBuf::from);
    let Some(found) = install::discover(&home, env_bin.as_deref())
        .into_iter()
        .find(|d| {
            d.version
                .as_ref()
                .map(|v| format!("{}.{}.{}", v.major, v.minor, v.patch) == EXPECTED_VERSION)
                .unwrap_or(false)
        })
    else {
        eprintln!(
            "skipping: no Factorio {EXPECTED_VERSION} install found. The expected fixture is \
             version-specific, so another version would fail for the wrong reason."
        );
        return;
    };

    let work = tempfile::Builder::new()
        .prefix("factorio-oracle-acceptance-")
        .tempdir()
        .unwrap();

    let probe: ProbeSpec =
        serde_json::from_value(serde_json::json!({ "mode": "dump-data", "timeout_seconds": 300 }))
            .unwrap();
    let layout = found.layout.clone();
    let request = RunRequest {
        map_gen_settings: probe.resolved_map_gen_settings(),
        spec: probe,
        layout: found.layout,
        version: found.version.unwrap(),
        work_dir: work.path().to_path_buf(),
    };
    let result = run_probe(&request, &RealSpawner).unwrap();
    assert_eq!(result["ok"], true, "the dump-data run failed: {result}");

    let dump: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(work.path().join("write/script-output/data-raw-dump.json"))
            .unwrap(),
    )
    .unwrap();
    let spec: TrimSpec = serde_json::from_str(&read("factoriotools-trim-spec.json")).unwrap();
    let mods: Vec<String> = serde_json::from_value(result["loadedMods"].clone()).unwrap();

    let fixture = build_fixture(&TrimInputs {
        dump: &dump,
        spec: &spec,
        data_dir: &layout.data_dir,
        doc_dir: &layout.doc_dir,
        factorio_version: EXPECTED_VERSION,
        loaded_mods: &mods,
    })
    .unwrap();

    assert_eq!(
        to_canonical_json(&fixture),
        read("expected-factorio-oracle-2.1.14.json"),
        "the real install did not reproduce the committed fixture"
    );
}
```

- [ ] **Step 5: Run the install-gated half first**

Run: `cargo test --test acceptance the_real_install`
Expected: PASS on a machine with 2.1.14. This half needs no committed stand-in directories, so it is the quickest way to find a real porting error.

If it fails, the panic names the first differing line. Work through them one at a time; do not adjust the expected file.

- [ ] **Step 6: Commit the stand-in game directories for the offline half**

The offline test needs migrations and a `runtime-api.json`. Copy only what the spec reads:

```bash
mkdir -p tests/fixtures/doc-html tests/fixtures/data
A="$HOME/Library/Application Support/Steam/steamapps/common/Factorio/factorio.app/Contents"
for d in "$A"/data/*/migrations; do
  mod=$(basename "$(dirname "$d")")
  mkdir -p "tests/fixtures/data/$mod/migrations"
  cp "$d"/*.json "tests/fixtures/data/$mod/migrations/" 2>/dev/null || true
done
python3 - <<'EOF'
import json, os
a = os.path.expanduser("~/Library/Application Support/Steam/steamapps/common/Factorio/factorio.app/Contents")
api = json.load(open(f"{a}/doc-html/runtime-api.json"))
# Only the defines array is read, and only the direction table is asked for.
# The full file is 1,554 entries and several megabytes.
slim = {"defines": [d for d in api.get("defines", []) if d.get("name") == "direction"]}
json.dump(slim, open("tests/fixtures/doc-html/runtime-api.json", "w"), indent=2, sort_keys=True)
EOF
du -sh tests/fixtures
```

Expected: a few hundred kilobytes in total.

- [ ] **Step 7: Run the offline half**

Run: `cargo test --test acceptance the_committed_fixture`
Expected: PASS.

- [ ] **Step 8: Prove the offline half runs with no game**

Run: `HOME=/tmp/no-factorio cargo test --test acceptance`
Expected: PASS, with the install-gated test printing its skip message.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "Reproduce FactorioTools' fixture byte for byte

This is the gate on whether this tool can replace
tools/capture-factorio-oracle.sh. Semantic equality would not be enough: the
script's --check mode is a diff against a committed file, so an output
differing by a float's last digit or a key's position would turn every
future check permanently red for no real reason.

Two halves. The offline one uses a committed 163 KB slice of data.raw and
runs in CI with no game. All ten names appear in three prototype types and
stone-wall in four, so the collision-box disambiguation is genuinely
exercised rather than assumed. The install-gated one runs the real game and
skips unless the install is 2.1.14, because the expected file is
version-specific and another version would fail for the wrong reason."
```

---

### Task 11: Fix FactorioTools#83, on purpose and on its own

Only start this once Task 10 passes. The point of the ordering is that a port error and a deliberate change must never appear in the same diff.

**Files:**
- Modify: `src/trim/defines.rs`
- Modify: `src/trim/mod.rs`
- Modify: `src/trim/spec.rs`
- Modify: `tests/acceptance.rs`

**Interfaces:**
- Consumes: the `create` mode from plan 1.
- Produces: `pub fn defines_from_probe(probe_dump: &Value, table: &str) -> anyhow::Result<Value>`, and a `defines_from` field on `TrimSpec`.

- [ ] **Step 1: Know what the answer has to be**

Already measured. A `create` probe on 2.1.14 calling `helpers.write_file` with `defines.direction` returned:

```json
{"north":0,"northnortheast":1,"northeast":2,"eastnortheast":3,"east":4,
 "eastsoutheast":5,"southeast":6,"southsoutheast":7,"south":8,
 "southsouthwest":9,"southwest":10,"westsouthwest":11,"west":12,
 "westnorthwest":13,"northwest":14,"northnorthwest":15}
```

That is identical to what `order` produces today, so **this fix changes no bytes in the fixture**. That is the ideal shape for it: the method becomes sound, and the acceptance test proves the change is safe rather than merely plausible.

- [ ] **Step 2: Write the failing test**

Add to the test module in `src/trim/defines.rs`:

```rust
    #[test]
    fn a_probe_dump_gives_the_value_the_game_actually_uses() {
        // Measured on 2.1.14 with a create probe. This is a read, not an
        // inference: only the running game knows east is 4.
        let probe = serde_json::json!({
            "directions": { "north": 0, "east": 4, "south": 8, "west": 12 }
        });
        let table = defines_from_probe(&probe, "directions").unwrap();
        assert_eq!(table["east"], 4);
        assert_eq!(table["west"], 12);
    }

    #[test]
    fn a_probe_dump_can_express_what_order_cannot() {
        // The reason the fix matters. `order` is a dense 0..n-1 index, so it
        // cannot represent a gap, a duplicate, or a non-zero start. A real
        // reading can.
        let probe = serde_json::json!({ "t": { "a": 10, "b": 10, "c": 40 } });
        let table = defines_from_probe(&probe, "t").unwrap();
        assert_eq!(table["a"], 10);
        assert_eq!(table["b"], 10);
        assert_eq!(table["c"], 40);
    }

    #[test]
    fn a_probe_dump_missing_the_table_names_it() {
        let probe = serde_json::json!({ "other": {} });
        let err = defines_from_probe(&probe, "directions").unwrap_err().to_string();
        assert!(err.contains("directions"), "got {err}");
    }
```

- [ ] **Step 3: Run it to see it fail**

Run: `cargo test --lib trim::defines`
Expected: FAIL, `cannot find function defines_from_probe`.

- [ ] **Step 4: Write the implementation**

Add to `src/trim/defines.rs`:

```rust
/// Reads a defines table out of a probe mod's dump.
///
/// This is the sound way to answer "what number means east". `collect_define`
/// above infers it from a documentation index; this reads it from the running
/// game, which is the only authority. FactorioTools#83.
///
/// The probe writes the table under a key of its own choosing, so the caller
/// names it. Measured on 2.1.14, the values match what `order` produces, which
/// is why adopting this changes no bytes today. It changes what happens the
/// next time Factorio introduces a gap, a duplicate, or a non-zero start, none
/// of which a dense index can express.
pub fn defines_from_probe(probe_dump: &Value, key: &str) -> anyhow::Result<Value> {
    let table = probe_dump
        .get(key)
        .ok_or_else(|| anyhow::anyhow!("the probe dump has no `{key}` table"))?;
    if !table.is_object() {
        anyhow::bail!("the probe dump's `{key}` is not an object");
    }
    Ok(table.clone())
}
```

Add to `TrimSpec` in `src/trim/spec.rs`:

```rust
    /// Where `defines` values come from.
    ///
    /// `doc-index` reads `order` out of `runtime-api.json`, which is a
    /// documentation index rather than the runtime value. It is right only
    /// while a table is a dense sequence from zero. `probe` reads the value the
    /// running game uses, and needs a `create` run's dump.
    ///
    /// The default is `doc-index` so existing callers keep working. New callers
    /// should choose `probe`. See FactorioTools#83.
    #[serde(default)]
    pub defines_from: DefinesSource,
```

and, in the same file:

```rust
/// Where defines values are read from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefinesSource {
    /// `order` from `runtime-api.json`. An inference, not a reading.
    #[default]
    DocIndex,
    /// The value the running game reports, via a `create` probe.
    Probe,
}
```

In `src/trim/mod.rs`, add `pub probe_dump: Option<&'a Value>` to `TrimInputs`, and replace the defines loop with:

```rust
    for (output_key, table) in &inputs.spec.defines {
        let value = match inputs.spec.defines_from {
            spec::DefinesSource::DocIndex => defines::collect_define(inputs.doc_dir, table)?,
            spec::DefinesSource::Probe => {
                let probe = inputs.probe_dump.ok_or_else(|| {
                    anyhow::anyhow!(
                        "defines_from is `probe`, so a probe dump is required. Run a \
                         create-mode probe that writes defines.{table} and pass it in."
                    )
                })?;
                defines::defines_from_probe(probe, output_key)?
            }
        };
        fixture.insert(output_key.clone(), value);
    }
```

Update every `TrimInputs` construction in tests and in `main.rs` to pass `probe_dump: None`.

- [ ] **Step 5: Add the acceptance test that proves the fix changes nothing**

Add to `tests/acceptance.rs`:

```rust
#[test]
fn reading_defines_from_the_game_produces_the_same_bytes() {
    // FactorioTools#83. The doc-index route infers direction values from a
    // dense documentation ordering; the probe route reads what the game uses.
    // Measured on 2.1.14 they agree, so adopting the sound method is a
    // no-op on the output. This test is what makes that claim checkable
    // rather than asserted.
    let dump: serde_json::Value = serde_json::from_str(&read("data-raw-slice.json")).unwrap();
    let mut spec: TrimSpec = serde_json::from_str(&read("factoriotools-trim-spec.json")).unwrap();
    spec.defines_from = factorio_oracle::trim::spec::DefinesSource::Probe;

    // Exactly what a create probe wrote on 2.1.14.
    let probe = serde_json::json!({ "directions": {
        "north": 0, "northnortheast": 1, "northeast": 2, "eastnortheast": 3,
        "east": 4, "eastsoutheast": 5, "southeast": 6, "southsoutheast": 7,
        "south": 8, "southsouthwest": 9, "southwest": 10, "westsouthwest": 11,
        "west": 12, "westnorthwest": 13, "northwest": 14, "northnorthwest": 15
    }});

    let mods = loaded_mods();
    let fixture = build_fixture(&TrimInputs {
        dump: &dump,
        spec: &spec,
        data_dir: &fixtures().join("data"),
        doc_dir: &fixtures().join("doc-html"),
        factorio_version: EXPECTED_VERSION,
        loaded_mods: &mods,
        probe_dump: Some(&probe),
    })
    .unwrap();

    assert_eq!(
        to_canonical_json(&fixture),
        read("expected-factorio-oracle-2.1.14.json"),
        "reading defines from the game changed the fixture, which was not expected"
    );
}
```

- [ ] **Step 6: Run everything**

Run: `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test --all-targets`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Read defines from the game, not from a documentation index

FactorioTools#83. runtime-api.json does not contain the values of defines:
across all 1,554 entries in 2.1.14 the only keys are name, order and
description. Reading order as the value is right only while a table is a
dense sequence from zero, and it cannot express a gap, a duplicate, or a
non-zero start.

Direction encoding is the exact constant that silently broke in 2.0, and it
was the one thing here that was inferred rather than read.

Measured on 2.1.14 with a create probe: the values match what order
produces, so adopting the sound method changes no bytes today. An acceptance
test asserts exactly that, which turns 'this fix is safe' from a claim into
something checked. What changes is the next time the two disagree.

doc-index stays the default so existing callers keep working. New callers
should pass defines_from: probe."
```

---

### Task 12: Point FactorioTools at the new tool

The last step is to say, in the repo that has the fixture, what now produces it.

**Files:**
- Modify: `/Users/ericjohnson/GitHub/FactorioTools/CLAUDE.md`
- Modify: `/Users/ericjohnson/GitHub/FactorioTools/tools/capture-factorio-oracle.sh`

**Interfaces:**
- Consumes: the finished `trim` command.
- Produces: documentation only. No planner code changes.

- [ ] **Step 1: Correct the four-sources claim**

`CLAUDE.md` says the capture pulls four sources and lists `data/changelog.txt`. It pulls three. There is no changelog handling anywhere in `tools/`. The changelog is a research source a human reads; it was written into the table as if it were automated. Remove that row, and keep the changelog mentioned in prose as a thing to read.

- [ ] **Step 2: Record that the shell script has a replacement**

Add a note to `tools/capture-factorio-oracle.sh`'s header comment and to `CLAUDE.md`'s oracle section: `FactoryGameFan/factorio-oracle` reproduces this fixture byte for byte, the acceptance test proves it, and the migration rule is new probes only, so this script stays until someone has a reason to touch it.

- [ ] **Step 3: Commit in the FactorioTools repo**

```bash
cd /Users/ericjohnson/GitHub/FactorioTools
git add -A
git commit -m "Correct the capture's source count, and name its replacement

The table said four sources and listed data/changelog.txt. The capture
reads three, and there is no changelog handling anywhere in tools/. The
changelog is a research source a human reads; it had been written into the
table as though it were automated.

Also records that FactoryGameFan/factorio-oracle now reproduces this
fixture byte for byte, with an acceptance test that proves it. The script
stays: the agreed migration rule is new probes only."
```

---

## Self-Review

**1. Spec coverage.** This plan covers build-order step 4 in full, plus the parts of "The output contract", "Determinism" and "Testing" that step 4 needs. Checked section by section against the spec:

- *Commands*: `trim` and `--check` are Task 9. `installs list` and `run` were plan 1.
- *The probe spec*: unchanged except `defines_from`, added in Task 11.
- *The output contract*: `loadedMods` added in Task 7; the rest was plan 1.
- *Determinism*: Task 4 covers sorted keys, indent, trailing newline and the float trap. The `BTreeMap` requirement is a global constraint and is satisfied by not enabling `preserve_order`.
- *Guards*: the contamination report becomes a real value in Task 7 rather than an echo. The freshness guard is designed out by plan 1's isolated `write-data`, now verified.
- *Testing*: Task 10 is the split the spec asks for.

Still deferred, each to its own plan:

- **Plan 3:** `provenance check`, the always-on completeness test, and the `unknown` ratchet. Task 8 writes a `captureInfo` block, which is provenance's smallest form; plan 3 generalises it and adds the evidence grade.
- **Plan 4:** `refs` sync, grep at a tag, worktree, the archive cache, and the three knowledge documents.

**2. Placeholder scan.** No TBD, TODO, "add error handling", or "similar to Task N". Every code step carries its code and every test step carries its assertions. Task 10 Steps 1 and 6 are shell scripts that generate committed fixtures rather than inline data, which is deliberate: a 163 KB slice cannot be pasted into a plan, and hand-writing one would test a shape nobody's game produces.

**3. Type consistency.** Checked across tasks. `TrimSpec` (Task 1) is consumed by Tasks 3, 8, 9, 10 and extended in Task 11; every construction in the plan uses the same field names. `find_prototype` returns `Option<(String, &Value)>` in Task 2 and is destructured that way in Task 8. `trim_entity(kind: &str, proto: &Value, spec: &TrimSpec)` in Task 3 is called with exactly those types in Task 8. `collect_renames(&Path) -> Value` (Task 5) and `collect_define(&Path, &str) -> anyhow::Result<Value>` (Task 6) match their call sites. `normalise_numbers` and `to_canonical_json` (Task 4) are used in Tasks 5, 8, 9 and 10. `loaded_mods(&str) -> Vec<String>` (Task 7) feeds `TrimInputs::loaded_mods: &[String]` (Task 8).

One consistency note for the implementer, the same one plan 1 hit: Task 11 adds `probe_dump` to `TrimInputs`, so every construction in Tasks 8, 9 and 10 stops compiling until it is added. That is intended. Fix them by passing `probe_dump: None`, not by giving the field a default.

**4. Ordering risk.** Task 4 changes a crate-wide `serde_json` feature, and plan 1's tests are the safety net. If enabling `arbitrary_precision` breaks something in `run.rs`, fix it there. Do not drop the feature, or Task 10 will pass today and fail silently the first time Factorio puts a long literal in a wanted field.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-08-17-factorio-oracle-trimmer.md`. Two execution options:

1. **Subagent-Driven (recommended)** - a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - execute tasks in this session with checkpoints for review.
