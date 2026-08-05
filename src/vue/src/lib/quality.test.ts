import { describe, expect, it } from "vitest"
import { Quality } from "./FactorioToolsApi"
import { qualityLevel, qualityPipRadius, qualityPips, QUALITY_ORDER } from "./quality"
import { QUALITY_LEVELS } from "./plannerDefaults"

describe("quality", () => {
  // Exercises quality.ts's own derivation (the Quality enum -> levels -> QUALITY_LEVELS
  // wiring in Step 3): the expected values here are hand-written literals, independent
  // of QUALITY_LEVELS, so a transposition bug in that wiring (e.g. Epic copied from
  // QUALITY_LEVELS.Rare) would fail this even though plannerDefaults.test.ts's own
  // artifact check would still pass.
  it("maps quality to its bonus level", () => {
    expect(qualityLevel(Quality.Normal)).toBe(0)
    expect(qualityLevel(Quality.Uncommon)).toBe(1)
    expect(qualityLevel(Quality.Rare)).toBe(2)
    expect(qualityLevel(Quality.Epic)).toBe(3)
    expect(qualityLevel(Quality.Legendary)).toBe(5)
  })

  // PINNED: the hidden level 4 skip is Factorio's own gap, not a planner tuning choice.
  // Reads the artifact against hand-written literals - see plannerDefaults.test.ts's
  // top-of-file comment on the pinned-vs-shape distinction.
  it("keeps the hidden level 4 skipped, so Legendary is 5", () => {
    expect(QUALITY_LEVELS.Legendary).toBe(5)
    expect(QUALITY_LEVELS.Epic).toBe(3)
  })

  it("orders qualities from normal to legendary", () => {
    expect(QUALITY_ORDER).toEqual([
      Quality.Normal,
      Quality.Uncommon,
      Quality.Rare,
      Quality.Epic,
      Quality.Legendary,
    ])
  })

  it("uses the in-game pip count for each quality glyph", () => {
    expect(qualityPips(Quality.Normal)).toHaveLength(1)
    expect(qualityPips(Quality.Uncommon)).toHaveLength(2)
    expect(qualityPips(Quality.Rare)).toHaveLength(3)
    expect(qualityPips(Quality.Epic)).toHaveLength(4)
    expect(qualityPips(Quality.Legendary)).toHaveLength(5)
  })

  it("keeps every pip within the 0-24 glyph viewBox", () => {
    for (const quality of QUALITY_ORDER) {
      const radius = qualityPipRadius(quality)
      for (const [cx, cy] of qualityPips(quality)) {
        expect(cx - radius).toBeGreaterThanOrEqual(0)
        expect(cx + radius).toBeLessThanOrEqual(24)
        expect(cy - radius).toBeGreaterThanOrEqual(0)
        expect(cy + radius).toBeLessThanOrEqual(24)
      }
    }
  })
})
