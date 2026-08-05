# Shared planner defaults between C# and the Vue app

Design for issue #54: planner constants are hand-copied across C#, Lua and Vue, and nothing fails when the copies diverge.

## Problem

The same game facts are typed out by hand on both sides of the C#/TypeScript boundary. Commit `aa62b44` is the clearest evidence - the same one-character edit, `ElectricPoleWireReach` from 30 to 32, applied three times in three files, with a commit message that says so.

The failure mode is silent and points the wrong way. The Vue app sends every geometry value explicitly in the plan request (`OilFieldPlanner.ts:47-52`), so a stale TypeScript number **overrides** the correct C# default. The C# `.verified.txt` snapshots keep passing, because they exercise the C# presets. The result is a wrong plan in the browser with a fully green test suite.

Nothing currently guards it:

- No codegen covers it. The swagger client emits types with all fields optional and no default values.
- No test. `quality.test.ts:7-11` asserts `qualityLevel` against hardcoded literals rather than consulting the C# enum. `persistence.test.ts` asserts against `getDefaults()` itself, which is self-referential.
- No type. The store's `defaults` object is its own source of truth (`OilFieldStoreState = typeof defaults`).

### What has already changed

The issue describes this as a three-way problem. It is now two-way.

`src/lua` is generated output that happens to be committed, and as of `e948e2c` the `transpile-lua` CI job runs `Invoke-LuaBuild.ps1` and fails if the committed output no longer matches. A stale Lua copy is now a red build. That copy needs nothing further here; it follows from the C# automatically.

## What is duplicated

Four groups, all in scope.

**1. Planner defaults and pole presets.** Roughly 30 numbers: the default pole (medium, supply 7x7, wire reach 9, size 1x1), the default beacon (supply 9x9, size 3x3), and five preset rows in `OilFieldOptions.ForSmallIronElectricPole` through `ForSubstation`, against `ELECTRIC_POLE_PRESETS` in `advancedOptions.ts` and `defaults` in `OilFieldStore.ts`. A divergence produces a wrong blueprint in the browser.

**2. Quality integer levels.** `Quality.cs` encodes that Factorio skips a hidden level 4, so Legendary is 5; `quality.ts` repeats it. The consequence is narrower than the issue implies: the Vue sends quality as a string and C# maps it to its own enum, so planning uses the C# value regardless. The TypeScript table feeds only the "Effective coverage at Legendary: supply 17x17, wire reach 19" readout in `ElectricPoleForm`. A divergence shows the user a wrong number, not a wrong plan.

**3. Default strategy selection.** Not listed in the issue. `DefaultPipeStrategies` and `DefaultBeaconStrategies` encode the same selection as the store's strategy booleans - FbeOriginal off, the rest on. A divergence silently changes which algorithms run: valid output, different plan quality and runtime.

**4. Module counts.** The bare literal `2` at `OilFieldPlanner.ts:72` and `beaconModuleSlots`, against the `PumpjackModules` and `BeaconModules` dictionaries.

## Approach

One generated artifact, produced by C# and consumed by the Vue app. Divergence becomes impossible rather than merely detected.

```
OilFieldOptions.cs + Quality.cs
        |
        v  [PlannerDefaultsTest, Verify]
src/vue/src/lib/plannerDefaults.verified.json   (committed)
        |
        +--> OilFieldStore.ts      (defaults)
        +--> advancedOptions.ts    (pole presets)
        +--> quality.ts            (integer levels)
        +--> OilFieldPlanner.ts    (module count)
```

### Why Verify rather than a build step

This adds no machinery. `SetVerifySettings.cs` already calls `VerifierSettings.AutoVerify(includeBuildServer: false)`, which gives exactly the contract wanted: locally, changing a C# default rewrites the JSON on the next `dotnet test` and the developer commits it; on CI, AutoVerify is off, so a stale committed copy fails with a diff.

That is the same contract `src/lua` now has, and it means the Vue build never needs the .NET SDK - the artifact is committed, so `npm install && npm run build` works in a checkout with no .NET at all. CI's `build-vue` job has no .NET and must stay that way.

`src/lib/FactorioToolsApi.ts` is precedent for a generated file living in the Vue tree: it is already excluded in `.prettierignore` and in `eslint.config`. The new artifact follows it. `.gitattributes` already covers `*.verified.json`, and `.gitignore` already covers `*.received.*`.

### Why not the alternatives

Extending `swagger-gen` was the issue's first option. It runs through WebApp's Swashbuckle and Microsoft.OpenApi stack, which is deliberately pinned (see the Renovate holds and issue on that migration), so it would couple this fix to a deferred upgrade. Rejected.

Generating a TypeScript module as a build step gives the strongest typing but couples the Vue build to the .NET SDK, or needs a freshness job of its own. Rejected as more cost for no additional guarantee.

Asserting equality in a test rather than consuming the artifact was considered and rejected: it leaves the copies in place, and every fact would still be written twice.

## Artifact shape

The JSON mirrors **C# vocabulary**, not Vue vocabulary, so the emitter never needs to know store key names.

