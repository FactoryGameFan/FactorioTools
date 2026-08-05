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
    [Fact]
    public Task PlannerDefaults()
    {
        var options = new OilFieldOptions();

        var payload = new
        {
            options = new
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
            },
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

        var json = JsonSerializer.Serialize(payload);

        // UseStrictJson is required: Verify's default writes unquoted keys and string
        // values, which the Vue app cannot import.
        return VerifyJson(json)
            .UseStrictJson()
            .UseDirectory("../../../src/vue/src/lib")
            .UseFileName("plannerDefaults");
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
}
