# Shared Planner Defaults Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the C# the single source of truth for planner constants, so the Vue app cannot hold a stale copy that silently overrides it.

**Architecture:** A Verify snapshot test in the .NET suite emits `src/vue/src/lib/plannerDefaults.verified.json`. The Vue store, pole presets, quality levels and module count import that file instead of declaring literals. Locally `AutoVerify` rewrites the artifact when C# changes; on CI `AutoVerify` is off, so a stale committed copy fails `dotnet test`.

**Tech Stack:** C# / xUnit / Verify.Xunit 31.12.5, TypeScript / Vue 3 / Pinia / vitest.

Spec: `docs/superpowers/specs/2026-08-05-shared-planner-defaults-design.md`
Issue: #54

## Global Constraints

- The core library `src/FactorioTools` must stay free of JSON and serialization dependencies. The emitter lives in the **test** project, never in the core.
- The Vue build must never require the .NET SDK. The artifact is committed; CI's `build-vue` job has no .NET and must stay that way.
- Do not touch `swagger-gen`, `src/WebApp`, or the Microsoft.OpenApi / Swashbuckle versions. They are deliberately pinned.
- Do not add `small-iron-electric-pole` to the Vue dropdown. It ships in the artifact only.
- Use hyphens, not em or en dashes, in all files.
- Verify facts established by spike, do not re-derive: `.UseStrictJson()` is required for real JSON (the default writes unquoted keys); the produced file carries a UTF-8 BOM, and vitest, `vue-tsc` and `vite build` all handle it correctly.

---

## File Structure

**Create:**
- `test/FactorioTools.Test/OilField/PlannerDefaultsTest.cs` - the emitter. Builds the payload from `OilFieldOptions` and `Quality`, verifies it as strict JSON.
- `src/vue/src/lib/plannerDefaults.verified.json` - generated. Never hand-edited.
- `src/vue/src/lib/plannerDefaults.ts` - the typed adapter. Imports the JSON, converts C# vocabulary to Vue vocabulary (strategy lists to booleans, module dictionary to name plus count), and is the only file the rest of the app imports from.
- `src/vue/src/lib/plannerDefaults.test.ts` - tests for the adapter and for store coverage.

**Modify:**
- `src/vue/src/stores/OilFieldStore.ts` - `defaults` splits into derived and Vue-only.
- `src/vue/src/lib/advancedOptions.ts` - `ELECTRIC_POLE_PRESETS` comes from the adapter.
- `src/vue/src/lib/quality.ts` - `levels` comes from the adapter.
- `src/vue/src/lib/OilFieldPlanner.ts:72` - the bare `2` comes from the adapter.
- `src/vue/src/lib/quality.test.ts` - stops asserting literals against itself.
- `src/vue/.prettierignore` and `src/vue/eslint.config.ts` - exclude the generated JSON.
- `src/FactorioTools/OilField/OilFieldOptions.cs` - one comment pointing at the emitter.

The adapter file exists so that exactly one file knows the artifact's shape. Consumers import named values from it and never reach into the JSON directly.

---

### Task 1: Emit the artifact from C#

**Files:**
- Create: `test/FactorioTools.Test/OilField/PlannerDefaultsTest.cs`
- Creates as output: `src/vue/src/lib/plannerDefaults.verified.json`
- Modify: `src/vue/.prettierignore`, `src/vue/eslint.config.ts`

**Interfaces:**
- Consumes: `OilFieldOptions` (instance properties and the five static `For*` presets), `OilFieldOptions.DefaultPipeStrategies`, `OilFieldOptions.DefaultBeaconStrategies`, the `Quality` enum.
- Produces: `src/vue/src/lib/plannerDefaults.verified.json` with top-level keys `options`, `electricPolePresets`, `qualityLevels`. Task 2 reads exactly these.

- [ ] **Step 1: Write the emitter test**

Create `test/FactorioTools.Test/OilField/PlannerDefaultsTest.cs`:

