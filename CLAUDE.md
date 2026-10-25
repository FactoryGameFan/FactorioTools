# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

FactorioTools is a Factorio oil-field (outpost) blueprint planner. Given a blueprint with pumpjacks, it outputs a new blueprint wiring them with pipes, beacons, and electric poles, choosing pumpjack orientations and running several competing planning algorithms to return the best result.

The planner is written in Rust, in `rust/` (core logic in `rust/planner`). The site, in `src/vue`, runs it in the browser as WebAssembly. The planner began as C# (.NET) and was ported to Rust one stage at a time in issue #138; stage 13b removed the C#, the .NET apps and the Lua build. See "History" under Architecture.

## Prerequisites

- **Rust, through rustup.** `rust/rust-toolchain.toml` pins the toolchain (1.99.1, with rustfmt, clippy and the `wasm32-unknown-unknown` target). Inside `rust/`, rustup reads that file and installs the pin on first use, so there is nothing to choose. The pin is deliberate: the planner's whole job is exact, repeatable output, and a compiler change is a codegen change.
- **Node 24 (Active LTS) and npm** for the front-end in `src/vue`. It uses npm, not pnpm: `package-lock.json` is the lockfile, and CI deploys with the `wrangler` pinned there.
- That is all for building and testing. There are no submodules and no .NET. `python3` and a real Factorio install are needed only to capture the Factorio oracle (see below).

## Common commands

```bash
# Rust planner (from rust/)
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test --all-targets                      # quick tests; the slow snapshot sets are #[ignore]d here
cargo test -p planner --test factorio_oracle  # one test file
cargo test --release -p snapshots -p contract -- --include-ignored   # release gate, about 1.5 min
cargo build --workspace --target wasm32-unknown-unknown
cargo run --release -p snapshots --bin normalize-lists   # re-normalize the test blueprint lists
cargo run --release -p bench                    # time the planner on the small list (see "Benchmark" below)

# Vue front-end (from src/vue)
npm ci
npm run build-wasm     # build the Rust planner (rust/planner-wasm) and copy it into src/lib/planner-wasm
npm run dev            # vite dev server
npm run build          # type-checks (vue-tsc), builds
npm test               # vitest
```

CI (`.github/workflows/ci.yml`) runs the five Rust check and build commands above, a three-field smoke run of the benchmark, plus a check that the WebAssembly build gives the same bytes as the native build (below), and builds and tests the front-end.

- The Vue app plans in-browser with the **Rust planner** (`rust/planner-wasm`), compiled to
  WebAssembly. `npm run build-wasm` in `src/vue` builds it (`cargo build --release -p
  planner-wasm --target wasm32-unknown-unknown`, run inside `rust/` so the pinned toolchain
  applies) and copies `planner.js` and `planner_wasm.wasm` into `src/vue/src/lib/planner-wasm/`.
  That folder is gitignored except for `planner.d.ts`, which types the wrapper, so run
  `build-wasm` before `npm run dev` or `npm run build`: the Vite build imports those files and
  fails without them. Vite gives the `.wasm` a content-hashed name under `/assets/`.
- **Planning runs in a Web Worker** (`src/lib/plannerWorker.ts`), started by
  `src/lib/wasmPlanner.ts`, so the page stays responsive while it plans. If the worker cannot
  start (it fails to construct, reports an error, or does not answer within 15 s), the planner
  falls back for good to the main thread and logs one warning. Add `?planner=main-thread` to
  the page URL to force the fallback. The old .NET build could never run in a worker (its
  runtime deadlocked in `create()`); the Rust module has no runtime, so it can.
  `deploy-cloudflare.yml` runs `build-wasm` and checks the planner landed in `dist/assets`, on
  pull requests as well as on deploys.
- **The WebAssembly build is checked against the native one.** The `rust` job in `ci.yml` replays every request in `rust/contract/fixtures/contract.jsonl` through the native contract (`cargo run --release -p contract --example replay`) and through the `.wasm` in Node (`rust/planner-wasm/tests/node-replay.mjs`), and fails on the first byte that differs.

### Devcontainer

