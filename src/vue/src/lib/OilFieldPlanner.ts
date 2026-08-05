import { OilFieldStoreState, useOilFieldStore } from "../stores/OilFieldStore"
import {
  BeaconStrategy,
  HttpResponse,
  OilFieldNormalizeRequest,
  OilFieldNormalizeResponse,
  OilFieldPlan,
  OilFieldPlanRequest,
  OilFieldPlanResponse,
  PipeStrategy,
  Quality,
} from "./FactorioToolsApi"
import { getEntries } from "./helpers"
import { PUMPJACK_MODULE_DEFAULT } from "./plannerDefaults"
import * as wasmPlanner from "./wasmPlanner"

type RequestPropertyGetters = {
  [Property in keyof OilFieldPlanRequest]-?: (
    state: OilFieldStoreState,
  ) => Exclude<OilFieldPlanRequest[Property], undefined>
}

const requestPropertyGetters: RequestPropertyGetters = {
  addBeacons: (state) => state.addBeacons,
  addElectricPoles: (state) => state.addElectricPoles,
  addFbeOffset: (_) => false,
  addHeatPipes: (state) => state.addHeatPipes,
  beaconEntityName: (state) => state.beaconEntityName.trim(),
  beaconHeight: (state) => state.beaconHeight,
  beaconModules: (state) => {
    const output: Record<string, number> = {}
    const module = state.beaconModule.trim()
    if (module) {
      output[module] = state.beaconModuleSlots
    }
    return output
  },
  beaconStrategies: (state) =>
    [
      state.beaconStrategyFbeOriginal ? BeaconStrategy.FbeOriginal : undefined,
      state.beaconStrategyFbe ? BeaconStrategy.Fbe : undefined,
      state.beaconStrategySnug ? BeaconStrategy.Snug : undefined,
    ].filter((b): b is BeaconStrategy => !!b),
  beaconSupplyHeight: (state) => state.beaconSupplyHeight,
  beaconSupplyWidth: (state) => state.beaconSupplyWidth,
  beaconWidth: (state) => state.beaconWidth,
  blueprint: (state) => state.inputBlueprint.trim(),
  electricPoleEntityName: (state) => state.electricPoleEntityName.trim(),
  electricPoleHeight: (state) => state.electricPoleHeight,
  electricPoleSupplyHeight: (state) => state.electricPoleSupplyHeight,
  electricPoleSupplyWidth: (state) => state.electricPoleSupplyWidth,
  electricPoleWidth: (state) => state.electricPoleWidth,
  electricPoleWireReach: (state) => state.electricPoleWireReach,
  heatPipeEntityName: (_) => "heat-pipe",
  optimizePipes: (state) => state.optimizePipes,
  overlapBeacons: (state) => state.overlapBeacons,
  pipeStrategies: (state) =>
    [
      state.pipeStrategyFbeOriginal ? PipeStrategy.FbeOriginal : undefined,
      state.pipeStrategyFbe ? PipeStrategy.Fbe : undefined,
      state.pipeStrategyConnectedCentersDelaunay
        ? PipeStrategy.ConnectedCentersDelaunay
        : undefined,
      state.pipeStrategyConnectedCentersDelaunayMst
        ? PipeStrategy.ConnectedCentersDelaunayMst
        : undefined,
      state.pipeStrategyConnectedCentersFlute ? PipeStrategy.ConnectedCentersFlute : undefined,
    ].filter((b): b is PipeStrategy => !!b),
  pumpjackModules: (state) => {
    const output: Record<string, number> = {}
    const module = state.pumpjackModule.trim()
    if (module) {
      output[module] = PUMPJACK_MODULE_DEFAULT.count
    }
    return output
  },
  useUndergroundPipes: (state) => state.useUndergroundPipes,
  validateSolution: (state) => state.validateSolution,
  pumpjackQuality: (state) => state.pumpjackQuality as Quality,
  pumpjackModuleQuality: (state) => state.pumpjackModuleQuality as Quality,
  beaconQuality: (state) => state.beaconQuality as Quality,
  beaconModuleQuality: (state) => state.beaconModuleQuality as Quality,
  electricPoleQuality: (state) => state.electricPoleQuality as Quality,
} as const

export type ApiError = {
  isError: true
  title: string
  errors?: Record<string, string[]>
  errorDetails?: string[]
  response?: HttpResponse<unknown, unknown>
}

export interface ApiResult<Data> {
  isError: false
  data: Data
}

export type PlanProgress = { current: number; total: number; label: string }

