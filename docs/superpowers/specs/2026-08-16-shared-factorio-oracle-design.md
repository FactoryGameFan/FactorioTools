# A shared Factorio oracle CLI

Status: design agreed, not yet built.
Tracking issues: FactorioTools#82, factorio-blueprint-editor#235,
FactorioMapWebUI#232, FactorioWikiDamageThresholds#11.
Related: FactorioTools#83.

This spec lives here because FactorioTools already has the `docs/superpowers/specs/`
convention and no oracle repo exists yet. Move or copy it into that repo once it is
created.

## The problem

Four repos depend on facts that only Factorio can answer. Three of them run the
game headless to ask. Each wrote the same plumbing from scratch, and a fourth copy
turned up in a repo that has nothing to do with Factorio.

| Repo | Oracle tooling | Lines | Install discovery |
| --- | --- | --- | --- |
| FactorioTools | `tools/capture-factorio-oracle.sh` + `trim-factorio-oracle.py` | 421 | own copy |
| factorio-blueprint-editor | `tools/oracle/`, 18 probes | 7,900 | own copy, x18 |
| FactorioMapWebUI | `test/oracle/` | 9,504 | own copy |
| FactorioWikiDamageThresholds | a hand-rolled serpent mod, output committed | - | none |

The plumbing does not scale with the question. In factorio-blueprint-editor it runs
**50 to 69 lines with a median of 56**, across probes ranging from 79 to 872 lines
total - 1,071 lines in all. In FactorioMapWebUI the same ten-step sequence appears
four times (249 lines). That is about 1,320 lines of near-identical plumbing in
those two repos alone, before counting the roughly 80% of FactorioTools' 421 lines
that is equally generic. More to the point, it is about 23 independent copies, and
so 23 independent chances to get it subtly wrong.

That risk is measured, not theoretical:

- **14 of 18** factorio-blueprint-editor probes hardcode `factorio_version: '2.1'`.
  Six of those call `--version` anyway, but only to stamp a fixture. A mismatch
  makes Factorio skip the mod in silence; the run ends on "No dump" and nothing
  names the cause.
- **Only 1 of 18** checks the binary exists before spawning.
- **None of the three repos has a timeout.** A hung game hangs the capture forever.
- FactorioWikiDamageThresholds' committed 21.3 MB dump is Lua serpent rather than
  JSON, is truncated mid-capture, and carries no version stamp - and about 40
  hardcoded values derived from it feed public wiki pages.

## What this is

A single Rust binary, `factorio-oracle`, in its own public repo. It owns the
plumbing. It does not own the questions, and it does not own the analysis.

**JSON in, JSON out.** A consumer describes a probe, the tool runs it, the consumer
reads the result and compares it against its own model.

### Why the analysis cannot be shared

This is the constraint that decides the whole shape. A probe compares the game
against **the consumer's own reimplementation**, so its analysis has to run in the
consumer's language.

factorio-blueprint-editor's README says so directly about `probe-rail-placement.mjs`:
it reads `packages/exporter/data/output/data.json` rather than reading a bounding
box back out of the game, because the question is what `PositionGrid`'s integer
tile grid sees, so the footprint has to come from the same data `getEntitySize`
reads. FactorioMapWebUI's probes compare against `fmw-noise`; FactorioTools'
compare against C#.

So a shared *probe framework* is impossible. A shared *runner* is not.

### Why Rust

The three repos pin Node at 24.19.0, 26.7.0, and nothing. FactorioMapWebUI also
declares `devEngines.packageManager: pnpm 11.18.0`, which already breaks `npx`-based
tools there. A shared Node library would run under whichever Node the shell
inherited, two majors apart depending on the directory.

A single static binary has no such problem, and it serves a .NET repo, a Node repo,
a Rust repo and a Python repo identically. Since the analysis stays with the
consumer, the tool's language is invisible across the boundary - so it should be
whichever produces the most reliable artifact. `serde` also gives a typed,
versioned provenance schema for free, and MapWebUI already treats Rust as a pinned
first-class toolchain.

Subprocess overhead is irrelevant: Factorio's own headless launch is about 1.7
seconds.

## Commands