```csharp
using System.Text.Json;

namespace Knapcode.FactorioTools.OilField;

/// <summary>
/// Emits the planner constants the Vue app needs, so they are not typed out a second
/// time in TypeScript. The Vue app imports the verified file directly.
///
/// This test IS the generator. Locally, AutoVerify rewrites the file when the C#
/// changes and the test passes, so commit the rewritten file. On CI, AutoVerify is
/// off, so a stale committed file fails here with a diff.
/// </summary>
public class PlannerDefaultsTest
{
    [Fact]
    public Task PlannerDefaults()
    {
        var options = new OilFieldOptions();

        var payload = new
        {
            options = new
            {
                useUndergroundPipes = options.UseUndergroundPipes,
                addBeacons = options.AddBeacons,
                optimizePipes = options.OptimizePipes,
                overlapBeacons = options.OverlapBeacons,
                addElectricPoles = options.AddElectricPoles,
                addHeatPipes = options.AddHeatPipes,
                heatPipeEntityName = options.HeatPipeEntityName,
                pipeStrategies = options.PipeStrategies.Select(s => s.ToString()).ToList(),
                beaconStrategies = options.BeaconStrategies.Select(s => s.ToString()).ToList(),
                electricPoleEntityName = options.ElectricPoleEntityName,
                electricPoleSupplyWidth = options.ElectricPoleSupplyWidth,
                electricPoleSupplyHeight = options.ElectricPoleSupplyHeight,
                electricPoleWireReach = options.ElectricPoleWireReach,
                electricPoleWidth = options.ElectricPoleWidth,
                electricPoleHeight = options.ElectricPoleHeight,
                beaconEntityName = options.BeaconEntityName,
                beaconSupplyWidth = options.BeaconSupplyWidth,
                beaconSupplyHeight = options.BeaconSupplyHeight,
                beaconWidth = options.BeaconWidth,
                beaconHeight = options.BeaconHeight,
                validateSolution = options.ValidateSolution,
                pumpjackModules = options.PumpjackModules,
                beaconModules = options.BeaconModules,
                pumpjackQuality = options.PumpjackQuality.ToString(),
                beaconQuality = options.BeaconQuality.ToString(),
                electricPoleQuality = options.ElectricPoleQuality.ToString(),
                pumpjackModuleQuality = options.PumpjackModuleQuality.ToString(),
                beaconModuleQuality = options.BeaconModuleQuality.ToString(),
            },
            electricPolePresets = new Dictionary<string, object>
            {
                [OilFieldOptions.ForSmallIronElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForSmallIronElectricPole),
                [OilFieldOptions.ForSmallElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForSmallElectricPole),
                [OilFieldOptions.ForMediumElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForMediumElectricPole),
                [OilFieldOptions.ForBigElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForBigElectricPole),
                [OilFieldOptions.ForSubstation.ElectricPoleEntityName] = Preset(OilFieldOptions.ForSubstation),
            },
            qualityLevels = Enum
                .GetValues<Quality>()
                .ToDictionary(q => q.ToString(), q => (int)q),
        };

        var json = JsonSerializer.Serialize(payload);

        // UseStrictJson is required: Verify's default writes unquoted keys and string
        // values, which the Vue app cannot import.
        return VerifyJson(json)
            .UseStrictJson()
            .UseDirectory("../../../src/vue/src/lib")
            .UseFileName("plannerDefaults");
    }

    private static object Preset(OilFieldOptions options)
    {
        return new
        {
            width = options.ElectricPoleWidth,
            height = options.ElectricPoleHeight,
            supplyWidth = options.ElectricPoleSupplyWidth,
            supplyHeight = options.ElectricPoleSupplyHeight,
            wireReach = options.ElectricPoleWireReach,
        };
    }
}
```

- [ ] **Step 2: Run it and confirm the artifact appears**

```bash
dotnet test --filter "FullyQualifiedName~PlannerDefaultsTest" --logger "console;verbosity=minimal"
cat src/vue/src/lib/plannerDefaults.verified.json
```

Expected: PASS (AutoVerify writes the file on first run), and the file contains `"electricPoleWireReach": 9`, five entries under `electricPolePresets`, and `"Legendary": 5`.

- [ ] **Step 3: Confirm the freshness gate is real**

Do not assume it. Corrupt the file and run as CI does:

```bash
python3 - <<'PY'
p = "src/vue/src/lib/plannerDefaults.verified.json"
s = open(p, encoding="utf-8-sig").read().replace('"electricPoleWireReach": 9', '"electricPoleWireReach": 99')
open(p, "w", encoding="utf-8-sig").write(s)
PY
CI=true dotnet test --filter "FullyQualifiedName~PlannerDefaultsTest" --logger "console;verbosity=minimal"
```

