import { defineStore, Store } from "pinia"
import { StorageLike } from "pinia-plugin-persistedstate"
import { LocationQuery } from "vue-router"
import { getEntries } from "../lib/helpers"
import {
  BEACON_MODULE_DEFAULT,
  BEACON_STRATEGY_DEFAULTS,
  PIPE_STRATEGY_DEFAULTS,
  PLANNER_OPTION_DEFAULTS,
  PUMPJACK_MODULE_DEFAULT,
} from "../lib/plannerDefaults"

// Values that mirror the planner come from plannerDefaults.verified.json, which the Rust
// test rust/contract/tests/planner_defaults.rs generates from OilFieldOptions (run it with
// UPDATE_PLANNER_DEFAULTS=1). Do not retype them here: the Vue app sends these explicitly
// in the plan request, so a stale copy would override the correct planner default and
// produce a wrong blueprint with a green test suite. See issue #54.
const defaults = {
  // Vue-only state, with no counterpart in the planner options.
  usingQueryString: false,
  inputBlueprint: "",
  useAdvancedOptions: false,
  pumpjackModuleIsCustom: false,
  beaconModuleIsCustom: false,
  electricPoleIsCustom: false,
  showProgress: false,

  // Derived from the planner.
  pumpjackModule: PUMPJACK_MODULE_DEFAULT.name,
  beaconModule: BEACON_MODULE_DEFAULT.name,
  beaconModuleSlots: BEACON_MODULE_DEFAULT.count,
  addBeacons: PLANNER_OPTION_DEFAULTS.addBeacons,
  addElectricPoles: PLANNER_OPTION_DEFAULTS.addElectricPoles,
  overlapBeacons: PLANNER_OPTION_DEFAULTS.overlapBeacons,
  beaconEntityName: PLANNER_OPTION_DEFAULTS.beaconEntityName,
  beaconSupplyWidth: PLANNER_OPTION_DEFAULTS.beaconSupplyWidth,
  beaconSupplyHeight: PLANNER_OPTION_DEFAULTS.beaconSupplyHeight,
  beaconWidth: PLANNER_OPTION_DEFAULTS.beaconWidth,
  beaconHeight: PLANNER_OPTION_DEFAULTS.beaconHeight,
  electricPoleEntityName: PLANNER_OPTION_DEFAULTS.electricPoleEntityName,
  electricPoleWidth: PLANNER_OPTION_DEFAULTS.electricPoleWidth,
  electricPoleHeight: PLANNER_OPTION_DEFAULTS.electricPoleHeight,
  electricPoleSupplyWidth: PLANNER_OPTION_DEFAULTS.electricPoleSupplyWidth,
  electricPoleSupplyHeight: PLANNER_OPTION_DEFAULTS.electricPoleSupplyHeight,
  electricPoleWireReach: PLANNER_OPTION_DEFAULTS.electricPoleWireReach,
  useUndergroundPipes: PLANNER_OPTION_DEFAULTS.useUndergroundPipes,
  optimizePipes: PLANNER_OPTION_DEFAULTS.optimizePipes,
  validateSolution: PLANNER_OPTION_DEFAULTS.validateSolution,
  pipeStrategyFbeOriginal: PIPE_STRATEGY_DEFAULTS.FbeOriginal,
  pipeStrategyFbe: PIPE_STRATEGY_DEFAULTS.Fbe,
  pipeStrategyConnectedCentersDelaunay: PIPE_STRATEGY_DEFAULTS.ConnectedCentersDelaunay,
  pipeStrategyConnectedCentersDelaunayMst: PIPE_STRATEGY_DEFAULTS.ConnectedCentersDelaunayMst,
  pipeStrategyConnectedCentersFlute: PIPE_STRATEGY_DEFAULTS.ConnectedCentersFlute,
  beaconStrategyFbeOriginal: BEACON_STRATEGY_DEFAULTS.FbeOriginal,
  beaconStrategyFbe: BEACON_STRATEGY_DEFAULTS.Fbe,
  beaconStrategySnug: BEACON_STRATEGY_DEFAULTS.Snug,
  addHeatPipes: PLANNER_OPTION_DEFAULTS.addHeatPipes,
  pumpjackQuality: PLANNER_OPTION_DEFAULTS.pumpjackQuality,
  pumpjackModuleQuality: PLANNER_OPTION_DEFAULTS.pumpjackModuleQuality,
  beaconQuality: PLANNER_OPTION_DEFAULTS.beaconQuality,
  beaconModuleQuality: PLANNER_OPTION_DEFAULTS.beaconModuleQuality,
  electricPoleQuality: PLANNER_OPTION_DEFAULTS.electricPoleQuality,
}
export type OilFieldStoreState = typeof defaults

