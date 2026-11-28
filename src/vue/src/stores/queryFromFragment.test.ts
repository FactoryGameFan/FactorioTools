import { describe, expect, it } from "vitest"
import { queryFromFragment } from "./OilFieldStore"

// What Factorio Blueprint Editor's G key sends: the blueprint string run through
// encodeURIComponent, so its base64 "+" and "/" survive URLSearchParams.
const BLUEPRINT = "0eJyVkcuKg0AQRf+lrmu+/+/=="
const fromFbe = `#source=${encodeURIComponent(BLUEPRINT)}`

describe("queryFromFragment", () => {
  it("reads the blueprint FBE puts in the fragment, byte for byte", () => {
    expect(queryFromFragment(fromFbe)).toEqual({ source: BLUEPRINT })
  })

  it("reads every store key the query string can carry", () => {
    expect(queryFromFragment("#source=abc&adv=1&pumpMod=speed-module")).toEqual({
      source: "abc",
      adv: "1",
      pumpMod: "speed-module",
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
