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
    public void RawGeometryBehindTheOptionsPresetsIsUnchanged(string entity, string field, double expected)
    {
        var actual = Oracle().GetProperty("entities").GetProperty(entity).GetProperty(field).GetDouble();

        Assert.True(
            expected == actual,
            $"{entity}.{field} moved from {expected} to {actual}, so the OilFieldOptions presets need review. {ReCaptureHint}");
    }
}
