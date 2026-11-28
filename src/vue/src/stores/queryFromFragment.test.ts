import { describe, expect, it } from "vitest"
import { createMemoryHistory, createRouter } from "vue-router"
import { createPinia, setActivePinia } from "pinia"
import { beforeEach, vi } from "vitest"
import {
  generateShareUrl,
  initialQuery,
  initializeOilFieldStore,
  queryFromFragment,
  setReadOnly,
  useOilFieldStore,
} from "./OilFieldStore"

// What Factorio Blueprint Editor's G key sends: the blueprint string run through
// encodeURIComponent. The "+" and "/" are the base64 characters that can break.
const BLUEPRINT = "0eJyVkcuK+g0AQRf+lrmu+/+/=="
const fromFbe = `#source=${encodeURIComponent(BLUEPRINT)}`

// A real router, so the tests see a query or hash exactly as the route guard does.
const router = createRouter({
  history: createMemoryHistory(),
  routes: [{ path: "/oil-field", component: {} }],
})

describe("queryFromFragment", () => {
  it("reads the blueprint FBE puts in the fragment, byte for byte", () => {
    expect(fromFbe).toContain("%2B")
    expect(queryFromFragment(fromFbe)).toEqual({ source: BLUEPRINT })
  })

  it("reads every store key the query string can carry", () => {
    expect(queryFromFragment("#source=abc&adv=1&pumpMod=speed-module")).toEqual({
      source: "abc",
      adv: "1",
      pumpMod: "speed-module",
    })
  })

  // The next three are what a real router gives for the same text as a query string,
  // so a fragment link and a query link load the same settings.
  it.each([
    ["a repeated key, as an array in order", "source=abc&adv=1&adv=0"],
    ["a bare key as null, an empty value as an empty string", "source=abc&beaconModSlots&adv="],
    ["a malformed escape as written, and + as a space", "source=a%E0%A4%A&pumpMod=a+b"],
  ])("reads %s, as Vue Router reads the query", (_, text) => {
    const asQuery = router.resolve(`/oil-field?${text}`).query
    expect(queryFromFragment(`#${text}`)).toEqual(asQuery)
  })

  it("spells out what the router gives for those cases", () => {
    expect(queryFromFragment("#source=abc&adv=1&adv=0")).toEqual({ source: "abc", adv: ["1", "0"] })
    expect(queryFromFragment("#source=abc&beaconModSlots&adv=")).toEqual({
      source: "abc",
      beaconModSlots: null,
      adv: "",
    })
  })

  it("ignores a fragment that names none of the store's keys", () => {
    expect(queryFromFragment("#about")).toBeUndefined()
    expect(queryFromFragment("#unknown=1")).toBeUndefined()
  })

  it("ignores an empty or missing fragment", () => {
    expect(queryFromFragment("")).toBeUndefined()
    expect(queryFromFragment("#")).toBeUndefined()
  })
})

describe("initialQuery", () => {
  it("reads the raw fragment, so a + in the blueprint survives the router", () => {
    const to = router.resolve(`/oil-field${fromFbe}`)
    // the router has decoded %2B, which is why to.hash itself cannot be used
    expect(queryFromFragment(to.hash)).not.toEqual({ source: BLUEPRINT })
    expect(initialQuery(to, fromFbe)).toEqual({ source: BLUEPRINT })
  })

  it("falls back to the query string when there is no fragment", () => {
    const to = router.resolve("/oil-field?source=abc")
    expect(initialQuery(to, "")).toEqual({ source: "abc" })
  })

  it("falls back to the query string when the fragment carries no settings", () => {
    const to = router.resolve("/oil-field?source=abc#top")
    expect(initialQuery(to, "#top")).toEqual({ source: "abc" })
  })
})

describe("generateShareUrl", () => {
  beforeEach(() => {
    vi.stubGlobal("localStorage", { getItem: () => null, setItem: () => {}, removeItem: () => {} })
    setActivePinia(createPinia())
    setReadOnly(false)
    vi.spyOn(console, "log").mockImplementation(() => {})
  })

  it("puts the settings in the fragment, with no query string", () => {
    const url = new URL(generateShareUrl("https://planner.test/oil-field"))
    expect(url.search).toBe("")
    expect(url.hash.startsWith("#source=")).toBe(true)
  })

  it("loads back the same settings, a blueprint with + in it included", () => {
    useOilFieldStore().inputBlueprint = BLUEPRINT
    useOilFieldStore().addBeacons = false
    const before = { ...useOilFieldStore().$state }
    const url = new URL(generateShareUrl("https://planner.test/oil-field"))

    setActivePinia(createPinia())
    initializeOilFieldStore(initialQuery(router.resolve(`/oil-field${url.hash}`), url.hash))
    const after = { ...useOilFieldStore().$state }
    expect(after.usingQueryString).toBe(true)
    expect({ ...after, usingQueryString: false }).toEqual({ ...before, usingQueryString: false })
  })
})