Expected: FAIL with a diff showing 99 against 9. Then restore it:

```bash
dotnet test --filter "FullyQualifiedName~PlannerDefaultsTest" --logger "console;verbosity=minimal"
git diff --stat src/vue/src/lib/plannerDefaults.verified.json
```

Expected: PASS and no remaining diff. If `CI=true` did **not** fail, stop and investigate before continuing - the whole design rests on this gate.

- [ ] **Step 4: Exclude the generated file from formatting and linting**

`npm run format` rewrites all of `src/`, and reformatting a Verify-managed file would break the test. Follow the existing treatment of `src/lib/FactorioToolsApi.ts`.

In `src/vue/.prettierignore`, after the `src/lib/FactorioToolsApi.ts` line, add:

```
src/lib/plannerDefaults.verified.json
```

In `src/vue/eslint.config.ts`, extend the existing `ignores` array to include the same path:

```ts
ignores: ['**/dist/**', '**/public/framework/**', 'src/lib/FactorioToolsApi.ts', 'src/lib/plannerDefaults.verified.json'],
```

- [ ] **Step 5: Confirm formatting leaves the artifact alone**

```bash
cd src/vue && npm run format && cd ../.. && git diff --stat src/vue/src/lib/plannerDefaults.verified.json
```

Expected: no diff on the artifact.

- [ ] **Step 6: Point the C# at the emitter**

In `src/FactorioTools/OilField/OilFieldOptions.cs`, directly above `public class OilFieldOptions`, add:

```csharp
// These defaults and the For* presets below also ship to the Vue app, emitted by
// PlannerDefaultsTest into src/vue/src/lib/plannerDefaults.verified.json. Change a
// number here and `dotnet test` will rewrite that file; commit it with your change.
```

- [ ] **Step 7: Commit**

```bash
git add test/FactorioTools.Test/OilField/PlannerDefaultsTest.cs \
        src/vue/src/lib/plannerDefaults.verified.json \
        src/vue/.prettierignore src/vue/eslint.config.ts \
        src/FactorioTools/OilField/OilFieldOptions.cs
git commit -m "Emit planner defaults from C# for the Vue app"
```

---

### Task 2: The typed adapter

**Files:**
- Create: `src/vue/src/lib/plannerDefaults.ts`
- Test: `src/vue/src/lib/plannerDefaults.test.ts`

**Interfaces:**
- Consumes: `plannerDefaults.verified.json` from Task 1.
- Produces, all imported by Tasks 3 and 4:
  - `PLANNER_OPTION_DEFAULTS: Readonly<Record<string, string | number | boolean>>` - the raw `options` block.
  - `ELECTRIC_POLE_PRESETS: Record<string, ElectricPolePreset>` where `ElectricPolePreset = { width: number; height: number; supplyWidth: number; supplyHeight: number; wireReach: number }`.
  - `QUALITY_LEVELS: Record<string, number>`.
  - `strategyFlags(all: readonly string[], enabled: readonly string[]): Record<string, boolean>`.
  - `PIPE_STRATEGY_DEFAULTS` and `BEACON_STRATEGY_DEFAULTS`, both `Record<string, boolean>` keyed by strategy name.
  - `moduleNameAndCount(modules: Record<string, number>): { name: string; count: number }`.
  - `PUMPJACK_MODULE_DEFAULT` and `BEACON_MODULE_DEFAULT`, both `{ name: string; count: number }`.

- [ ] **Step 1: Write the failing test**

Create `src/vue/src/lib/plannerDefaults.test.ts`:

