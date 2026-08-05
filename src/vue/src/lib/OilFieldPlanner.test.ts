import { beforeEach, describe, expect, it, vi } from "vitest"
import { createPinia, setActivePinia } from "pinia"
import { PipeStrategy } from "./FactorioToolsApi"
import { useOilFieldStore } from "../stores/OilFieldStore"
import { buildPlanRequest, getPlanWithProgress, PlanProgress } from "./OilFieldPlanner"
import * as wasmPlanner from "./wasmPlanner"

vi.mock("./wasmPlanner")

type Metrics = { effects: number; beacons: number; pipes: number }

function planResponse(strategy: PipeStrategy, metrics: Metrics, blueprint: string): string {
  return JSON.stringify({
    request: {},
    blueprint,
    summary: {
      missingPumpjacks: 0,
      rotatedPumpjacks: 0,
      heatDroppedPumpjacks: 0,
      unheatedPumpjacks: 0,
      unheatedPipes: 0,
      selectedPlans: [
        {
          pipeStrategy: strategy,
          optimizePipes: true,
          beaconStrategy: null,
          beaconEffectCount: metrics.effects,
          beaconCount: metrics.beacons,
          pipeCount: metrics.pipes,
          pipeCountWithoutUnderground: metrics.pipes,
        },
      ],
      alternatePlans: [],
      unusedPlans: [],
    },
  })
}

function errorResponse(): string {
  // Matches the error envelope shape runWasm detects: status + errors, no blueprint.
  return JSON.stringify({
    title: "Bad input was provided.",
    status: 400,
    errors: { FactorioToolsException: ["boom"] },
  })
}

// Select exactly the given pipe strategies via the store flags. Unset flags default off.
function selectStrategies(opts: {
  fbeOriginal?: boolean
  fbe?: boolean
  ccDt?: boolean
  ccDtMst?: boolean
  ccFlute?: boolean
}) {
  const store = useOilFieldStore()
  store.pipeStrategyFbeOriginal = !!opts.fbeOriginal
  store.pipeStrategyFbe = !!opts.fbe
  store.pipeStrategyConnectedCentersDelaunay = !!opts.ccDt
  store.pipeStrategyConnectedCentersDelaunayMst = !!opts.ccDtMst
  store.pipeStrategyConnectedCentersFlute = !!opts.ccFlute
}

function mockPerStrategy(responses: Partial<Record<PipeStrategy, string>>) {
  vi.mocked(wasmPlanner.plan).mockImplementation(async (json: string) => {
    const request = JSON.parse(json)
    const strategy = request.pipeStrategies[0] as PipeStrategy
    const response = responses[strategy]
    if (!response) {
      throw new Error(`no mock response for strategy ${strategy}`)
    }
    return response
  })
}

