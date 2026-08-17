using System.Reflection;
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
        "Two likely causes: either Factorio changed (re-run tools/capture-factorio-oracle.sh "
        + "and review the diff), or a name was added to EntityNames.Vanilla without adding it "
        + "to WANTED_ENTITIES in tools/trim-factorio-oracle.py.";

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

    /// <summary>
    /// This reflects EntityNames.Vanilla only, so EntityNames.AaiIndustry is deliberately NOT
    /// checked. Those names come from the AAI Industry mod, and the oracle is captured with
    /// mods disabled on purpose, so their absence is expected rather than drift.
    /// </summary>
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
    // Beacon strength itself, not just its supply area. distribution_effectivity is the
    // multiplier a beacon applies to the module effects it broadcasts (1.5 = 150%). Nothing
    // in the planner reads this value today, but pinning it means a Factorio balance change
    // that alters beacon strength is noticed here rather than silently changing plan quality.
    [InlineData(EntityNames.Vanilla.Beacon, "distribution_effectivity", 1.5)]
    public void RawGeometryBehindTheOptionsPresetsIsUnchanged(string entity, string field, double expected)
    {
        var actual = Oracle().GetProperty("entities").GetProperty(entity).GetProperty(field).GetDouble();

        Assert.True(
            expected == actual,
            $"{entity}.{field} moved from {expected} to {actual}, so the OilFieldOptions presets need review. {ReCaptureHint}");
    }

    private static double[] Coordinates(JsonElement point)
    {
        return point.EnumerateArray().Select(x => x.GetDouble()).ToArray();
    }

    /// <summary>
    /// GridToBlueprintString.EntityNameToSize hardcodes each entity's tile footprint (3x3
    /// pumpjack, 2x2 substation, etc.), used only to compute FBE's coordinate offset.
    ///
    /// The fixture stores the raw collision_box, not a tile footprint: tile_width/tile_height
    /// are absent from the dump for most of these prototypes (data.raw does not always carry
    /// them; Factorio derives the tile footprint itself). So the tile footprint is derived here
    /// the same way the game does it: round the collision box's width/height up to a whole
    /// number of tiles. This was verified against every hardcoded size below before being
    /// trusted as an assertion; if a future entity's derivation disagrees with the hardcoded
    /// size, that is a real finding to report, not something to paper over by hand-adjusting
    /// either side.
    ///
    /// EntityNames.AaiIndustry.SmallIronElectricPole is skipped: it is a mod entity and the
    /// oracle is captured with mods disabled on purpose, so it is never in the fixture.
    /// </summary>
    [Fact]
    public void EntityNameToSizeMatchesTheDerivedCollisionBoxFootprint()
    {
        var sizes = (IReadOnlyDictionary<string, (float Width, float Height)>)typeof(GridToBlueprintString)
            .GetField("EntityNameToSize", BindingFlags.NonPublic | BindingFlags.Static)!
            .GetValue(null)!;

        var entities = Oracle().GetProperty("entities");
        var mismatches = new List<string>();

        foreach (var (name, size) in sizes)
        {
            if (!entities.TryGetProperty(name, out var entity))
            {
                continue;
            }

            var collisionBox = entity.GetProperty("collision_box").EnumerateArray().Select(Coordinates).ToArray();
            var min = collisionBox[0];
            var max = collisionBox[1];
            var width = (float)Math.Ceiling(max[0] - min[0]);
            var height = (float)Math.Ceiling(max[1] - min[1]);

            if (width != size.Width || height != size.Height)
            {
                mismatches.Add($"{name}: EntityNameToSize says {size.Width}x{size.Height}, collision_box derives {width}x{height}");
            }
        }

        Assert.True(mismatches.Count == 0, $"{string.Join("; ", mismatches)}. {ReCaptureHint}");
    }

    /// <summary>
    /// PlanUndergroundPipes.MaxUnderground (11) counts both underground ends of a run, while
    /// Factorio's max_underground_distance (10, on pipe-to-ground) counts only the gap between
    /// them - the two ends are not part of the distance. So the two are off by exactly one, not
    /// equal, and that relationship is the thing worth pinning.
    /// </summary>
    [Fact]
    public void MaxUndergroundIsOneMoreThanFactoriosGapDistance()
    {
        var maxUnderground = (int)typeof(PlanUndergroundPipes)
            .GetField("MaxUnderground", BindingFlags.NonPublic | BindingFlags.Static)!
            .GetValue(null)!;

        var maxUndergroundDistance = Oracle()
            .GetProperty("entities")
            .GetProperty(EntityNames.Vanilla.PipeToGround)
            .GetProperty("fluid_box")
            .GetProperty("pipe_connections")
            .EnumerateArray()
            .First(c => c.TryGetProperty("max_underground_distance", out _))
            .GetProperty("max_underground_distance")
            .GetInt32();

        Assert.True(
            maxUnderground == maxUndergroundDistance + 1,
            $"PlanUndergroundPipes.MaxUnderground is {maxUnderground} but max_underground_distance "
                + $"is now {maxUndergroundDistance}, so the relationship (MaxUnderground == "
                + $"max_underground_distance + 1) no longer holds. {ReCaptureHint}");
    }
}