// Display names mirror OilFieldPlan.ToString in the C# core.
const pipeStrategyLabels: Record<PipeStrategy, string> = {
  [PipeStrategy.FbeOriginal]: "FBE",
  [PipeStrategy.Fbe]: "FBE*",
  [PipeStrategy.ConnectedCentersDelaunay]: "CC-DT",
  [PipeStrategy.ConnectedCentersDelaunayMst]: "CC-DT-MST",
  [PipeStrategy.ConnectedCentersFlute]: "CC-FLUTE",
}

// Mirrors the planner's ranking: more beacon effects, then fewer beacons, then
// fewer pipes. Equal plans are NOT "strictly better", so the earliest strategy
// wins on an exact three-field tie.
function isStrictlyBetterPlan(candidate: OilFieldPlan, best: OilFieldPlan): boolean {
  if (candidate.beaconEffectCount !== best.beaconEffectCount) {
    return candidate.beaconEffectCount > best.beaconEffectCount
  }
  if (candidate.beaconCount !== best.beaconCount) {
    return candidate.beaconCount < best.beaconCount
  }
  return candidate.pipeCount < best.pipeCount
}

export function buildPlanRequest(state: OilFieldStoreState): OilFieldPlanRequest {
  const request: OilFieldPlanRequest = { blueprint: "" }
  for (const [requestKey, getter] of getEntries(requestPropertyGetters)) {
    ;(request as unknown as Record<string, unknown>)[requestKey] = getter(state)
  }
  return request
}

async function runWasm<Data>(
  requestJson: string,
  invoke: (json: string) => Promise<string>,
): Promise<ApiResult<Data> | ApiError> {
  try {
    const responseJson = await invoke(requestJson)
    const parsed = JSON.parse(responseJson)

    // Error envelope shape: { title, status, errors }
    if (
      parsed &&
      typeof parsed === "object" &&
      "status" in parsed &&
      "errors" in parsed &&
      !("blueprint" in parsed)
    ) {
      const errors: Record<string, string[]> = {}
      if (parsed.errors && typeof parsed.errors === "object") {
        for (const [key, values] of Object.entries(parsed.errors)) {
          if (Array.isArray(values)) {
            errors[key] = (values as unknown[]).filter((v): v is string => typeof v === "string")
          }
        }
      }
      return { isError: true, title: parsed.title ?? "An error occurred.", errors }
    }

    return { isError: false, data: parsed as Data }
  } catch (e) {
    return {
      isError: true,
      title: "An unexpected error occurred.",
      errorDetails: [e instanceof Error ? (e.stack ?? e.toString()) : JSON.stringify(e)],
    }
  }
}

export async function normalize(): Promise<ApiResult<OilFieldNormalizeResponse> | ApiError> {
  const store = useOilFieldStore()
  const request: OilFieldNormalizeRequest = { blueprint: store.$state.inputBlueprint }
  return await runWasm<OilFieldNormalizeResponse>(JSON.stringify(request), wasmPlanner.normalize)
}

export async function getPlan(): Promise<ApiResult<OilFieldPlanResponse> | ApiError> {
  const store = useOilFieldStore()
  const request = buildPlanRequest(store.$state)
  return await runWasm<OilFieldPlanResponse>(JSON.stringify(request), wasmPlanner.plan)
}

export async function getPlanWithProgress(
  onProgress: (progress: PlanProgress) => void,
): Promise<ApiResult<OilFieldPlanResponse> | ApiError> {
  const store = useOilFieldStore()
  const baseRequest = buildPlanRequest(store.$state)
  const strategies = baseRequest.pipeStrategies ?? []

  // No split possible: defer to the single combined call so behavior and
  // validation exactly match the non-progress path.
  if (strategies.length === 0) {
    return await runWasm<OilFieldPlanResponse>(JSON.stringify(baseRequest), wasmPlanner.plan)
  }

  const total = strategies.length
  let best: ApiResult<OilFieldPlanResponse> | null = null
  let bestPlan: OilFieldPlan | null = null

  for (let i = 0; i < total; i++) {
    const strategy = strategies[i]
    onProgress({ current: i, total, label: pipeStrategyLabels[strategy] })
    // Yield a macrotask so the browser paints the bar before the next blocking
    // WASM call (mirrors the paint yield in OilField.vue's invokeApi).
    await new Promise((resolve) => setTimeout(resolve, 0))

    const request: OilFieldPlanRequest = { ...baseRequest, pipeStrategies: [strategy] }
    const result = await runWasm<OilFieldPlanResponse>(JSON.stringify(request), wasmPlanner.plan)
    if (result.isError) {
      return result
    }

    const plan = result.data.summary.selectedPlans[0] ?? null
    if (
      best === null ||
      bestPlan === null ||
      (plan !== null && isStrictlyBetterPlan(plan, bestPlan))
    ) {
      best = result
      bestPlan = plan
    }
  }

  onProgress({ current: total, total, label: "Finalizing" })
  return best as ApiResult<OilFieldPlanResponse>
}
