# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

FactorioTools is a Factorio oil-field (outpost) blueprint planner. Given a blueprint with pumpjacks, it outputs a new blueprint wiring them with pipes, beacons, and electric poles, choosing pumpjack orientations and running several competing planning algorithms to return the best result. Core planning logic is in `src/FactorioTools/OilField`.

## Prerequisites

- **.NET SDK 10.0.302** (pinned in `global.json`, `rollForward: latestMajor`). This is a floor, not an exact pin - `latestMajor` means any newer 10.x SDK works, but an *older* one now fails, so raising this number raises the bar for every contributor. Keep it in sync with `global.json`; CI installs exactly this version via `setup-dotnet`'s `global-json-file`.
- **Git submodules are required** (`FluteSharp`, `delaunator-sharp`, `CSharp.lua`). Clone/update with `git submodule update --init --recursive`. CI checks out with `submodules: recursive`.
- Node 24 (Active LTS) for the Vue front-end.
- The browser-WASM project needs `dotnet workload restore`.

## Common commands

```bash
# Build / test the .NET solution
dotnet build
dotnet test                                                 # all tests (xUnit + Verify snapshots)
dotnet test --filter "FullyQualifiedName~PlannerTest.ExecuteSample"   # single test

# Run the planner sample from the CLI (prints the grid)
dotnet run --project src/FactorioTools.Cli -- oil-field sample
dotnet run --project src/FactorioTools.Cli -- oil-field normalize     # re-normalize test blueprint lists

# Benchmarks
dotnet run --project src/Benchmark -c Release

# Vue front-end (from src/vue)
npm install
npm run dev            # vite dev server
npm run build          # regenerates TS API client from swagger, type-checks, builds
npm run swagger-gen    # regenerate src/lib API client from ../WebApp/swagger.json
npm run build-wasm     # publish BrowserWasm and copy the bundle (incl. dotnet.js) into public/framework
```

- The Vue app plans in-browser via .NET WASM. After changing C# planner code, run
  `npm run build-wasm` in `src/vue` to refresh `public/framework` (the bundle,
  incl. `dotnet.js`, lives in `public/framework`). Note: `src/vue/public/` is
  gitignored and absent in a fresh checkout, so the copy must create it first
  (`mkdir -p public`) - otherwise `cp -r .../_framework public/...` flattens the
  bundle's contents into the `public/` root and the `framework/dotnet.js` import
  404s on the deployed site. Requires the .NET 10 SDK plus the wasm-tools
  workload; without a local .NET 10 SDK, publish via `./docker-build.sh` (the SDK image
  also needs `python3` on PATH for the emscripten native relink step) and copy
  `src/BrowserWasm/bin/Release/net10.0/browser-wasm/AppBundle/_framework` into
  `src/vue/public/framework`. `npm run dev` / `npm run build` serve those assets.

### Building without a local .NET 10 SDK (Docker / OrbStack)

If the machine only has a different SDK than the `global.json` pin, run the build inside the pinned SDK image instead of installing .NET 10. `docker-build.sh` wraps this (works with OrbStack or Docker Desktop) - it mounts the checkout, caches NuGet on the host, and runs as the current user so artifacts are not root-owned:

```bash
./docker-build.sh                                  # default: test the core dev loop
./docker-build.sh build src/FactorioTools/FactorioTools.csproj -c Release /p:UseLuaSettings=true
./docker-build.sh run --project src/FactorioTools.Cli -- oil-field sample
```

A full-solution `dotnet build` also builds `BrowserWasm`/`BlazorWebApp`, which need the wasm-tools workload - prepend a workload restore for that: `./docker-build.sh bash -c "dotnet workload restore && dotnet build -c Release"`.

## Architecture

