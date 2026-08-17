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
  incl. `dotnet.js`, lives in `public/framework`). The copy must land as
  `public/framework/`, not flattened into `public/` - a flattened bundle puts
  `dotnet.js` at the root, the `framework/dotnet.js` import 404s, and the site
  loads but cannot plan. `build-wasm` does `mkdir -p public` first to guarantee
  that. (Only `src/vue/public/framework/` is gitignored, not all of
  `src/vue/public/`; the directory itself exists in a fresh checkout because
  `_worker.js` is tracked in it, so the `mkdir -p` is belt-and-braces rather
  than load-bearing.) `deploy-cloudflare.yml` runs `build-wasm` and asserts the
  resulting shape, on pull requests as well as on deploys, so this is now
  checked rather than trusted. Requires the .NET 10 SDK plus the wasm-tools
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
### Checking the generated Lua locally

Run `tools/check-lua.sh`. It does exactly what the `transpile-lua` CI job does, against the same Lua version, in Docker:

1. syntax-checks every generated file with `luac` 5.2.4
2. runs `sample.lua`, which is the step that actually matters

Those two are not redundant. LINQ transpiles cleanly *and* parses cleanly - it emits `local Linq = System.Linq.Enumerable`, which is nil because `Collections.Linq` is not in the CoreSystem load list. It only fails when the module loads (`attempt to index field 'Linq'`), so the check has to run the planner, not just parse it. That takes well under a second.

Use the script rather than your own `luac`. Homebrew no longer ships Lua 5.2, so a Mac checkout typically has 5.4 or newer, and parsing 5.2-targeted code with a 5.5 parser proves very little. The image (`nickblah/lua:5.2-alpine`, ~11MB) is Lua 5.2.4 - the exact version the performance log above was measured against. Needs Docker (OrbStack or Docker Desktop); regenerate first with `pwsh src/lua/Invoke-LuaBuild.ps1` if you changed the core.

### Factorio reference

- API docs root: <https://lua-api.factorio.com/latest/index.html>
- Runtime API: <https://lua-api.factorio.com/latest/index-runtime.html>
- Libraries/functions Factorio adds or modifies (incl. `require()` restrictions): <https://lua-api.factorio.com/latest/auxiliary/libraries.html>
- Lua 5.2 manual: <https://www.lua.org/manual/5.2/>
- Prefer official Factorio docs over forum/blog/wiki advice when changing runtime behavior.

### The Factorio oracle (re-capture after a game update)

The planner hardcodes entity names, item names, direction values, and entity sizes. When Factorio changes any of those, nothing here notices - plans keep generating, they are just wrong. Factorio 2.0 renamed `effectivity-module-N` to `efficiency-module-N` and widened directions from 8-way to 16-way, and both went unnoticed for a long time.

So don't trust memory or the wiki. The game is the only authority on what the game accepts, and the capture reads three machine-readable sources:

| Source | Answers |
| --- | --- |
| `factorio --dump-data` | Every prototype: names, collision boxes, pipe connections, pole supply and wire reach, beacon stats |
| `data/*/migrations/*.json` | Every rename, as a table. This is a complete list, not a guess |
| `doc-html/runtime-api.json` | The `defines.*` tables, version-stamped to the install. See the caveat below: it publishes a documentation index, not the values |

A fourth source, `data/changelog.txt`, records behavior changes per patch. It is worth reading after an update, but nothing automates it - no code in `tools/` touches it.

`tools/capture-factorio-oracle.sh` pulls those three into `test/FactorioTools.Test/OilField/factorio-oracle.json`:

```bash
tools/capture-factorio-oracle.sh                        # auto-detects a Steam or /Applications install
tools/capture-factorio-oracle.sh --check                # report drift, change nothing, exit 1 on mismatch
tools/capture-factorio-oracle.sh --factorio /path/to/factorio.app
```

Notes on using it:

- **The committed fixture targets the experimental branch**, currently 2.1.14, not stable. That is deliberate: the bug reports come from 2.1 and that is where the game is heading. `captureInfo.factorioVersion` in the fixture records which build it came from.
- **Stable and experimental are not identical.** Comparing 2.0.77 stable against 2.1.14 experimental, everything the planner reads is byte-identical except one thing: the pumpjack's output fluid box went from 2 distinct corners to 4, one per rotation (FFF #442). That difference did mean the hardcoded terminal offsets in `Helpers.cs` were wrong for 2.1 on east and west, which is issue #81, now fixed. A probe mod settled it by reading `PipeConnection.target_position` out of both running games rather than deriving it; `FactorioOracleTest.TerminalOffsetsMatchTheFactorioOutputCorners` now pins the offsets to the fixture so the next corner move fails a test. Capture any second version with `--factorio <path>` and `--out <path>` to compare.
- **Re-capture after every Factorio update and commit the diff.** A changed fixture is the signal that a hardcoded constant needs review. `--check` answers "has the game moved past what we committed?" without dirtying the tree.
- **The installed binary is the authority** on which version gets captured. Steam updates it without asking, so it decides and everything else follows. Same convention as `scripts/sync-factorio-refs.sh` in FactorioMapWebUI.
- **It runs with user mods disabled** (`--mod-directory` pointed at an empty directory). Mods rewrite prototypes freely, so a capture that loads them describes one person's modded game rather than Factorio. The script prints which mods loaded; expect only `core base elevated-rails quality recycler space-age`.

  Careful with what that buys. Measured on 2.1.14: **an empty mod directory keeps out user mods and nothing else.** Factorio rewrites `mod-list.json` at startup and adds back every bundled mod the file does not mention, with `enabled: true`. The file this script writes names only `base`, and all six still load. An explicit `enabled: false` is honoured, so naming a mod is the only way to get a smaller game than the install ships with. Loading the full set is the right default here, since that is what the fixture records, but "the directory is empty" must not be read as "only base is loaded".
