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
- `src/vue` - the primary front-end (Vue 3 + Vite + Pinia, persisted settings). This is what's deployed to Cloudflare Pages (the `factoriotools` project, via `.github/workflows/deploy-cloudflare.yml`); it plans in-browser via the WASM bundle and no longer calls a hosted API.
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

The core + serialization libs are transpiled to Lua via the `CSharp.lua` submodule so the planner can run inside Factorio/Lua. Output lives in `src/lua`; rebuild with `src/lua/Invoke-LuaBuild.ps1` (PowerShell). Target is **Lua 5.2** - Factorio mods run on a modified Lua 5.2 environment, and the transpiled output is exercised with Lua 5.2.4 (see the "Lua performance log" in `README.md`).

- Avoid C# constructs the existing code avoids in hot paths under Lua settings: `yield return`, LINQ, named tuples, try/catch, and struct dictionary keys have all been removed for Lua performance.
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

## Conventions

- Use hyphens, not em/en dashes, in files.
