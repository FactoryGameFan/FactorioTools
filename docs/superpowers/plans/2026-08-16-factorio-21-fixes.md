# Factorio 2.1 Compatibility Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the oil field planner speak Factorio 2.1 instead of 1.1, fixing mis-rotated pumpjacks and unrecognized renamed items.

**Architecture:** The internal `Direction` enum stays 1.1-style four-way, because it is the planner's logical concept and is transpiled to Lua. All version knowledge lives at the serialization boundary. A committed oracle fixture, captured from the game, becomes the thing that fails when Factorio moves again.

**Tech Stack:** C# / .NET 10, xUnit v3 + Verify, Vue 3 + Pinia + Vitest, CSharp.lua transpilation, .NET WASM.

**Spec:** `docs/superpowers/specs/2026-08-16-factorio-21-oracle-design.md`

## Global Constraints

- Use hyphens, never em dashes or en dashes, in all files.
- Prose written for humans targets Flesch-Kincaid grade 12 max, aim 9-11.
- The core library `src/FactorioTools` must stay free of JSON/serialization dependencies. It is transpiled to Lua; `FactorioTools.Serialization` is not.
- In core library hot paths avoid `yield return`, LINQ, named tuples, try/catch, and struct dictionary keys. LINQ in the core is a runtime failure inside Factorio, not a build error, because `Linq.lua` is never shipped.
- Any change to the core requires regenerating `src/lua` via `src/lua/Invoke-LuaBuild.ps1` and committing it, or the `transpile-lua` CI job fails.
- Build and test under both default flags and `/p:UseLuaSettings=true`.
- Verify snapshots auto-accept locally but fail on CI. Commit regenerated snapshots. Setting `CI=1` locally does NOT reproduce CI behavior, so do not try to simulate it.
- Confirmed encodings, do not re-derive: blueprint version is `major<<48 | minor<<32 | patch<<16 | dev`; the 2.0 threshold is `562949953421312`. Blueprint directions are north omitted, east `4`, south `8`, west `12`. A flipped entity carries `"mirror": true`.

## Corrections to the spec, found while planning

Three spec statements were wrong. The plan below is correct; fix the spec in Task 1.

1. **`FormatVersion` and `ParseVersion` already exist** at `GridToBlueprintString.cs:243` and `:233`, and they already match the confirmed layout. Do not write new ones.
2. **Output is already version-stamped.** `GridToBlueprintString.cs:222` emits `FormatVersion(2, 0, 32, 0)`. The spec's claim that output needs to start stamping a version is wrong.
3. **The fixture needs no `.csproj` change.** `small-list.txt` is `CopyToOutputDirectory: Never` and tests resolve paths through `BaseTest.GetRepositoryRoot()`. Follow that pattern.

The corpus carries `version: 0` because `CleanBlueprint.cs:34` builds a `new Blueprint` without copying `Version`, and normalize serializes that object.

---

### Task 1: Oracle assertion test

Locks the oracle in before any behavior changes. Pure test addition. It should FAIL on the current code, which is the point: it reproduces the reported bugs as a test.

**Files:**
- Create: `test/FactorioTools.Test/OilField/FactorioOracleTest.cs`
- Modify: `docs/superpowers/specs/2026-08-16-factorio-21-oracle-design.md` (apply the three corrections above)

**Interfaces:**
- Consumes: `BaseTest.GetRepositoryRoot()` (public static, returns repo root), the committed fixture `test/FactorioTools.Test/OilField/factorio-oracle.json`.
- Produces: nothing other tasks consume.

- [ ] **Step 1: Write the failing test**

Create `test/FactorioTools.Test/OilField/FactorioOracleTest.cs`:

```csharp
using System.Text.Json;
using Knapcode.FactorioTools.Data;

namespace Knapcode.FactorioTools.OilField;

/// <summary>
/// Asserts the planner's hardcoded Factorio facts against an oracle captured from the game
/// itself (tools/capture-factorio-oracle.sh).
///
/// This reads the COMMITTED fixture, never the game, so CI needs no Factorio install.
/// Re-capture after a Factorio update and commit the diff; a changed fixture failing here
/// is the signal that a constant needs review.
/// </summary>
public class FactorioOracleTest : BaseTest
{
    private static readonly string FixturePath = Path.Combine(
        GetRepositoryRoot(), "test", "FactorioTools.Test", "OilField", "factorio-oracle.json");

    private const string ReCaptureHint =
        "If Factorio changed, re-run tools/capture-factorio-oracle.sh and review the diff.";

    private static JsonElement Oracle()
    {
        return JsonDocument.Parse(File.ReadAllText(FixturePath)).RootElement;
    }

    private static HashSet<string> Names(JsonElement parent, string property)
    {
        var element = parent.GetProperty(property);
        if (element.ValueKind == JsonValueKind.Object)
        {
            return element.EnumerateObject().Select(x => x.Name).ToHashSet();
        }

        return element.EnumerateArray().Select(x => x.GetString()!).ToHashSet();
    }

    [Fact]
    public void EveryVanillaEntityNameExistsInFactorio()
    {
        var entities = Names(Oracle(), "entities");

        var missing = typeof(EntityNames.Vanilla)
            .GetFields()
            .Select(f => (string)f.GetValue(null)!)
            .Where(name => !entities.Contains(name))
            .ToList();

        Assert.True(missing.Count == 0, $"Not in Factorio: {string.Join(", ", missing)}. {ReCaptureHint}");
    }

    /// <summary>
    /// EntityNames.AaiIndustry is deliberately NOT checked. Those names come from the AAI
    /// Industry mod, and the oracle is captured with mods disabled on purpose, so their
    /// absence is expected rather than drift.
    /// </summary>
    [Fact]
    public void EveryVanillaModuleNameExistsInFactorio()
    {
        var modules = Names(Oracle(), "modules");

        var missing = typeof(ItemNames.Vanilla)
            .GetFields()
            .Select(f => (string)f.GetValue(null)!)
            // "blueprint" is an item, not a module, so it is not in the module list.
            .Where(name => name != ItemNames.Vanilla.Blueprint)
            .Where(name => !modules.Contains(name))
            .ToList();

        Assert.True(missing.Count == 0, $"Not a Factorio module: {string.Join(", ", missing)}. {ReCaptureHint}");
    }

    [Theory]
    [InlineData(Direction.Up, "north")]
    [InlineData(Direction.Right, "east")]
    [InlineData(Direction.Down, "south")]
    [InlineData(Direction.Left, "west")]
    public void InternalDirectionDoublesToTheFactorioValue(Direction direction, string factorioName)
    {
        var expected = Oracle().GetProperty("directions").GetProperty(factorioName).GetInt32();

        // The internal enum is deliberately 1.1-style four-way (N=0, E=2, S=4, W=6).
        // Factorio 2.0 is 16-way, so the blueprint value is always exactly double.
        Assert.Equal(expected, (int)direction * 2);
    }

    [Theory]
    [InlineData(EntityNames.Vanilla.SmallElectricPole, "supply_area_distance", 2.5)]
    [InlineData(EntityNames.Vanilla.MediumElectricPole, "supply_area_distance", 3.5)]
    [InlineData(EntityNames.Vanilla.BigElectricPole, "supply_area_distance", 2)]
    [InlineData(EntityNames.Vanilla.Substation, "supply_area_distance", 9)]
    [InlineData(EntityNames.Vanilla.SmallElectricPole, "maximum_wire_distance", 7.5)]
    [InlineData(EntityNames.Vanilla.MediumElectricPole, "maximum_wire_distance", 9)]
    [InlineData(EntityNames.Vanilla.BigElectricPole, "maximum_wire_distance", 32)]
    [InlineData(EntityNames.Vanilla.Substation, "maximum_wire_distance", 18)]
    [InlineData(EntityNames.Vanilla.Beacon, "supply_area_distance", 3)]
    public void RawGeometryBehindTheOptionsPresetsIsUnchanged(string entity, string field, double expected)
    {
        var actual = Oracle().GetProperty("entities").GetProperty(entity).GetProperty(field).GetDouble();

        Assert.True(
            expected == actual,
            $"{entity}.{field} moved from {expected} to {actual}, so the OilFieldOptions presets need review. {ReCaptureHint}");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails for the right reason**

Run: `dotnet test test/FactorioTools.Test/FactorioTools.Test.csproj --filter "FullyQualifiedName~FactorioOracleTest"`

Expected: `EveryVanillaModuleNameExistsInFactorio` FAILS with `Not a Factorio module: effectivity-module-3`. The direction and geometry tests PASS. If the geometry tests fail, stop and investigate; the oracle says they should not.

The failing module test IS the reported bug reproduced. Do not fix it here.

- [ ] **Step 3: Apply the three spec corrections**

In `docs/superpowers/specs/2026-08-16-factorio-21-oracle-design.md`:
- In the C3 section, replace the claim that output should "start writing a real version stamp" with: output already stamps `FormatVersion(2, 0, 32, 0)` at `GridToBlueprintString.cs:222`, and `ParseVersion`/`FormatVersion` already exist at `:233` and `:243` matching the confirmed layout.
- In the C2 section, delete the note claiming a `<None Update=...>` csproj entry is needed. Replace with: tests resolve the fixture through `BaseTest.GetRepositoryRoot()`, matching `BasePlannerTest.SmallListFilePath`.
- In the C6 section, add: the corpus carries `version: 0` because `CleanBlueprint.cs:34` builds a `new Blueprint` without copying `Version`.

- [ ] **Step 4: Commit**

```bash
git add test/FactorioTools.Test/OilField/FactorioOracleTest.cs docs/superpowers/specs/2026-08-16-factorio-21-oracle-design.md
git commit -m "Assert planner constants against the captured Factorio oracle

Reads the committed fixture, never the game, so CI needs no Factorio install.
EveryVanillaModuleNameExistsInFactorio fails on effectivity-module-3, which is
the reported bug reproduced as a test. Fixed in the next commit.

