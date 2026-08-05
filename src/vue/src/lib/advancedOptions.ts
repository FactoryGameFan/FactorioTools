import { getDefaults, type OilFieldStoreState } from "../stores/OilFieldStore"

// Turning advanced options back off hides the advanced controls with v-show,
// which is CSS only - the store values survive, keep going out in the plan
// request, keep getting persisted to localStorage, and keep riding along in
// share links. So every option the user can only reach in advanced mode has to
// be restored here when they switch back.
//
// The classification below covers the whole store, and the tests enumerate the
// store rather than a hand-written list, so a field added later lands in the
// advanced bucket by default. That way the mistake is a visible "my setting got
// reset" rather than an invisible setting that silently changes the plan.

/** Not user options at all - routing and mode state. */
export const INTERNAL_KEYS = ["usingQueryString", "useAdvancedOptions"] as const

/** Still visible and editable with advanced options off, so they are left alone. */
export const SIMPLE_OPTION_KEYS = [
  "inputBlueprint",
  "pumpjackModule",
  "addBeacons",
  "beaconModule",
  "addElectricPoles",
  "electricPoleEntityName",
  "addHeatPipes",
] as const

/**
 * Cleared by CustomizeSelect's own showAdvancedOptions watcher, which drops the
 * "Custom" selection when advanced options go away. It needs the mounted
 * <select> to know which values are allowed, so it cannot live here.
 */
export const COVERED_BY_CUSTOMIZE_SELECT = [
  "pumpjackModuleIsCustom",
  "beaconModuleIsCustom",
  "electricPoleIsCustom",
] as const

/**
 * The slice of the store a reset touches. Each function asks for only the keys
 * it writes, so a form component can pass itself in - the components hold their
 * store fields directly, via storeToRefs.
 */
export type AdvancedOptionState<K extends keyof OilFieldStoreState> = {
  -readonly [P in K]: OilFieldStoreState[P]
}

function restore<K extends keyof OilFieldStoreState>(state: AdvancedOptionState<K>, ...keys: K[]) {
  const defaults = getDefaults()
  for (const key of keys) {
    state[key] = defaults[key]
  }
}

export type PumpjackAdvancedOptions = AdvancedOptionState<
  "pumpjackQuality" | "pumpjackModuleQuality"
>

export function resetPumpjackAdvancedOptions(state: PumpjackAdvancedOptions) {
  restore(state, "pumpjackQuality", "pumpjackModuleQuality")
}

export type BeaconAdvancedOptions = AdvancedOptionState<
  | "beaconEntityName"
  | "beaconModuleSlots"
  | "beaconWidth"
  | "beaconHeight"
  | "beaconSupplyWidth"
  | "beaconSupplyHeight"
  | "overlapBeacons"
  | "beaconQuality"
  | "beaconModuleQuality"
>

export function resetBeaconAdvancedOptions(state: BeaconAdvancedOptions) {
  restore(
    state,
    "beaconEntityName",
    "beaconModuleSlots",
    "beaconWidth",
    "beaconHeight",
    "beaconSupplyWidth",
    "beaconSupplyHeight",
    "overlapBeacons",
    "beaconQuality",
    "beaconModuleQuality",
  )
}

export type PlannerAdvancedOptions = AdvancedOptionState<
  | "useUndergroundPipes"
  | "useStagingApi"
  | "optimizePipes"
  | "validateSolution"
  | "showProgress"
  | "pipeStrategyFbeOriginal"
  | "pipeStrategyFbe"
  | "pipeStrategyConnectedCentersDelaunay"
  | "pipeStrategyConnectedCentersDelaunayMst"
  | "pipeStrategyConnectedCentersFlute"
  | "beaconStrategyFbeOriginal"
  | "beaconStrategyFbe"
  | "beaconStrategySnug"
> & { autoPlan: boolean }

export function resetPlannerAdvancedOptions(state: PlannerAdvancedOptions) {
  restore(
    state,
    "useUndergroundPipes",
    "useStagingApi",
    "optimizePipes",
    "validateSolution",
    "showProgress",
    "pipeStrategyFbeOriginal",
    "pipeStrategyFbe",
    "pipeStrategyConnectedCentersDelaunay",
    "pipeStrategyConnectedCentersDelaunayMst",
    "pipeStrategyConnectedCentersFlute",
    "beaconStrategyFbeOriginal",
    "beaconStrategyFbe",
    "beaconStrategySnug",
  )
  // autoPlan lives in AutoPlanStore rather than OilFieldStore, but its checkbox
  // is inside the planner fieldset, which is advanced-only.
  state.autoPlan = false
}

export type ElectricPolePreset = {
  width: number
  height: number
  supplyWidth: number
  supplyHeight: number
  wireReach: number
}

// Mirrors the C# presets in OilFieldOptions.cs (ForSmallElectricPole and
// friends). Kept in sync by hand - see the "planner constants are hand-copied
// across C#, Lua and Vue" issue.
export const ELECTRIC_POLE_PRESETS: Record<string, ElectricPolePreset> = {
  "small-electric-pole": { width: 1, height: 1, supplyWidth: 5, supplyHeight: 5, wireReach: 7.5 },
  "medium-electric-pole": { width: 1, height: 1, supplyWidth: 7, supplyHeight: 7, wireReach: 9 },
  "big-electric-pole": { width: 2, height: 2, supplyWidth: 4, supplyHeight: 4, wireReach: 32 },
  substation: { width: 2, height: 2, supplyWidth: 18, supplyHeight: 18, wireReach: 18 },
}

export type ElectricPoleGeometry = AdvancedOptionState<
  | "electricPoleWidth"
  | "electricPoleHeight"
  | "electricPoleSupplyWidth"
  | "electricPoleSupplyHeight"
  | "electricPoleWireReach"
>

/**
 * Applies the geometry of a known pole. Returns false, leaving the geometry
 * untouched, when the name is a custom or modded one.
 */
export function setKnownElectricPole(
  state: ElectricPoleGeometry,
  electricPoleEntityName: string,
): boolean {
  const preset = ELECTRIC_POLE_PRESETS[electricPoleEntityName]
  if (!preset) {
    return false
  }

  state.electricPoleWidth = preset.width
  state.electricPoleHeight = preset.height
  state.electricPoleSupplyWidth = preset.supplyWidth
  state.electricPoleSupplyHeight = preset.supplyHeight
  state.electricPoleWireReach = preset.wireReach
  return true
}

export type ElectricPoleAdvancedOptions = ElectricPoleGeometry &
  AdvancedOptionState<"electricPoleEntityName" | "electricPoleQuality">

export function resetElectricPoleAdvancedOptions(state: ElectricPoleAdvancedOptions) {
  // The pole itself is a simple option, so the user's choice survives - the
  // geometry just goes back to that pole's stock numbers. A custom pole has no
  // stock numbers, so it falls back to the default pole, whose geometry the
  // entity-name watcher then applies.
  if (!setKnownElectricPole(state, state.electricPoleEntityName)) {
    restore(state, "electricPoleEntityName")
  }
  restore(state, "electricPoleQuality")
}
