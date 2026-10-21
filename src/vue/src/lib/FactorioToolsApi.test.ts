import { describe, expect, it } from "vitest"
import {
  BeaconStrategy,
  OilFieldNormalizeResponse,
  OilFieldPlan,
  OilFieldPlanRequestResponse,
  OilFieldPlanResponse,
  OilFieldPlanSummary,
  PipeStrategy,
  Quality,
} from "./FactorioToolsApi"
// The contract fixtures, as text. A Vite raw import rather than node:fs keeps Node's types
// out of the app's type-check; vite/client (src/vite-env.d.ts) types `*?raw` as a string.
import contractJsonl from "../../../../rust/contract/fixtures/contract.jsonl?raw"

// Checks the hand-maintained types in FactorioToolsApi.ts against the responses the Rust
// contract test replays (rust/contract/FIXTURES.md has the line format). The samples below
// are checked by the compiler (`npm run build` runs vue-tsc over this file): `satisfies
// Required<...>` fails on a missing or extra property. The tests then check every recorded
// response has exactly the samples' keys and value types, so a field renamed on either side
// fails here.
const planSample = {
  pipeStrategy: PipeStrategy.Fbe,
  optimizePipes: true,
  beaconStrategy: BeaconStrategy.Snug,
  beaconEffectCount: 1,
  beaconCount: 1,
  pipeCount: 1,
  pipeCountWithoutUnderground: 1,
} satisfies Required<OilFieldPlan>

const planResponseSample = {
  request: {
    useUndergroundPipes: true,
    addBeacons: true,
    optimizePipes: true,
    overlapBeacons: true,
    addElectricPoles: true,
    addHeatPipes: false,
    heatPipeEntityName: "heat-pipe",
    pipeStrategies: [PipeStrategy.Fbe],
    beaconStrategies: [BeaconStrategy.Fbe],
    electricPoleEntityName: "medium-electric-pole",
    electricPoleSupplyWidth: 7,
    electricPoleSupplyHeight: 7,
    electricPoleWireReach: 9,
    electricPoleWidth: 1,
    electricPoleHeight: 1,
    beaconEntityName: "beacon",
    beaconSupplyWidth: 9,
    beaconSupplyHeight: 9,
    beaconWidth: 3,
    beaconHeight: 3,
    validateSolution: false,
    pumpjackModules: { "productivity-module-3": 2 },
    beaconModules: { "speed-module-3": 2 },
    pumpjackQuality: Quality.Normal,
    beaconQuality: Quality.Normal,
    electricPoleQuality: Quality.Normal,
    pumpjackModuleQuality: Quality.Normal,
    beaconModuleQuality: Quality.Normal,
    blueprint: "0e...",
    addFbeOffset: false,
  } satisfies Required<OilFieldPlanRequestResponse>,
  blueprint: "0e...",
  summary: {
    missingPumpjacks: 0,
    rotatedPumpjacks: 0,
    heatDroppedPumpjacks: 0,
    unheatedPumpjacks: 0,
    unheatedPipes: 0,
    selectedPlans: [planSample],
    alternatePlans: [planSample],
    unusedPlans: [planSample],
  } satisfies Required<OilFieldPlanSummary>,
} satisfies Required<OilFieldPlanResponse>

const normalizeResponseSample = {
  request: { blueprint: "0e..." },
  blueprint: "0e...",
} satisfies Required<OilFieldNormalizeResponse>

// Map-typed fields: the keys are module names, so only the values are checked.
const RECORD_FIELDS = new Set(["pumpjackModules", "beaconModules"])

// Fields the contract may write as null.
const NULLABLE_FIELDS = new Set(["beaconStrategy"])

// The C# echoed a quality integer it had no name for as that integer, and the Rust port
// matches it. The site only ever sends names, so the type stays Quality and this one
// recorded case may hold a number there.
const NUMBER_QUALITY = { name: "read_quality-undefined-integer", path: ".request.pumpjackQuality" }

type Line = { name: string; op: string; outcome: string; response: string }
type Envelope = { title: unknown; status: unknown; errors: unknown }

function readFixtures(): { name: string; op: "plan" | "normalize"; response: unknown }[] {
  return contractJsonl
    .split("\n")
    .filter((line) => line.length > 0)
    .map((line) => JSON.parse(line) as Line)
    .filter((line) => (line.op === "plan" || line.op === "normalize") && line.outcome === "ok")
    .map((line) => ({
      name: line.name,
      op: line.op as "plan" | "normalize",
      response: JSON.parse(line.response),
    }))
}

// The same check runWasm in OilFieldPlanner.ts uses to tell an error envelope from a response.
function isEnvelope(value: unknown): value is Envelope {
  return (
    typeof value === "object" &&
    value !== null &&
    "status" in value &&
    "errors" in value &&
    !("blueprint" in value)
  )
}