```
factorio-oracle installs list              discover every install; print version, build, path
factorio-oracle run --probe spec.json      run a probe; emit result + provenance
factorio-oracle refs sync <version>        pin factorio-data and cache the API docs
factorio-oracle refs grep <pattern>        search factorio-data at a tag, without moving HEAD
factorio-oracle refs worktree <tag>        materialise a tree at a tag, for tools that need one
factorio-oracle provenance check <dir>     report which fixtures predate the selected install
```

### Install discovery

`FACTORIO_BIN` wins if set. Otherwise the union of every candidate list found
across the four repos, plus the one from the stray benchmark script:

- `~/Library/Application Support/Steam/steamapps/common/Factorio/factorio.app`
- `/Applications/factorio.app`
- `~/.steam/steam/steamapps/common/Factorio`
- `~/.factorio`
- `/opt/factorio`

macOS ships an `.app` bundle and Linux a plain directory, so the tool resolves
three paths per install: the binary, `data/`, and `doc-html/`.

**Multi-version is a requirement, not a feature.** The consumers target different
versions on purpose - FactorioTools 2.1.14, factorio-blueprint-editor 2.0.45 to
2.0.73 against a corpus spanning 2.0.32 to 2.1.12, FactorioMapWebUI whatever Steam
last pushed. There is also an unstripped 2.0.77 build outside any install
directory. So `installs list` enumerates rather than picking, and every command
takes `--factorio <path>` or `--version <x.y.z>`.

**The version is always derived, never assumed.** Two values come out of
`--version`: the full build line (`Version: 2.0.77 (build 84539, mac-arm64, full)`),
which is what fixtures stamp, and the `major.minor` a mod's `info.json` needs. The
implementation to port is `probe-entity-tile-size.mjs:103-114` together with its
failure message at `:202-210`, which names the version mismatch as the likely cause
of an empty dump. Those two belong together.

## Run modes

There is no single execution shape. Five are needed, and the differences are not
cosmetic.

| Mode | Launch | Mod | Success is | Used by |
| --- | --- | --- | --- | --- |
| `dump-data` | `--dump-data` | none | exit 0, then dump exists | FactorioTools, wiki repo |
| `create` | `--create <save>` | generated | dump exists (exit is 1) | FBE, MapWebUI |
| `interactive` | `--load-scenario <s>` | generated | consumer decides | FBE |
| `preview` | `--generate-map-preview` | none | **exit 0** and the PNG exists | MapWebUI |
| `read-only` | no binary at all | none | files read | FactorioTools |

Three things follow from this table.

**The success predicate is per mode.** `error("DUMPED-OK")` makes Factorio exit
non-zero, and that is success - so `create` keys off the dump file. But
`--generate-map-preview` exits 0 on success, and MapWebUI's `render.mjs:59-61` is
right to check the code. And for `dump-data` a non-zero exit is real information:
the diagnostic is the last 30 lines of the log, not a missing file. One global rule
would break two of the five.

**`dump-data` scaffolds no mod.** Its mod directory exists only to be *empty* - a
contamination control, not a scaffold. Mods rewrite prototypes freely, so a capture
that loads them describes one person's game rather than Factorio.

**`read-only` needs no game.** Migrations and `runtime-api.json` are files on disk,
and `wube/factorio-data` ships byte-identical migrations. So renames can be checked
with no install at all.

## The probe spec

A JSON document. Every Lua field accepts an inline string or a file path.

```jsonc
{
  "mode": "create",
  "factorio": { "version": "2.0.77" },       // or "path", or omit for the default
  "mod": {
    "name": "bp_rail_placement",
    "version": "0.0.1",
    "dependencies": ["base", "elevated-rails", "space-age"],
    "control_lua": "...",                     // or control_lua_file
    "data_lua": null,
    "data_final_fixes_lua": null
  },
  "map_gen_settings": { "seed": 123456 },     // object or file path; required for create
  "literals": { "blueprint": "0eNq..." },     // become Lua locals; see below
  "timeout_seconds": 300,
  "capture_active_mods": true
}
```

Rules the consumers force:

**Lua goes in opaque.** All 18 factorio-blueprint-editor probes generate their Lua
in JavaScript, interpolating case lists, sweep window sizes and base64 blueprint
strings. The runner must not template, escape or rewrite any of it.