### Core library: `src/FactorioTools` (`Knapcode.FactorioTools`)
Pure planning logic with **no JSON/serialization dependencies** - this is intentional so the project can be transpiled to Lua (see Lua section). Key areas under `OilField/`:
- `Planner.cs` - entry point. `Planner.Execute(options, blueprint)` runs the full pipeline; `ExecuteSample()` builds a fixed 4-pumpjack blueprint for demos/tests.
- `Steps/` - the pipeline, roughly in numbered order: `InitializeContext`, `AddPipes.*` (pipe strategies), `PlanBeacons.*` (beacon strategies), `AddElectricPoles`, `PlanUndergroundPipes`, `RotateOptimize`, `AddPipeEntities`, `Validate`, `CleanBlueprint`.
- The planner tries multiple **pipe strategies** (`FbeOriginal`, `Fbe`, `ConnectedCentersDelaunay`, `ConnectedCentersDelaunayMst`, `ConnectedCentersFlute`) and **beacon strategies** (`FbeOriginal`, `Fbe`, `Snug`) - see `Models/PipeStrategy.cs` / `BeaconStrategy.cs` - then selects the best plan (most beacon effects, then fewest beacons, then fewest pipes).
- `Algorithms/` - graph/geometry primitives (A*, Dijkstra, Prim's, BFS, Bresenham's line). Delaunay triangulation and the FLUTE rectilinear-Steiner-tree algorithm come from the submodules.
- `Grid/` - the `SquareGrid` and entity types (`PumpjackCenter`, `Pipe`, `BeaconCenter`, `ElectricPoleCenter`, `Terminal`, etc.); `Location.cs` is a hot type.
- `Containers/` - hand-rolled set/dictionary implementations keyed by `Location` (e.g. `LocationBitSet`, `LocationIntSet`, `LocationHashSet`). Which one is used is controlled by build symbols below; these exist for performance and Lua compatibility.

### Serialization: `src/FactorioTools.Serialization`
Blueprint string parsing and emitting live here, separate from the core lib: `ParseBlueprint`, `GridToBlueprintString`, `NormalizeBlueprints`, plus the `System.Text.Json` source-gen context. Front-ends and the CLI reference this project, not just the core.

### Front-ends and hosts
- `src/WebApp` - ASP.NET Core API (`OilFieldController`, routes under `api/v1/oil-field`: `normalize`, `plan`; the actions delegate to `PlanOrchestrator`). Produces `swagger.json` consumed by the Vue client's `swagger-gen`. No longer deployed (the Azure target was retired when the front-end moved to in-browser WASM); kept for local API use, swagger generation, and the `Dockerfile` if self-hosting is wanted.
- `src/vue` - the primary front-end (Vue 3 + Vite + Pinia, persisted settings). This is what's deployed to Cloudflare Pages (the `factoriotools` project, via `.github/workflows/deploy-cloudflare.yml`); it plans in-browser via the WASM bundle and no longer calls a hosted API. Planner constants (pole presets, geometry defaults, strategy defaults, quality levels) are not retyped in TypeScript - they come from `src/vue/src/lib/plannerDefaults.verified.json`, which `PlannerDefaultsTest` generates from `OilFieldOptions` (most fields) and the `Quality` enum (`qualityLevels`). Change a default in the C# and `dotnet test` rewrites that file; commit it with your change.
- `src/BrowserWasm` - runs the planner fully client-side via .NET WASM AOT (trimmed). Lets the SPA plan without the API.
- `src/BlazorWebApp` - alternate Blazor host.
- `src/FactorioTools.Cli` (`System.CommandLine`) - `oil-field` subcommands `sample`, `normalize`, `sandbox`. Output assembly is `Knapcode.FactorioTools.Sandbox`.
- `src/Benchmark` - BenchmarkDotNet harness.

## Performance build flags (and Lua compatibility)

The core library is heavily perf-tuned through conditional compilation, configured in `Directory.Build.props`. Many features can be toggled per-build with MSBuild properties, e.g.:

```bash
dotnet build /p:UseHashSets=false
dotnet build /p:LocationAsStruct=false
dotnet build /p:UseLuaSettings=true     # turns OFF the perf features that Lua can't use
```

Flags include `UseHashSets` (`USE_HASHSETS`), `UseBitArray` (`USE_BITARRAY`), `LocationAsStruct` (`LOCATION_AS_STRUCT`), `UseSharedInstances`, `UseVectors`, `UseStackalloc`, `RentNeighbors`, `AllowDynamicFluteDegree`, `EnableVisualizer`, `EnableGridToString`. `UseLuaSettings=true` sets the Lua-safe combination. **CI builds the solution under many of these combinations** (see `.github/workflows/ci.yml`) - if you touch core data structures, build/test under both default and `UseLuaSettings=true` before assuming it's green.

## Lua transpilation

The core lib is transpiled to Lua via the `CSharp.lua` submodule so the planner can run inside Factorio/Lua. `FactorioTools.Serialization` is **not** transpiled - `Invoke-LuaBuild.ps1` never passes it, which is why the core must stay free of JSON/serialization dependencies (see the core library section above). Output lives in `src/lua`; rebuild with `src/lua/Invoke-LuaBuild.ps1` (PowerShell). Target is **Lua 5.2** - Factorio mods run on a modified Lua 5.2 environment, and the transpiled output is exercised with Lua 5.2.4 (see the "Lua performance log" in `README.md`).

