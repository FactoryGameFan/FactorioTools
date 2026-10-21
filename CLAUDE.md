# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

FactorioTools is a Factorio oil-field (outpost) blueprint planner. Given a blueprint with pumpjacks, it outputs a new blueprint wiring them with pipes, beacons, and electric poles, choosing pumpjack orientations and running several competing planning algorithms to return the best result. Core planning logic is in `src/FactorioTools/OilField`.

## Prerequisites

- **.NET SDK 10.0.402** (pinned in `global.json`, `rollForward: latestMajor`). This is a floor, not an exact pin - `latestMajor` means any newer 10.x SDK works, but an *older* one now fails, so raising this number raises the bar for every contributor. Keep it in sync with `global.json`; CI installs exactly this version via `setup-dotnet`'s `global-json-file`.

  **Renovate bumps `global.json` and cannot see this line**, so the two drift apart silently every time it does. #103 raised the pin 10.0.400 -> 10.0.401 and left this paragraph reading 10.0.400. After any SDK bump, `grep -rn` the old version string and fix whatever the bot could not reach.

  **Expect Homebrew to trail the pin.** Measured 2026-09-21, right after #103: `brew info dotnet` was still 10.0.400 while the pin had moved to 10.0.401, and `dotnet --version` inside the checkout failed outright with "A compatible .NET SDK was not found" - `rollForward` only ever rolls *forward*. That is not a broken checkout, it is the bar being raised ahead of the package manager. Until Homebrew catches up, build through `./docker-build.sh` (its `mcr.microsoft.com/dotnet/sdk:10.0` tag floats and already carries the newer SDK), or install the exact SDK from Microsoft.
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
npm run build-wasm     # build the Rust planner (rust/planner-wasm) and copy it into src/lib/planner-wasm
```

- The Vue app plans in-browser with the **Rust planner** (`rust/planner-wasm`), compiled to
  WebAssembly. `npm run build-wasm` in `src/vue` builds it (`cargo build --release -p
  planner-wasm --target wasm32-unknown-unknown`, run inside `rust/` so the pinned toolchain
  applies) and copies `planner.js` and `planner_wasm.wasm` into `src/vue/src/lib/planner-wasm/`.
  That folder is gitignored except for `planner.d.ts`, which types the wrapper, so run
  `build-wasm` before `npm run dev` or `npm run build`: the Vite build imports those files and
  fails without them. Vite gives the `.wasm` a content-hashed name under `/assets/`. Needs a
  Rust toolchain (rustup); no .NET.
- **Planning runs in a Web Worker** (`src/lib/plannerWorker.ts`), started by
  `src/lib/wasmPlanner.ts`, so the page stays responsive while it plans. If the worker cannot
  start (it fails to construct, reports an error, or does not answer within 15 s), the planner
  falls back for good to the main thread and logs one warning. Add `?planner=main-thread` to
  the page URL to force the fallback. The .NET build could never run in a worker (its runtime
  deadlocked in `create()`); the Rust module has no runtime, so it can.
  `deploy-cloudflare.yml` runs `build-wasm` and checks the planner landed in `dist/assets`, on
  pull requests as well as on deploys.

### Building without a local .NET 10 SDK (Docker / OrbStack)

If the machine only has a different SDK than the `global.json` pin, run the build inside the pinned SDK image instead of installing .NET 10. `docker-build.sh` wraps this (works with OrbStack or Docker Desktop) - it mounts the checkout, caches NuGet on the host, and runs as the current user so artifacts are not root-owned:

```bash
./docker-build.sh                                  # default: test the core dev loop
./docker-build.sh build src/FactorioTools/FactorioTools.csproj -c Release /p:UseLuaSettings=true
./docker-build.sh run --project src/FactorioTools.Cli -- oil-field sample
```

A full-solution `dotnet build` also builds `BrowserWasm`/`BlazorWebApp`, which need the wasm-tools workload - prepend a workload restore for that: `./docker-build.sh bash -c "dotnet workload restore && dotnet build -c Release"`.

### Devcontainer

`.devcontainer/` builds a Linux container carrying every toolchain this repo needs, which is more than the SDK: **Rust 1.98.1 (rustup 1.29.1, with rustfmt, clippy, rust-src and the wasm32 target), Node 24.21.0 and python3, plus dotnet 10.0.402 with wasm-tools, PowerShell 7.6.5 and Lua 5.2.4** for the C#, until it is removed. Before it, a full local loop meant separate host installs of each, one of which (`tools/check-lua.sh`) started a second container of its own.

It is not a replacement for `docker-build.sh`. That script is the quick way to run one `dotnet` command in the pinned SDK; this is the whole environment, with an editor attached.

**The Dockerfile pins the SDK from `global.json` by digest**, which is the point. `rollForward: latestMajor` only rolls forward, so a host whose package manager trails the pin cannot build the repo at all - that is what happened on 2026-09-21 when the pin moved to 10.0.401 and Homebrew still shipped 10.0.400. **When `global.json` moves, the `FROM` line must move with it.** The Devcontainer CI job compares `dotnet --version` against `global.json` and fails if they disagree, so a forgotten re-pin is a red check rather than a container that quietly contradicts the repo.

**Rust is pre-installed, and `rust/rust-toolchain.toml` still decides.** The image installs the toolchain named by `RUST_VERSION` in the Dockerfile, so creating the container downloads no compiler. Inside `rust/`, rustup reads the toolchain file as it does anywhere else. A stale `RUST_VERSION` therefore breaks nothing - rustup just installs the new pin on first use (about 10 s, measured) - which is exactly why nobody would notice. So the Devcontainer CI job compares `RUST_VERSION` with the toolchain file, checks that `rustc` and `cargo` in `rust/` report that version, and checks that the wasm32 target is installed. **When `rust/rust-toolchain.toml` moves, move `RUST_VERSION` with it.** The toolchain file is in that job's paths filter for this reason.

The container keeps its own `rust/target`, cargo registry and cargo git cache in named volumes, for the same reason as `node_modules`: Linux build output in the bind mount would fight a Mac host's own. `postCreateCommand` ends with `npm run build-wasm`, so `npm run dev` and `npm run build` work as soon as the container is up. The VS Code extension list includes rust-analyzer, pointed at `rust/Cargo.toml`.

Do not try to change the pre-installed toolchain's components from inside the container. `rustup target remove` failed there with `Invalid cross-device link (os error 18)`, because the files sit in a read-only image layer. Change the Dockerfile and rebuild instead. Installing another toolchain works.

Measured 2026-10-21 on tealc (Linux x86-64, 8 cores, Docker Desktop), with `@devcontainers/cli@0.89.0` running the Devcontainer job's own steps against a fresh clone with tags. Two full runs; where they differ, both times are shown:

| step | result |
| --- | --- |
| image build with `--no-cache`, base image already pulled | 2 min 58 s; the Rust layer took 17 s of it |
| `devcontainer up` | 45 s with the image cached, 88 s when the Rust layer had to be rebuilt. `postCreateCommand` took 34 s and 52 s, of which `build-wasm` took 17 s and 33 s and `npm ci` 10 s |
| toolchain check | Rust 1.98.1 in the image and in the toolchain file; `rustc` and `cargo` 1.98.1 in `rust/`; wasm32 target present |
| `cargo test --all-targets` in `rust/` | 175 passed, 7 ignored; 152 s and 188 s, of which about 11 s is compiling |
| `npm run build-wasm && npm run build && npm run test` in `src/vue` | build OK, 51 tests passed; 12.5 s and 14 s (`build-wasm` already up to date) |
| `dotnet test` on the test project | 4302 passed; 307 s and 241 s |
| Lua transpile step | reproduced the committed `src/lua`, parses and runs under 5.2; 41 s and 29 s |

The release gates (`cargo test --release -p snapshots -p contract -- --include-ignored`) also pass in the container, in 96 s. The Devcontainer job leaves them to `ci.yml`'s Rust job: they test the planner, not the container.

Measured 2026-09-21 in the built image on macOS arm64 (OrbStack), against a real clone with tags:

| step | result |
| --- | --- |
| `dotnet --version` inside the repo | 10.0.401, matching `global.json` |
| `dotnet test` on the test project | 4302 passed |
| `pwsh -File src/lua/Invoke-LuaBuild.ps1` | 88 files, reproduced the committed `src/lua` byte-for-byte |
| `luac5.2 -p` over the generated Lua | all 127 files parse |
| `lua5.2 sample.lua` | runs, prints the grid |
| `npm ci && npm run build && npm test` in `src/vue` | build OK, 51 tests passed |
| `npm run build-wasm` | Release publish OK, bundle lands at `public/framework/dotnet.js` (the .NET planner; `build-wasm` has built the Rust planner since stage 12b) |

Two notes on why specific things are the way they are:

- **Lua comes from apt, not source.** Ubuntu 24.04's `lua5.2` package is 5.2.4 - the exact version the README's Lua performance log was measured against, and the same package `ci.yml` installs. It provides `lua5.2` and `luac5.2`, the names the `transpile-lua` job uses.
- **No .NET 7 runtime is needed**, despite the CSharp.lua launcher targeting net7.0. `Invoke-LuaBuild.ps1` sets `DOTNET_ROLL_FORWARD=Major` so it runs on the .NET 10 runtime.

**`npm run dev` needs `--host`.** Vite binds `localhost` only, so a port forwarder dialling `127.0.0.1` gets a connection reset. Measured in the container: bare `npm run dev` printed `Network: use --host to expose` and the host got `HTTP 000` (connection reset by peer); with `npm run dev -- --host` the same request returned `HTTP 200`. So:

```bash
cd src/vue && npm run dev -- --host
```

**It does not do two things**, both of which need the host:

- **Capture the Factorio oracle.** `tools/capture-factorio-oracle.sh` reads a real Factorio install - `--dump-data` from the game binary, the bundled `migrations/*.json`, and `doc-html/runtime-api.json`. There is no Factorio in the container, and the installed binary is the authority on which version gets captured. The `python3` in the image covers the trim half only.
- **Run benchmarks.** `src/Benchmark` is BenchmarkDotNet, and container timings are too noisy to compare against the README's performance log. Run those on the host.

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
- `src/vue` - the primary front-end (Vue 3 + Vite + Pinia, persisted settings). This is what's deployed, as a Cloudflare Worker with static assets (see "Deployment" below); it plans in-browser via the WASM bundle and no longer calls a hosted API. Planner constants (pole presets, geometry defaults, strategy defaults, quality levels) are not retyped in TypeScript - they come from `src/vue/src/lib/plannerDefaults.verified.json`, which `PlannerDefaultsTest` generates from `OilFieldOptions` (most fields) and the `Quality` enum (`qualityLevels`). Change a default in the C# and `dotnet test` rewrites that file; commit it with your change.

  **Keep the FLUTE credit in the footer of `src/vue/src/App.vue`.** FLUTE (the `ConnectedCentersFlute` strategy, via the FluteSharp submodule) is under Chris Chu's Attribution Assurance License, `submodules/FluteSharp/flute-c/license.txt`. It requires a prominent display of "Dr. Chris C. N. Chu", "Iowa State University" and `http://home.engineering.iastate.edu/~cnchu/` every time the program launches. The footer shows on every page, which is what meets that. A Rust port of FLUTE would be under the same terms (#138).
- `src/BrowserWasm` - the planner as .NET WASM (trimmed). **The site no longer loads it**: it loads `rust/planner-wasm` instead (see the Rust port section). It stays until stage 13 of #138 removes the .NET projects, and the notes below describe how it was built.

  **It shipped as AOT with `Location` as a class, and the two flags only work together.** `BrowserWasm.csproj` sets `RunAOTCompilation=true`, and `build-wasm` passes `--property:LocationAsStruct=false`. That flag must stay on the command line: it is a global property that has to reach the referenced projects, and setting it in the `.csproj` does not. Measured 2026-10-06 in issue #139, against native .NET on the same machine:

  | Build | Planning speed | Time to first plan | Download (brotli) |
  | --- | --- | --- | --- |
  | Interpreter (shipped until #139) | 17x slower | 1.22 s | 1.35 MiB |
  | AOT, `Location` a struct | 14x slower | 0.78 s | 2.6 MiB |
  | **AOT, `Location` a class (shipped)** | **4.3x slower** | **0.54 s** | **2.5 MiB** |

  AOT alone barely helps because Mono's AOT falls back to the interpreter for generic code over value types, and `Location` is a struct by default. A `dotnet publish` of `BrowserWasm` without the property quietly gives you that middle row. The interpreter was first chosen in `docs/superpowers/specs/2026-06-18-in-browser-wasm-planner-design.md` (decision 2) for a smaller download and faster CI; the AOT publish takes about 64 s against 23 s. The `test-location-as-class` CI job tests the shipped setting on its own. Read #139 before changing either flag, and measure again after.
- `src/BlazorWebApp` - alternate Blazor host.
- `src/FactorioTools.Cli` (`System.CommandLine`) - `oil-field` subcommands `sample`, `normalize`, `sandbox`. Output assembly is `Knapcode.FactorioTools.Sandbox`.
- `src/Benchmark` - BenchmarkDotNet harness.

### Rust port (in progress): `rust/`
The planner is being ported to Rust/WASM, one stage per PR, following the plan on issue #138. The C# stays the live planner until the Rust version matches every Verify snapshot. Until then:

- **The port must reproduce the C# output exactly**, including how ties are broken. That depends on .NET details: `HashSet`/`Dictionary` enumeration order (insertion order, with freed slots reused last-in first-out), the 4-ary `PriorityQueue`, the unstable introsort behind `List.Sort`, and `Math.Round` rounding half to even. `rust/dotnet-compat` holds Rust copies of these. Use them, not std's `HashMap`, `BinaryHeap` or `sort_by`, wherever the C# result depends on order.
- **Those copies are tested against fixtures recorded from real .NET** (`rust/dotnet-compat/fixtures`, format in `rust/dotnet-compat/FIXTURES.md`). The `test-default` CI job reruns the recorder on the pinned SDK and fails if the fixtures drift, so an SDK bump that changes .NET's behavior shows up as a red check.
- **`rust/delaunator-net` and `rust/flute` are ports of the two submodules**, DelaunatorSharp and FluteSharp, kept byte-for-byte compatible with them. Do not swap in the `delaunator` crate: it is a newer algorithm whose tie-breaking differs, which changes the planner's pipes. `rust/flute` is under FLUTE's Attribution Assurance License (`rust/flute/LICENSE-FLUTE`), the same terms as the C#, so the site's footer credit covers it too.
- **`rust/snapshots` is the end-to-end gate.** It plans every case the C# Verify tests plan, from the same `small-list.txt` and `big-list.txt`, and compares the output with the committed `*.verified.txt` files byte for byte, plus `Score.HasExpectedScore`. It also fails if a snapshot file exists that no Rust case checks. Run it in release: `cargo test --release -p snapshots -- --include-ignored` (about a minute). `rust/blueprint` parses blueprint strings for it.
- **`rust/contract` and `rust/planner-wasm` are what the site loads.** `contract` is the JSON the site sends and gets back (`PlanOrchestrator`'s `Plan` and `Normalize`, the error envelope), checked against responses recorded from the C# (`rust/contract/fixtures`). `planner-wasm` builds it to WebAssembly with no extra toolchain, and `planner-wasm/js/planner.js` exposes `plan(json)` and `normalize(json)` with the same signatures as `src/vue/src/lib/wasmPlanner.ts`. CI checks the WebAssembly build's output against the native contract byte for byte. **The site loads this module** (stage 12b of #138), through `src/vue/src/lib/wasmPlanner.ts` and a Web Worker.
- **Commands**, from `rust/`: `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`, and `cargo build --workspace --target wasm32-unknown-unknown`. The toolchain is pinned in `rust/rust-toolchain.toml`; rustup installs it on first use.
- **A change to the C# planner during the port** has to land in the Rust version too, or wait for it. `rust/planner/fixtures` are recorded from the C# planner (format and the command to regenerate them in `rust/planner/FIXTURES.md`), and the `test-default` CI job reruns every recorder under `rust/`. So a C# change that alters planner output turns CI red until the fixtures are regenerated and the Rust port matches them.

## Deployment

The Vue app runs at `https://oilfieldplanner.factorygamefan.com` as a Cloudflare **Worker with static assets**, named `oilfieldplanner-factorygamefan-com`. It moved there from the Cloudflare Pages project `factoriotools` in issue #126.

- **Config:** `src/vue/wrangler.jsonc`. It is an assets-only Worker: there is no Worker script, and every request is served straight from `dist/`. It attaches the hostname as a Custom Domain. `workers_dev` and `preview_urls` are off, so the site has one public hostname.
- **CI deploys on every push to `main`.** `deploy-cloudflare.yml` runs `wrangler deploy` from `src/vue`, using the `wrangler` pinned in `package-lock.json`. Pull requests build and check the bundle but never deploy. So merging a PR that touches the site is a live deploy.
- **Client routes use `"not_found_handling": "404-page"`.** A path with no file gets `dist/404.html` with status 404. That file is the SPA shell (`npm run build` copies `index.html` to it), so a hard load of a client route like `/oil-field` still starts the app. The 404 status is what Pages returned too. Do not swap this for `"single-page-application"` without meaning to: that serves `index.html` with status 200, which changes what crawlers see.
- **Headers come from `src/vue/public/_headers`**, which Vite copies to `dist/` and Cloudflare reads without serving. It gives `/assets/*` a one-year `immutable` cache, since Vite puts a content hash in those file names. That includes the planner's `planner_wasm-<hash>.wasm` and the worker script, so a deploy always gets new URLs. (The old .NET bundle lived under `/framework/` with fixed names and so stayed on the default cache; it is gone.) HTML stays on the default (`max-age=0, must-revalidate`, with an ETag).
- **Two Pages default headers are restored in `_headers`.** Pages added `x-content-type-options: nosniff`, `referrer-policy: strict-origin-when-cross-origin` and `access-control-allow-origin: *` to every response, and a Worker adds none of them. The first two are back, in a `/*` block; `access-control-allow-origin: *` is left out on purpose: no other site is known to fetch these files. Add it back if one ever needs to.
- **The old Pages host still redirects.** The Pages project `factoriotools` stays alive only to 301 `factoriotools-5jg.pages.dev` to the new hostname, keeping the path. Its whole deploy is `tools/pages-redirect-stub/_redirects`. CI never touches it. Never deploy it while `oilfieldplanner.factorygamefan.com` is still attached to the Pages project, or that hostname redirects to itself. To redeploy it, from `src/vue`:

  ```bash
  npx wrangler pages deploy ../../tools/pages-redirect-stub --project-name factoriotools --branch main
  ```

Test locally against a real build (run `npm run build-wasm` first, or the Vite build fails for want of `src/lib/planner-wasm/planner.js`):

```bash
cd src/vue
npm run build
npx wrangler dev                 # serves dist/ as the Worker would, on http://localhost:8787
npx wrangler deploy --dry-run    # checks the config and reads the assets, uploads nothing
```

`wrangler dev` writes a `.wrangler/` state folder, which is gitignored.

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
- Friday Facts blog (Wube's own, so it counts as official): <https://factorio.com/blog/>. Best source for *why* a behavior changed. See the oracle section below for how to use it and its limits.
- Prefer official Factorio docs over forum/blog/wiki advice when changing runtime behavior. The Friday Facts blog is written by Wube, so it is on the official side of that line.

### The Factorio oracle (re-capture after a game update)

The planner hardcodes entity names, item names, direction values, and entity sizes. When Factorio changes any of those, nothing here notices - plans keep generating, they are just wrong. Factorio 2.0 renamed `effectivity-module-N` to `efficiency-module-N` and widened directions from 8-way to 16-way, and both went unnoticed for a long time.

So don't trust memory or the wiki. The game is the only authority on what the game accepts, and the capture reads three machine-readable sources:

| Source | Answers |
| --- | --- |
| `factorio --dump-data` | Every prototype: names, collision boxes, pipe connections, pole supply and wire reach, beacon stats |
| `data/*/migrations/*.json` | Every rename, as a table. This is a complete list, not a guess |
| `doc-html/runtime-api.json` | The `defines.*` tables, version-stamped to the install. See the caveat below: it publishes a documentation index, not the values |

Those three are *authoritative*: they say what the game accepts, they ship with the install, and a capture reads them offline and gets the same bytes every time.

Two more sources are *reference* rather than authoritative. They say **why** something changed, which is what tells you which captured value now needs review. Nothing automates either one, and nothing should.

- **`data/changelog.txt`** records behavior changes per patch, terse, in the install. No code in `tools/` touches it.
- **The Friday Facts blog, <https://factorio.com/blog/>**, is Wube's own writing, so it outranks forum, wiki and third-party blog advice. It is still not authoritative in the sense above - it describes intent, and the shipped game can differ, so check any claim from it against a capture. It is also the only source here that needs the network, which is why it can never gate a capture.

  **Searching it: `https://factorio.com/blog/search/<term>`.** This is the way in, and it covers the whole archive - a search for `mirroring` returns FFF #80 from 2015. Measured 2026-08-17: HTML only, no JSON, about 10 results with no total shown. **Word choice matters more than you would expect.** Searching `mirroring` does *not* return FFF #442, the post that introduced entity mirroring, while `flip` and `fluid` both do. So try two or three wordings before concluding a post does not exist. A bare number works too: `search/442`.

  Direct post URLs are `https://factorio.com/blog/post/fff-<number>`. There is also an Atom feed at <https://factorio.com/blog/rss>, but it carries the **10 most recent posts only**, so it answers "what changed lately" and not "what shipped in version X" - use the search for that.

  Worked example: the pumpjack change behind issue #81 shows up in a dump as `output_fluid_box` pipe connections going from `[[1,-1],[1,-1],[-1,1],[-1,1]]` to `[[1,-1],[1,1],[-1,1],[-1,-1]]`. The dump does not say why. FFF #442, "Flip, Flow, and Fresh Paint", does, and the same release added `use_mirroring` and `migrate_horizontal_mirroring` to that prototype - which is how you learn the change is about entity mirroring rather than a typo fix.

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