type StoreToQuery = {
  [Property in keyof OilFieldStoreState as Exclude<Property, "usingQueryString">]: string
}

const storeToQuery: StoreToQuery = {
  inputBlueprint: "source",
  useAdvancedOptions: "adv",
  pumpjackModule: "pumpMod",
  pumpjackModuleIsCustom: "pumpModCust",
  addBeacons: "beacons",
  addElectricPoles: "poles",
  overlapBeacons: "overlapBeacons",
  beaconModule: "beaconMod",
  beaconModuleIsCustom: "beaconModCust",
  beaconModuleSlots: "beaconModSlots",
  beaconEntityName: "beacon",
  beaconSupplyWidth: "beaconSupW",
  beaconSupplyHeight: "beaconSupH",
  beaconWidth: "beaconW",
  beaconHeight: "beaconH",
  electricPoleEntityName: "pole",
  electricPoleIsCustom: "poleCust",
  electricPoleWidth: "poleW",
  electricPoleHeight: "poleH",
  electricPoleSupplyWidth: "poleSupW",
  electricPoleSupplyHeight: "poleSupH",
  electricPoleWireReach: "poleReach",
  useUndergroundPipes: "underground",
  optimizePipes: "optimize",
  validateSolution: "val",
  pipeStrategyFbeOriginal: "pipesFbeO",
  pipeStrategyFbe: "pipesFbe",
  pipeStrategyConnectedCentersDelaunay: "pipesCcDt",
  pipeStrategyConnectedCentersDelaunayMst: "pipesCcDtMst",
  pipeStrategyConnectedCentersFlute: "pipesCcFlute",
  beaconStrategyFbeOriginal: "beaconsFbeO",
  beaconStrategyFbe: "beaconsFbe",
  beaconStrategySnug: "beaconsSnug",
  addHeatPipes: "heatPipes",
  showProgress: "progress",
  pumpjackQuality: "pumpQ",
  pumpjackModuleQuality: "pumpModQ",
  beaconQuality: "beaconQ",
  beaconModuleQuality: "beaconModQ",
  electricPoleQuality: "poleQ",
} as const

// Factorio 2.0 renamed the efficiency modules. Anyone who picked one before this fix
// has the dead name in localStorage, or in a shared query-string link (storeToQuery
// maps pumpjackModule/beaconModule into the URL), and would keep sending it to the
// planner forever, so rewrite on load rather than only fixing the dropdown.
const RENAMED_MODULES: Record<string, string> = {
  "effectivity-module": "efficiency-module",
  "effectivity-module-2": "efficiency-module-2",
  "effectivity-module-3": "efficiency-module-3",
}

export function migrateModuleNames<T extends Record<string, unknown>>(state: T): T {
  for (const key of Object.keys(state)) {
    const value = state[key]
    if (typeof value === "string" && value in RENAMED_MODULES) {
      ;(state as Record<string, unknown>)[key] = RENAMED_MODULES[value]
    }
  }
  return state
}

type OilFieldStore = Store<"OilFieldStore", OilFieldStoreState>

class ToggleStorage implements StorageLike {
  private readOnly: boolean = false

  getItem(key: string): string | null {
    return localStorage.getItem(key)
  }
  setItem(key: string, value: string): void {
    if (!this.readOnly) {
      localStorage.setItem(key, value)
    }
  }
  setReadOnly(readOnly: boolean) {
    this.readOnly = readOnly
  }
}

const toggleStorage = new ToggleStorage()

function getStore(): OilFieldStore {
  const store = defineStore("OilFieldStore", {
    state: () => Object.assign({}, defaults),
    persist: {
      storage: toggleStorage,
      // Rewrites dead module names hydrated from localStorage. This does not cover
      // the query-string load path (populateStoreFromQuery below), which never
      // hydrates and needs its own call.
      afterHydrate: (ctx) => {
        migrateModuleNames(ctx.store.$state as Record<string, unknown>)
      },
    },
  })()
  return store
}

export function hasMatchingQueryString(
  query: LocationQuery | URLSearchParams,
  writeLog: boolean = true,
) {
  const keys = query instanceof URLSearchParams ? Array.from(query.keys()) : Object.keys(query)
  let matching = 0
  if (keys.length > 0) {
    for (const [_, queryKey] of getEntries(storeToQuery)) {
      if (keys.includes(queryKey)) {
        matching++
      }
    }

    if (writeLog) {
      console.log(`matched ${matching} query params, ignored ${keys.length - matching}`)
    }
  }
  return matching > 0
}