**The runner must not wrap the body.** Wrapping in `script.on_init` would be a
convenient default and would make `probe-zoom-limits.mjs` impossible, because it
uses `on_tick` plus three `commands.add_command` registrations.

**`literals` is the one exception**, and it exists to kill a real gotcha. Each key
becomes a Lua local declared above the consumer's `control_lua`, with its value
inside a long bracket, so base64 and quotes survive verbatim:

```lua
local blueprint = [==[0eNq...]==]
-- the consumer's control_lua follows, unmodified
```

The bracket level is chosen so it cannot collide with the value's own contents. A
consumer that would rather build the string itself simply passes no `literals` and
embeds it in its own Lua, which is what every probe does today.

**Both data-stage files are supported.** A probe mod declares no dependencies, so
its `data.lua` may run before `space-age`'s and `data.raw.resource[...]` will not
exist yet - a silent no-op. Prototype overrides belong in `data_final_fixes_lua`.

**`map_gen_settings` is optional for `create`, and so is the seed flag.**
Measured 2026-08-16 against 2.1.14, by FactorioMapWebUI, with a probe mod that
reads back `game.surfaces[1].map_gen_settings.seed` - the seed the surface
actually got:

| arm | file seed | flag seed | surface seed | result |
| --- | --- | --- | --- | --- |
| both | 111111 | 222222 | **222222** | flag wins |
| flag only, no settings file | - | 222222 | 222222 | works |
| neither | - | - | 3972429021 | works, random |
| settings file only | 111111 | - | 111111 | file used |

Arms two and three generated a map, loaded the mod and produced a dump with no
settings file at all. So "always passed" was habit in the consumer repos, not a
requirement of the game. The CLI passes it when the caller supplies one and omits
it otherwise.

**`--map-gen-seed` overrides the seed inside the settings file.** That is arm one:
the file says 111111, the flag says 222222, and the surface comes out 222222. Arm
four rules out the file simply being ignored - without the flag, the file's seed is
what you get. Precedence is flag over file.

**So the CLI takes one `seed` field and writes both channels.** With the precedence
now known, this is not merely defensive: a tool that wrote only the file while a
caller also passed a flag would be silently overridden. Writing both from one
source makes them agree, which makes the precedence irrelevant.

The failure this avoids is the bad kind - everything runs, nothing errors, and the
numbers come from a different map. FactorioMapWebUI has already paid for a
seed-provenance mistake once: a correct field compared against the wrong seed
convention scored 0.5% overlap where the right convention scored 99.9%, and nothing
about the failing run looked like a seed problem.

A note for anyone reading a consumer's harness: the same measurement showed that
FactorioMapWebUI's `seed` field inside its map-gen settings JSON **has never done
anything**, because that harness always passes the flag too. No fixture there is
wrong, since the two values always agreed - but it is a dead write that looks
load-bearing, and its `mapGenOverrides` path silently discards a `seed` passed
through it. Tracked on that repo's #232.

**The mod directory does both jobs.** For `create` and `interactive` it is an
isolated directory the runner owns *containing* the generated mod. For `dump-data`
and `preview` it is the same isolated directory, empty. The directory name must
carry the `_<version>` suffix matching `info.json`.

**Space Age needs three mods**, not one: `space-age` depends on `elevated-rails`
and `quality`.

## The output contract

**The runner returns the work directory, not "the dump".**

This is the single most important interface decision, and it comes from
`probe-zoom-limits.mjs`. That probe runs the graphics client with a human at the
keyboard, streams newline-delimited JSON in append mode as the person scrolls,
takes input through in-game commands, writes three files, and has a three-level
success ladder: dump missing, dump present but *voided* because no character
controller ever arrived, or usable. Four of the guarantees a dump-centric interface
would offer invert for it.

Returning the directory also covers the multi-dump and JSONL cases for free, and it
keeps freshness and voiding decisions with the consumer, who is the only party that
can make them.