describe("getPlanWithProgress", () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    vi.clearAllMocks()
  })

  it("returns the best result by effects, then beacons, then pipes", async () => {
    selectStrategies({ fbe: true, ccDt: true, ccFlute: true })
    mockPerStrategy({
      [PipeStrategy.Fbe]: planResponse(
        PipeStrategy.Fbe,
        { effects: 10, beacons: 5, pipes: 40 },
        "FBE_BP",
      ),
      [PipeStrategy.ConnectedCentersDelaunay]: planResponse(
        PipeStrategy.ConnectedCentersDelaunay,
        { effects: 12, beacons: 6, pipes: 50 },
        "CCDT_BP",
      ),
      [PipeStrategy.ConnectedCentersFlute]: planResponse(
        PipeStrategy.ConnectedCentersFlute,
        { effects: 12, beacons: 6, pipes: 45 },
        "CCFLUTE_BP",
      ),
    })

    const result = await getPlanWithProgress(() => {})

    expect(result.isError).toBe(false)
    if (!result.isError) {
      // CC-Flute wins: ties CC-DT on effects(12)/beacons(6) but has fewer pipes(45 < 50).
      expect(result.data.blueprint).toBe("CCFLUTE_BP")
    }
  })

  it("keeps the earliest strategy on an exact three-field tie", async () => {
    selectStrategies({ fbe: true, ccDt: true })
    const metrics: Metrics = { effects: 8, beacons: 4, pipes: 30 }
    mockPerStrategy({
      [PipeStrategy.Fbe]: planResponse(PipeStrategy.Fbe, metrics, "FIRST"),
      [PipeStrategy.ConnectedCentersDelaunay]: planResponse(
        PipeStrategy.ConnectedCentersDelaunay,
        metrics,
        "SECOND",
      ),
    })

    const result = await getPlanWithProgress(() => {})

    expect(result.isError).toBe(false)
    if (!result.isError) {
      expect(result.data.blueprint).toBe("FIRST")
    }
  })

  it("reports progress once per strategy plus a final tick", async () => {
    selectStrategies({ fbe: true, ccDt: true })
    mockPerStrategy({
      [PipeStrategy.Fbe]: planResponse(PipeStrategy.Fbe, { effects: 1, beacons: 1, pipes: 1 }, "A"),
      [PipeStrategy.ConnectedCentersDelaunay]: planResponse(
        PipeStrategy.ConnectedCentersDelaunay,
        { effects: 1, beacons: 1, pipes: 1 },
        "B",
      ),
    })

    const progress: PlanProgress[] = []
    await getPlanWithProgress((p) => progress.push({ ...p }))

    expect(progress.map((p) => p.current)).toEqual([0, 1, 2])
    expect(progress.every((p) => p.total === 2)).toBe(true)
    expect(progress[0].label).toBe("FBE*")
    expect(progress[1].label).toBe("CC-DT")
    expect(progress[2].label).toBe("Finalizing")
  })

  it("aborts on the first error and does not call later strategies", async () => {
    selectStrategies({ fbe: true, ccDt: true, ccFlute: true })
    vi.mocked(wasmPlanner.plan).mockImplementation(async (json: string) => {
      const request = JSON.parse(json)
      const strategy = request.pipeStrategies[0] as PipeStrategy
      if (strategy === PipeStrategy.ConnectedCentersDelaunay) {
        return errorResponse()
      }
      return planResponse(strategy, { effects: 1, beacons: 1, pipes: 1 }, "OK")
    })

    const result = await getPlanWithProgress(() => {})

    expect(result.isError).toBe(true)
    // FBE* then CC-DT only; CC-FLUTE is never reached.
    expect(vi.mocked(wasmPlanner.plan)).toHaveBeenCalledTimes(2)
  })

  it("falls back to a single call when no strategies are selected", async () => {
    selectStrategies({})
    vi.mocked(wasmPlanner.plan).mockResolvedValue(
      planResponse(PipeStrategy.Fbe, { effects: 1, beacons: 1, pipes: 1 }, "FALLBACK"),
    )

    const progress: PlanProgress[] = []
    const result = await getPlanWithProgress((p) => progress.push(p))

    expect(vi.mocked(wasmPlanner.plan)).toHaveBeenCalledTimes(1)
    expect(progress).toHaveLength(0)
    expect(result.isError).toBe(false)
  })

  it("passes quality fields through to the request", () => {
    const store = useOilFieldStore()
    store.pumpjackQuality = "Legendary"
    store.beaconQuality = "Rare"
    store.electricPoleQuality = "Uncommon"
    store.pumpjackModuleQuality = "Epic"
    store.beaconModuleQuality = "Legendary"
    const request = buildPlanRequest(store.$state)
    expect(request.pumpjackQuality).toBe("Legendary")
    expect(request.beaconQuality).toBe("Rare")
    expect(request.electricPoleQuality).toBe("Uncommon")
    expect(request.pumpjackModuleQuality).toBe("Epic")
    expect(request.beaconModuleQuality).toBe("Legendary")
  })
})