/**
 * The settings the query string carries (`source`, `adv`, `pumpMod` and the rest),
 * read from the URL fragment instead: `#source=...`. A browser never sends the
 * fragment to the server, so a blueprint handed over this way stays on the
 * visitor's machine, which the query string cannot promise. Factorio Blueprint
 * Editor's G key links here like that. The fragment also has no server-side
 * length limit, where Cloudflare refuses a URL over 16 KB.
 *
 * `hash` must be the raw fragment, still percent-encoded. A blueprint is base64,
 * so its `+` arrives as `%2B`, and decoding it before `URLSearchParams` does would
 * leave a bare `+` that it reads as a space. A key given twice comes back as an
 * array, the shape Vue Router gives a repeated query key, so the first one wins
 * in either place.
 *
 * Returns undefined when the fragment names none of the store's keys, so an
 * ordinary `#anchor` is ignored and the query string is read as before.
 */
export function queryFromFragment(hash: string): LocationQuery | undefined {
  if (!hash.startsWith("#") || hash.length < 2) {
    return undefined
  }
  const params = new URLSearchParams(hash.slice(1))
  if (!hasMatchingQueryString(params, false)) {
    return undefined
  }
  const query: LocationQuery = {}
  for (const key of new Set(params.keys())) {
    const values = params.getAll(key)
    query[key] = values.length === 1 ? values[0] : values
  }
  return query
}

/**
 * The settings to start the oil field page with: the fragment's when it carries
 * any (see queryFromFragment), otherwise the query string's.
 *
 * Vue Router has already decoded `to.hash`, which turns a blueprint's `%2B` into
 * `+` and so into a space. The raw fragment is read from the address bar
 * (`locationHash`) instead. The two agree on the navigation that matters, the
 * page load a link starts; `to.hash` only says whether there is one.
 */
export function initialQuery(
  to: { hash: string; query: LocationQuery },
  locationHash: string = globalThis.location?.hash ?? "",
): LocationQuery {
  return (to.hash ? queryFromFragment(locationHash) : undefined) ?? to.query
}

function populateStoreFromQuery(query: LocationQuery) {
  const store = useOilFieldStore()
  for (const [storeKey, storeValue] of getEntries(store.$state)) {
    if (storeKey == "usingQueryString") {
      continue
    }

    const queryKey = storeToQuery[storeKey]
    let queryValue = query[queryKey]
    if (Array.isArray(queryValue)) {
      queryValue = queryValue.length > 0 ? queryValue[0] : null
    }

    let newValue = queryValue ?? defaults[storeKey]
    switch (typeof storeValue) {
      case "boolean":
        newValue = newValue == "true" || newValue == "1"
        break
      case "number":
        newValue = parseFloat(newValue?.toString())
        break
    }

    ;(store as unknown as Record<string, unknown>)[storeKey] = newValue
  }

  // Module names travel in the query string too (pumpjackModule -> pumpMod,
  // beaconModule -> beaconMod), and this path never hydrates from storage, so it
  // never runs the afterHydrate migration above. Migrate explicitly here as well.
  migrateModuleNames(store.$state as unknown as Record<string, unknown>)

  return store
}

export function getDefaults(): Readonly<OilFieldStoreState> {
  return defaults
}

export function setReadOnly(readOnly: boolean) {
  toggleStorage.setReadOnly(readOnly)
}

export function initializeOilFieldStore(query: LocationQuery) {
  if (hasMatchingQueryString(query)) {
    console.log("initializing read-only store from query")
    toggleStorage.setReadOnly(true)
    const store = populateStoreFromQuery(query)
    store.usingQueryString = true
  } else {
    console.log("initializing store from local storage")
    getStore().usingQueryString = false
  }
}

export function generateQueryString() {
  const store = useOilFieldStore()
  const pieces = []
  for (const [storeKey, storeValue] of getEntries(store.$state)) {
    if (storeKey == "usingQueryString") {
      continue
    }

    const queryKey = storeToQuery[storeKey]
    if (queryKey) {
      let queryValue = storeValue
      if (typeof queryValue == "boolean") {
        queryValue = queryValue ? "1" : "0"
      }
      pieces.push(`${queryKey}=${encodeURIComponent(queryValue)}`)
    }
  }

  return pieces.join("&")
}

export function persistStore() {
  const store = getStore()
  if (!store.usingQueryString) {
    throw new Error("cannot persist from a store that is not using a query string")
  }

  toggleStorage.setReadOnly(false)
  store.usingQueryString = false
  store.$persist()
  toggleStorage.setReadOnly(true)
  store.usingQueryString = true
}

export function useOilFieldStore() {
  return getStore()
}
