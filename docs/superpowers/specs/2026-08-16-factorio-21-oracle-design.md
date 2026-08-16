# Speaking Factorio 2.1 to a 2.1 game

Design for the bugs reported since Factorio 2.1: mis-rotated pumpjacks, and items the game does not recognize.

## Problem

Both symptoms have one root cause. The planner still speaks Factorio 1.1.

Two things changed in Factorio 2.0 that this repo never followed:

1. `effectivity-module-N` was renamed to `efficiency-module-N`.
2. Directions widened from 8 values to 16. North stayed 0, but east moved from 2 to 4, south from 4 to 8, and west from 6 to 12.

The second one is the nastier of the pair, because the old values are still *legal*. Emitting `4` for south does not fail - the game reads it as east and rotates the pumpjack. There is no error anywhere, just a wrong blueprint.

### Why the tests did not catch it

The committed corpus is 1.1-encoded. Decoding every blueprint in `small-list.txt` and `big-list.txt` gives pumpjack direction values of `{0, 2, 4, 6}`, and both `2` and `6` are impossible for a 2.x pumpjack - they would mean northeast and southeast. Every blueprint also carries `version: 0`.

So the parser is *correct for the corpus* and wrong for anything a user pastes. The tests and the bug report cannot both be satisfied by the current code, and the tests won.

### Why an oracle, rather than reading the wiki

This class of bug is invisible from inside the repo, so the fix has to include a way to notice it next time. Commit `bfef7ba` added that: `tools/capture-factorio-oracle.sh` pulls prototype facts out of the game itself into `test/FactorioTools.Test/OilField/factorio-oracle.json`.

Pointing that fixture at the current constants reports:

```
ItemNames.EfficiencyModule3  "effectivity-module-3" -> efficiency-module-3
ModuleSelect.vue             effectivity-module{,-2,-3} -> efficiency-module*
Direction.Right = 2          2 now means northeast; east is 4
Direction.Down  = 4          4 now means east;      south is 8
Direction.Left  = 6          6 now means southeast; west is 12
```

The oracle also **ruled things out**, which narrows the work considerably. Every pole and beacon number still matches the game: supply distances 2.5/3.5/2/9, wire reach 7.5/9/32/18, beacon supply 3 and distribution effectivity 1.5. `PlanUndergroundPipes.MaxUnderground = 11` still agrees with the game's `max_underground_distance: 10` (which counts the gap, not the ends). None of the geometry drifted.

## Defects in scope

**D1 - Renamed module.** `src/FactorioTools/Data/ItemNames.cs:7` emits `effectivity-module-3`. `src/vue/src/components/ModuleSelect.vue:12-14` offers all three stale names.

**D2 - Direction is converted on output but not on input.** `GridToBlueprintString.cs:38` multiplies by 2, which is right. `ParseBlueprint.cs` does nothing, so `InitializeContext.cs:250` reads a raw 2.x value into the 1.1-style enum. An east pumpjack (4) is read as `Direction.Down`; south (8) and west (12) are not valid enum members at all.

**D3 - Pumpjack flips are dropped.** Changelog 2.1.7 added pumpjack flipping. `src/FactorioTools/Data/Entity.cs` has no `mirror` property, so the flag vanishes on parse.

**D4 - The corpus encodes 1.1 directions.** Left alone, it would keep hiding D2.

## Design

### C1 - Oracle capture (landed in `bfef7ba`)

Already done. Not repeated here.

### C2 - Oracle assertion test

`FactorioOracleTest` reads the **committed fixture**, never the game, so CI needs no Factorio install. It asserts:

- every `EntityNames.Vanilla` value exists in `entities`
- every `ItemNames.Vanilla` module value exists in `modules`
- the `Direction` enum members match `directions` for north/east/south/west
- the pole and beacon raw values behind `OilFieldOptions` presets are unchanged

Two carve-outs, both deliberate and both needing a comment saying why:

- `EntityNames.AaiIndustry` is a mod entity. A vanilla capture will never contain it. Absence is expected, not drift.
- `ItemNames.Vanilla.Blueprint` is an item, not a module or entity, so it is checked against a different part of the fixture or excluded outright.

This mirrors `PlannerDefaultsTest`, which is already the repo's "the test is the generator" pattern. When Factorio 2.2 lands, re-capture, and a changed fixture fails here with a diff.

The failure message must name `tools/capture-factorio-oracle.sh`, so a red CI run says how to fix itself.

Note: `factorio-oracle.json` will need a `<None Update=... CopyToOutputDirectory>` entry in `test/FactorioTools.Test/FactorioTools.Test.csproj`, alongside the existing `small-list.txt` entry.

### C3 - Direction conversion

**The internal `Direction` enum does not change.** It stays `Up=0, Right=2, Down=4, Left=6`. It is the planner's logical four-way concept, it is transpiled to Lua, and renumbering it would churn the whole core, every snapshot, and the Lua output for no benefit. All version knowledge lives at the serialization boundary.

Output keeps `ToOutputDirection` (multiply by 2) and starts writing a real version stamp instead of 0.

Input gains the inverse, in `FactorioTools.Serialization`:

```
ToInternalDirection(raw, version):
    16-way (version >= 2.0, or version missing/0)  ->  raw / 2
    8-way  (version below 2.0)                     ->  raw
    result not in {0, 2, 4, 6}                     ->  throw, naming the entity and the raw value
```

Missing or zero defaults to 16-way, because that is what every user pastes today. That choice is what forces D4: the existing corpus has `version: 0` and 1.1 values, so it must be re-normalized in the same change or it will be read wrong.

**Value-sniffing was considered and rejected.** A blueprint whose directions are all in `{0, 4}` is valid under both encodings with different meanings, so inference cannot always be correct. The version field is the only sound signal.

