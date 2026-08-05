import { describe, expect, it } from "vitest"
import artifact from "./plannerDefaults.verified.json"

// This file deliberately mixes two kinds of assertion against the artifact:
//
// - PINNED (exact value/list): reserved for a semantic contract - a fact about
//   Factorio, or a design decision someone must consciously review if it changes.
//   Examples: the quality levels skipping level 4 (Factorio's own gap), the full
//   pipe/beacon strategy universe (enum membership), which electric pole presets
//   exist. These must keep failing loudly on any drift.
// - SHAPE (presence / positivity / non-emptiness): reserved for pure tuning numbers
//   that the planner's author may retune at will - beacon/pole geometry, which
//   strategies happen to be enabled by default. The C# Verify snapshot
//   (plannerDefaults.verified.json's own generation test) already gates those
//   numbers; pinning them here too means every retune forces an unrelated Vue test
//   edit, which defeats the point of sharing the artifact.
//
// Do not tighten a shape check into a pinned value, or loosen a pinned value into a
// shape check, without re-reading this comment and deciding which kind it is.
//
// advancedOptions.test.ts pins the substation preset's geometry (18/18/18/2/2) exactly,
// which looks like a contradiction of the SHAPE rule above - it isn't. That test is
// pinning a specific preset's numbers as a regression check on the reset-to-preset
// behavior, not asserting a semantic contract about pole geometry in general. Don't
// "fix" either file to match the other.
import {
  BEACON_MODULE_DEFAULT,
  BEACON_STRATEGY_DEFAULTS,
  ELECTRIC_POLE_PRESETS,
  PIPE_STRATEGY_DEFAULTS,
  PLANNER_OPTION_DEFAULTS,
  PUMPJACK_MODULE_DEFAULT,
  QUALITY_LEVELS,
  moduleNameAndCount,
  strategyFlags,
} from "./plannerDefaults"
import { getDefaults } from "../stores/OilFieldStore"

describe("the emitted artifact", () => {
  // SHAPE: these are tuning numbers - the C# Verify snapshot already gates their exact
  // values, so this only checks the fields are present and physically sensible.
  it("carries the planner geometry", () => {
    const geometryFields = [
      "electricPoleSupplyWidth",
      "electricPoleSupplyHeight",
      "electricPoleWireReach",
      "electricPoleWidth",
      "electricPoleHeight",
      "beaconSupplyWidth",
      "beaconSupplyHeight",
      "beaconWidth",
      "beaconHeight",
    ] as const
    const options = PLANNER_OPTION_DEFAULTS as Record<string, unknown>
    for (const field of geometryFields) {
      expect(options, field).toHaveProperty(field)
      expect(options[field], field).toBeGreaterThan(0)
    }
  })

  // PINNED (which presets exist) + SHAPE (their geometry): the set of five preset keys,
  // including the modded small-iron-electric-pole, is a design decision worth pinning.
  // The geometry of each preset is a tuning number, so only its shape is checked.
  it("carries all five pole presets, including the modded one", () => {
    expect(Object.keys(ELECTRIC_POLE_PRESETS).sort()).toEqual([
      "big-electric-pole",
      "medium-electric-pole",
      "small-electric-pole",
      "small-iron-electric-pole",
      "substation",
    ])
    for (const [name, preset] of Object.entries(ELECTRIC_POLE_PRESETS)) {
      expect(preset.width, `${name}.width`).toBeGreaterThan(0)
      expect(preset.height, `${name}.height`).toBeGreaterThan(0)
      expect(preset.supplyWidth, `${name}.supplyWidth`).toBeGreaterThan(0)
      expect(preset.supplyHeight, `${name}.supplyHeight`).toBeGreaterThan(0)
      expect(preset.wireReach, `${name}.wireReach`).toBeGreaterThan(0)
    }
  })

  // PINNED: Legendary skipping level 4 is Factorio's own gap, not a planner tuning
  // choice. A change here means the game changed.
  it("carries the quality levels, including the skipped level 4", () => {
    expect(QUALITY_LEVELS).toEqual({ Normal: 0, Uncommon: 1, Rare: 2, Epic: 3, Legendary: 5 })
  })

  // PINNED, deliberately hardcoded, not derived from the artifact: PIPE_STRATEGY_DEFAULTS
  // and BEACON_STRATEGY_DEFAULTS are built directly from artifact.allPipeStrategies /
  // artifact.allBeaconStrategies (see strategyFlags below), so comparing them back
  // against the same artifact fields would be true by construction and could never
  // catch a missing enum member. A hardcoded list in a test is not the bug the
  // hardcoded list in production code was - here it's the point: if a C# enum grows a
  // member, this expectation fails loudly and forces a human to update it. Do not
  // "fix" this by deriving the expected value from the artifact.
  it("carries the full strategy universe from the C# enums", () => {
    expect(artifact.allPipeStrategies).toEqual([
      "FbeOriginal",
      "Fbe",
      "ConnectedCentersDelaunay",
      "ConnectedCentersDelaunayMst",
      "ConnectedCentersFlute",
    ])
    expect(artifact.allBeaconStrategies).toEqual(["FbeOriginal", "Fbe", "Snug"])
  })
})