```json
{
  "options": {
    "electricPoleEntityName": "medium-electric-pole",
    "electricPoleSupplyWidth": 7,
    "electricPoleSupplyHeight": 7,
    "electricPoleWireReach": 9,
    "electricPoleWidth": 1,
    "electricPoleHeight": 1,
    "beaconEntityName": "beacon",
    "beaconSupplyWidth": 9,
    "beaconSupplyHeight": 9,
    "beaconWidth": 3,
    "beaconHeight": 3,
    "addBeacons": true,
    "useUndergroundPipes": true,
    "optimizePipes": true,
    "overlapBeacons": true,
    "validateSolution": false,
    "pumpjackModules": { "productivity-module-3": 2 },
    "beaconModules": { "speed-module-3": 2 },
    "pipeStrategies": ["Fbe", "ConnectedCentersDelaunay", "ConnectedCentersDelaunayMst", "ConnectedCentersFlute"],
    "beaconStrategies": ["Fbe", "Snug"]
  },
  "electricPolePresets": {
    "small-iron-electric-pole": { "width": 1, "height": 1, "supplyWidth": 5, "supplyHeight": 5, "wireReach": 7.5 },
    "small-electric-pole": { "width": 1, "height": 1, "supplyWidth": 5, "supplyHeight": 5, "wireReach": 7.5 },
    "medium-electric-pole": { "width": 1, "height": 1, "supplyWidth": 7, "supplyHeight": 7, "wireReach": 9 },
    "big-electric-pole": { "width": 2, "height": 2, "supplyWidth": 4, "supplyHeight": 4, "wireReach": 32 },
    "substation": { "width": 2, "height": 2, "supplyWidth": 18, "supplyHeight": 18, "wireReach": 18 }
  },
  "qualityLevels": { "Normal": 0, "Uncommon": 1, "Rare": 2, "Epic": 3, "Legendary": 5 }
}
```

The exact key list under `options` is whatever `OilFieldOptions` actually declares at implementation time; the list above is the expected set, not a hand-maintained allowlist. The emitter reads the properties off a default-constructed `OilFieldOptions`.

Most C# property names camelCase to exactly the store's key names, so consumption is a direct spread. Two places do not line up and get a conversion on the TypeScript side:

- `pipeStrategies` / `beaconStrategies` (a list) becomes the store's per-strategy booleans.
- `pumpjackModules` / `beaconModules` (a dictionary) becomes a module name plus a count.

Keeping those conversions in TypeScript is deliberate. Putting them in C# would leak Vue naming into the planner library, which is meant to stay free of front-end concerns.

### small-iron-electric-pole

The C# has a fifth preset, `ForSmallIronElectricPole` (AAI Industry), that the Vue dropdown does not offer. The artifact carries all five so it stays a faithful mirror of the C# table. The dropdown keeps offering the four vanilla poles. Adding a modded pole to the UI is a product decision, not a deduplication one.

## Consumers

**`OilFieldStore.ts`.** `defaults` splits into the block derived from the artifact and the Vue-only keys - `usingQueryString`, `useStagingApi`, `inputBlueprint`, `useAdvancedOptions`, the `*IsCustom` flags, `addHeatPipes`, `showProgress`, and the five quality selections. `export type OilFieldStoreState = typeof defaults` keeps working, because the derived block has a concrete inferred type.

**`advancedOptions.ts`.** `ELECTRIC_POLE_PRESETS` is built from `electricPolePresets`, keeping its current `Record<string, ElectricPolePreset>` shape so `setKnownElectricPole` is unchanged.

**`quality.ts`.** The `levels` record is built from `qualityLevels`. The label, color, pip and pip-radius tables stay as they are - those are presentation, with no C# counterpart.

**`OilFieldPlanner.ts`.** The bare `2` in the `pumpjackModules` getter comes from the artifact's module count.

## Testing

**C# side.** `PlannerDefaultsTest` builds the object and verifies it, using Verify's `UseDirectory` to point at `src/vue/src/lib` and `UseFileName("plannerDefaults")`, which produces `src/vue/src/lib/plannerDefaults.verified.json`. The test is the emitter; there is no separate generator to keep in sync.

**TypeScript side**, three tests:

1. **Nothing that should be derived is a literal.** Enumerate the store keys, classify each as derived-from-the-artifact or Vue-only, and assert every derived key's value equals the artifact's. Same style as `advancedOptions.test.ts`: enumerate the store rather than a list, so a planner option added later has to be classified before it can pass.
2. **The strategy conversion round-trips.** The booleans derived from `pipeStrategies` convert back to the same list, for both pipe and beacon strategies.
3. **`quality.test.ts` stops being self-referential.** It currently asserts `qualityLevel` against hardcoded literals; it should assert against the artifact, so the hidden-level-4 skip is checked against the C# enum rather than against a second copy of the same assumption.

## Verification

- `dotnet test` passes, including the new snapshot test.
- `npm run test`, `vue-tsc` and eslint pass in `src/vue`.
- Deleting a value from the committed JSON and re-running `dotnet test` on a build-server-like run fails - confirming the freshness gate is real rather than assumed.
- Changing a C# default and running `dotnet test` rewrites the JSON, and the Vue tests then reflect the new value without any TypeScript edit. This is the property the whole design exists for and should be demonstrated, not assumed.

## Non-goals

- No `swagger-gen` or WebApp OpenApi changes.
- `small-iron-electric-pole` is not added to the dropdown.
- No Lua work; `transpile-lua` already covers that copy.
- The presentation tables in `quality.ts` (labels, colors, pips) stay in TypeScript. They have no C# counterpart and are not duplication.

## Risks

**Prettier and eslint must skip the artifact.** `npm run format` writes all of `src/`, and reformatting a Verify-managed file would break the snapshot on the next `dotnet test`. Mitigated by adding it to `.prettierignore` and the eslint ignore list, exactly as `FactorioToolsApi.ts` is handled. Worth confirming rather than assuming, since a reformat would show up as a confusing C# test failure.

**AutoVerify hides staleness locally.** A developer who changes a C# default sees the JSON rewritten and the test pass, and can forget to commit it. CI catches it. This is the existing contract for every other snapshot in the repo, so it is a known cost rather than a new one.

**The artifact is a fourth copy in one narrow sense.** It is generated and gated, so it cannot drift from the C# - but anyone reading `OilFieldOptions.cs` should be able to tell that the numbers ship elsewhere. A short comment on the C# side pointing at the emitter is worth including.