Also corrects three spec claims found while planning: FormatVersion/ParseVersion
already exist, output is already version-stamped, and the fixture needs no
csproj entry."
```

The suite is RED after this commit, deliberately. If that is not acceptable in this repo, squash Task 1 and Task 2 together.

---

### Task 2: Rename the module in C#

**Files:**
- Modify: `src/FactorioTools/Data/ItemNames.cs:7`

**Interfaces:**
- Consumes: `FactorioOracleTest.EveryVanillaModuleNameExistsInFactorio` from Task 1.
- Produces: `ItemNames.Vanilla.EfficiencyModule3 == "efficiency-module-3"`, consumed by Task 3's Vue values.

- [ ] **Step 1: Make the minimal change**

In `src/FactorioTools/Data/ItemNames.cs`, change line 7 from:

```csharp
        public const string EfficiencyModule3 = "effectivity-module-3";
```

to:

```csharp
        // Renamed in Factorio 2.0. "effectivity-module-3" no longer exists, so the game
        // silently rejects it. See base/migrations/2.0.0.json in the game data.
        public const string EfficiencyModule3 = "efficiency-module-3";
```

The C# identifier `EfficiencyModule3` was already correct; only the string value was stale.

- [ ] **Step 2: Run the oracle test to verify it passes**

Run: `dotnet test test/FactorioTools.Test/FactorioTools.Test.csproj --filter "FullyQualifiedName~FactorioOracleTest"`

Expected: all PASS.

- [ ] **Step 3: Run the full suite and accept any snapshot changes**

Run: `dotnet test`

Expected: snapshots that embed the module name change. Review each diff and confirm the only change is `effectivity-module-3` becoming `efficiency-module-3`. `Score.HasExpectedScore.verified.txt` must NOT change, because module names do not affect plan quality. If the score moves, stop and investigate.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "Emit efficiency-module-3, the name Factorio has used since 2.0

effectivity-module-3 has not existed since 2.0, so the game silently rejected
it. The C# identifier was already EfficiencyModule3; only the string was stale."
```

---

### Task 3: Rename in the Vue app, including persisted settings

A rename alone is not enough. `OilFieldStore` persists to `localStorage`, so a user who ever picked an efficiency module has the dead name saved and would keep sending it forever.

**Files:**
- Modify: `src/vue/src/components/ModuleSelect.vue:12-14`
- Modify: `src/vue/src/stores/OilFieldStore.ts`
- Test: `src/vue/src/stores/persistence.test.ts`

**Interfaces:**
- Consumes: `ItemNames.Vanilla.EfficiencyModule3` value from Task 2.
- Produces: a `migrateModuleNames(state)` helper exported from `OilFieldStore.ts`, taking and returning the persisted state object.

- [ ] **Step 1: Write the failing test**

Add to `src/vue/src/stores/persistence.test.ts`:

```typescript
import { migrateModuleNames } from "./OilFieldStore"

describe("migrateModuleNames", () => {
  it("rewrites module names Factorio renamed in 2.0", () => {
    const migrated = migrateModuleNames({
      pumpjackModule: "effectivity-module-3",
      beaconModule: "effectivity-module",
    })

    expect(migrated.pumpjackModule).toBe("efficiency-module-3")
    expect(migrated.beaconModule).toBe("efficiency-module")
  })

  it("leaves current names alone", () => {
    const migrated = migrateModuleNames({
      pumpjackModule: "productivity-module-3",
      beaconModule: "speed-module-3",
    })

    expect(migrated.pumpjackModule).toBe("productivity-module-3")
    expect(migrated.beaconModule).toBe("speed-module-3")
  })
})
```

Adjust the property names to match the real store state shape. Read `OilFieldStore.ts` first and use its actual module property names.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd src/vue && npx vitest run src/stores/persistence.test.ts`

Expected: FAIL with `migrateModuleNames is not a function`.

- [ ] **Step 3: Implement the migration**

In `src/vue/src/stores/OilFieldStore.ts`, add and export:

```typescript
// Factorio 2.0 renamed the efficiency modules. Anyone who picked one before this fix
// has the dead name in localStorage and would keep sending it to the planner forever,
// so rewrite on load rather than only fixing the dropdown.
const RENAMED_MODULES: Record<string, string> = {
  "effectivity-module": "efficiency-module",
  "effectivity-module-2": "efficiency-module-2",
  "effectivity-module-3": "efficiency-module-3",
}

export function migrateModuleNames<T extends Record<string, unknown>>(state: T): T {
  for (const key of Object.keys(state)) {
    const value = state[key]
    if (typeof value === "string" && value in RENAMED_MODULES) {
      ;(state as Record<string, unknown>)[key] = RENAMED_MODULES[value]
    }
  }
  return state
}
```

Then call it from the store's persisted-state restore path. `pinia-plugin-persistedstate` exposes `afterHydrate`, so in the `persist` options object add:

```typescript
      afterHydrate: (ctx) => {
        migrateModuleNames(ctx.store.$state as Record<string, unknown>)
      },
