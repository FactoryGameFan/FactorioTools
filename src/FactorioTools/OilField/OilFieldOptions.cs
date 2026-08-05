using System.Collections.Generic;
using Knapcode.FactorioTools.Data;

namespace Knapcode.FactorioTools.OilField;

// These defaults and the For* presets below also ship to the Vue app, emitted by
// PlannerDefaultsTest into src/vue/src/lib/plannerDefaults.verified.json. Change a
// number here and `dotnet test` will rewrite that file; commit it with your change.
public class OilFieldOptions
{
    public static OilFieldOptions ForSmallIronElectricPole
    {
        get
        {
            return new OilFieldOptions
            {
                ElectricPoleEntityName = EntityNames.AaiIndustry.SmallIronElectricPole,
                ElectricPoleSupplyWidth = 5,
                ElectricPoleSupplyHeight = 5,
                ElectricPoleWireReach = 7.5,
                ElectricPoleWidth = 1,
                ElectricPoleHeight = 1,
            };
        }
    }

    public static OilFieldOptions ForSmallElectricPole
    {
        get
        {
            return new OilFieldOptions
            {
                ElectricPoleEntityName = EntityNames.Vanilla.SmallElectricPole,
                ElectricPoleSupplyWidth = 5,
                ElectricPoleSupplyHeight = 5,
                ElectricPoleWireReach = 7.5,
                ElectricPoleWidth = 1,
                ElectricPoleHeight = 1,
            };
        }
    }

    public static OilFieldOptions ForMediumElectricPole
    {
        get
        {
            return new OilFieldOptions
            {
                ElectricPoleEntityName = EntityNames.Vanilla.MediumElectricPole,
                ElectricPoleSupplyWidth = 7,
                ElectricPoleSupplyHeight = 7,
                ElectricPoleWireReach = 9,
                ElectricPoleWidth = 1,
                ElectricPoleHeight = 1,
            };
        }
    }

    public static OilFieldOptions ForBigElectricPole
    {
        get
        {
            return new OilFieldOptions
            {
                ElectricPoleEntityName = EntityNames.Vanilla.BigElectricPole,
                ElectricPoleSupplyWidth = 4,
                ElectricPoleSupplyHeight = 4,
                ElectricPoleWireReach = 32,
                ElectricPoleWidth = 2,
                ElectricPoleHeight = 2,
            };
        }
    }

    public static OilFieldOptions ForSubstation
    {
        get
        {
            return new OilFieldOptions
            {
                ElectricPoleEntityName = EntityNames.Vanilla.Substation,
                ElectricPoleSupplyWidth = 18,
                ElectricPoleSupplyHeight = 18,
                ElectricPoleWireReach = 18,
                ElectricPoleWidth = 2,
                ElectricPoleHeight = 2,
            };
        }
    }

    public static IReadOnlyList<PipeStrategy> AllPipeStrategies { get; } = new[]
    {
        PipeStrategy.FbeOriginal,
        PipeStrategy.Fbe,
        PipeStrategy.ConnectedCentersDelaunay,
        PipeStrategy.ConnectedCentersDelaunayMst,
        PipeStrategy.ConnectedCentersFlute,
    };

    public static IReadOnlyList<PipeStrategy> DefaultPipeStrategies { get; } = new[]
    {
        PipeStrategy.Fbe,
        PipeStrategy.ConnectedCentersDelaunay,
        PipeStrategy.ConnectedCentersDelaunayMst,
        PipeStrategy.ConnectedCentersFlute,
    };

    public static IReadOnlyList<BeaconStrategy> AllBeaconStrategies { get; } = new[]
    {
        BeaconStrategy.FbeOriginal,
        BeaconStrategy.Fbe,
        BeaconStrategy.Snug,
    };

    public static IReadOnlyList<BeaconStrategy> DefaultBeaconStrategies { get; } = new[]
    {
        BeaconStrategy.Fbe,
        BeaconStrategy.Snug,
    };

    /// <summary>
    /// Whether or not underground pipes (pipe-to-ground) should be used.
    /// </summary>
    public bool UseUndergroundPipes { get; set; } = true;

    /// <summary>
    /// Whether or not to add beacons around the pumpjacks.
    /// </summary>
    public bool AddBeacons { get; set; } = true;

    /// <summary>
    /// Whether or not to use the pipe optimizer after each pipe strategy is executed. If set to true, the best solution
    /// found will still be used, meaning if the unoptimized pipe plan performs better, it will be preferred over the
    /// corresponding optimized pipe plan.
    /// </summary>
    public bool OptimizePipes { get; set; } = true;

    /// <summary>
    /// Whether or to allow beacon effects to overlap. For Factorio mods like Space Exploration, beacon effects cannot
    /// overlap otherwise pumpjacks will break down with a beacon overload. For vanilla Factorio, this should be true.
    /// </summary>
    public bool OverlapBeacons { get; set; } = true;

    /// <summary>
    /// Whether or not to add electric poles around the pumpjacks and (optionally) beacons.
    /// </summary>
    public bool AddElectricPoles { get; set; } = true;