```jsonc
{
  "ok": true,
  "workDir": "/tmp/factorio-oracle-abc123",
  "scriptOutput": "/tmp/factorio-oracle-abc123/write/script-output",
  "files": ["oracle-dump.json", "zoom-samples.jsonl"],
  "exitCode": 1,
  "sentinelSeen": true,
  "provenance": { "factorioVersion": "2.0.77", "...": "the block described below" }
}
```

On failure the tool returns the same shape with `ok: false` **and the tail of
Factorio's stdout and stderr**. Every existing probe prints that tail by hand
because it is the only diagnostic there is; a JSON-out CLI that dropped it would be
a regression. The best failure message pairs the tail with the derived
`factorio_version` and the binary's own version line, because a mismatch between
them is the most common cause of an empty dump.

`sentinelSeen` reports whether `DUMPED-OK` appeared in stderr. Seventeen probes
share that convention and nothing checks it, so it cannot currently distinguish
"the mod ran and finished" from "the mod crashed". The runner can append the
`error()` call itself and check for it.

### Guards

- **Timeout.** No consumer has one today. Default a few minutes, configurable, and
  never applied in `interactive` mode.
- **Freshness.** Design it out rather than check for it. Every mode gets an
  isolated `config.ini` whose `write-data` points at a scratch directory that
  started empty, so a leftover dump from an older capture cannot be picked up. That
  is what seventeen probes already achieve with a fresh `mkdtemp`, and it is what
  FactorioTools' current script lacks - it discovers a shared user data directory
  and therefore needs an mtime check to prove the dump belongs to this run.

  The mtime check stays, but as belt and braces rather than the primary defence,
  and it is **optional**: `probe-zoom-limits.mjs` deliberately reuses a named
  directory and appends across a session, so a mandatory check would break it.

  One thing to verify during implementation rather than assume: that `--dump-data`
  honours `write-data` for `script-output`. If it does not, `dump-data` keeps the
  discovery path and the mtime check is load-bearing for that mode alone.
- **Contamination. On by default.** Report which mods actually loaded, and fail if
  the set is not the expected one. Mods rewrite prototypes freely, so a capture
  that loads them describes one person's game rather than Factorio - and it looks
  completely normal, which is why this defaults on rather than being opt-in. Today
  six factorio-blueprint-editor probes capture it, FactorioTools only greps stdout
  for `Loading mod` (which works for `dump-data` alone), and FactorioMapWebUI
  captures none.

  **The prelude registers no event at all.** Measured 2026-08-16 on 2.1.14:
  `helpers.write_file` works at `control.lua` toplevel with no event, and
  `script.active_mods` is populated there. So the whole prelude is one line.

  **Toplevel is for metadata, not for sampling, and the distinction matters.**
  `game.surfaces[1]` does not exist at control-stage toplevel, so anything calling
  `calculate_tile_properties` or `get_tile` still needs `on_init`. That is why
  every sampling probe in FactorioMapWebUI registers one, and why those
  registrations are load-bearing rather than habit. The two requirements compose
  rather than compete: the prelude registers nothing, a sampling probe registers
  exactly one `on_init`, and nothing contests the single slot. Do not read the
  toplevel finding as "probes should stop using `on_init`".

  That matters because `script.on_init` takes exactly one handler, which the same
  measurement proved rather than assumed: an `instrument-control.lua` registering
  `on_init` had its handler silently discarded once `control.lua` registered one
  too. No error - the handler simply never ran. 17 of 18 factorio-blueprint-editor
  probes register an `on_init`, so any prelude using one would vanish. A toplevel
  write has no collision surface and costs no ticks.

  The reported set should include the probe's own throwaway mod - it is proof the
  mod loaded, which is the thing most worth knowing when a run produces no dump.
- **Binary exists** before spawning.
- **Large output buffer** always, so a big dump cannot truncate the diagnostic.

## Provenance

Copied from FactorioMapWebUI, which has the best version of this.

**Provenance lives beside the fixtures, not inside them.** Several fixtures are
verbatim copies of the game's own JSON and are asserted key for key, so an added
metadata key is data pollution.

