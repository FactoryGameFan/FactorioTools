import { describe, expect, it } from "vitest"
import artifact from "./plannerDefaults.verified.json"
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
  it("carries the planner geometry", () => {
    expect(PLANNER_OPTION_DEFAULTS.electricPoleSupplyWidth).toBe(7)
    expect(PLANNER_OPTION_DEFAULTS.electricPoleWireReach).toBe(9)
    expect(PLANNER_OPTION_DEFAULTS.beaconSupplyWidth).toBe(9)
    expect(PLANNER_OPTION_DEFAULTS.beaconWidth).toBe(3)
  })

  it("carries all five pole presets, including the modded one", () => {
    expect(Object.keys(ELECTRIC_POLE_PRESETS).sort()).toEqual([
      "big-electric-pole",
      "medium-electric-pole",
      "small-electric-pole",
      "small-iron-electric-pole",
      "substation",
    ])
    expect(ELECTRIC_POLE_PRESETS["substation"]).toEqual({
      width: 2,
      height: 2,
      supplyWidth: 18,
      supplyHeight: 18,
      wireReach: 18,
    })
  })

  it("carries the quality levels, including the skipped level 4", () => {
    expect(QUALITY_LEVELS).toEqual({ Normal: 0, Uncommon: 1, Rare: 2, Epic: 3, Legendary: 5 })
  })

  // Deliberately hardcoded, not derived from the artifact: PIPE_STRATEGY_DEFAULTS and
  // BEACON_STRATEGY_DEFAULTS are built directly from artifact.allPipeStrategies /
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
  it("turns an enabled list into one boolean per strategy", () => {
    expect(strategyFlags(["A", "B", "C"], ["A", "C"])).toEqual({ A: true, B: false, C: true })
  })

  it("defaults every pipe strategy except FbeOriginal to on", () => {
    expect(PIPE_STRATEGY_DEFAULTS).toEqual({
      FbeOriginal: false,
      Fbe: true,
      ConnectedCentersDelaunay: true,
      ConnectedCentersDelaunayMst: true,
      ConnectedCentersFlute: true,
    })
  })

  it("defaults every beacon strategy except FbeOriginal to on", () => {
    expect(BEACON_STRATEGY_DEFAULTS).toEqual({ FbeOriginal: false, Fbe: true, Snug: true })
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