```

Read the existing `persist: { ... }` block first and add the hook alongside what is already there.

- [ ] **Step 4: Run the test to verify it passes**

Run: `cd src/vue && npx vitest run src/stores/persistence.test.ts`

Expected: PASS.

- [ ] **Step 5: Update the dropdown values**

In `src/vue/src/components/ModuleSelect.vue`, change lines 12-14 from `value="effectivity-module"`, `value="effectivity-module-2"`, `value="effectivity-module-3"` to `value="efficiency-module"`, `value="efficiency-module-2"`, `value="efficiency-module-3"`. The visible labels already read "Efficiency module" and do not change.

- [ ] **Step 6: Run the whole front-end suite**

Run: `cd src/vue && npx vitest run && npm run type-check`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "Rename efficiency modules in the Vue app and migrate saved settings

Changing the dropdown alone would leave every existing user sending
effectivity-module-3, which Factorio has not accepted since 2.0, because the
choice is persisted in localStorage. migrateModuleNames rewrites it on hydrate."
```

---

### Task 4: Preserve the mirror flag

Factorio 2.1.7 added pumpjack flipping. Confirmed by round-tripping a blueprint through 2.1.14: a flipped entity carries `"mirror": true`.

**Files:**
- Modify: `src/FactorioTools/Data/Entity.cs`
- Test: `test/FactorioTools.Test/OilField/ParseBlueprintTest.cs` (create)

**Interfaces:**
- Consumes: nothing from earlier tasks.
- Produces: `Entity.Mirror` as `bool?`, read by nothing else. Task 6's regression fixtures exercise it.

- [ ] **Step 1: Write the failing test**

Create `test/FactorioTools.Test/OilField/ParseBlueprintTest.cs`:

```csharp
namespace Knapcode.FactorioTools.OilField;

public class ParseBlueprintTest : BaseTest
{
    /// <summary>
    /// Generated by a probe mod running inside Factorio 2.1.14: three pumpjacks, the
    /// second and third mirrored. See CLAUDE.md for the probe-mod technique.
    /// </summary>
    private const string MirroredPumpjacks = "0eNqN0MkKgzAQBuB3mfNQNC5UX6WU4jKUaU0MSSwVybs36qFQK3gaZvm/w0xQdwNpw8pBOQE3vbJQXiawfFdVN89UJQlK0IPUj6p5gkdg1dIbythfEUg5dkxramnGmxpkTSYc4CaNoHsbAr2a7YBEpwxhXGqQWzbUrNvU4wYUB8B4X0SQbEwfIGcG+uMnB3yx659//PAddiSD9n0ywouMXc6zXBRZKtIiifI4Trz/ADGoghU=";

    [Fact]
    public void KeepsTheMirrorFlagOnAFlippedEntity()
    {
        var blueprint = ParseBlueprint.Execute(MirroredPumpjacks);

        Assert.Collection(
            blueprint.Entities,
            e => Assert.Null(e.Mirror),
            e => Assert.True(e.Mirror),
            e => Assert.True(e.Mirror));
    }
}
```

The string above is real output from Factorio 2.1.14, already generated, so use it as written. It decodes to version `562954249306113` and:

```
{entity_number: 1, direction: 4}
{entity_number: 2, direction: 4, mirror: True}
{entity_number: 3, direction: 8, mirror: True}
```

For reference, it came from the probe-mod technique in CLAUDE.md with these entities. Only regenerate if you need to change what is being tested:

```lua
stack.set_blueprint_entities{
  {entity_number = 1, name = "pumpjack", position = {x = 0.5,  y = 0.5}, direction = defines.direction.east},
  {entity_number = 2, name = "pumpjack", position = {x = 10.5, y = 0.5}, direction = defines.direction.east, mirror = true},
  {entity_number = 3, name = "pumpjack", position = {x = 20.5, y = 0.5}, direction = defines.direction.south, mirror = true},
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `dotnet test test/FactorioTools.Test/FactorioTools.Test.csproj --filter "FullyQualifiedName~ParseBlueprintTest"`

Expected: FAIL to compile, `Entity` has no `Mirror`.

- [ ] **Step 3: Add the property**

In `src/FactorioTools/Data/Entity.cs`, after the `Direction` property:

```csharp
    // Factorio 2.1.7 added pumpjack and burner mining drill flipping, and a flipped entity
    // carries "mirror": true (confirmed by round-tripping a blueprint through 2.1.14).
    // Parsed so the flag is not silently lost. The planner re-chooses every pumpjack
    // orientation itself, so nothing reads this and it is never emitted.
    [JsonPropertyName("mirror")]
    public bool? Mirror { get; set; }
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `dotnet test test/FactorioTools.Test/FactorioTools.Test.csproj --filter "FullyQualifiedName~ParseBlueprintTest"`

Expected: PASS.

- [ ] **Step 5: Run the full suite**

Run: `dotnet test`