describe("strategyFlags", () => {
  // PINNED: exercises the function's own logic with inputs it owns, not the artifact.
  it("turns an enabled list into one boolean per strategy", () => {
    expect(strategyFlags(["A", "B", "C"], ["A", "C"])).toEqual({ A: true, B: false, C: true })
  })

  // SHAPE: which strategies are enabled by default is a tuning choice (see
  // OilFieldOptions.DefaultPipeStrategies / DefaultBeaconStrategies in C#), so this
  // does not pin the exact set. It only asserts the defaults aren't vacuous.
  //
  // Note: an assertion of the form "every strategy in the universe has a boolean
  // entry" is deliberately NOT included here - PIPE_STRATEGY_DEFAULTS /
  // BEACON_STRATEGY_DEFAULTS are built by strategyFlags from
  // artifact.allPipeStrategies / artifact.allBeaconStrategies directly, so that would
  // be true by construction and could never fail. The strategyFlags unit test above
  // and "carries the full strategy universe from the C# enums" above already cover
  // that ground.
  it("enables at least one pipe strategy by default", () => {
    expect(Object.values(PIPE_STRATEGY_DEFAULTS).some(Boolean)).toBe(true)
  })

  it("enables at least one beacon strategy by default", () => {
    expect(Object.values(BEACON_STRATEGY_DEFAULTS).some(Boolean)).toBe(true)
  })
})

describe("moduleNameAndCount", () => {
  it("reads the single module entry", () => {
    expect(moduleNameAndCount({ "speed-module-3": 2 })).toEqual({
      name: "speed-module-3",
      count: 2,
    })
  })

  it("treats an empty dictionary as no module", () => {
    expect(moduleNameAndCount({})).toEqual({ name: "", count: 0 })
  })

  it("exposes the pumpjack and beacon defaults from the artifact", () => {
    expect(PUMPJACK_MODULE_DEFAULT).toEqual({ name: "productivity-module-3", count: 2 })
    expect(BEACON_MODULE_DEFAULT).toEqual({ name: "speed-module-3", count: 2 })
  })
})

// Every store key must be classified into exactly one of these three groups. That is
// the point of the test: a planner option added later lands in none of them and fails
// here, so it cannot quietly become a TypeScript literal. Same enumerate-the-store
// approach as advancedOptions.test.ts.

/** Store keys with no counterpart in the C# planner options. */
const VUE_ONLY_KEYS = [
  "usingQueryString",
  "useStagingApi",
  "inputBlueprint",
  "useAdvancedOptions",
  "pumpjackModuleIsCustom",
  "beaconModuleIsCustom",
  "electricPoleIsCustom",
  "showProgress",
] as const

/** Store keys whose name matches an artifact option key, and whose value must equal it. */
const NAME_MATCHED_KEYS = [
  "addBeacons",
  "addElectricPoles",
  "addHeatPipes",
  "overlapBeacons",
  "beaconEntityName",
  "beaconSupplyWidth",
  "beaconSupplyHeight",
  "beaconWidth",
  "beaconHeight",
  "electricPoleEntityName",
  "electricPoleWidth",
  "electricPoleHeight",
  "electricPoleSupplyWidth",
  "electricPoleSupplyHeight",
  "electricPoleWireReach",
  "useUndergroundPipes",
  "optimizePipes",
  "validateSolution",
  "pumpjackQuality",
  "pumpjackModuleQuality",
  "beaconQuality",
  "beaconModuleQuality",
  "electricPoleQuality",
] as const