```ts
import { describe, expect, it } from "vitest"
import {
  BEACON_MODULE_DEFAULT,
  BEACON_STRATEGY_DEFAULTS,
  ELECTRIC_POLE_PRESETS,
  PIPE_STRATEGY_DEFAULTS,
  PLANNER_OPTION_DEFAULTS,
  PUMPJACK_MODULE_DEFAULT,
  QUALITY_LEVELS,
  moduleNameAndCount,
  strategyFlags,
} from "./plannerDefaults"

describe("the emitted artifact", () => {
  it("carries the planner geometry", () => {
    expect(PLANNER_OPTION_DEFAULTS.electricPoleSupplyWidth).toBe(7)
    expect(PLANNER_OPTION_DEFAULTS.electricPoleWireReach).toBe(9)
    expect(PLANNER_OPTION_DEFAULTS.beaconSupplyWidth).toBe(9)
    expect(PLANNER_OPTION_DEFAULTS.beaconWidth).toBe(3)
  })

  it("carries all five pole presets, including the modded one", () => {
    expect(Object.keys(ELECTRIC_POLE_PRESETS).sort()).toEqual([
      "big-electric-pole",
      "medium-electric-pole",
      "small-electric-pole",
      "small-iron-electric-pole",
      "substation",
    ])
    expect(ELECTRIC_POLE_PRESETS["substation"]).toEqual({
      width: 2,
      height: 2,
      supplyWidth: 18,
      supplyHeight: 18,
      wireReach: 18,
    })
  })

  it("carries the quality levels, including the skipped level 4", () => {
    expect(QUALITY_LEVELS).toEqual({ Normal: 0, Uncommon: 1, Rare: 2, Epic: 3, Legendary: 5 })
  })
})

describe("strategyFlags", () => {
  it("turns an enabled list into one boolean per strategy", () => {
    expect(strategyFlags(["A", "B", "C"], ["A", "C"])).toEqual({ A: true, B: false, C: true })
  })

  it("defaults every pipe strategy except FbeOriginal to on", () => {
    expect(PIPE_STRATEGY_DEFAULTS).toEqual({
      FbeOriginal: false,
      Fbe: true,
      ConnectedCentersDelaunay: true,
      ConnectedCentersDelaunayMst: true,
      ConnectedCentersFlute: true,
    })
  })

  it("defaults every beacon strategy except FbeOriginal to on", () => {
    expect(BEACON_STRATEGY_DEFAULTS).toEqual({ FbeOriginal: false, Fbe: true, Snug: true })
  })
})

describe("moduleNameAndCount", () => {
  it("reads the single module entry", () => {
    expect(moduleNameAndCount({ "speed-module-3": 2 })).toEqual({ name: "speed-module-3", count: 2 })
  })

  it("treats an empty dictionary as no module", () => {
    expect(moduleNameAndCount({})).toEqual({ name: "", count: 0 })
  })

  it("exposes the pumpjack and beacon defaults from the artifact", () => {
    expect(PUMPJACK_MODULE_DEFAULT).toEqual({ name: "productivity-module-3", count: 2 })
    expect(BEACON_MODULE_DEFAULT).toEqual({ name: "speed-module-3", count: 2 })
  })
})
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd src/vue && npx vitest run src/lib/plannerDefaults.test.ts
```

Expected: FAIL with "Cannot find module './plannerDefaults'".

- [ ] **Step 3: Write the adapter**

Create `src/vue/src/lib/plannerDefaults.ts`:

```ts
import artifact from "./plannerDefaults.verified.json"

// plannerDefaults.verified.json is generated by PlannerDefaultsTest in the .NET test
// suite and must not be hand-edited - `dotnet test` rewrites it from OilFieldOptions.
// This module is the only place that knows its shape; everything else imports from here.
//
// The artifact speaks C# vocabulary. The conversions to Vue vocabulary (strategy lists
// to per-strategy booleans, module dictionaries to a name and a count) live on this side
// on purpose, so front-end naming does not leak into the planner library.

export const PLANNER_OPTION_DEFAULTS = artifact.options

export type ElectricPolePreset = {
  width: number
  height: number
  supplyWidth: number
  supplyHeight: number
  wireReach: number
}

export const ELECTRIC_POLE_PRESETS: Record<string, ElectricPolePreset> =
  artifact.electricPolePresets

export const QUALITY_LEVELS: Record<string, number> = artifact.qualityLevels

/** One boolean per strategy: true when the strategy is in the enabled list. */
export function strategyFlags(
  all: readonly string[],
  enabled: readonly string[],
): Record<string, boolean> {
  const flags: Record<string, boolean> = {}
  for (const strategy of all) {
    flags[strategy] = enabled.includes(strategy)
  }
  return flags
}

const ALL_PIPE_STRATEGIES = [
  "FbeOriginal",
  "Fbe",
  "ConnectedCentersDelaunay",
  "ConnectedCentersDelaunayMst",
  "ConnectedCentersFlute",
] as const

const ALL_BEACON_STRATEGIES = ["FbeOriginal", "Fbe", "Snug"] as const

export const PIPE_STRATEGY_DEFAULTS = strategyFlags(
  ALL_PIPE_STRATEGIES,
  artifact.options.pipeStrategies,
)

export const BEACON_STRATEGY_DEFAULTS = strategyFlags(
  ALL_BEACON_STRATEGIES,
  artifact.options.beaconStrategies,
)

/**
 * The C# carries modules as a dictionary of name to count. The UI offers one module
 * and one count, so take the single entry, or nothing when the dictionary is empty.
 */
export function moduleNameAndCount(modules: Record<string, number>): {
  name: string
  count: number
} {
  const entries = Object.entries(modules)
  if (entries.length === 0) {
    return { name: "", count: 0 }
  }
  const [name, count] = entries[0]
  return { name, count }
}

export const PUMPJACK_MODULE_DEFAULT = moduleNameAndCount(artifact.options.pumpjackModules)
export const BEACON_MODULE_DEFAULT = moduleNameAndCount(artifact.options.beaconModules)
```