`.devcontainer/` builds a Linux container carrying every toolchain this repo needs: **Rust 1.99.1 through rustup (with rustfmt, clippy, rust-src and the wasm32 target), Node 24.21.0, python3, git and gcc** (cargo's linker), on plain `ubuntu:26.04` pinned by digest. Node is pinned exactly and checksummed. The base already has user `ubuntu` at uid 1000, which is the container's `remoteUser`. Before stage 13b the image was built on the .NET SDK and also carried dotnet, PowerShell and Lua 5.2; those are gone with the C#.

**Rust is pre-installed, and `rust/rust-toolchain.toml` still decides.** The image installs the toolchain named by `RUST_VERSION` in the Dockerfile, so creating the container downloads no compiler. Inside `rust/`, rustup reads the toolchain file as it does anywhere else. A stale `RUST_VERSION` therefore breaks nothing - rustup just installs the new pin on first use (about 10 s, measured) - which is exactly why nobody would notice. So the Devcontainer CI job compares `RUST_VERSION` with the toolchain file, checks that `rustc` and `cargo` in `rust/` report that version, and checks that the wasm32 target is installed. **When `rust/rust-toolchain.toml` moves, move `RUST_VERSION` with it.** The toolchain file is in that job's paths filter for this reason.

The container keeps its own `rust/target`, cargo registry and cargo git cache in named volumes, for the same reason as `node_modules`: Linux build output in the bind mount would fight a Mac host's own. `postCreateCommand` ends with `npm run build-wasm`, so `npm run dev` and `npm run build` work as soon as the container is up. The VS Code extension list includes rust-analyzer, pointed at `rust/Cargo.toml`.

Do not try to change the pre-installed toolchain's components from inside the container. `rustup target remove` failed there with `Invalid cross-device link (os error 18)`, because the files sit in a read-only image layer. Change the Dockerfile and rebuild instead. Installing another toolchain works.

Measured 2026-10-21 on tealc (Linux x86-64, 8 cores, Docker Desktop), on the stage 13b image, with `@devcontainers/cli@0.89.0` running the Devcontainer job's own steps against a fresh clone with tags:

| step | result |
| --- | --- |
| image build | 118 s with the base image already pulled. 1.93 GB on disk, down from 4.97 GB on the old .NET SDK base |
| `devcontainer up` | 36 s with the image cached, of which `build-wasm` took 17 s and `npm ci` 7 s |
| toolchain check | Rust 1.98.1 in the image and in the toolchain file; `rustc` and `cargo` 1.98.1 in `rust/`; wasm32 target present; `cc` 13.3.0 |
| `cargo test --all-targets` in `rust/` | 190 passed, 7 ignored, in 175 s |
| `npm run build-wasm && npm run build && npm run test` in `src/vue` | build OK, 55 tests passed, 10.4 s (`build-wasm` already up to date) |

The release gates (`cargo test --release -p snapshots -p contract -- --include-ignored`) also pass in the container, in 96 s. The Devcontainer job leaves them to `ci.yml`'s Rust job: they test the planner, not the container.

**`npm run dev` needs `--host`.** Vite binds `localhost` only, so a port forwarder dialling `127.0.0.1` gets a connection reset. Measured in the container: bare `npm run dev` printed `Network: use --host to expose` and the host got `HTTP 000` (connection reset by peer); with `npm run dev -- --host` the same request returned `HTTP 200`. So:

```bash
cd src/vue && npm run dev -- --host
```

**It cannot capture the Factorio oracle.** `tools/capture-factorio-oracle.sh` reads a real Factorio install - `--dump-data` from the game binary, the bundled `migrations/*.json`, and `doc-html/runtime-api.json`. There is no Factorio in the container, and the installed binary is the authority on which version gets captured. The `python3` in the image covers the trim half only. Run the capture on the host.

## Architecture

### The Rust workspace: `rust/`

Nine crates, in `rust/Cargo.toml`. All of them build for `wasm32-unknown-unknown` as well as natively.

| crate | what it is |
| --- | --- |
| `planner` | The planner itself, with no JSON or serde. `planner::execute(options, blueprint)` runs the full pipeline; `execute_sample()` plans a fixed 4-pumpjack blueprint for tests. |
| `dotnet-compat` | Rust copies of the .NET collection, sort and rounding behavior the planner's output depends on (see "Ordering rules" below). |
| `delaunator-net` | Delaunay triangulation, a line-for-line port of DelaunatorSharp (MIT, `rust/delaunator-net/LICENSE`). |
| `flute` | FLUTE, the rectilinear Steiner minimal tree algorithm, ported from FluteSharp. Under FLUTE's Attribution Assurance License (`rust/flute/LICENSE-FLUTE`) and FluteSharp's MIT license (`rust/flute/LICENSE-FluteSharp`). Its lookup tables are vendored in `rust/flute/data`. |
| `blueprint` | Blueprint strings: reading them into the planner's types, and writing a planned grid back out, with .NET's JSON text formats so the output matches the snapshots. |
| `contract` | The JSON the site sends and gets back: `plan_json` and `normalize_json`, plus the error envelope. |
| `planner-wasm` | `contract` as a WebAssembly module with plain-number exports, so no binding generator is needed. `planner-wasm/js/planner.js` wraps it in `plan(json)` and `normalize(json)`. This is what the site loads. |
| `snapshots` | The end-to-end gate: plans every test case and compares the output with the committed snapshots in `testdata/oil-field/`, byte for byte. Also holds the `normalize-lists` binary. |
| `bench` | Times the planner on the test blueprint lists through `contract::plan_json`, the call the site makes (see "Benchmark" below). |

Inside `rust/planner/src`:

- `planner.rs` - the entry point and the loop over strategies.
- `steps/` - the pipeline: `initialize_context`, `add_pipes*` (pipe strategies), `plan_beacons*` (beacon strategies), `add_electric_poles`, `plan_underground_pipes`, `rotate_optimize`, `add_pipe_entities`, `validate`, `clean_blueprint`, plus heat pipes and FLUTE setup.
- The planner tries several **pipe strategies** (`FbeOriginal`, `Fbe`, `ConnectedCentersDelaunay`, `ConnectedCentersDelaunayMst`, `ConnectedCentersFlute`) and **beacon strategies** (`FbeOriginal`, `Fbe`, `Snug`) - see `PipeStrategy` and `BeaconStrategy` in `models.rs` - then picks the best plan (most beacon effects, then fewest beacons, then fewest pipes).
- `algorithms/` - A*, Dijkstra, Prim's, breadth-first search and Bresenham's line. Delaunay triangulation and FLUTE come from the `delaunator-net` and `flute` crates.
- `grid/` - the `SquareGrid` and its entity kinds (pumpjacks, pipes, beacons, poles, terminals); `location.rs` is a hot type.
- `containers/` - hand-rolled sets, dictionaries and bit arrays keyed by `Location`, and the sorted batches that pole placement uses.
- `data.rs` - entity names, item names and sizes, which the Factorio oracle checks (see below).

### Benchmark

`cargo run --release -p bench` (from `rust/`) plans each small-list blueprint 5 times through `contract::plan_json` with the site's default options, natively, and keeps each field's median. It prints the list's total, median, 90th percentile and maximum, and the five slowest fields. `--list big` or `--list all` adds the 1,147-field big list (about a minute per run), `--runs N` sets the runs per field, and `--limit N` plans only the first N fields of each list.

To measure a change, save a run before it and compare after:

```bash
cargo run --release -p bench -- --json /tmp/before.json   # on the old code
cargo run --release -p bench -- --baseline /tmp/before.json   # on the new code
```

The comparison gives the speed-up of the list's total and the per-field geometric mean, and says how many responses changed. Each field's input and response are stored as 64-bit FNV-1a hashes, and only fields found in both runs with the same input are compared. Fields missing from the baseline, or with a different blueprint, are skipped and listed on their own lines. So a change meant to keep the output identical must report `all N identical`, with N equal to the list's full field count and no skipped fields. The command prints the result but exits 0 even when responses changed, so read its output; the byte-for-byte proof is the snapshot release gate. Times depend on the machine and its load, so compare runs from the same machine only. Two back-to-back small-list runs on tealc agreed within about 1%. CI only smoke-tests the benchmark; it never checks timing. These are native times. The site runs the same code as WebAssembly in a browser, so its times differ.

Measured on tealc (8 cores, Linux), 2026-10-21. Small list: 5 runs per field, median kept. Big list: one run each.

| code | small list (61 fields) | big list (1,147 fields) | slowest big field (#84, 134 pumpjacks) |
| --- | ---: | ---: | ---: |
| end of the port (#178) | 1.46 s | 58.0 s | 5.4 s |
| after #179 (fast hasher for the .NET-style sets) | 1.08 s | 44.7 s | not measured |
| after #182 (A* scratch in flat arrays) | 0.91 s | 35.8 s | 3.5 s |

Every response stayed identical through both changes.

### Profiling

samply works on tealc (`pacman --sync samply`). It needs `sudo sysctl --write kernel.perf_event_paranoid=1`, which resets at every reboot. Build the benchmark with line tables into a separate target dir, so the normal build is untouched, then record without opening a browser:

```bash
CARGO_PROFILE_RELEASE_DEBUG=line-tables-only CARGO_TARGET_DIR=/tmp/prof-target cargo build --release -p bench
samply record --save-only --unstable-presymbolicate --rate 2000 --output prof.json.gz -- /tmp/prof-target/release/bench --list big --limit 93 --runs 1
```

The first 93 big-list fields include the five slowest and take about 10 s. To rank functions and lines from the saved profile, use `llvm-addr2line`: GNU `addr2line` cannot read the LTO line tables and prints `??`. After #182, A* is about 68% of the time, and a third of all time is its heuristic's scan over every goal (`a_star.rs`, `heuristic`). That scan is already vectorized. Sorting the goals by x with an early exit was 5% slower, and an iterator `min` made no difference, so a real gain there needs a different algorithm. Next comes `NetPriorityQueue::dequeue`, at about 9.5% self.

### Ordering rules

- **Planner output must not change by accident**, including how ties are broken. The committed snapshots were recorded from the C# planner, and the Rust port reproduces them byte for byte. That depends on .NET details: `HashSet`/`Dictionary` enumeration order (insertion order, with freed slots reused last-in first-out), the 4-ary `PriorityQueue`, the unstable introsort behind `List.Sort`, and `Math.Round` rounding half to even. `rust/dotnet-compat` holds Rust copies of these. Use them, not std's `HashMap`, `BinaryHeap` or `sort_by`, wherever the result depends on order.
- **A lookup-only container can use anything fast.** When nothing ever enumerates a container, its storage cannot change the output. The index inside `NetHashSet`/`NetDictionary` uses `dotnet_compat::FastHasher` (FxHash, no seed), `LocationMap`/`LocationLookup` use `LocationHasher`, and A* keeps its cost and came-from data in flat per-cell arrays on a `SquareGrid` (`GridNeighbors::bounds`). Before swapping one in, check that nothing iterates it. Then run `bench --baseline`, which must show `all N identical` for every field with none skipped (it compares hashes and does not fail on a change; see "Benchmark"), and the snapshot release gate, which checks the output byte for byte.
- **Do not swap in the `delaunator` crate** for `delaunator-net`. It is a newer algorithm whose tie-breaking differs, which changes the planner's pipes. `rust/flute` likewise matches FluteSharp tree for tree.
- **The fixtures are frozen.** `rust/*/fixtures` hold inputs and outputs recorded from the C# code (the planner, DelaunatorSharp, FluteSharp and .NET itself) at commit `0532d923`, and each crate's tests check the Rust against them. Each crate's `FIXTURES.md` gives the format. The recorders that wrote them were C# programs, deleted in stage 13b, so the fixtures cannot be re-recorded. A deliberate change that one of them pins means editing that fixture by hand and saying why in the commit.

### Front-end: `src/vue`

- The site (Vue 3 + Vite + Pinia, persisted settings). This is what's deployed, as a Cloudflare Worker with static assets (see "Deployment" below). It plans in-browser with `rust/planner-wasm`, through `src/vue/src/lib/wasmPlanner.ts` and a Web Worker, and calls no hosted API.
- Planner constants (pole presets, geometry defaults, strategy defaults, quality levels) are not retyped in TypeScript. They come from `src/vue/src/lib/plannerDefaults.verified.json`, which the Rust test `rust/contract/tests/planner_defaults.rs` generates from `OilFieldOptions` and `Quality`. The test fails when the file is stale. After changing a default, run `UPDATE_PLANNER_DEFAULTS=1 cargo test -p contract --test planner_defaults` from `rust/` and commit the rewritten file. Adding a field to `OilFieldOptions` without emitting it fails to compile.
- `src/vue/src/lib/FactorioToolsApi.ts` holds the request and response types for the Rust planner, hand-maintained since stage 13a (it began as a swagger-generated client). `FactorioToolsApi.test.ts` checks it against the plan and normalize responses in `rust/contract/fixtures/contract.jsonl`: key shape, value types, and every strategy and quality name. When the contract changes, update the types and the test's samples together.
- **Keep the FLUTE credit in the footer of `src/vue/src/App.vue`.** FLUTE (the `ConnectedCentersFlute` strategy, in `rust/flute`) is under Chris Chu's Attribution Assurance License, `rust/flute/LICENSE-FLUTE`. It requires a prominent display of "Dr. Chris C. N. Chu", "Iowa State University" and `http://home.engineering.iastate.edu/~cnchu/` every time the program launches. The footer shows on every page, which is what meets that.
- The site's "Add sample" blueprints are read from `testdata/oil-field/small-list.txt` at build time (`src/vue/vite.config.ts`).

### History

The planner was first written in C#, with a CLI, an ASP.NET API, a Blazor host, a BenchmarkDotNet harness, a Lua build for running inside Factorio, and a .NET WebAssembly build that the site loaded. Issue #138 ported it to Rust stage by stage. Stage 12b (#174) switched the site to the Rust build, and stage 13b deleted the C# and everything built from it. To read the old code, check out a commit from before that stage, such as `ee83ffcc` (the stage 13a merge).

The dated plans and specs in `docs/superpowers/` are kept as history and are not updated. Most describe the C# code and name files that no longer exist. For example, `docs/superpowers/specs/2026-06-18-in-browser-wasm-planner-design.md` is the design of the .NET WebAssembly planner that the site ran before the Rust one.

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

## Factorio reference

- API docs root: <https://lua-api.factorio.com/latest/index.html>
- Runtime API: <https://lua-api.factorio.com/latest/index-runtime.html>
- Libraries/functions Factorio adds or modifies (incl. `require()` restrictions): <https://lua-api.factorio.com/latest/auxiliary/libraries.html>
- Lua 5.2 manual, for writing probe mods: <https://www.lua.org/manual/5.2/>
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

`tools/capture-factorio-oracle.sh` pulls those three into `testdata/oil-field/factorio-oracle.json`. The oracle checks are `rust/planner/tests/factorio_oracle.rs`, plus an entity-size check in the tests module of `rust/blueprint/src/write.rs`. Rust has no reflection, so `entity_names::vanilla::ALL` and `item_names::vanilla::ALL` in `rust/planner/src/data.rs` list the names by hand, and `data::tests::all_arrays_match_their_constants` fails if a name constant is added without adding it to `ALL`.

```bash
tools/capture-factorio-oracle.sh                        # auto-detects a Steam or /Applications install
tools/capture-factorio-oracle.sh --check                # report drift, change nothing, exit 1 on mismatch
tools/capture-factorio-oracle.sh --factorio /path/to/factorio.app
```

Notes on using it:

- **The committed fixture targets the experimental branch**, currently 2.1.14, not stable. That is deliberate: the bug reports come from 2.1 and that is where the game is heading. `captureInfo.factorioVersion` in the fixture records which build it came from.
- **Stable and experimental are not identical.** Comparing 2.0.77 stable against 2.1.14 experimental, everything the planner reads is byte-identical except one thing: the pumpjack's output fluid box went from 2 distinct corners to 4, one per rotation (FFF #442). That difference did mean the hardcoded terminal offsets (then in the C# `Helpers.cs`, now `TERMINAL_OFFSETS` in `rust/planner/src/helpers.rs`) were wrong for 2.1 on east and west, which is issue #81, now fixed. A probe mod settled it by reading `PipeConnection.target_position` out of both running games rather than deriving it; `terminal_offsets_match_the_factorio_output_corners` in `rust/planner/tests/factorio_oracle.rs` now pins the offsets to the fixture so the next corner move fails a test. Capture any second version with `--factorio <path>` and `--out <path>` to compare.
- **Re-capture after every Factorio update and commit the diff.** A changed fixture is the signal that a hardcoded constant needs review. `--check` answers "has the game moved past what we committed?" without dirtying the tree.
- **The installed binary is the authority** on which version gets captured. Steam updates it without asking, so it decides and everything else follows. Same convention as `scripts/sync-factorio-refs.sh` in FactorioMapWebUI.
- **It runs with user mods disabled** (`--mod-directory` pointed at an empty directory). Mods rewrite prototypes freely, so a capture that loads them describes one person's modded game rather than Factorio. The script prints which mods loaded; expect only `core base elevated-rails quality recycler space-age`.

  Careful with what that buys. Measured on 2.1.14: **an empty mod directory keeps out user mods and nothing else.** Factorio rewrites `mod-list.json` at startup and adds back every bundled mod the file does not mention, with `enabled: true`. The file this script writes names only `base`, and all six still load. An explicit `enabled: false` is honoured, so naming a mod is the only way to get a smaller game than the install ships with. Loading the full set is the right default here, since that is what the fixture records, but "the directory is empty" must not be read as "only base is loaded".
- **`defines` values in the fixture are inferred, not read.** `runtime-api.json` has no value field at all: across all 1,554 entries the only keys are `name`, `order` and `description`, and `trim-factorio-oracle.py:150` uses `order`. That is right today only because Factorio declares directions clockwise from `north = 0` with no gaps, and a dense index cannot express a gap, a duplicate, or a non-zero start. Issue #83. Reading the real value needs a probe mod; `factorio-oracle` does that now, and confirmed the two agree on 2.1.14.
- **CI never runs the capture** and needs no Factorio install - it reads the committed fixture. That is why the fixture is committed rather than generated on demand.
- Capture needs `python3` (for JSON trimming) and a Factorio install. Neither is needed to build or test.
- The `entity_names::aai_industry` names in `data.rs` come from a mod, so they are deliberately absent from a vanilla capture. That is expected, not drift.
- Output is deterministic: two captures of the same install are byte-identical.

**This script now has a replacement, and it is proven equivalent.**
[`FactoryGameFan/factorio-oracle`](https://github.com/FactoryGameFan/factorio-oracle)
is a shared Rust CLI doing the same job for four repos. Its acceptance test
reproduces the committed `factorio-oracle.json` **byte for byte** from a real
2.1.14 install, so this is a checked claim rather than an intention:

```bash
factorio-oracle run  --probe dump-data.json --work-dir /tmp/w > /tmp/run.json
factorio-oracle trim --run /tmp/run.json --spec trim-spec.json \
  --out testdata/oil-field/factorio-oracle.json [--check]
```

The allowlists that live in `trim-factorio-oracle.py` move into that `trim-spec.json`
unchanged. **This script stays** regardless: the agreed migration rule across the four
repos is new probes only, so nothing existing changes until there is a reason to touch
it. See issue #82.

Two related sources, for when the game binary is not the easiest thing to reach:

- **`wube/factorio-data`** (cloned at `~/GitHub/factorio-data`) is the official prototype source, tagged per version. Its `*/migrations/*.json` files are byte-identical to the installed game's, so renames can be checked with no Factorio install at all. Only the resolved geometry from `--dump-data` genuinely needs the binary.
- **A throwaway mod is the way to generate blueprint fixtures.** Docs describe prototypes, not what the blueprint exporter writes. A mod whose `on_init` calls `stack.set_blueprint_entities{...}` then `helpers.write_file(name, stack.export_stack())`, run headless via `factorio --create <save> --mod-directory <dir>`, produces a real blueprint string stamped with the real game version. This is how the direction values and the `mirror` field were established rather than assumed. Check runtime API names against `doc-html/runtime-api.json` first - `game.write_file` became `helpers.write_file` in 2.0.

## Testing notes

- **The planner's tests are Rust** (`cargo test`, from `rust/`). The main ones compare planner output with the committed `*.verified.txt` snapshots under `testdata/oil-field/`, byte for byte: `rust/snapshots` plans every case and also fails if a snapshot file exists that no case checks. The full sets are slow, so they are `#[ignore]`d in debug builds; run them in release with `--include-ignored`, as CI does. The front-end has its own vitest suite (`npm test` in `src/vue`).
- **The snapshots are still in the Verify library's file format**, because the C# tests that first wrote them used Verify: a UTF-8 byte order mark, then the text, with LF line ends. `.gitattributes` pins `*.verified.txt` (and `.xml`, `.json`) to `eol=lf` because the comparison is byte for byte. Never edit a snapshot by hand; regenerate it (see "Changing planner output on purpose").
- `Score.HasExpectedScore.verified.txt` is the planner-quality scoreboard across the 61 blueprints in `small-list.txt` (the file also holds 11 `#` comment lines, so do not read its line count as the blueprint count). `small-list.txt` / `big-list.txt` hold the blueprint corpus; `big-list.txt` is 1147 blueprints and is not scored.
- **Adding a test blueprint.** Paste the blueprint string as a new line at the end of `testdata/oil-field/small-list.txt` (scored, may hold `#` comments), or in `big-list.txt` (not scored, blueprints only). Then, from `rust/`:

  ```bash
  cargo run --release -p snapshots --bin normalize-lists
  UPDATE_SNAPSHOTS=1 cargo test --release -p snapshots -- --include-ignored
  cargo test --release -p snapshots -p contract -- --include-ignored
  ```

  The first command cleans each blueprint down to its pumpjacks, drops repeats, and moves a blueprint found in both lists to the small list only. It compares blueprints by their decoded JSON, not the compressed string, and leaves a line that is already normalized exactly as written, so the diff shows only real changes. A line that does not parse stops it with the file and line number; fix or remove the line, or pass `--keep-invalid`. (It replaced the C# CLI's `oil-field normalize`, which on .NET 10 would have rewritten all 1,208 lines because the committed strings came from an older zlib.) The second command writes the new snapshots, and the third checks everything in compare mode. A new small-list blueprint at index N creates 26 files: 24 `PlannerWithAllOptions/Theory.E_<N>_<pole>{_b}{_o}{_u}.verified.txt`, plus `AllStrategiesOnSmallList/Theory.E_<N>.verified.txt` and `NonStandardBeacon/Theory.E_<N>.verified.txt`. It also changes `Score.HasExpectedScore.verified.txt`. If the blueprint came out of `big-list.txt` at index M, every `AllStrategiesOnBigList/Theory.E_<n>` from M on shifts down one and the last is deleted. No test has a count to edit. Commit the lists and every new, changed and deleted snapshot together.
- **Changing planner output on purpose.** From `rust/`, `UPDATE_SNAPSHOTS=1 cargo test --release -p snapshots -- --include-ignored` rewrites every snapshot and the scoreboard from the Rust output: it creates missing files, overwrites changed ones, and deletes any `*.verified.txt` no case produces. Only the value `1` turns it on, and CI never sets it, so a stale snapshot fails there. Run it in release and with `--include-ignored`, or only the quick `PlannerTest` snapshots are written. Stage 13a checked that Rust regenerates all 2,748 files byte for byte and that Verify accepted files Rust wrote. Read the diff before committing: it is the only record of what the change did to real plans. A change like this may also break a frozen fixture under `rust/*/fixtures` (see "Ordering rules").

## Dependency automation

Dependency updates are handled by **Renovate**, configured in `.github/renovate.json5`. It is JSON5 so the reasoning lives in comments beside each rule - the comments are load-bearing, not decoration.

- **Updates arrive as one weekly batch**, Monday morning `America/Los_Angeles`. Security fixes deliberately skip that window. To pull a run forward, tick the "trigger a request for Renovate to run again" checkbox on the dependency dashboard.
- **Nothing automerges.** `automerge: false` is global with no exceptions. A green CI run proves the repo is *consistent*, not that a bump is *correct* - and planner correctness lives in the snapshots, where a subtle output change surfaces as a snapshot diff to review rather than an obvious failure.
- **There are deliberate holds.** Before "fixing" a version that looks stale, read the rule that holds it. Each carries its reason and the condition under which to revisit.
- **The dependency dashboard issue is the live inventory** of everything held back, everything queued, and everything detected. It is a better place to look than reading manifests by hand.
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