/** Store keys derived from the artifact through a conversion, checked by their own tests below. */
const CONVERTED_KEYS = [
  "pumpjackModule",
  "beaconModule",
  "beaconModuleSlots",
  "pipeStrategyFbeOriginal",
  "pipeStrategyFbe",
  "pipeStrategyConnectedCentersDelaunay",
  "pipeStrategyConnectedCentersDelaunayMst",
  "pipeStrategyConnectedCentersFlute",
  "beaconStrategyFbeOriginal",
  "beaconStrategyFbe",
  "beaconStrategySnug",
] as const

describe("store defaults", () => {
  it("classifies every store key exactly once", () => {
    const classified: string[] = [...VUE_ONLY_KEYS, ...NAME_MATCHED_KEYS, ...CONVERTED_KEYS]
    expect(new Set(classified).size, "a key is classified twice").toBe(classified.length)
    // Both directions: no unclassified store key, and no classification for a key that
    // no longer exists. An unclassified key is a planner option nobody wired up.
    expect(classified.sort()).toEqual(Object.keys(getDefaults()).sort())

    // A key classified as VUE_ONLY or CONVERTED must not also be a literal artifact
    // option name - otherwise the store could hardcode a value under that name (see
    // NAME_MATCHED_KEYS above) while claiming it has no C# counterpart, and this test
    // would still pass. This is what let beaconSupplyWidth silently diverge from the
    // C# default before it was caught: moving it into VUE_ONLY_KEYS and hardcoding a
    // stale value in the store left every other assertion here green.
    for (const key of [...VUE_ONLY_KEYS, ...CONVERTED_KEYS]) {
      expect(
        PLANNER_OPTION_DEFAULTS,
        `${key} shares a name with an artifact option`,
      ).not.toHaveProperty(key)
    }
  })

  it("takes every name-matched value from the artifact", () => {
    const defaults = getDefaults() as Record<string, unknown>
    const options = PLANNER_OPTION_DEFAULTS as Record<string, unknown>

    const mismatched: string[] = []
    for (const key of NAME_MATCHED_KEYS) {
      expect(options, `${key} is missing from the artifact`).toHaveProperty(key)
      if (options[key] !== defaults[key]) {
        mismatched.push(key)
      }
    }
    expect(mismatched).toEqual([])
  })

  it("derives the strategy booleans from the artifact", () => {
    const defaults = getDefaults()
    expect(defaults.pipeStrategyFbeOriginal).toBe(PIPE_STRATEGY_DEFAULTS.FbeOriginal)
    expect(defaults.pipeStrategyFbe).toBe(PIPE_STRATEGY_DEFAULTS.Fbe)
    expect(defaults.pipeStrategyConnectedCentersDelaunay).toBe(
      PIPE_STRATEGY_DEFAULTS.ConnectedCentersDelaunay,
    )
    expect(defaults.pipeStrategyConnectedCentersDelaunayMst).toBe(
      PIPE_STRATEGY_DEFAULTS.ConnectedCentersDelaunayMst,
    )
    expect(defaults.pipeStrategyConnectedCentersFlute).toBe(
      PIPE_STRATEGY_DEFAULTS.ConnectedCentersFlute,
    )
    expect(defaults.beaconStrategyFbeOriginal).toBe(BEACON_STRATEGY_DEFAULTS.FbeOriginal)
    expect(defaults.beaconStrategyFbe).toBe(BEACON_STRATEGY_DEFAULTS.Fbe)
    expect(defaults.beaconStrategySnug).toBe(BEACON_STRATEGY_DEFAULTS.Snug)
  })

  it("derives the module selections from the artifact", () => {
    const defaults = getDefaults()
    expect(defaults.pumpjackModule).toBe(PUMPJACK_MODULE_DEFAULT.name)
    expect(defaults.beaconModule).toBe(BEACON_MODULE_DEFAULT.name)
    expect(defaults.beaconModuleSlots).toBe(BEACON_MODULE_DEFAULT.count)
  })
})