- [ ] **Step 4: Run the tests and the type check**

```bash
cd src/vue && npx vitest run src/lib/plannerDefaults.test.ts && npx vue-tsc --noEmit
```

Expected: all tests PASS, `vue-tsc` silent.

- [ ] **Step 5: Commit**

```bash
git add src/vue/src/lib/plannerDefaults.ts src/vue/src/lib/plannerDefaults.test.ts
git commit -m "Add the typed adapter over the emitted planner defaults"
```

---

### Task 3: Consume the artifact in the store and the pole presets

**Files:**
- Modify: `src/vue/src/stores/OilFieldStore.ts:6-49`
- Modify: `src/vue/src/lib/advancedOptions.ts` (the `ELECTRIC_POLE_PRESETS` block)
- Test: `src/vue/src/lib/plannerDefaults.test.ts` (add a coverage test)

**Interfaces:**
- Consumes: everything Task 2 produces.
- Produces: `getDefaults()` keeps its existing signature, `Readonly<OilFieldStoreState>`. No consumer of the store changes.

- [ ] **Step 1: Write the failing coverage test**

Append to `src/vue/src/lib/plannerDefaults.test.ts`:

```ts
import { getDefaults } from "../stores/OilFieldStore"

// Store keys with no counterpart in the C# planner options. Everything else in the
// store must come from the artifact, so a planner option added later cannot quietly
// become a TypeScript literal - the same enumerate-the-store approach that
// advancedOptions.test.ts uses.
const VUE_ONLY_KEYS = [
  "usingQueryString",
  "useStagingApi",
  "inputBlueprint",
  "useAdvancedOptions",
  "pumpjackModuleIsCustom",
  "beaconModuleIsCustom",
  "electricPoleIsCustom",
  "showProgress",
] as const

describe("store defaults", () => {
  it("takes every planner-derived value from the artifact", () => {
    const defaults = getDefaults() as Record<string, unknown>
    const options = PLANNER_OPTION_DEFAULTS as Record<string, unknown>

    const mismatched: string[] = []
    for (const [key, value] of Object.entries(defaults)) {
      if ((VUE_ONLY_KEYS as readonly string[]).includes(key)) {
        continue
      }
      if (key in options && options[key] !== value) {
        mismatched.push(key)
      }
    }
    expect(mismatched).toEqual([])
  })

  it("derives the strategy booleans from the artifact", () => {
    const defaults = getDefaults()
    expect(defaults.pipeStrategyFbeOriginal).toBe(PIPE_STRATEGY_DEFAULTS.FbeOriginal)
    expect(defaults.pipeStrategyFbe).toBe(PIPE_STRATEGY_DEFAULTS.Fbe)
    expect(defaults.pipeStrategyConnectedCentersDelaunay).toBe(
      PIPE_STRATEGY_DEFAULTS.ConnectedCentersDelaunay,
    )
    expect(defaults.pipeStrategyConnectedCentersDelaunayMst).toBe(
      PIPE_STRATEGY_DEFAULTS.ConnectedCentersDelaunayMst,
    )
    expect(defaults.pipeStrategyConnectedCentersFlute).toBe(
      PIPE_STRATEGY_DEFAULTS.ConnectedCentersFlute,
    )
    expect(defaults.beaconStrategyFbeOriginal).toBe(BEACON_STRATEGY_DEFAULTS.FbeOriginal)
    expect(defaults.beaconStrategyFbe).toBe(BEACON_STRATEGY_DEFAULTS.Fbe)
    expect(defaults.beaconStrategySnug).toBe(BEACON_STRATEGY_DEFAULTS.Snug)
  })

  it("derives the module selections from the artifact", () => {
    const defaults = getDefaults()
    expect(defaults.pumpjackModule).toBe(PUMPJACK_MODULE_DEFAULT.name)
    expect(defaults.beaconModule).toBe(BEACON_MODULE_DEFAULT.name)
    expect(defaults.beaconModuleSlots).toBe(BEACON_MODULE_DEFAULT.count)
  })
})
```