```jsonc
{
  "_comment": ["array of strings, so it stays readable in a diff"],
  "fixtures": {
    "<filename>": {
      "factorioVersion": "2.0.77",
      "factorioBuild": "build 84539, mac-arm64, full",
      "branch": "stable",
      "loadedMods": ["base", "core", "elevated-rails", "quality", "recycler", "space-age"],
      "capturedOn": "2026-08-16",
      "capturedBy": "tools/oracle/probe-rail-placement.mjs",
      "targetVersionRange": "2.0.45-2.0.73",
      "evidence": "stated | inferred | unknown, plus free text"
    }
  }
}
```

- `evidence` grades how the version was established. `stated` beats `inferred`
  beats `unknown`.
- `branch` records stable versus experimental. FactorioTools deliberately targets
  experimental, and the only planner-relevant difference between 2.0.77 stable and
  2.1.14 experimental is the pumpjack's output fluid box. A capture with no branch
  marker cannot answer "which game is this".
- `targetVersionRange` is what the *consumer* targets, which can differ from the
  binary captured.
- Keys are bare filenames and cover PNGs too, since nothing about a PNG hints at
  which game produced it.

**Enforcement splits in two, and the split is the point.**

1. An always-on test that needs no Factorio: every fixture has an entry, no entry
   is dangling, every entry is well formed, and a **ratchet caps the number of
   `unknown` entries** so the gap can only shrink.
2. A version-comparison report that needs a binary and **always exits 0**, because
   deciding whether a version gap matters needs a human. A fixture captured on
   2.1.11 is not wrong because the binary moved on.

**A fixture's provenance is a record of the moment it was captured, not a live
claim.** Never hand-edit one to make it current, and never edit one to make a test
pass. A mismatch is a finding.

## Reference material

### factorio-data: read at a tag, never move HEAD

`~/GitHub/factorio-data` is one clone with one working tree, and at least two repos
already name it as their prototype source while targeting different versions. Only
FactorioMapWebUI can currently pin it, and it does so with `git checkout`.

The problem is live, not hypothetical. The clone is on branch `master` right now,
not detached at any tag. `refs:sync --check` reports "in sync" only because
`master` happens to equal the newest tag. That is a coincidence. MapWebUI's own
notes already record the failure inside one repo, when a second binary is used:
pointing `FACTORIO_BIN` at the 2.0.77 install de-syncs both references from the
binary the fixtures were validated against.

So: **`refs grep` and an internal `show` read at a tag without touching `HEAD`.**
This is already where the repos are drifting - MapWebUI re-ran an entire audit at
2.1.14 "without repinning anything, because the question only needs the tags".

For tools that need a real directory tree - ripgrep, an editor, a Lua parser -
`refs worktree <tag>` gives each repo its own tree off one object store, with no
contention. That is better than forcing everything through `git show`.

Two details: `git fetch --tags` is still needed before a never-seen tag can be
read, and `git grep <tree-ish>` prefixes every output path with `<tag>:`, so
anything parsing the output must strip it.

The blast radius is small. No runtime code in MapWebUI reads factorio-data - every
hit under `src/`, `test/`, `crates/`, `scripts/` and `preview-service/` is a
comment - and CI never touches it. What changes is three functions in
`sync-factorio-refs.sh` plus some doc recipes.

### API docs: cache the archive, extract on demand

`factorioLuaAPI/` is **286 MB and 3,371 files per version**. Three repos on three
versions is about 860 MB before any history, so caching extracted trees does not
scale. Cache the published archive per version and extract when needed.

Caching only the JSON will not do: the JSON is not a superset of the HTML.
`control:temperature:frequency` appears in `noise-expressions.html` and nowhere in
`runtime-api.json`.

**And the JSON does not contain what people assume it does.** Verified against the
installed 2.1.14 file: 0 of 1,408 define values carry a value field, and `order` is
a dense `0..n-1` index across all 137 define tables, with values stored
alphabetically by name. So `runtime-api.json` cannot answer "what number is east",
and neither can `defines.html`. Only the running game knows. This is FactorioTools#83,
and it is the strongest argument for the `create` mode existing at all.

## Determinism

`--check`-style drift detection is a `diff -u` against a committed file, so any
nondeterminism turns it into a permanent false alarm.

- Sort every map. Rust's `serde_json` does not sort by default and `HashMap`
  iteration order is randomised per process, so use `BTreeMap` or sort explicitly.
