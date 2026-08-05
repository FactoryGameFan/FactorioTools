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

  it("has a default entry for every pipe strategy the artifact knows about", () => {
    for (const strategy of artifact.allPipeStrategies) {
      expect(Object.keys(PIPE_STRATEGY_DEFAULTS)).toContain(strategy)
    }
  })

  it("has a default entry for every beacon strategy the artifact knows about", () => {
    for (const strategy of artifact.allBeaconStrategies) {
      expect(Object.keys(BEACON_STRATEGY_DEFAULTS)).toContain(strategy)
    }
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
