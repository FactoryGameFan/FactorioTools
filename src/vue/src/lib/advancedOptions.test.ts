import { describe, expect, it } from "vitest"
import {
  COVERED_BY_CUSTOMIZE_SELECT,
  ELECTRIC_POLE_PRESETS,
  INTERNAL_KEYS,
  SIMPLE_OPTION_KEYS,
  resetBeaconAdvancedOptions,
  resetElectricPoleAdvancedOptions,
  resetPlannerAdvancedOptions,
  resetPumpjackAdvancedOptions,
  setKnownElectricPole,
} from "./advancedOptions"
import { getDefaults } from "../stores/OilFieldStore"

// A value guaranteed to differ from the default, so "was it reset?" is a real
// question for every key rather than an accident of the default's value.
function dirty<T>(value: T): T {
  switch (typeof value) {
    case "boolean":
      return !value as T
    case "number":
      return (value + 1234) as T
    default:
      return ("dirty-" + String(value)) as T
  }
}

/** Every option set to something other than its default, as if the user had. */
function dirtyState() {
  const state = { ...getDefaults(), autoPlan: true }
  const view = state as Record<string, unknown>
  for (const key of Object.keys(state)) {
    view[key] = dirty(view[key])
  }
  return state
}

function resetAll(state: ReturnType<typeof dirtyState>) {
  resetPumpjackAdvancedOptions(state)
  resetBeaconAdvancedOptions(state)
  resetElectricPoleAdvancedOptions(state)
  resetPlannerAdvancedOptions(state)
}

function asRecord(state: object) {
  return state as Record<string, unknown>
}

describe("resetting advanced options", () => {
  // The bug this guards against: quality was added to the store and to the
  // forms, but not to the per-form reset lists, so a hidden Legendary
  // electricPoleQuality kept changing the plan after the user had switched back
  // to simple options. Enumerating from the store rather than from a list of
  // keys means a field added later is covered here whether or not anyone
  // remembers this file.
  it("restores every advanced-only key to its default", () => {
    const state = dirtyState()
    // The entity name is a simple option, so the user's choice survives the
    // toggle - but the reset re-derives the pole geometry from it, so it has to
    // be a real entity name rather than the dirtied string.
    state.electricPoleEntityName = getDefaults().electricPoleEntityName

    resetAll(state)

    const defaults = asRecord(getDefaults())
    const stillDirty = Object.keys(defaults).filter(
      (key) =>
        !(SIMPLE_OPTION_KEYS as readonly string[]).includes(key) &&
        !(INTERNAL_KEYS as readonly string[]).includes(key) &&
        !(COVERED_BY_CUSTOMIZE_SELECT as readonly string[]).includes(key) &&
        asRecord(state)[key] !== defaults[key],
    )
    expect(stillDirty).toEqual([])
  })

  it("resets the quality of every entity that has one", () => {
    const state = dirtyState()
    state.electricPoleEntityName = getDefaults().electricPoleEntityName

    resetAll(state)

    expect(state.pumpjackQuality).toBe("Normal")
    expect(state.pumpjackModuleQuality).toBe("Normal")
    expect(state.beaconQuality).toBe("Normal")
    expect(state.beaconModuleQuality).toBe("Normal")
    expect(state.electricPoleQuality).toBe("Normal")
  })

  it("resets autoPlan, which is only reachable in advanced options", () => {
    const state = dirtyState()
    resetPlannerAdvancedOptions(state)
    expect(state.autoPlan).toBe(false)
  })

  it("leaves the simple options alone", () => {
    const state = dirtyState()
    // A known pole, but not the default one: the user's pick has to survive the
    // toggle. Only a *custom* name is replaced, because the Custom option in the
    // dropdown is itself advanced-only.
    state.electricPoleEntityName = "substation"
    const before = asRecord({ ...state })

    resetAll(state)

    for (const key of SIMPLE_OPTION_KEYS) {
      expect(asRecord(state)[key], key).toBe(before[key])
    }
  })

  it("classifies every store key exactly once", () => {
    const classified: string[] = [
      ...SIMPLE_OPTION_KEYS,
      ...INTERNAL_KEYS,
      ...COVERED_BY_CUSTOMIZE_SELECT,
    ]
    expect(new Set(classified).size, "a key is classified twice").toBe(classified.length)
    for (const key of classified) {
      expect(Object.keys(getDefaults()), key).toContain(key)
    }
  })
})

describe("electric pole presets", () => {
  it("restores the geometry of the selected pole rather than the default pole", () => {
    const state = dirtyState()
    state.electricPoleEntityName = "substation"

    resetElectricPoleAdvancedOptions(state)

    expect(state.electricPoleEntityName).toBe("substation")
    expect(state.electricPoleSupplyWidth).toBe(18)
    expect(state.electricPoleSupplyHeight).toBe(18)
    expect(state.electricPoleWireReach).toBe(18)
    expect(state.electricPoleWidth).toBe(2)
    expect(state.electricPoleHeight).toBe(2)
  })

  it("falls back to the default pole when the name is a custom one", () => {
    const state = dirtyState()
    state.electricPoleEntityName = "some-modded-pole"

    resetElectricPoleAdvancedOptions(state)

    expect(state.electricPoleEntityName).toBe(getDefaults().electricPoleEntityName)
  })

  it("reports whether the name was a known pole", () => {
    const state = dirtyState()
    expect(setKnownElectricPole(state, "medium-electric-pole")).toBe(true)
    expect(setKnownElectricPole(state, "some-modded-pole")).toBe(false)
  })

  it("agrees with the store defaults for the default pole", () => {
    const defaults = getDefaults()
    const preset = ELECTRIC_POLE_PRESETS[defaults.electricPoleEntityName]
    expect(preset).toBeDefined()
    expect(preset.supplyWidth).toBe(defaults.electricPoleSupplyWidth)
    expect(preset.supplyHeight).toBe(defaults.electricPoleSupplyHeight)
    expect(preset.wireReach).toBe(defaults.electricPoleWireReach)
    expect(preset.width).toBe(defaults.electricPoleWidth)
    expect(preset.height).toBe(defaults.electricPoleHeight)
  })
})