- Match the existing formatting: two-space indent, trailing newline.
- **Float formatting is the trap.** `0.29`, `2.5`, `1.5` and `0.2` all appear in
  FactorioTools' fixture. Any difference from Python's printer makes every future
  check red.

Acceptance test: the Rust tool must reproduce the current committed
`factorio-oracle.json` byte for byte before it replaces anything.

### Sampled values must round-trip f32 exactly

This is a separate requirement from the one above, on a separate path. Prototype
values are trimmed from a dump and have to match Python's printer. **Sampled
numeric values come back from the running game and have to survive as the exact
bits the game produced.**

Requested by FactorioMapWebUI, and it is not a preference. Scoring a port by
**count of exactly matching f32 values** is a sharper instrument than any error
bound, and it only works if the capture preserves the bits. The evidence: two
candidate noise kernels had the *identical* worst absolute error, 2.682e-7, and
differed by 42 exact matches out of 512. An error bound could not tell them apart
at all. The winning variant went from 132 of 512 exact to 473 of 512.

So:

- Emit each value with a **shortest round-trip** representation. Rust's `{}` on
  `f32` does this; `ryu` if it should be explicit. Never a fixed precision -
  `{:.6}` or `%.9g` destroys the instrument.
- **Never widen f32 to f64** anywhere along the path. If the game produced an f32,
  the output says so and keeps it.
- Self-test: capture, re-read, and assert `parse(serialize(v)) == v` bitwise for
  every value. Cheap, and it fails loudly the day somebody tidies the formatter.

The failure mode is what makes this worth spelling out: a capture that loses
precision still looks completely fine. Nothing errors. The consumer simply can
never again distinguish "bit-exact" from "very close". On the consumer side the
check is one line - `Math.fround(v) === v` across the fixture - which
FactorioMapWebUI now asserts before scoring anything.

## What the tool must never do

**Emit derived values.** Store raw prototype numbers only. Factorio's rule for
turning `supply_area_distance` into covered tiles is not one formula - poles come
out as `2*distance`, a beacon as `2*distance` plus its own footprint, and
substation fits neither reading. A guessed formula inside a fixture is confidently
wrong and drifts invisibly. Derive in the consumer, where a wrong derivation fails
loudly against a hardcoded value.

**Hardcode an allowlist.** FactorioTools' ten wanted entities are exactly
`EntityNames.Vanilla`. A blueprint editor wants hundreds; a map tool wants none of
them. Allowlists are caller-supplied config.

What the tool *should* keep from FactorioTools' trimmer is `find_prototype`'s type
search and its collision-box disambiguation. Most of these names exist twice, once
as the placeable entity and once as the carried item, and `data.raw["item"]["pumpjack"]`
has none of the geometry.

## The knowledge base

Three documents, lifted with attribution to the repo and issue each lesson came
from:

- `docs/gotchas.md` - factorio-blueprint-editor's ~25 entries plus
  FactorioMapWebUI's, each of which cost a run.
- `docs/method.md` - the epistemics. A control must be able to fail while the
  hypothesis holds. Last man standing is not a measurement. Refute the rival. A
  probe entity is part of the question. Ask the cheapest question that settles it.
  Transcribe a proposed rule into the probe before writing code. Sweep two window
  sizes and make "the wider one finds nothing new" an explicit control.
- `docs/order-of-attack.md` - factorio-data first, then the oracle, then the
  binary.

factorio-blueprint-editor's README already says its method was borrowed from
FactorioMapWebUI by hand. This gives that borrowing a home instead of a copy that
drifts.

## Testing

Mirror the split MapWebUI already proves works. The pure builders - `info.json`,
control-Lua assembly, `config.ini`, the argv vector, dump parsing, provenance
serialisation - are unit-tested with no Factorio present. The spawn boundary is
injectable, so a fake can assert the argv, write the dump the real game would have
written, and return a non-zero exit with `DUMPED-OK` on stderr.

One integration test runs only when an install is found. CI stays offline, matching
all four consumers.

## Repo setup

- Public GitHub repo at **`FactoryGameFan/factorio-oracle`**. Five of the six
  Factorio repos moved to that org on 2026-08-16, so a new shared tool starting
  anywhere else would be the odd one out from its first commit.