- [ ] **Step 2: Run it to verify it fails**

```bash
cd src/vue && npx vitest run src/lib/plannerDefaults.test.ts
```

Expected: FAIL - the store still declares its own literals, so at minimum the strategy and module assertions compare a literal against the artifact by identity and the coverage test reports keys.

If it passes by accident because every literal happens to agree with the C# today, that is expected - the literals are currently correct. Confirm the test is real by temporarily changing `electricPoleSupplyWidth` in the store to `70`, re-running to see it fail, then changing it back.

- [ ] **Step 3: Rewrite the store defaults**

In `src/vue/src/stores/OilFieldStore.ts`, replace the `defaults` object (currently lines 6-49) with:

```ts
import {
  BEACON_MODULE_DEFAULT,
  BEACON_STRATEGY_DEFAULTS,
  PIPE_STRATEGY_DEFAULTS,
  PLANNER_OPTION_DEFAULTS,
  PUMPJACK_MODULE_DEFAULT,
} from "../lib/plannerDefaults"

// Values that mirror the C# planner come from plannerDefaults.verified.json, which the
// .NET test suite generates from OilFieldOptions. Do not retype them here: the Vue app
// sends these explicitly in the plan request, so a stale copy would override the correct
// C# default and produce a wrong blueprint with a green test suite. See issue #54.
const defaults = {
  // Vue-only state, with no counterpart in the planner options.
  usingQueryString: false,
  useStagingApi: false,
  inputBlueprint: "",
  useAdvancedOptions: false,
  pumpjackModuleIsCustom: false,
  beaconModuleIsCustom: false,
  electricPoleIsCustom: false,
  showProgress: false,

  // Derived from the C#.
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
```

The key set and order are unchanged from the original object, so `storeToQuery` and `OilFieldStoreState` still line up. Do not add or remove keys in this step.

- [ ] **Step 4: Point the pole presets at the artifact**

In `src/vue/src/lib/advancedOptions.ts`, delete the local `ElectricPolePreset` type and the `ELECTRIC_POLE_PRESETS` literal, and re-export from the adapter so existing importers keep working:

```ts
import {
  ELECTRIC_POLE_PRESETS,
  type ElectricPolePreset,
} from "./plannerDefaults"

export { ELECTRIC_POLE_PRESETS }
export type { ElectricPolePreset }
```

Remove the "Mirrors the C# presets in OilFieldOptions.cs ... Kept in sync by hand" comment - it is no longer true.

- [ ] **Step 5: Run the whole Vue suite and the type check**

```bash
cd src/vue && npx vitest run && npx vue-tsc --noEmit && npx eslint src/
```

Expected: all tests PASS. `advancedOptions.test.ts` has a test named "agrees with the store defaults for the default pole" that must still pass - it now compares the artifact against a store that derives from the same artifact, which is weaker than before, so also confirm `plannerDefaults.test.ts` covers the same ground against the C#.

- [ ] **Step 6: Confirm the whole chain end to end**

This is the property the design exists for, so demonstrate it rather than assume it:

```bash
cd /Users/ericjohnson/GitHub/FactorioTools
sed -i '' 's/public int BeaconSupplyWidth { get; set; } = 9;/public int BeaconSupplyWidth { get; set; } = 11;/' src/FactorioTools/OilField/OilFieldOptions.cs
dotnet test --filter "FullyQualifiedName~PlannerDefaultsTest" --logger "console;verbosity=minimal"
grep beaconSupplyWidth src/vue/src/lib/plannerDefaults.verified.json
cd src/vue && npx vitest run src/lib/plannerDefaults.test.ts
```

