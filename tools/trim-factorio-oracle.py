#!/usr/bin/env python3
"""
Trims a full Factorio data.raw dump down to the small set of facts the oil field
planner actually depends on, and merges in two other authoritative sources:
the machine-readable runtime API (for direction values) and the game's own
migration files (for renames).

Invoked by tools/capture-factorio-oracle.sh, which supplies every input as an
environment variable. It is not meant to be run directly.

Design note: this stores RAW prototype values, not values derived from them.

It is tempting to store, say, "smallElectricPoleSupplyWidth: 5" so a test can
compare it to OilFieldOptions directly. Resist that. Factorio's rule for turning
supply_area_distance into a covered tile area is not a single formula - poles
come out as 2*distance while a beacon comes out as 2*distance + its own footprint,
and substation's collision box does not fit either reading. Encoding a guessed
formula here would produce a fixture that is confidently wrong and that drifts
in a way no test can see. Raw values have no such problem: if Factorio changes
supply_area_distance, the raw number changes and the diff is undeniable.
"""

import json
import os
import sys
from pathlib import Path

# Entities the planner names, places, or reasons about the size of.
# Keys are the prototype names as the game knows them.
WANTED_ENTITIES = [
    "pumpjack",
    "pipe",
    "pipe-to-ground",
    "small-electric-pole",
    "medium-electric-pole",
    "big-electric-pole",
    "substation",
    "beacon",
    "heat-pipe",
    "stone-wall",
]

# Prototype fields worth pinning. Anything absent on a given prototype is simply
# skipped, so this list can stay a single flat set rather than per-type tables.
WANTED_FIELDS = [
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
    "energy_usage",
]

# Deliberately NOT captured: the beacon "profile" array, which is how Factorio 2.0 models
# diminishing returns as more beacons reach the same machine. It is ~100 floats carrying
# binary representation noise (0.44719999999999993), so it would be a third of this file
# and would churn the diff for no reason.
#
# It is left out because the planner does not model diminishing returns at all - it scores
# a plan by counting beacon effects, as though every beacon contributed equally. That is a
# real gap, but it is a planner gap, not a drift-detection gap: pinning a number nothing
# reads would not catch anything. If the planner ever learns to score beacon profiles,
# capture this then.

# Fluid box keys worth keeping. Everything else on a pipe connection is graphics
# (pipe_covers alone is several hundred lines of sprite definitions per entity).
WANTED_CONNECTION_FIELDS = [
    "connection_type",
    "direction",
    "position",
    "positions",
    "flow_direction",
    "max_underground_distance",
]


def fail(message):
    print(f"trim-factorio-oracle: {message}", file=sys.stderr)
    sys.exit(1)


def env(name):
    value = os.environ.get(name)
    if not value:
        fail(f"missing required environment variable {name}")
    return value


def find_prototype(data_raw, name):
    """data.raw is keyed by prototype TYPE, not name, and the planner's entities are
    scattered across types you would not guess (pumpjack is a mining-drill, stone-wall
    is a wall). Searching every type is cheaper than maintaining a name->type table
    that silently rots when Factorio reclassifies something.

    The catch is that most of these names exist TWICE: once as the placeable entity and
    once as the item you carry. data.raw["item"]["pumpjack"] is a real prototype, it is
    just the wrong one, and it is missing every geometry field we care about. Preferring
    the candidate that has a collision_box picks the entity without needing a hardcoded
    name->type table."""
    candidates = [
        (category, prototypes[name])
        for category, prototypes in data_raw.items()
        if isinstance(prototypes, dict) and name in prototypes
    ]
    for category, prototype in candidates:
        if isinstance(prototype, dict) and "collision_box" in prototype:
            return category, prototype
    return candidates[0] if candidates else (None, None)


def trim_connections(fluid_box):
    connections = []
    for connection in fluid_box.get("pipe_connections", []):
        connections.append({k: connection[k] for k in WANTED_CONNECTION_FIELDS if k in connection})
    return connections


def trim_entity(category, prototype):
    trimmed = {"prototypeType": category}
    for field in WANTED_FIELDS:
        if field in prototype:
            trimmed[field] = prototype[field]

    for box_name in ("fluid_box", "output_fluid_box", "input_fluid_box"):
        box = prototype.get(box_name)
        if isinstance(box, dict):
            connections = trim_connections(box)
            if connections:
                trimmed[box_name] = {"pipe_connections": connections}
    return trimmed


def collect_directions(doc_dir):
    """defines.direction is the authoritative answer to 'what number means east'.
    Factorio 2.0 widened this from 8 to 16 values, which silently reinterpreted every
    direction in every blueprint."""
    api_path = Path(doc_dir) / "runtime-api.json"
    if not api_path.is_file():
        fail(f"expected runtime API at {api_path}")

    api = json.loads(api_path.read_text(encoding="utf-8"))
    for define in api.get("defines", []):
        if define.get("name") == "direction":
            return {v["name"]: v["order"] for v in define.get("values", [])}
    fail("could not find defines.direction in runtime-api.json")


def collect_renames(data_dir):
    """Every rename the game knows about, taken from its own migration files.

    This is the difference between 'I think effectivity-module was renamed' and
    knowing it, along with every other rename shipped in the same window.
    The .lua migrations are skipped: they are arbitrary code, not data.
    """
    renames = {}
    for migration in sorted(Path(data_dir).glob("*/migrations/*.json")):
        try:
            content = json.loads(migration.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        if not isinstance(content, dict):
            continue
        for category, pairs in content.items():
            if not isinstance(pairs, list):
                continue
            for pair in pairs:
                if isinstance(pair, list) and len(pair) == 2 and all(isinstance(x, str) for x in pair):
                    renames.setdefault(category, {})[pair[0]] = pair[1]
    return {category: dict(sorted(pairs.items())) for category, pairs in sorted(renames.items())}


def main():
    dump_path = env("DUMP")
    data_dir = env("DATA_DIR")
    doc_dir = env("DOC_DIR")
    out_path = Path(env("OUT"))

    data_raw = json.loads(Path(dump_path).read_text(encoding="utf-8"))

    entities = {}
    missing = []
    for name in WANTED_ENTITIES:
        category, prototype = find_prototype(data_raw, name)
        if prototype is None:
            missing.append(name)
            continue
        entities[name] = trim_entity(category, prototype)

    if missing:
        # A planner entity that no longer exists is exactly the failure this tool is
        # built to catch, so make it loud rather than writing a quietly incomplete file.
        fail(
            "these entities are named by the planner but do not exist in this Factorio "
            f"version: {', '.join(missing)}. That is a real finding - fix the planner, "
            "do not delete them from WANTED_ENTITIES."
        )

    modules = sorted(data_raw.get("module", {}).keys())

    fixture = {
        "_comment": (
            "Generated by tools/capture-factorio-oracle.sh. Do not hand-edit. "
            "Raw Factorio prototype values that the oil field planner depends on. "
            "Re-capture after a Factorio update and commit the diff."
        ),
        "captureInfo": {
            "factorioVersion": env("VERSION"),
            "loadedMods": sorted(env("LOADED_MODS").split()),
        },
        "directions": collect_directions(doc_dir),
        "entities": dict(sorted(entities.items())),
        "modules": modules,
        "renames": collect_renames(data_dir),
    }

    out_path.parent.mkdir(parents=True, exist_ok=True)
    # sort_keys plus a trailing newline keeps re-captures diffing cleanly instead of
    # reshuffling on every run.
    out_path.write_text(json.dumps(fixture, indent=2, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
