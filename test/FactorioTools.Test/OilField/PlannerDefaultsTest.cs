using System.Reflection;
using System.Text.Json;

namespace Knapcode.FactorioTools.OilField;

/// <summary>
/// Emits the planner constants the Vue app needs, so they are not typed out a second
/// time in TypeScript. The Vue app imports the verified file directly.
///
/// This test IS the generator. Locally, AutoVerify rewrites the file when the C#
/// changes and the test passes, so commit the rewritten file. On CI, AutoVerify is
/// off, so a stale committed file fails here with a diff.
/// </summary>
public class PlannerDefaultsTest
{
    /// <summary>
    /// <see cref="OilFieldOptions"/> properties (camelCased) that are deliberately not
    /// emitted into the artifact. Empty today: every public property is emitted. If you
    /// add one here, say why - this set exists so an exclusion is a visible decision,
    /// not an accidental omission caught by <see cref="EmitsEveryOilFieldOptionsProperty"/>.
    /// </summary>
    private static readonly HashSet<string> ExcludedFromArtifact = new();

    [Fact]
    public Task PlannerDefaults()
    {
        var payload = BuildPayload();

        var json = JsonSerializer.Serialize(payload);

        // UseStrictJson is required: Verify's default writes unquoted keys and string
        // values, which the Vue app cannot import.
        return VerifyJson(json)
            .UseStrictJson()
            .UseDirectory("../../../src/vue/src/lib")
            .UseFileName("plannerDefaults");
    }

    /// <summary>
    /// Guards against a new <see cref="OilFieldOptions"/> property silently never making
    /// it into the artifact: <see cref="BuildPayload"/> hand-lists which properties are
    /// emitted (so the Vue app gets a stable, deliberate shape), and this test asserts
    /// that hand-list is exhaustive. Adding a property to <see cref="OilFieldOptions"/>
    /// without emitting it (or excluding it, with a reason, in
    /// <see cref="ExcludedFromArtifact"/>) fails this test.
    /// </summary>
    [Fact]
    public void EmitsEveryOilFieldOptionsProperty()
    {
        var optionsJson = JsonSerializer.Serialize(BuildOptions());
        using var optionsDocument = JsonDocument.Parse(optionsJson);
        var emittedKeys = optionsDocument.RootElement
            .EnumerateObject()
            .Select(p => p.Name)
            .ToHashSet();

        var expectedKeys = typeof(OilFieldOptions)
            .GetProperties(BindingFlags.Public | BindingFlags.Instance)
            .Select(p => ToCamelCase(p.Name))
            .Where(name => !ExcludedFromArtifact.Contains(name))
            .ToHashSet();

        var missing = expectedKeys.Except(emittedKeys).OrderBy(x => x).ToList();
        var extra = emittedKeys.Except(expectedKeys).OrderBy(x => x).ToList();

        Assert.True(
            missing.Count == 0 && extra.Count == 0,
            "OilFieldOptions properties and the emitted artifact 'options' keys are out of sync.\n"
                + (missing.Count > 0
                    ? "Missing from the artifact - emit them in PlannerDefaultsTest.BuildPayload, or add "
                        + $"to ExcludedFromArtifact with a comment saying why: {string.Join(", ", missing)}\n"
                    : string.Empty)
                + (extra.Count > 0
                    ? "Emitted in the artifact but not a public OilFieldOptions property (stale key?): "
                        + $"{string.Join(", ", extra)}\n"
                    : string.Empty));
    }

    private static object BuildOptions()
    {
        var options = new OilFieldOptions();

        return new
        {
            useUndergroundPipes = options.UseUndergroundPipes,
            addBeacons = options.AddBeacons,
            optimizePipes = options.OptimizePipes,
            overlapBeacons = options.OverlapBeacons,
            addElectricPoles = options.AddElectricPoles,
            addHeatPipes = options.AddHeatPipes,
            heatPipeEntityName = options.HeatPipeEntityName,
            pipeStrategies = options.PipeStrategies.Select(s => s.ToString()).ToList(),
            beaconStrategies = options.BeaconStrategies.Select(s => s.ToString()).ToList(),
            electricPoleEntityName = options.ElectricPoleEntityName,
            electricPoleSupplyWidth = options.ElectricPoleSupplyWidth,
            electricPoleSupplyHeight = options.ElectricPoleSupplyHeight,
            electricPoleWireReach = options.ElectricPoleWireReach,
            electricPoleWidth = options.ElectricPoleWidth,
            electricPoleHeight = options.ElectricPoleHeight,
            beaconEntityName = options.BeaconEntityName,
            beaconSupplyWidth = options.BeaconSupplyWidth,
            beaconSupplyHeight = options.BeaconSupplyHeight,
            beaconWidth = options.BeaconWidth,
            beaconHeight = options.BeaconHeight,
            validateSolution = options.ValidateSolution,
            pumpjackModules = options.PumpjackModules,
            beaconModules = options.BeaconModules,
            pumpjackQuality = options.PumpjackQuality.ToString(),
            beaconQuality = options.BeaconQuality.ToString(),
            electricPoleQuality = options.ElectricPoleQuality.ToString(),
            pumpjackModuleQuality = options.PumpjackModuleQuality.ToString(),
            beaconModuleQuality = options.BeaconModuleQuality.ToString(),
        };
    }

    private static object BuildPayload()
    {
        return new
        {
            options = BuildOptions(),
            electricPolePresets = new Dictionary<string, object>
            {
                [OilFieldOptions.ForSmallIronElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForSmallIronElectricPole),
                [OilFieldOptions.ForSmallElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForSmallElectricPole),
                [OilFieldOptions.ForMediumElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForMediumElectricPole),
                [OilFieldOptions.ForBigElectricPole.ElectricPoleEntityName] = Preset(OilFieldOptions.ForBigElectricPole),
                [OilFieldOptions.ForSubstation.ElectricPoleEntityName] = Preset(OilFieldOptions.ForSubstation),
            },
            qualityLevels = Enum
                .GetValues<Quality>()
                .ToDictionary(q => q.ToString(), q => (int)q),
            allPipeStrategies = OilFieldOptions.AllPipeStrategies.Select(s => s.ToString()).ToList(),
            allBeaconStrategies = OilFieldOptions.AllBeaconStrategies.Select(s => s.ToString()).ToList(),
        };
    }

    private static object Preset(OilFieldOptions options)
    {
        return new
        {
            width = options.ElectricPoleWidth,
            height = options.ElectricPoleHeight,
            supplyWidth = options.ElectricPoleSupplyWidth,
            supplyHeight = options.ElectricPoleSupplyHeight,
            wireReach = options.ElectricPoleWireReach,
        };
    }

    private static string ToCamelCase(string name)
    {
        if (string.IsNullOrEmpty(name) || char.IsLower(name[0]))
        {
            return name;
        }

        return char.ToLowerInvariant(name[0]) + name[1..];
    }
}