- **`defines` values in the fixture are inferred, not read.** `runtime-api.json` has no value field at all: across all 1,554 entries the only keys are `name`, `order` and `description`, and `trim-factorio-oracle.py:150` uses `order`. That is right today only because Factorio declares directions clockwise from `north = 0` with no gaps, and a dense index cannot express a gap, a duplicate, or a non-zero start. Issue #83. Reading the real value needs a probe mod; `factorio-oracle` does that now, and confirmed the two agree on 2.1.14.
- **CI never runs the capture** and needs no Factorio install - it reads the committed fixture. That is why the fixture is committed rather than generated on demand.
- Capture needs `python3` (for JSON trimming) and a Factorio install. Neither is needed to build or test.
- `EntityNames.AaiIndustry` names come from a mod, so they are deliberately absent from a vanilla capture. That is expected, not drift.
- Output is deterministic: two captures of the same install are byte-identical.

**This script now has a replacement, and it is proven equivalent.**
[`FactoryGameFan/factorio-oracle`](https://github.com/FactoryGameFan/factorio-oracle)
is a shared Rust CLI doing the same job for four repos. Its acceptance test
reproduces the committed `factorio-oracle.json` **byte for byte** from a real
2.1.14 install, so this is a checked claim rather than an intention:

```bash
factorio-oracle run  --probe dump-data.json --work-dir /tmp/w > /tmp/run.json
factorio-oracle trim --run /tmp/run.json --spec trim-spec.json \
  --out test/FactorioTools.Test/OilField/factorio-oracle.json [--check]
```

The allowlists that live in `trim-factorio-oracle.py` move into that `trim-spec.json`
unchanged. **This script stays** regardless: the agreed migration rule across the four
repos is new probes only, so nothing existing changes until there is a reason to touch
it. See issue #82.

Two related sources, for when the game binary is not the easiest thing to reach:

- **`wube/factorio-data`** (cloned at `~/GitHub/factorio-data`) is the official prototype source, tagged per version. Its `*/migrations/*.json` files are byte-identical to the installed game's, so renames can be checked with no Factorio install at all. Only the resolved geometry from `--dump-data` genuinely needs the binary.
- **A throwaway mod is the way to generate blueprint fixtures.** Docs describe prototypes, not what the blueprint exporter writes. A mod whose `on_init` calls `stack.set_blueprint_entities{...}` then `helpers.write_file(name, stack.export_stack())`, run headless via `factorio --create <save> --mod-directory <dir>`, produces a real blueprint string stamped with the real game version. This is how the direction values and the `mirror` field were established rather than assumed. Check runtime API names against `doc-html/runtime-api.json` first - `game.write_file` became `helpers.write_file` in 2.0.

## Testing notes

- Tests use **xUnit v3 + Verify** (`xunit.v3` + `Verify.XunitV3`). Many tests assert against committed `*.verified.txt` snapshots under `test/FactorioTools.Test/OilField`. When behavior legitimately changes, update snapshots via Verify's accept workflow (received vs verified) rather than editing expected files by hand. A stale committed snapshot **does** fail on CI: `AutoVerify` is off there, so the test itself throws `VerifyException` with the diff (confirmed on a real runner, not just inferred - note that setting `CI`/`GITHUB_ACTIONS` locally does *not* reproduce the build-server detection, so local simulation of this is misleading). A "Check no Verify snapshots drifted" step backs that up by failing if `dotnet test` leaves any `*.verified.*` file dirty, in case the detection ever regresses. Commit regenerated snapshots with your change.
- `Score.HasExpectedScore.verified.txt` is the planner-quality scoreboard across the 61 blueprints in `small-list.txt` (the file also holds 11 `#` comment lines, so do not read its line count as the blueprint count). `small-list.txt` / `big-list.txt` hold the blueprint corpus; `big-list.txt` is 1147 blueprints and is not scored.
- Test data blueprints are normalized via the CLI `oil-field normalize` command.

## Dependency automation

Dependency updates are handled by **Renovate**, configured in `.github/renovate.json5`. It is JSON5 so the reasoning lives in comments beside each rule - the comments are load-bearing, not decoration.

- **Updates arrive as one weekly batch**, Monday morning `America/Los_Angeles`. Security fixes deliberately skip that window. To pull a run forward, tick the "trigger a request for Renovate to run again" checkbox on the dependency dashboard.
- **Nothing automerges.** `automerge: false` is global with no exceptions. A green CI run proves the repo is *consistent*, not that a bump is *correct* - and planner correctness lives in Verify snapshots, where a subtle output change surfaces as a snapshot diff to accept rather than an obvious failure.
- **There are deliberate holds.** Before "fixing" a version that looks stale, read the rule that holds it. Each carries its reason and the condition under which to revisit. Several also mirror a comment in the corresponding `.csproj`; keep the two in sync if you change either.
- **The dependency dashboard issue is the live inventory** of everything held back, everything queued, and everything detected. It is a better place to look than reading manifests by hand - it is how a missing hold on `swashbuckle.aspnetcore.cli` (which lives in `src/WebApp/.config/dotnet-tools.json` - a tool manifest under the project, not at the repo root, and not a `.csproj`) was found.
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
