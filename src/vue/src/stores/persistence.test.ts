import { beforeEach, afterEach, describe, expect, it, vi } from "vitest"
import { createPinia, setActivePinia } from "pinia"
import piniaPluginPersistedstate from "pinia-plugin-persistedstate"
import { createApp, nextTick } from "vue"
import {
  getDefaults,
  hasMatchingQueryString,
  initializeOilFieldStore,
  persistStore,
  setReadOnly,
  useOilFieldStore,
} from "./OilFieldStore"
import { useAutoPlanStore } from "./AutoPlanStore"

// The vitest environment is "node" (see vitest.config.ts), so there is no
// localStorage. Persistence is the whole point of these tests, so stub a minimal
// in-memory Storage rather than pulling in jsdom as a dependency. setItem calls
// are recorded so the read-only gate can be asserted on writes, not just on
// resulting state.
class MemoryStorage {
  private readonly entries = new Map<string, string>()
  writes: { key: string; value: string }[] = []

  getItem(key: string): string | null {
    return this.entries.has(key) ? this.entries.get(key)! : null
  }
  setItem(key: string, value: string): void {
    this.entries.set(key, value)
    this.writes.push({ key, value })
  }
  removeItem(key: string): void {
    this.entries.delete(key)
  }
  clear(): void {
    this.entries.clear()
    this.writes = []
  }
  key(index: number): string | null {
    return Array.from(this.entries.keys())[index] ?? null
  }
  get length(): number {
    return this.entries.size
  }
}

let storage: MemoryStorage

// pinia-plugin-persistedstate reads window.localStorage for its default storage,
// and OilFieldStore's ToggleStorage delegates to the localStorage global, so both
// need to point at the stub.
function installStorage() {
  storage = new MemoryStorage()
  vi.stubGlobal("localStorage", storage)
  vi.stubGlobal("window", { localStorage: storage })
}

// The stores persist through a pinia plugin, so a bare createPinia() (as used by
// OilFieldPlanner.test.ts) silently makes `persist` a no-op and leaves $persist
// undefined. These tests need the real plugin installed.
//
// The createApp({}).use(pinia) step is load-bearing and easy to get wrong.
// pinia.use() parks a plugin in an internal `toBeInstalled` queue while no Vue app
// has installed the pinia instance, and only promotes it to the live plugin list
// from install(app). Call pinia.use() without an app and the plugin never runs:
// $persist stays undefined and every `persist` option is silently ignored, so
// persistence tests would pass while testing nothing.
function freshPinia() {
  const pinia = createPinia()
  createApp({}).use(pinia)
  pinia.use(piniaPluginPersistedstate)
  setActivePinia(pinia)
}

/** Writes a persisted payload for `storeId` as if a previous session had saved it. */
function seedPersisted(storeId: string, state: Record<string, unknown>) {
  storage.setItem(storeId, JSON.stringify(state))
  storage.writes = []
}