Expected: PASS with no snapshot changes. `Mirror` is never emitted, so nothing serialized should move.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Parse the mirror flag instead of dropping it

Factorio 2.1.7 added pumpjack flipping, and a flipped entity carries
mirror: true. Confirmed by round-tripping a blueprint through 2.1.14, since
FFF #442 describes the feature but not its blueprint representation.

Parsed only. The planner re-chooses every pumpjack orientation itself, so
honoring a flip is a separate question and is out of scope."
```

---

### Task 5: Version-gated direction parsing, and the corpus, together

**This task is atomic and cannot be split.** Fixing the parser alone misreads the 1.1-encoded corpus. Re-normalizing the corpus alone produces 2.x values the unfixed parser misreads. Either half on its own turns the suite red, so they land in one commit.

**Files:**
- Modify: `src/FactorioTools.Serialization/OilField/Steps/ParseBlueprint.cs`
- Modify: `src/FactorioTools.Serialization/OilField/Steps/GridToBlueprintString.cs` (`SerializeBlueprint`)
- Modify: `test/FactorioTools.Test/OilField/small-list.txt` (regenerated)
- Modify: `test/FactorioTools.Test/OilField/big-list.txt` (regenerated)
- Test: `test/FactorioTools.Test/OilField/ParseBlueprintTest.cs` (extend)

**Do NOT change `CleanBlueprint.cs`.** Carrying the source version through looks right and is wrong: `ToOutputDirection` always emits 2.x values, so a 1.1 input would come out with 2.x directions under a 1.1 stamp, and re-parsing would halve values that were already converted. Stamping belongs next to the code that does the doubling, which is `SerializeBlueprint`. That also keeps the core library unchanged here, which is what the design wants.

**Interfaces:**
- Consumes: `Entity.Mirror` from Task 4; `GridToBlueprintString.FormatVersion(ushort, ushort, ushort, ushort)` and `ParseVersion(ulong)` which already exist.
- Produces: `ParseBlueprint.ToInternalDirection(Direction raw, ulong version)` returning `Direction`.

- [ ] **Step 1: Write the failing tests**

Add to `test/FactorioTools.Test/OilField/ParseBlueprintTest.cs`:

```csharp
    private const ulong Version2_1_14 = 562954249306113UL;   // confirmed against a real export
    private const ulong Version1_1_0 = 281479271677952UL;    // FormatVersion(1, 1, 0, 0)

    [Theory]
    // Factorio 2.x blueprints: north omitted, east 4, south 8, west 12.
    [InlineData(0, Version2_1_14, Direction.Up)]
    [InlineData(4, Version2_1_14, Direction.Right)]
    [InlineData(8, Version2_1_14, Direction.Down)]
    [InlineData(12, Version2_1_14, Direction.Left)]
    // An unstamped blueprint is assumed to be modern, because that is what users paste.
    [InlineData(4, 0UL, Direction.Right)]
    // Genuine 1.1 blueprints still parse, when they say so.
    [InlineData(2, Version1_1_0, Direction.Right)]
    [InlineData(6, Version1_1_0, Direction.Left)]
    public void ConvertsBlueprintDirectionsToInternalOnes(int raw, ulong version, Direction expected)
    {
        Assert.Equal(expected, ParseBlueprint.ToInternalDirection((Direction)raw, version));
    }

    [Theory]
    [InlineData(2)]   // northeast in 2.x, not a cardinal
    [InlineData(6)]   // southeast in 2.x, not a cardinal
    public void RejectsDirectionsAPumpjackCannotHave(int raw)
    {
        var ex = Assert.Throws<FactorioToolsException>(
            () => ParseBlueprint.ToInternalDirection((Direction)raw, Version2_1_14));

        Assert.Contains(raw.ToString(), ex.Message);
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `dotnet test test/FactorioTools.Test/FactorioTools.Test.csproj --filter "FullyQualifiedName~ParseBlueprintTest"`

Expected: FAIL to compile, no `ToInternalDirection`.

- [ ] **Step 3: Implement the conversion**

In `src/FactorioTools.Serialization/OilField/Steps/ParseBlueprint.cs`, add to the class:

```csharp
    /// <summary>
    /// The blueprint version at which Factorio widened directions from 8-way to 16-way.
    /// GridToBlueprintString.FormatVersion(2, 0, 0, 0). Confirmed against a real 2.1.14
    /// export, whose version is 562954249306113 (2.1.14.1).
    /// </summary>
    private const ulong FirstSixteenWayVersion = 562949953421312UL;

    /// <summary>
    /// Converts a blueprint's direction to the internal 1.1-style four-way
    /// <see cref="Direction"/> (N=0, E=2, S=4, W=6).
    ///
    /// Factorio 2.0 widened directions to 16-way (N=0, E=4, S=8, W=12). The old values are
    /// still legal, so a 2.x east read as 1.1 is not an error, it is a silently rotated
    /// pumpjack. Sniffing the values cannot resolve it either: a blueprint whose directions
    /// are all in {0, 4} is valid under both readings with different meanings. The version
    /// is the only sound signal.
    ///
    /// A missing or zero version is treated as modern, because that is what users paste
    /// today. The trade-off is spelled out in the design doc.
    /// </summary>
    public static Direction ToInternalDirection(Direction direction, ulong version)
    {
        var raw = (int)direction;
        var internalValue = version == 0 || version >= FirstSixteenWayVersion ? raw / 2 : raw;

        if (internalValue != (int)Direction.Up
            && internalValue != (int)Direction.Right
            && internalValue != (int)Direction.Down
            && internalValue != (int)Direction.Left)
        {
            throw new FactorioToolsException(
                $"Blueprint direction {raw} is not one of the four directions a pumpjack can face.",
                badInput: true);
        }

        return (Direction)internalValue;
    }
```

- [ ] **Step 4: Run the unit tests to verify they pass**

Run: `dotnet test test/FactorioTools.Test/FactorioTools.Test.csproj --filter "FullyQualifiedName~ParseBlueprintTest"`

Expected: PASS.

- [ ] **Step 5: Apply the conversion when parsing, and carry the version through**

In `ParseBlueprint.Execute`, just before `return root.Blueprint;`, add:

```csharp
        for (var i = 0; i < root.Blueprint.Entities.Length; i++)
        {
            var entity = root.Blueprint.Entities[i];
            if (entity.Direction.HasValue)
            {
                entity.Direction = ToInternalDirection(entity.Direction.Value, root.Blueprint.Version);
            }
        }
```

Then, in `src/FactorioTools.Serialization/OilField/Steps/GridToBlueprintString.cs`, at the top of `SerializeBlueprint` (before the FBE offset block), add:

```csharp
        // ToOutputDirection always emits Factorio 2.0's 16-way values, so the stamp has to
        // say 2.0 or later. Otherwise a normalized blueprint carries 2.x directions under
        // whatever version the source had, and reparsing it halves values that were already
        // converted. The planner's own path (see the Blueprint built above) already sets
        // this; normalize reaches SerializeBlueprint with a cleaned blueprint that does not.
        blueprint.Version = FormatVersion(2, 0, 32, 0);
```

`CleanBlueprint.cs` is deliberately left alone; see the note under Files.

- [ ] **Step 6: Re-normalize both corpus files**

The sequence matters. Directions are converted on parse now, and multiplied by 2 on serialize, so a re-normalize rewrites 1.1 values into 2.x values. But the corpus has `version: 0`, which the new parser treats as modern, so a straight re-normalize would halve values that are already 1.1.

So stamp first, in one throwaway pass. Run from the repo root:

```bash
python3 - <<'PY'
import base64, zlib, json, pathlib
# FormatVersion(1, 1, 0, 0) - say out loud what the corpus already is, so the parser
# stops guessing. After the re-normalize below they come back stamped 2.x.
V = (1 << 48) | (1 << 32)
for name in ["small-list.txt", "big-list.txt"]:
    p = pathlib.Path("test/FactorioTools.Test/OilField") / name
    out = []
    for line in p.read_text().splitlines():
        s = line.strip()
        if not s or s.startswith("#"):
            out.append(line)
            continue
        j = json.loads(zlib.decompress(base64.b64decode(s[1:])))
        j["blueprint"]["version"] = V
        raw = json.dumps(j, separators=(",", ":")).encode()
        out.append("0" + base64.b64encode(zlib.compress(raw, 9)).decode())
    p.write_text("\n".join(out) + "\n")
    print(f"stamped {name}")
PY
```

Then re-normalize through the CLI, which reparses (now correctly, as 1.1) and re-emits as 2.x:

```bash
dotnet run --project src/FactorioTools.Cli -- oil-field normalize
```

- [ ] **Step 7: Verify the corpus actually moved to 2.x**

```bash
python3 - <<'PY'
import base64, zlib, json
for name in ["small-list.txt", "big-list.txt"]:
    path = f"test/FactorioTools.Test/OilField/{name}"
    dirs, vers = {}, set()
    for line in open(path):
        s = line.strip()
        if not s or s.startswith("#"):
            continue
        bp = json.loads(zlib.decompress(base64.b64decode(s[1:])))["blueprint"]
        vers.add(bp.get("version"))
        for e in bp.get("entities", []):
            if e.get("name") == "pumpjack":
                dirs[e.get("direction")] = dirs.get(e.get("direction"), 0) + 1
    print(name, "directions:", dict(sorted(dirs.items(), key=lambda x: (x[0] is not None, x[0]))))
    print(name, "versions:", vers)
PY
```

Expected: directions are now a subset of `{None, 4, 8, 12}` with NO `2` and NO `6`. Versions are all `562949953421312` or higher, none `0`. If any `2` or `6` survives, stop; the conversion is not symmetric.

- [ ] **Step 8: Run the full suite and check the invariant**

Run: `dotnet test`

Expected: PASS, and **`Score.HasExpectedScore.verified.txt` unchanged**. Re-normalizing and fixing the parser are inverse operations, so they must cancel. Verify with:

```bash
git diff --stat test/FactorioTools.Test/OilField/Score.HasExpectedScore.verified.txt
```

Expected: no output. If the scoreboard moved, the change is WRONG. Do not accept the diff. Investigate whether the conversion is asymmetric or the corpus rewrite altered something beyond direction.

- [ ] **Step 9: Commit**

```bash
git add -A
git commit -m "Read blueprint directions using the blueprint's own version

Output already multiplied directions by 2 for Factorio 2.0's 16-way encoding,
but input did nothing, so a 2.x east pumpjack (4) parsed as Direction.Down.
That is the reported mis-rotation.

Sniffing the values cannot fix it: a blueprint whose directions are all in
{0, 4} is valid under both encodings with different meanings. Version is the
only sound signal, so ToInternalDirection gates on it, treating an unstamped
blueprint as modern because that is what users paste.

The corpus was 1.1-encoded, which is exactly why no test caught this. Both
lists are re-normalized to 2.x. SerializeBlueprint now stamps the version next
to the code that doubles the directions, so the two can never disagree; the
normalize path previously emitted 2.x directions under version 0.

Score.HasExpectedScore.verified.txt is unchanged, as it must be: the corpus
rewrite and the parser fix are inverse operations."
```

---

### Task 6: Regression test for the reported bug

Task 5 proves the conversion is self-consistent. This proves it against a blueprint the game actually produced.

**Files:**
- Modify: `test/FactorioTools.Test/OilField/ParseBlueprintTest.cs`

**Interfaces:**
- Consumes: `ParseBlueprint.ToInternalDirection` from Task 5, `Entity.Mirror` from Task 4.
- Produces: nothing.

- [ ] **Step 1: Confirm the fixture**

The string in Step 2 is real output from Factorio 2.1.14, already generated, so use it as written. It decodes to version `562954249306113` and:

```
{entity_number: 1}                  <- north, field omitted entirely
{entity_number: 2, direction: 4}    <- east
{entity_number: 3, direction: 8}    <- south
{entity_number: 4, direction: 12}   <- west
```

For reference, it came from the probe-mod technique in CLAUDE.md with these entities. Only regenerate if you need to change what is being tested:

```lua
stack.set_blueprint_entities{
  {entity_number = 1, name = "pumpjack", position = {x = 0.5,  y = 0.5}, direction = defines.direction.north},
  {entity_number = 2, name = "pumpjack", position = {x = 10.5, y = 0.5}, direction = defines.direction.east},
  {entity_number = 3, name = "pumpjack", position = {x = 20.5, y = 0.5}, direction = defines.direction.south},
  {entity_number = 4, name = "pumpjack", position = {x = 30.5, y = 0.5}, direction = defines.direction.west},
}
```

- [ ] **Step 2: Write the test**

```csharp
    /// <summary>
    /// Exported from Factorio 2.1.14: four pumpjacks facing north, east, south, west.
    /// This is the case the corpus cannot cover, because the corpus is 1.1-encoded.
    /// It is the test that would have caught the original bug report.
    /// </summary>
    private const string FourCardinalPumpjacks = "0eNqN0csKwyAQBdB/mbWU+Eho/JVSSh5Dsa1G1JSG4L/XJIsWkkBWg3rvWTgj1K8erVMmgBxBNZ3xIC8jeHU31Wu6M5VGkGB7bR9V84RIQJkWPyBpvBJAE1RQuLTmw3Azva7RpQBZtQnYzqdCZyY7IdkpJzDMM0ayItgBgv4bBFrlsFmexYbID4hsVzxviOKAyHdFyqZvVAF1An7bIPBG5+dEXrAyF0yUPCso5TF+Abr9jgg=";

    [Fact]
    public void ReadsEveryCardinalFromARealFactorio21Blueprint()
    {
        var blueprint = ParseBlueprint.Execute(FourCardinalPumpjacks);

        Assert.Collection(
            blueprint.Entities,
            e => Assert.Equal(Direction.Up, e.Direction ?? Direction.Up),
            e => Assert.Equal(Direction.Right, e.Direction),
            e => Assert.Equal(Direction.Down, e.Direction),
            e => Assert.Equal(Direction.Left, e.Direction));
    }
```

- [ ] **Step 3: Run to verify it passes**

Run: `dotnet test test/FactorioTools.Test/FactorioTools.Test.csproj --filter "FullyQualifiedName~ParseBlueprintTest"`

Expected: PASS. If east reads as `Direction.Down`, Task 5 did not take effect.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "Pin the reported bug with a blueprint the game actually produced

The corpus is 1.1-encoded, so it cannot cover a real 2.x blueprint. This
fixture came out of Factorio 2.1.14 with all four cardinals, which is the case
that was silently rotating."
```

---

### Task 7: Regenerate Lua, rebuild WASM, verify the build matrix

The core changed (`Entity.cs` in Task 4, `ItemNames.cs` in Task 2), so generated artifacts must follow or CI fails. `CleanBlueprint.cs` is deliberately untouched, so the core diff is small.

**Files:**
- Modify: `src/lua/**` (regenerated)
- Modify: `src/vue/public/framework/**` (regenerated, gitignored)

**Interfaces:**
- Consumes: every earlier task.
- Produces: nothing.

- [ ] **Step 1: Regenerate the Lua**

Run: `pwsh src/lua/Invoke-LuaBuild.ps1`

- [ ] **Step 2: Syntax-check the generated Lua**

Run (bash): `find src/lua -name '*.lua' -exec luac5.2 -p {} \;`

Expected: no output. Any output is a syntax error.

- [ ] **Step 3: Build and test under Lua settings**

Run: `dotnet build /p:UseLuaSettings=true && dotnet test /p:UseLuaSettings=true`

Expected: PASS. This catches core changes that break the Lua-safe configuration.

- [ ] **Step 4: Rebuild the WASM bundle**

Run: `cd src/vue && npm run build-wasm`

Then confirm the bundle landed in the right shape, not flattened:

```bash
test -f src/vue/public/framework/dotnet.js && echo "bundle shape ok" || echo "WRONG: dotnet.js is not in public/framework"
```

- [ ] **Step 5: Verify the front end still plans in a real browser**

Run: `cd src/vue && npm run build && npm run preview`

`npm run dev` cannot run the WASM planner; use build plus preview. Load the page, paste the four-cardinal blueprint from Task 6, and confirm the plan comes back with pumpjacks facing the right way.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "Regenerate the Lua for the 2.1 compatibility fixes

The core changed (Entity, ItemNames), so the committed Lua has to follow or
transpile-lua fails. Syntax-checked with luac5.2 and tested under
UseLuaSettings=true."
```

- [ ] **Step 7: Open the pull request**

```bash
git push -u origin fix/factorio-21-oracle
gh pr create --title "Speak Factorio 2.1 instead of 1.1" --body "$(cat <<'EOF'
Fixes mis-rotated pumpjacks and unrecognized renamed items reported since Factorio 2.1.

Both symptoms are one root cause: the planner still spoke Factorio 1.1.

## What was wrong

- `effectivity-module-3` has not existed since 2.0, so the game silently rejected it
- directions were multiplied by 2 on output but never divided on input, so a 2.x east pumpjack (4) parsed as south
- the `mirror` flag added for pumpjack flipping in 2.1.7 was dropped on parse
- the test corpus is 1.1-encoded, which is why none of this ever failed a test

## How it was found

Rather than trusting the wiki, `tools/capture-factorio-oracle.sh` pulls prototype facts out of the game and commits them as a fixture. `FactorioOracleTest` asserts the planner's constants against it on every build, with no Factorio install needed on CI. Re-capture after a game update and a changed fixture fails the test with a diff.

The version, direction and mirror encodings were confirmed by round-tripping blueprints through Factorio 2.1.14, and cross-checked against `wube/factorio-data` at tag 2.1.14.

## The safety check

Re-normalizing the corpus and fixing the parser are inverse operations, so `Score.HasExpectedScore.verified.txt` is **unchanged**. A moved scoreboard would have meant the change was wrong.

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review

**Spec coverage.** C1 landed already. C2 is Task 1. C3 is Task 5. C4 is Tasks 2 and 3. C5 is Task 4. C6 is Task 5 step 6. The invariant is Task 5 step 8. Lua and WASM are Task 7. The manual-drift-check decision needs no task. Out-of-scope items correctly have no task.

**Placeholder scan.** None. The two blueprint fixtures in Tasks 4 and 6 were generated from Factorio 2.1.14 and are embedded as literal strings, verified to decode to exactly what their tests assert. Every task is mechanical.

**Type consistency.** `ToInternalDirection(Direction, ulong)` is defined in Task 5 step 3 and used with that signature in Task 5 step 1 and Task 6. `Entity.Mirror` is `bool?` in Task 4 and asserted with `Assert.True(e.Mirror)` and `Assert.Null(e.Mirror)`, both valid for `bool?`. `migrateModuleNames` is generic over `T extends Record<string, unknown>` in Task 3 and called both ways consistently.

**Caught in self-review, already fixed above.** Task 5 originally had `CleanBlueprint` carry the source version through. That is wrong: `ToOutputDirection` always emits 2.x values, so a 1.1 input would come out with 2.x directions under a 1.1 stamp, and reparsing would halve values that were already converted. Stamping now happens in `SerializeBlueprint`, next to the doubling, so the two cannot disagree.

**Known soft spots for the executor.** Task 3 guesses the store's module property names; read `OilFieldStore.ts` and use the real ones. Task 3 also assumes `pinia-plugin-persistedstate` exposes `afterHydrate`; confirm against the installed version and use its documented hook if the name differs. Task 5 step 6's stamping pass rewrites the corpus with Python before the CLI re-normalize; run it on a clean tree so `git checkout` can undo it if the sequence needs a retry.