#### Must verify before implementing

`Blueprint.Version` is a `ulong` that this repo has never read. The layout is believed to be `major<<48 | minor<<32 | patch<<16 | dev`, which would make the 2.0 threshold `2<<48 = 562949953421312`, but **no blueprint in this repo carries a nonzero version**, so that is unconfirmed.

Confirm it first, and do not write the threshold from memory:

1. In Factorio 2.1, create any blueprint and export the string.
2. Base64-decode past the leading version byte, zlib-inflate, and read `blueprint.version`.
3. Check the value is >= the threshold and that a 1.1-era blueprint string falls below it.

If the layout differs, only the threshold constant changes; the rest of the design holds, because all that is needed is an ordered comparison in which the major version dominates.

### C4 - Rename, including persisted settings

`ItemNames.Vanilla` moves to `efficiency-module-3` (and siblings if added). `ModuleSelect.vue` option values move to `efficiency-module{,-2,-3}`. Display labels already read "Efficiency module" and do not change.

**A rename alone is not enough.** `src/vue/src/stores/OilFieldStore.ts` persists settings to `localStorage`. A user who ever picked an efficiency module has `effectivity-module-3` saved, and will keep sending that dead name after the fix ships. The store needs a one-time migration on load that rewrites any persisted `effectivity-*` value to `efficiency-*`.

The fixture's `renames` table is the source for that mapping, so the migration should not hand-type the pairs where it can avoid it.

### C5 - Mirror

Add `mirror` to `Entity` as `bool?`. Parse it so it survives deserialization; ignore it when planning, because the planner re-chooses every pumpjack orientation anyway; do not emit it.

This is the same treatment input direction already gets, and it is the minimum that stops a 2.1 blueprint from losing information silently. Honoring a flip is explicitly out of scope - see below.

### C6 - Corpus re-normalization

Extend `NormalizeBlueprints` to convert 1.1 direction values to 2.x and stamp a real version, then re-run `oil-field normalize` over both `small-list.txt` (61) and `big-list.txt` (1147).

Both lists, not just the scored one. `big-list.txt` is not scored so it carries no snapshot risk, but leaving it on 1.1 means the two corpora disagree and the next person to look hits exactly the confusion this document exists to clear up.

## The invariant that makes this safe

Re-normalizing the corpus to 2.x and fixing the parser are inverse operations. Applied together, they should cancel exactly.

**`Score.HasExpectedScore.verified.txt` should come out unchanged.**

That turns the 61-blueprint scoreboard from churn to be rubber-stamped into a free end-to-end check. If the scoreboard moves, the change is wrong - most likely the conversion is not symmetric, or the corpus rewrite altered something beyond direction. Investigate before accepting any diff there.

The same is expected of the per-blueprint plan snapshots. A moved snapshot is a signal, not a chore.

## Testing

- `FactorioOracleTest`, per C2.
- Direction round-trip unit tests: a 2.x blueprint parses east as `Right`; a 1.1 blueprint with an explicit sub-2.0 version parses east as `Right`; the ambiguous `{0, 4}` case resolves by version; a non-cardinal value throws with a useful message.
- A regression test pinning the actual reported bug: parse a 2.1-exported blueprint with a non-north pumpjack and assert the orientation survives a round trip. This is the test that would have caught the original report, and the corpus cannot provide it, so the blueprint string needs to come from the game.
- `Score.HasExpectedScore.verified.txt` unchanged, per the invariant above.
- Vue: a persistence test that a stored `effectivity-module-3` loads as `efficiency-module-3`.

## Also required, because the core changes

- Regenerate `src/lua` via `Invoke-LuaBuild.ps1` and commit it, or `transpile-lua` fails.
- Rebuild the WASM bundle (`npm run build-wasm` in `src/vue`).
- Build and test under `UseLuaSettings=true` as well as the default, per the repo's CI matrix.

## Out of scope

- **Honoring a pumpjack flip on output.** The planner picks orientations itself, so it is not clear what honoring an input flip would even mean, and answering that needs real investigation into whether a mirrored pumpjack changes valid terminal positions. C5 only stops the flag being lost.
- **Beacon diminishing returns.** Factorio 2.0 models this with a `profile` array; the planner scores beacons as though each contributes equally. A real gap, deliberately not captured in the fixture and not addressed here. Worth its own issue.
- **Quality-scaled beacon effects.** `distribution_effectivity_bonus_per_quality_level` is captured in the fixture but nothing reads it.
- **AAI Industry and other mod entities.** The oracle is vanilla by design.

## Risks

- **The version threshold is unconfirmed.** Mitigated by making confirmation the first implementation step, with a stated procedure.
- **The corpus rewrite is large.** 1208 blueprints across two files. Mitigated by the score invariant: a correct rewrite moves no scores.
- **A blueprint with a genuinely missing version stamp that really is 1.1** will now be read as 2.x and mis-rotated. This is a deliberate trade: it favors the many users pasting current blueprints over the few pasting decade-old ones.

  The error on non-cardinal values catches most of it, but not all, and it is worth being exact about which. Reading 1.1 values as 2.x halves them: east `2` becomes `1` and west `6` becomes `3`, neither of which is a cardinal, so both throw. South `4` becomes `2`, which is a valid `Direction.Right`. **A 1.1 south-facing pumpjack in an unstamped blueprint is therefore read as east, silently.** North is unaffected either way.

  So the guard is loud for two of the three non-north directions and silent for the third. Accepted, because an unstamped 1.1 blueprint is already a corner case and the alternative (defaulting to 1.1) mis-rotates the common case instead. If this proves to matter in practice, the fallback could be tightened by rejecting an unstamped blueprint whose directions are all even and non-zero, which is the signature of 1.1 content.