The `transpile-lua` CI job runs that script, fails if the committed `src/lua` no longer matches what the C# transpiles to, and syntax-checks the result with `luac5.2`. So if you change the core, regenerate `src/lua` and commit it in the same change.

- Avoid C# constructs the existing code avoids in hot paths under Lua settings: `yield return`, LINQ, named tuples, try/catch, and struct dictionary keys have all been removed for Lua performance. LINQ is worse than a perf problem: `CoreSystem.lua` does not load `Collections.Linq`, so `Invoke-LuaBuild.ps1` never copies `Linq.lua` and LINQ transpiles into calls on a module that was never shipped - a runtime failure inside Factorio, not a build error.
- Keep control flow deterministic: Factorio modifies `pairs()` and `math.random()` for determinism, so prefer simple, stable iteration and avoid order-dependent assumptions.
- Syntax-check generated Lua with `for f in src/lua/**/*.lua; luac5.2 -p $f; end` (fish). `luac5.2`/`lua5.2` only validate syntax, not Factorio runtime APIs.

### Factorio reference

- API docs root: <https://lua-api.factorio.com/latest/index.html>
- Runtime API: <https://lua-api.factorio.com/latest/index-runtime.html>
- Libraries/functions Factorio adds or modifies (incl. `require()` restrictions): <https://lua-api.factorio.com/latest/auxiliary/libraries.html>
- Lua 5.2 manual: <https://www.lua.org/manual/5.2/>
- Prefer official Factorio docs over forum/blog/wiki advice when changing runtime behavior.

## Testing notes

- Tests use **xUnit + Verify** (`Verify.Xunit`). Many tests assert against committed `*.verified.txt` snapshots under `test/FactorioTools.Test/OilField`. When behavior legitimately changes, update snapshots via Verify's accept workflow (received vs verified) rather than editing expected files by hand.
- `Score.HasExpectedScore.verified.txt` is the planner-quality scoreboard across the 57 test blueprints; `small-list.txt` / `big-list.txt` hold the blueprint corpus.
- Test data blueprints are normalized via the CLI `oil-field normalize` command.

## Dependency automation

Dependency updates are handled by **Renovate**, configured in `.github/renovate.json5`. It is JSON5 so the reasoning lives in comments beside each rule - the comments are load-bearing, not decoration.

- **Updates arrive as one weekly batch**, Monday morning `America/Los_Angeles`. Security fixes deliberately skip that window. To pull a run forward, tick the "trigger a request for Renovate to run again" checkbox on the dependency dashboard.
- **Nothing automerges.** `automerge: false` is global with no exceptions. A green CI run proves the repo is *consistent*, not that a bump is *correct* - and planner correctness lives in Verify snapshots, where a subtle output change surfaces as a snapshot diff to accept rather than an obvious failure.
- **There are deliberate holds.** Before "fixing" a version that looks stale, read the rule that holds it. Each carries its reason and the condition under which to revisit. Several also mirror a comment in the corresponding `.csproj`; keep the two in sync if you change either.
- **The dependency dashboard issue is the live inventory** of everything held back, everything queued, and everything detected. It is a better place to look than reading manifests by hand - it is how a missing hold on `swashbuckle.aspnetcore.cli` (which lives in `.config/dotnet-tools.json`, not a `.csproj`) was found.
- Holds that a migration would lift have a tracking issue named in the rule. Holds waiting on an external circumstance say explicitly that they have no issue *by design*, so an absent issue reads as a decision rather than an oversight.

### Editing the config

Validate every change - run this from **outside** the project root if a package manager is ever pinned via `devEngines` in `package.json`, or a bare `npx` fails with `EBADDEVENGINES` (not currently an issue here):

```bash
npx --yes --package renovate -- renovate-config-validator .github/renovate.json5
```

This matters more than normal linting because of the **silent failure mode**: the app runs with "Require config file" enabled, so if this file is absent or unparseable *on the default branch*, Renovate does nothing at all, silently. That is indistinguishable from "no updates available". If the bot appears to go quiet, check the config parses before assuming there is nothing to update.

Two related traps:

- Only **one** Renovate config file may exist. A second copy (e.g. at the repo root) is a config error, not an override - and the validator cannot catch it, since it only ever sees the file you hand it.
- Prefer matching by package name alone over narrowing with `matchDepTypes` / `matchFileNames`. A guard that does not match what Renovate actually reports silently matches nothing, and no validator catches that either.

## Conventions

- Use hyphens, not em/en dashes, in files.