function isObject(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

// Collects every place `actual` differs in shape from `sample`: a key missing or extra, a
// value of another type, or an array element unlike the sample's one element. `seen` gets
// every sample path that some value was checked against, so the caller can tell whether
// the fixtures reached all of a sample (an empty array checks nothing).
function compareShape(
  actual: unknown,
  sample: unknown,
  path: string,
  key: string,
  problems: string[],
  seen: Set<string>,
): void {
  seen.add(path)
  if (actual === null && NULLABLE_FIELDS.has(key)) {
    return
  }
  if (Array.isArray(sample)) {
    if (!Array.isArray(actual)) {
      problems.push(`${path}: expected an array, got ${JSON.stringify(actual)}`)
      return
    }
    for (const element of actual) {
      compareShape(element, sample[0], `${path}[]`, key, problems, seen)
    }
  } else if (isObject(sample)) {
    if (!isObject(actual)) {
      problems.push(`${path}: expected an object, got ${JSON.stringify(actual)}`)
    } else if (RECORD_FIELDS.has(key)) {
      for (const [name, value] of Object.entries(actual)) {
        if (typeof value !== "number") {
          problems.push(`${path}.${name}: expected a number, got ${JSON.stringify(value)}`)
        }
      }
    } else {
      const actualKeys = Object.keys(actual).sort()
      const sampleKeys = Object.keys(sample).sort()
      if (actualKeys.join() !== sampleKeys.join()) {
        problems.push(`${path}: keys [${actualKeys}] differ from the sample's [${sampleKeys}]`)
      }
      for (const name of sampleKeys) {
        if (name in actual) {
          compareShape(actual[name], sample[name], `${path}.${name}`, name, problems, seen)
        }
      }
    }
  } else if (typeof actual !== typeof sample) {
    problems.push(`${path}: expected ${typeof sample}, got ${JSON.stringify(actual)}`)
  }
}

// Every path in the sample, in the form compareShape adds to `seen`.
function samplePaths(sample: unknown, path: string, key: string, out: Set<string>): Set<string> {
  out.add(path)
  if (Array.isArray(sample)) {
    samplePaths(sample[0], `${path}[]`, key, out)
  } else if (isObject(sample) && !RECORD_FIELDS.has(key)) {
    for (const [name, value] of Object.entries(sample)) {
      samplePaths(value, `${path}.${name}`, name, out)
    }
  }
  return out
}

function checkShapes(op: "plan" | "normalize", sample: object) {
  const cases = readFixtures().filter((c) => c.op === op && !isEnvelope(c.response))
  const problems: string[] = []
  const seen = new Set<string>()
  for (const c of cases) {
    const caseProblems: string[] = []
    compareShape(c.response, sample, "", "", caseProblems, seen)
    const allowed = `${NUMBER_QUALITY.path}: expected string, got `
    const isAllowed = (p: string) =>
      c.name === NUMBER_QUALITY.name &&
      p.startsWith(allowed) &&
      /^-?\d+$/.test(p.slice(allowed.length))
    problems.push(...caseProblems.filter((p) => !isAllowed(p)).map((p) => `${c.name} ${p}`))
  }
  return { count: cases.length, problems, seen }
}

describe("FactorioToolsApi types against the contract fixtures", () => {
  it("match every recorded plan response", () => {
    const { count, problems, seen } = checkShapes("plan", planResponseSample)
    expect(count).toBeGreaterThan(200)
    expect(problems).toEqual([])
    expect(
      [...samplePaths(planResponseSample, "", "", new Set())].filter((p) => !seen.has(p)),
    ).toEqual([])
  })

  it("match every recorded normalize response", () => {
    const { count, problems, seen } = checkShapes("normalize", normalizeResponseSample)
    expect(count).toBeGreaterThan(100)
    expect(problems).toEqual([])
    expect(
      [...samplePaths(normalizeResponseSample, "", "", new Set())].filter((p) => !seen.has(p)),
    ).toEqual([])
  })

  it("name every strategy and quality the contract writes, and no others", () => {
    const pipe = new Set<unknown>()
    const beacon = new Set<unknown>()
    const quality = new Set<unknown>()
    const numberQualities: string[] = []
    for (const c of readFixtures()) {
      if (c.op !== "plan" || isEnvelope(c.response)) {
        continue
      }
      const response = c.response as OilFieldPlanResponse
      const request = response.request
      request.pipeStrategies.forEach((s) => pipe.add(s))
      request.beaconStrategies.forEach((s) => beacon.add(s))
      for (const q of [
        request.pumpjackQuality,
        request.beaconQuality,
        request.electricPoleQuality,
        request.pumpjackModuleQuality,
        request.beaconModuleQuality,
      ]) {
        if (typeof q === "number" && c.name === NUMBER_QUALITY.name) {
          numberQualities.push(c.name)
        } else {
          quality.add(q)
        }
      }
      const { selectedPlans, alternatePlans, unusedPlans } = response.summary
      for (const plan of [...selectedPlans, ...alternatePlans, ...unusedPlans]) {
        pipe.add(plan.pipeStrategy)
        if (plan.beaconStrategy !== null) {
          beacon.add(plan.beaconStrategy)
        }
      }
    }

    const sorted = (values: Iterable<unknown>) => [...values].map(String).sort()
    expect(sorted(pipe)).toEqual(sorted(Object.values(PipeStrategy)))
    expect(sorted(beacon)).toEqual(sorted(Object.values(BeaconStrategy)))
    expect(sorted(quality)).toEqual(sorted(Object.values(Quality)))
    // If this case goes away, so can NUMBER_QUALITY.
    expect(numberQualities).toEqual([NUMBER_QUALITY.name])
  })

  it("read every recorded error envelope as title, status and errors", () => {
    const envelopes = readFixtures()
      .map((c) => c.response)
      .filter(isEnvelope)
    expect(envelopes.length).toBeGreaterThan(20)
    for (const envelope of envelopes) {
      expect(Object.keys(envelope).sort()).toEqual(["errors", "status", "title"])
      expect(typeof envelope.title).toBe("string")
      expect(typeof envelope.status).toBe("number")
      expect(isObject(envelope.errors)).toBe(true)
      for (const messages of Object.values(envelope.errors as object)) {
        expect(Array.isArray(messages)).toBe(true)
        for (const message of messages as unknown[]) {
          expect(typeof message).toBe("string")
        }
      }
    }
  })
})