- `rust-toolchain.toml`, pinned. MapWebUI pins 1.97.1 as a correctness control
  rather than a convenience, and that reasoning transfers if a crate is ever shared.
- **Renovate in the first commit.** The app runs with "Require config file"
  enabled, so a new repo whose default branch has no valid config makes Renovate do
  nothing at all, silently - indistinguishable from "no updates available".
  `.github/renovate.json5`, JSON5 so the reasoning lives in comments beside each
  rule, one weekly batch on Monday morning `America/Los_Angeles` with security
  fixes outside that window, and `automerge: false` globally. Ecosystems are
  `cargo` and `github-actions`. Validate with
  `npx --yes --package renovate -- renovate-config-validator .github/renovate.json5`,
  and keep exactly one config file in the repo.

## Prior art, and why this is a build

Nothing existing covers it.

Four projects re-implement Factorio's data stage in embedded Lua: YAFC (C#,
patched Lua 5.2.1), factorio-draftsman (Python, lupa), KirkMcDonald/factorio-tools
(Go, cgo), factorio-scanner (Rust, mlua). They are fast and need no install, and
they are all approximations. YAFC ships the warning itself: "YAFC loads mods in
environment that is not completely compatible with Factorio." A tool whose premise
is that the game is the only authority should not be built on one.

`factorio-rust-tools` is the closest match and is worth reading. Its CI downloads a
**pinned headless Factorio** from `factorio.com/get-download/<version>/headless/linux64`
and diffs against a committed 37 MB golden file. That contradicts the assumption in
several of our repos that a CI machine can never have Factorio installed, and it is
a real option for closing a gap FactorioTools already names as accepted: nothing
automatically notices a new Factorio release.

Capturing real exported blueprint strings by running the game has **no prior art
anywhere**, confirmed on two independent search passes.

Also confirmed: `--dump-data-raw` is not a real flag. The real set is
`--dump-data`, `--dump-prototype-locale` and `--dump-icon-sprites`.

## Build order

This is one coherent tool but more than one sitting. A natural spine, each step
useful on its own:

1. **Repo skeleton.** Cargo project, pinned `rust-toolchain.toml`, Renovate config
   validated, CI that builds and tests with no Factorio present.
2. **`installs list`.** Discovery, both version values, the `.app` versus directory
   layouts. Smallest thing that is immediately useful, and it is the piece
   duplicated most.
3. **`run` for `dump-data` and `create`.** The two modes with real consumers today.
   Pure builders first, spawn boundary injectable, fake-game test.
4. **Determinism acceptance test.** Reproduce FactorioTools' committed
   `factorio-oracle.json` byte for byte. This gates whether the tool can ever
   replace that script.
5. **`interactive` and `preview`.** Both are one consumer each and neither blocks
   the others.
6. **`provenance check`** plus the always-on completeness test and the `unknown`
   ratchet.
7. **`refs`** - sync, grep at a tag, worktree, archive cache.
8. **The three knowledge documents.** Independent of all the code; can be done at
   any point, including first.

Steps 1 to 4 are the part that has to be right. Everything after is additive.

## Out of scope for v1

- A shared fixture *format* beyond provenance. Each consumer needs a different
  slice at a different version, so a common schema is guesswork until two of them
  actually want the same field.
- A Rust library crate. Only one consumer could use it, and subprocess cost is
  noise next to a 1.7 second game launch.
- Migrating any existing probe. The agreed rule is **new probes only**.
- Published binary releases. Build from source.
- Automated drift detection in CI via a pinned headless download. Worth doing, but
  it is its own decision with its own cost.

## Open questions