    /// <summary>
    /// Whether or not to route a heat pipe network adjacent to every pumpjack and pipe. This is required on Aquilo
    /// (Factorio 2.0 / Space Age) where unheated entities freeze. When enabled, the output blueprint is emitted in the
    /// Factorio 2.0 version format (2.0 directions and module item format). The planner only routes the heat pipe network
    /// and leaves it for the user to connect a heat source (heating tower or reactor). Heat pipes and beacons
    /// (see <see cref="AddBeacons"/>) can be enabled together: heat is the hard constraint, so the planner routes the
    /// heat network first and then places beacons around it, choosing a pipe layout it can fully heat. Beacons are
    /// best-effort and may be reduced (even to none) on tight fields where heat needs the contested tiles.
    /// </summary>
    public bool AddHeatPipes { get; set; } = false;

    /// <summary>
    /// The internal entity name for the heat pipe to use.
    /// </summary>
    public string HeatPipeEntityName { get; set; } = EntityNames.Vanilla.HeatPipe;

    /// <summary>
    /// The pipe planning strategies to attempt.
    /// </summary>
    public List<PipeStrategy> PipeStrategies { get; set; } = new List<PipeStrategy>(DefaultPipeStrategies);

    /// <summary>
    /// The beacon planning strategies to attempt. This will have no affect if <see cref="AddBeacons"/> is false.
    /// </summary>
    public List<BeaconStrategy> BeaconStrategies { get; set; } = new List<BeaconStrategy>(DefaultBeaconStrategies);

    /// <summary>
    /// The internal entity name for the electric pole to use.
    /// </summary>
    public string ElectricPoleEntityName { get; set; } = EntityNames.Vanilla.MediumElectricPole;

    /// <summary>
    /// The supply width (horizontal) for the electric pole. This is the width of the area that the electric pole will
    /// provide power to.
    /// </summary>
    public int ElectricPoleSupplyWidth { get; set; } = 7;

    /// <summary>
    /// The supply height (vertical) for the electric pole. This is the height of the area that the electric pole will
    /// provide power to.
    /// </summary>
    public int ElectricPoleSupplyHeight { get; set; } = 7;

    private const double DefaultElectricPoleWireReach = 9;

    /// <summary>
    /// The wire reach for the electric pole. This is how far apart electric poles can be but still be connected.
    /// </summary>
    public double ElectricPoleWireReach { get; set; } = DefaultElectricPoleWireReach;

    /// <summary>
    /// The width of the electric pole entity.
    /// </summary>
    public int ElectricPoleWidth { get; set; } = 1;

    /// <summary>
    /// The height of the electric pole entity.
    /// </summary>
    public int ElectricPoleHeight { get; set; } = 1;

    /// <summary>
    /// The internal entity name for the beacon to use.
    /// </summary>
    public string BeaconEntityName { get; set; } = EntityNames.Vanilla.Beacon;

    /// <summary>
    /// The supply width (horizontal) for the beacon. This is the width of the area that the beacon will provide
    /// module effects to.
    /// </summary>
    public int BeaconSupplyWidth { get; set; } = 9;

    /// <summary>
    /// The supply height (vertical) for the beacon. This is the height of the area that the beacon will provide
    /// module effects to.
    /// </summary>
    public int BeaconSupplyHeight { get; set; } = 9;

    /// <summary>
    /// The width of the beacon entity.
    /// </summary>
    public int BeaconWidth { get; set; } = 3;

    /// <summary>
    /// The height of the beacon entity.
    /// </summary>
    public int BeaconHeight { get; set; } = 3;

    /// <summary>
    /// Whether or not additional validations should be perform on the blueprint correctness. In most cases this should
    /// be false. If you see an invalid blueprint returned, try setting this to true and reporting a bug.
    /// </summary>
    public bool ValidateSolution { get; set; } = false;

    /// <summary>
    /// The modules to add to the pumpjacks. The string key is the internal item name for the module. The value is the
    /// count that kind of module to add to each pumpjack. There can be multiple module types provided.
    /// </summary>
    public Dictionary<string, int> PumpjackModules { get; set; } = new Dictionary<string, int>
    {
        { ItemNames.Vanilla.ProductivityModule3, 2 },
    };

    /// <summary>
    /// The modules to add to the beacons. The string key is the internal item name for the module. The value is the
    /// count that kind of module to add to each beacon. There can be multiple module types provided.
    /// </summary>
    public Dictionary<string, int> BeaconModules { get; set; } = new Dictionary<string, int>
    {
        { ItemNames.Vanilla.SpeedModule3, 2 },
    };

    /// <summary>
    /// The quality of the pumpjack entities in the output blueprint. Output-only; does not affect planning.
    /// </summary>
    public Quality PumpjackQuality { get; set; } = Quality.Normal;

    /// <summary>
    /// The quality of the beacon entities in the output blueprint. Output-only; does not affect planning.
    /// </summary>
    public Quality BeaconQuality { get; set; } = Quality.Normal;

    /// <summary>
    /// The quality of the electric pole entities. Higher quality enlarges the supply area and wire reach
    /// the planner uses (see Context), so this affects planning, not just output.
    /// </summary>
    public Quality ElectricPoleQuality { get; set; } = Quality.Normal;

    /// <summary>
    /// The quality of the modules inserted into pumpjacks. Output-only; does not affect planning.
    /// </summary>
    public Quality PumpjackModuleQuality { get; set; } = Quality.Normal;

    /// <summary>
    /// The quality of the modules inserted into beacons. Output-only; does not affect planning.
    /// </summary>
    public Quality BeaconModuleQuality { get; set; } = Quality.Normal;
}