describe("OilFieldStore persistence", () => {
  beforeEach(() => {
    installStorage()
    freshPinia()
    // toggleStorage is a module-level singleton, so its read-only flag survives
    // across tests. Reset it explicitly or ordering changes the results.
    setReadOnly(false)
    vi.spyOn(console, "log").mockImplementation(() => {})
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it("writes store mutations to storage when not read-only", async () => {
    const store = useOilFieldStore()

    store.inputBlueprint = "0eNrtvVBP"
    await nextTick()

    expect(storage.writes.some((w) => w.key === "OilFieldStore")).toBe(true)
    expect(JSON.parse(storage.getItem("OilFieldStore")!).inputBlueprint).toBe("0eNrtvVBP")
  })

  it("suppresses writes while read-only, without blocking reads", async () => {
    const store = useOilFieldStore()
    setReadOnly(true)

    store.inputBlueprint = "SHOULD_NOT_PERSIST"
    await nextTick()

    expect(storage.writes).toHaveLength(0)
    expect(storage.getItem("OilFieldStore")).toBeNull()
    // The in-memory store still holds the value; only the write-through is gated.
    expect(store.inputBlueprint).toBe("SHOULD_NOT_PERSIST")
  })

  it("resumes writing once read-only is lifted", async () => {
    const store = useOilFieldStore()

    setReadOnly(true)
    store.inputBlueprint = "DROPPED"
    await nextTick()
    expect(storage.writes).toHaveLength(0)

    setReadOnly(false)
    store.inputBlueprint = "KEPT"
    await nextTick()

    expect(JSON.parse(storage.getItem("OilFieldStore")!).inputBlueprint).toBe("KEPT")
  })

  it("hydrates a new store instance from previously persisted state", () => {
    seedPersisted("OilFieldStore", { inputBlueprint: "FROM_STORAGE", beaconModuleSlots: 4 })

    // A fresh pinia is a fresh store instance - the plugin rehydrates on creation.
    freshPinia()
    const store = useOilFieldStore()

    expect(store.inputBlueprint).toBe("FROM_STORAGE")
    expect(store.beaconModuleSlots).toBe(4)
    // Untouched keys keep their defaults.
    expect(store.addBeacons).toBe(getDefaults().addBeacons)
  })
})

describe("initializeOilFieldStore", () => {
  beforeEach(() => {
    installStorage()
    freshPinia()
    setReadOnly(false)
    vi.spyOn(console, "log").mockImplementation(() => {})
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it("populates from the query string and marks the store read-only", async () => {
    initializeOilFieldStore({
      source: "QUERY_BP",
      beacons: "1",
      poles: "0",
      beaconModSlots: "4",
      pumpMod: "speed-module-3",
    })

    const store = useOilFieldStore()
    expect(store.usingQueryString).toBe(true)
    expect(store.inputBlueprint).toBe("QUERY_BP")
    // Booleans coerce from "1"/"0", numbers through parseFloat, strings pass through.
    expect(store.addBeacons).toBe(true)
    expect(store.addElectricPoles).toBe(false)
    expect(store.beaconModuleSlots).toBe(4)
    expect(store.pumpjackModule).toBe("speed-module-3")
    // Keys absent from the query fall back to defaults.
    expect(store.overlapBeacons).toBe(getDefaults().overlapBeacons)

    // A query-string session must not overwrite the user's saved settings.
    store.inputBlueprint = "STILL_NOT_PERSISTED"
    await nextTick()
    expect(storage.getItem("OilFieldStore")).toBeNull()
  })

  it("falls back to persisted state when the query has no recognised keys", async () => {
    seedPersisted("OilFieldStore", { inputBlueprint: "FROM_STORAGE" })
    freshPinia()

    initializeOilFieldStore({ utm_source: "newsletter", ref: "somewhere" })

    const store = useOilFieldStore()
    expect(store.usingQueryString).toBe(false)
    expect(store.inputBlueprint).toBe("FROM_STORAGE")

    // Not read-only in this path, so edits persist as normal.
    store.inputBlueprint = "EDITED"
    await nextTick()
    expect(JSON.parse(storage.getItem("OilFieldStore")!).inputBlueprint).toBe("EDITED")
  })
})

describe("persistStore", () => {
  beforeEach(() => {
    installStorage()
    freshPinia()
    setReadOnly(false)
    vi.spyOn(console, "log").mockImplementation(() => {})
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it("refuses to persist a store that is not using a query string", () => {
    useOilFieldStore()

    expect(() => persistStore()).toThrow(
      "cannot persist from a store that is not using a query string",
    )
  })

  it("writes the query-string state to storage and restores the read-only gate", async () => {
    initializeOilFieldStore({ source: "QUERY_BP", beacons: "0" })
    const store = useOilFieldStore()
    expect(storage.getItem("OilFieldStore")).toBeNull()

    persistStore()

    // The query-string state is now the saved state.
    const persisted = JSON.parse(storage.getItem("OilFieldStore")!)
    expect(persisted.inputBlueprint).toBe("QUERY_BP")
    expect(persisted.addBeacons).toBe(false)

    // The gate is re-armed afterwards: usingQueryString stays true and further
    // edits are once again not written through.
    expect(store.usingQueryString).toBe(true)
    storage.writes = []
    store.inputBlueprint = "AFTER_PERSIST"
    await nextTick()
    expect(storage.writes).toHaveLength(0)
  })
})

describe("hasMatchingQueryString", () => {
  it("detects recognised keys in a LocationQuery", () => {
    expect(hasMatchingQueryString({ source: "BP" }, false)).toBe(true)
    expect(hasMatchingQueryString({ beaconModSlots: "2" }, false)).toBe(true)
  })

  it("detects recognised keys in URLSearchParams", () => {
    expect(hasMatchingQueryString(new URLSearchParams("source=BP"), false)).toBe(true)
    expect(hasMatchingQueryString(new URLSearchParams("utm_source=x"), false)).toBe(false)
  })

  it("ignores unrecognised and empty queries", () => {
    expect(hasMatchingQueryString({ utm_source: "newsletter" }, false)).toBe(false)
    expect(hasMatchingQueryString({}, false)).toBe(false)
  })
})

describe("AutoPlanStore persistence", () => {
  beforeEach(() => {
    installStorage()
    freshPinia()
    vi.spyOn(console, "log").mockImplementation(() => {})
  })

  afterEach(() => {
    vi.unstubAllGlobals()
    vi.restoreAllMocks()
  })

  it("persists autoPlan through the plugin default storage", async () => {
    const store = useAutoPlanStore()
    expect(store.autoPlan).toBe(false)

    store.autoPlan = true
    await nextTick()

    expect(JSON.parse(storage.getItem("AutoPlanStore")!).autoPlan).toBe(true)
  })

  it("hydrates autoPlan from previously persisted state", () => {
    seedPersisted("AutoPlanStore", { autoPlan: true })

    freshPinia()
    expect(useAutoPlanStore().autoPlan).toBe(true)
  })
})