1. ~~**The org move.**~~ **Done 2026-08-16/17. All six repos now live under
   `FactoryGameFan`**, so a new shared tool starting anywhere else would be the odd
   one out from its first commit. See "Repo setup" above.

   The blocker was specific to FactorioTools: `joelverhagen/FactorioTools#10` was
   open and cross-repository with head `wormeyman:main` and 466 files, and GitHub
   documents what happens to a fork on *deletion* and *detachment* but never on
   *transfer*. It was closed first, deliberately, and then the repo moved.

   The move answered the question, and the answer is worth keeping because GitHub
   does not document it: **a transfer rewrites a cross-repo pull request's head to
   the new owner and preserves the pull request.** Verified afterwards - PR #10
   reads `head: FactoryGameFan:main`, still closed, both comments intact. Caveat:
   it was closed before the transfer, so this establishes the closed case only.

   Everything else survived too: issues, fork links (FactorioTools still shows
   parent `joelverhagen/FactorioTools`, factorio-blueprint-editor still shows
   `teoxoy/factorio-blueprint-editor`), Actions secrets, private flags and branch
   rulesets. No repo has a Cloudflare Git integration, so no deploy broke.

   Two operational notes. **The transfer API is asynchronous**:
   `POST repos/{owner}/{repo}/transfer` returns the repo's pre-transfer state, so
   verify with a follow-up read rather than trusting the response. And **old URLs
   301 correctly, so stale `wormeyman/` references survive silently** and still
   need a sweep - the ones with teeth are each repo's `CLAUDE.md`, which is loaded
   into every session and can point a future session at the old path. Do not blind
   find-and-replace: the **Cloudflare account** is also called `wormeyman` and did
   not move, so each hit has to be read for which one it means.
2. ~~Whether the runner injects a Lua prelude by default for
   `script.active_mods`.~~ **Decided 2026-08-16: on by default**, using a
   self-cancelling `on_nth_tick` rather than `on_init`. See the contamination
   guard above.
3. ~~Whether `--instrument-mod` is a better launch path.~~ **Measured 2026-08-16
   on 2.1.14. Answer: no, and it makes the collision worse.**

   Instrument Mode does give earlier hooks. With `--instrument-mod`,
   `instrument-data.lua` ran at 0.045s against `data.lua` at 0.129s, and
   `instrument-control.lua` toplevel ran before `control.lua` toplevel. Without
   the flag, neither instrument file loads at all.

   But `instrument-control.lua`'s `script.on_init` handler **never fired**,
   because `control.lua` registered one too and the later registration replaced
   it. So Instrument Mode gives a probe an earlier hook whose event registration
   the consumer then silently destroys.

   That makes it **actively dangerous for a probe runner, not merely unhelpful.**
   A tool built on Instrument Mode would appear to work on every probe that does
   not register `on_init`, and fail silently on every probe that does. Keep this
   entry even though the feature is not being adopted: "we watched the earlier
   handler get destroyed by the later one" is a much stronger claim than "we
   inferred the rule from the docs", and it is the one that survives somebody
   proposing Instrument Mode again in a year.

   One quirk worth recording: `instrument-control.lua`'s toplevel logged twice in
   the same run. Not investigated further, since the mode is not being adopted.

   The useful finding came out of the same run: `helpers.write_file` works at
   plain `control.lua` toplevel with no event at all, and `script.active_mods` is
   populated there. That is what the contamination prelude now uses.

## First customer

FactorioMapWebUI#234 is the first real consumer, and it is a **new** probe, so it
sits on the right side of the "new probes only" rule.

After that repo's #214, `basisNoise` is 473 of 512 bit-exact and the remaining 39
points come from the game's own gradient table, produced by a minimax polynomial
inside `Noise::Noise(bool)` rather than by libm. No formula recovers them. A
capture does: with `input_scale = 1`, sampling at `(I + 1/256, J)` leaves exactly
one cell corner contributing, so the value inverts directly to that slot's
gradient x component, and `(I, J + 1/256)` gives y. 256 slots, both components,
one capture.

It needs nothing but "run this noise expression at these points and return the
numbers", which is the `create` mode shape. It is also the reason the f32
round-trip rule above is not optional: the entire point is recovering exact f32
constants.

Sequencing: that repo's #220 takes priority, so there is no rush.

## Decision log

Settled, and not worth relitigating without new information:

- Rust, not TypeScript. The three Node pins are mutually incompatible, and the
  analysis half cannot be shared in any language.
- JSON in, JSON out. Not a probe framework.
- The runner returns a work directory, not a dump.
- The success predicate is per mode.
- Read factorio-data at a tag; never move `HEAD` in a shared clone.
- Provenance beside the fixtures, with an evidence grade and an `unknown` ratchet.
- Migration is new probes only.
