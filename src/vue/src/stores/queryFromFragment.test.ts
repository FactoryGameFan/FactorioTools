import { describe, expect, it } from "vitest"
import { createMemoryHistory, createRouter } from "vue-router"
import { initialQuery, queryFromFragment } from "./OilFieldStore"

// What Factorio Blueprint Editor's G key sends: the blueprint string run through
// encodeURIComponent. The "+" and "/" are the base64 characters that can break.
const BLUEPRINT = "0eJyVkcuK+g0AQRf+lrmu+/+/=="
const fromFbe = `#source=${encodeURIComponent(BLUEPRINT)}`

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

  it("keeps a repeated key as an array, as Vue Router does for the query", () => {
    expect(queryFromFragment("#adv=1&adv=0")).toEqual({ adv: ["1", "0"] })
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
  // A real router, so the test sees the hash exactly as the route guard does.
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/oil-field", component: {} }],
  })

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