Expected: the artifact now says 11, and the Vue tests still pass with no TypeScript edit. Then revert:

```bash
cd /Users/ericjohnson/GitHub/FactorioTools
git checkout src/FactorioTools/OilField/OilFieldOptions.cs
dotnet test --filter "FullyQualifiedName~PlannerDefaultsTest" --logger "console;verbosity=minimal"
git diff --stat
```

Expected: no remaining diff.

- [ ] **Step 7: Commit**

```bash
git add src/vue/src/stores/OilFieldStore.ts src/vue/src/lib/advancedOptions.ts src/vue/src/lib/plannerDefaults.test.ts
git commit -m "Derive the Vue store defaults and pole presets from the C#"
```

---

### Task 4: Consume the artifact for quality levels and module count

**Files:**
- Modify: `src/vue/src/lib/quality.ts:11-17`
- Modify: `src/vue/src/lib/quality.test.ts`
- Modify: `src/vue/src/lib/OilFieldPlanner.ts:68-75`

**Interfaces:**
- Consumes: `QUALITY_LEVELS` and `PUMPJACK_MODULE_DEFAULT` from Task 2.
- Produces: no new exports. `qualityLevel(quality)` keeps its signature.

- [ ] **Step 1: Rewrite the self-referential quality test**

`quality.test.ts` currently asserts `qualityLevel` against hardcoded literals, which is a second copy of the same assumption rather than a check of it. Replace that assertion block with one that goes through the artifact:

```ts
import { QUALITY_LEVELS } from "./plannerDefaults"

it("takes its levels from the C# Quality enum", () => {
  for (const quality of QUALITY_ORDER) {
    expect(qualityLevel(quality), quality).toBe(QUALITY_LEVELS[quality])
  }
})

it("keeps the hidden level 4 skipped, so Legendary is 5", () => {
  expect(QUALITY_LEVELS.Legendary).toBe(5)
  expect(QUALITY_LEVELS.Epic).toBe(3)
})
```

Keep every other test in the file as it is - the label, color and pip tests cover presentation that has no C# counterpart.

- [ ] **Step 2: Run it to verify it fails**

```bash
cd src/vue && npx vitest run src/lib/quality.test.ts
```

Expected: FAIL with "Cannot find module './plannerDefaults'" only if Task 2 was skipped; otherwise it should fail on the import of `QUALITY_LEVELS` into a `levels` table that is still a literal. If it passes immediately, temporarily change `Legendary` in `quality.ts` to `4`, confirm it fails, and change it back.

- [ ] **Step 3: Derive the levels from the artifact**

In `src/vue/src/lib/quality.ts`, replace the `levels` record with:

```ts
import { QUALITY_LEVELS } from "./plannerDefaults"

// The integer levels come from the C# Quality enum via plannerDefaults.verified.json.
// Factorio skips a hidden level 4, which is why Legendary is 5 and not 4.
const levels: Record<Quality, number> = {
  [Quality.Normal]: QUALITY_LEVELS.Normal,
  [Quality.Uncommon]: QUALITY_LEVELS.Uncommon,
  [Quality.Rare]: QUALITY_LEVELS.Rare,
  [Quality.Epic]: QUALITY_LEVELS.Epic,
  [Quality.Legendary]: QUALITY_LEVELS.Legendary,
}
```

- [ ] **Step 4: Derive the module count**

In `src/vue/src/lib/OilFieldPlanner.ts`, the `pumpjackModules` getter hardcodes the count. Replace the bare `2`:

```ts
import { PUMPJACK_MODULE_DEFAULT } from "./plannerDefaults"

  pumpjackModules: (state) => {
    const output: Record<string, number> = {}
    const module = state.pumpjackModule.trim()
    if (module) {
      output[module] = PUMPJACK_MODULE_DEFAULT.count
    }
    return output
  },
```

- [ ] **Step 5: Run everything**

```bash
cd src/vue && npx vitest run && npx vue-tsc --noEmit && npx eslint src/ && npx prettier --check src/
```

Expected: all PASS. Prettier must not report `plannerDefaults.verified.json`; if it does, Task 1 Step 4 was not applied.

- [ ] **Step 6: Commit**

```bash
git add src/vue/src/lib/quality.ts src/vue/src/lib/quality.test.ts src/vue/src/lib/OilFieldPlanner.ts
git commit -m "Derive the quality levels and module count from the C#"
```

---

### Task 5: Full verification and documentation

**Files:**
- Modify: `CLAUDE.md` (the front-ends section)

**Interfaces:**
- Consumes: everything above.
- Produces: nothing consumed by later tasks.

- [ ] **Step 1: Run the full suite on both sides**

```bash
cd /Users/ericjohnson/GitHub/FactorioTools
dotnet test --configuration Release --logger "console;verbosity=minimal"
cd src/vue && npm run test && npx vue-tsc --noEmit && npx eslint src/
```

Expected: 4262 or more .NET tests pass (4261 before this work, plus the emitter), and all Vue tests pass.

- [ ] **Step 2: Confirm the Vue build needs no .NET**

CI's `build-vue` job has no .NET SDK, so the committed artifact must be enough:

```bash
cd src/vue && npm run build
```

Expected: builds clean. This exercises `swagger-gen`, `vue-tsc` and `vite build` exactly as CI does.

- [ ] **Step 3: Confirm the Lua output is untouched**

The core C# gained only a comment, so `src/lua` should not move. Do not assume it:

```bash
cd /Users/ericjohnson/GitHub/FactorioTools
pwsh -File src/lua/Invoke-LuaBuild.ps1 > /dev/null 2>&1
git status --porcelain -- src/lua
```

Expected: no output. If `src/lua` did change, commit the regenerated files - the `transpile-lua` CI job will fail otherwise.

- [ ] **Step 4: Document the artifact**

In `CLAUDE.md`, in the bullet describing `src/vue`, add a sentence after the existing text:

```
Planner constants (pole presets, geometry defaults, strategy defaults, quality levels)
are not retyped in TypeScript - they come from `src/vue/src/lib/plannerDefaults.verified.json`,
which `PlannerDefaultsTest` generates from `OilFieldOptions`. Change a default in the C#
and `dotnet test` rewrites that file; commit it with your change.
```

- [ ] **Step 5: Commit and open the PR**

```bash
git add CLAUDE.md
git commit -m "Document the shared planner defaults artifact"
git push -u origin feat/shared-planner-defaults
```

Then open a PR against `main` with `Resolves #54` in the body.

---

## Self-Review

**Spec coverage:**

| Spec requirement | Task |
|---|---|
| Verify snapshot emitter, `UseStrictJson`, `UseDirectory`, `UseFileName` | 1 |
| Artifact shape: `options`, `electricPolePresets`, `qualityLevels` | 1 |
| All five presets including small-iron, not in the dropdown | 1, 2 |
| Prettier and eslint exclusions | 1 |
| Comment on the C# side pointing at the emitter | 1 |
| Group 1: planner defaults and pole presets | 3 |
| Group 2: quality integer levels | 4 |
| Group 3: default strategy selection | 2, 3 |
| Group 4: module counts | 2, 4 |
| Strategy and module conversions live in TypeScript | 2 |
| Store splits into derived and Vue-only, `OilFieldStoreState` unchanged | 3 |
| Test: nothing derived is a literal, enumerating the store | 3 |
| Test: strategy conversion round-trips | 2 |
| Test: `quality.test.ts` stops being self-referential | 4 |
| Verify freshness gate is real, not assumed | 1 (Step 3) |
| Demonstrate a C# change flowing through | 3 (Step 6) |
| Vue build needs no .NET | 5 (Step 2) |
| No Lua work, but confirm it did not move | 5 (Step 3) |

No gaps.

**Placeholder scan:** No TBD, TODO, "similar to Task N", or "add appropriate error handling". Every code step carries the actual code. The only deliberately open item is the exact key list under `options`, which Task 1 Step 1 pins down explicitly rather than leaving to judgment.

**Type consistency:** `ElectricPolePreset` has the same five fields in Task 1's C# `Preset` helper, Task 2's TypeScript type, and Task 3's re-export. `strategyFlags`, `moduleNameAndCount`, `PLANNER_OPTION_DEFAULTS`, `QUALITY_LEVELS`, `PIPE_STRATEGY_DEFAULTS`, `BEACON_STRATEGY_DEFAULTS`, `PUMPJACK_MODULE_DEFAULT` and `BEACON_MODULE_DEFAULT` are named identically in Task 2 where they are defined and in Tasks 3 and 4 where they are used.
