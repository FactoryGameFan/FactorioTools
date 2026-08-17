using System.IO.Compression;
using System.Text;
using System.Text.Json.Serialization;
using System.Text.Json;
using Knapcode.FactorioTools.Data;
using System.Collections.Generic;
using System;
using System.Linq;
using System.IO;
using System.Runtime.CompilerServices;

namespace Knapcode.FactorioTools.OilField;

public static class GridToBlueprintString
{
    private static readonly IReadOnlyDictionary<string, (float Width, float Height)> EntityNameToSize = new Dictionary<string, (float Width, float Height)>()
    {
        { EntityNames.Vanilla.Beacon, (3, 3) },
        { EntityNames.Vanilla.BigElectricPole, (2, 2) },
        { EntityNames.Vanilla.MediumElectricPole, (1, 1) },
        { EntityNames.Vanilla.Pipe, (1, 1) },
        { EntityNames.Vanilla.PipeToGround, (1, 1) },
        { EntityNames.Vanilla.Pumpjack, (3, 3) },
        { EntityNames.Vanilla.HeatPipe, (1, 1) },
        { EntityNames.Vanilla.SmallElectricPole, (1, 1) },
        { EntityNames.Vanilla.Substation, (2, 2) },

        { EntityNames.AaiIndustry.SmallIronElectricPole, (1, 1) },
    };

    /// <summary>
    /// Converts an internal <see cref="Direction"/> (1.1-style 8-way values: N=0, E=2, S=4, W=6) to the value emitted in
    /// the output blueprint. Factorio 2.0 uses 16-way directions (N=0, E=4, S=8, W=12); always emit 2.0 directions.
    /// </summary>
    private static Direction ToOutputDirection(Direction direction)
    {
        // Factorio 2.0 uses 16-way directions (N=0, E=4, S=8, W=12); internal directions are 1.1-style 8-way.
        return (Direction)((int)direction * 2);
    }

    /// <summary>
    /// The version stamped on every blueprint this class emits. It must be 2.0 or later,
    /// because <see cref="ToOutputDirection"/> always writes Factorio 2.0's 16-way direction
    /// values, and <see cref="ParseBlueprint.ToInternalDirection"/> reads the stamp to decide
    /// how to read them back.
    /// </summary>
    private static readonly ulong OutputVersion = FormatVersion(2, 0, 32, 0);

    // defines.inventory.mining_drill_modules and defines.inventory.beacon_modules in Factorio 2.0, used to target the
    // module inventory in the 2.0 "items" array form (confirmed against a real 2.0 export).
    private const int MiningDrillModuleInventory = 2;
    private const int BeaconModuleInventory = 1;

    private static string? ToOutputQuality(Quality quality)
    {
        return quality == Quality.Normal ? null : Qualities.ToBlueprintString(quality);
    }

    /// <summary>
    /// Produces the value for an entity's "items" field as a list of <see cref="ModuleInsertPlan"/> targeting the
    /// given module inventory. Returns null when there are no modules so the field is omitted.
    /// </summary>
    private static object? ToOutputItems(Dictionary<string, int> modules, int inventory, Quality quality)
    {
        if (modules is null || modules.Count == 0)
        {
            return null;
        }

        var qualityString = ToOutputQuality(quality);
        var plans = new List<ModuleInsertPlan>();
        var stack = 0;
        foreach (var pair in modules)
        {
            plans.Add(new ModuleInsertPlan { Name = pair.Key, Inventory = inventory, StartStack = stack, Count = pair.Value, Quality = qualityString });
            stack += pair.Value;
        }

        return plans;
    }

    public static string Execute(Context context, bool addFbeOffset, bool addAvoidEntities)
    {
        var entities = new List<Entity>();
        var nextEntityNumber = 1;

        var gridIdToEntityNumber = new Dictionary<int, int>();

        int GetEntityNumber(GridEntity entity)
        {
            if (!gridIdToEntityNumber!.TryGetValue(entity.Id, out var entityNumber))
            {
                entityNumber = nextEntityNumber++;
                gridIdToEntityNumber.Add(entity.Id, entityNumber);
            }

            return entityNumber;
        }

        foreach (var location in context.Grid.EntityLocations.EnumerateItems())
        {
            var gridEntity = context.Grid[location];
            var position = new Position
            {
                X = location.X,
                Y = location.Y,
            };

            switch (gridEntity)
            {
                case PumpjackCenter pumpjackCenter:
                    entities.Add(new Entity
                    {
                        EntityNumber = nextEntityNumber++,
                        Direction = pumpjackCenter.Direction,
                        Name = EntityNames.Vanilla.Pumpjack,
                        Position = position,
                        Items = ToOutputItems(context.Options.PumpjackModules, MiningDrillModuleInventory, context.Options.PumpjackModuleQuality),
                        Quality = ToOutputQuality(context.Options.PumpjackQuality),
                    });
                    break;
                case PumpjackSide:
                    // Ignore
                    break;
                case UndergroundPipe undergroundPipe:
                    entities.Add(new Entity
                    {
                        EntityNumber = nextEntityNumber++,
                        Direction = undergroundPipe.Direction,
                        Name = EntityNames.Vanilla.PipeToGround,
                        Position = position,
                    });
                    break;
                case HeatPipe:
                    entities.Add(new Entity
                    {
                        EntityNumber = nextEntityNumber++,
                        Name = context.Options.HeatPipeEntityName,
                        Position = position,
                    });
                    break;
                case Pipe:
                    entities.Add(new Entity
                    {
                        EntityNumber = nextEntityNumber++,
                        Name = EntityNames.Vanilla.Pipe,
                        Position = position,
                    });
                    break;
                case ElectricPoleCenter electricPole:
                    if (context.Options.ElectricPoleWidth % 2 == 0)
                    {
                        position.X += 0.5f;
                    }

                    if (context.Options.ElectricPoleHeight % 2 == 0)
                    {
                        position.Y += 0.5f;
                    }

                    entities.Add(new Entity
                    {
                        EntityNumber = GetEntityNumber(electricPole),
                        Name = context.Options.ElectricPoleEntityName,
                        Position = position,
                        Quality = ToOutputQuality(context.Options.ElectricPoleQuality),
                        Neighbours = electricPole
                            .Neighbors
                            .Select(id => context.Grid[context.Grid.EntityIdToLocation[id]]!)
                            .Select(GetEntityNumber)
                            .ToArray(),
                    });
                    break;
                case ElectricPoleSide:
                    // Ignore
                    break;
                case BeaconCenter beacon:
                    if (context.Options.BeaconWidth % 2 == 0)
                    {
                        position.X += 0.5f;
                    }

                    if (context.Options.BeaconHeight % 2 == 0)
                    {
                        position.Y += 0.5f;
                    }

                    entities.Add(new Entity
                    {
                        EntityNumber = nextEntityNumber++,
                        Name = context.Options.BeaconEntityName,
                        Position = position,
                        Items = ToOutputItems(context.Options.BeaconModules, BeaconModuleInventory, context.Options.BeaconModuleQuality),
                        Quality = ToOutputQuality(context.Options.BeaconQuality),
                    });
                    break;
                case AvoidEntity:
                    if (addAvoidEntities)
                    {
                        entities.Add(new Entity
                        {
                            EntityNumber = nextEntityNumber++,
                            Name = EntityNames.Vanilla.Wall,
                            Position = position,
                        });
                    }
                    break;
                case BeaconSide:
                case TemporaryEntity:
                    break;
                default:
                    throw new NotImplementedException();
            }
        }

        var blueprint = new Blueprint
        {
            Icons = new[]
            {
                new Icon
                {
                    Index = 1,
                    Signal = new SignalID
                    {
                        Name = EntityNames.Vanilla.Pumpjack,
                        Type = SignalTypes.Vanilla.Item,
                    }
                }
            },
            Item = ItemNames.Vanilla.Blueprint,
            Entities = entities.ToArray(),
        };

        return SerializeBlueprint(blueprint, addFbeOffset);
    }

    /// <summary>
    /// Source: https://wiki.factorio.com/Version_string_format
    /// </summary>
    public static (ushort major, ushort minor, ushort patch, ushort developer) ParseVersion(ulong version)
    {
        return (
            (ushort)((version >> 48) & 0xFFFF),
            (ushort)((version >> 32) & 0xFFFF),
            (ushort)((version >> 16) & 0xFFFF),
            (ushort)(version & 0xFFFF)
        );
    }

    public static ulong FormatVersion(ushort major, ushort minor, ushort patch, ushort developer)
    {
        return ((ulong)major << 48) | ((ulong)minor << 32) | ((ulong)patch << 16) | developer;
    }

    /// <summary>
    /// Turns a blueprint holding internal directions into a blueprint string. This rewrites the
    /// blueprint it is given - version, directions, and with addFbeOffset the entity array - so
    /// call it once per blueprint. Every caller today builds a fresh blueprint for it.
    /// </summary>
    public static string SerializeBlueprint(Blueprint blueprint, bool addFbeOffset)
    {
        // Every blueprint string this class produces goes through here, so this is the one
        // place that converts internal directions to Factorio 2.0's 16-way values and the one
        // place that stamps the version saying so. Keeping them together means they can never
        // disagree. Before, only the planner path converted, so the normalize path (see
        // NormalizeBlueprints and PlanOrchestrator, which serialize a CleanBlueprint result)
        // emitted internal 1.1-style directions under version 0.
        blueprint.Version = OutputVersion;
        for (var i = 0; i < blueprint.Entities.Length; i++)
        {
            var entity = blueprint.Entities[i];
            if (entity.Direction.HasValue)
            {
                entity.Direction = ToOutputDirection(entity.Direction.Value);
            }
        }

        // FBE applies some offset to the blueprint coordinates. This makes it hard to compare the grid used in memory
        // with the rendered blueprint in FBE. To account for this, we can add an entity to the corner of the
        // blueprint with a position that makes FBE keep the original entity positions used by the grid.
        if (addFbeOffset && blueprint.Entities.Length > 0)
        {
            var maxX = float.MinValue;
            var maxY = float.MinValue;
            var maxEntityNumber = int.MinValue;

            foreach (var entity in blueprint.Entities)
            {
                // Fall back to a 1x1 footprint for entities not in the table (e.g. avoid-entity walls or custom
                // entity names) so the FBE offset calculation never throws on an unknown name.
                if (!EntityNameToSize.TryGetValue(entity.Name, out var size))
                {
                    size = (1, 1);
                }

                maxX = Math.Max(maxX, entity.Position.X + size.Width / 2);
                maxY = Math.Max(maxY, entity.Position.Y + size.Height / 2);
                maxEntityNumber = Math.Max(maxEntityNumber, entity.EntityNumber);
            }

            blueprint.Entities = blueprint.Entities.Append(new Entity
            {
                EntityNumber = maxEntityNumber + 1,
                Name = EntityNames.Vanilla.Wall,
                Position = new Position
                {
                    X = (float)-Math.Ceiling(maxX),
                    Y = (float)-Math.Ceiling(maxY),
                },
            }).ToArray();
        }

        var root = new BlueprintRoot { Blueprint = blueprint };

        var json = JsonSerializer.Serialize(root, typeof(BlueprintRoot), BlueprintSerialization.Context);

        var bytes = Encoding.UTF8.GetBytes(json);
        using var outputStream = new MemoryStream();

        using var zlibStream = new ZLibStream(outputStream, CompressionLevel.Optimal);
        zlibStream.Write(bytes, 0, bytes.Length);
        zlibStream.Flush();
        zlibStream.Dispose();
        var base64 = Convert.ToBase64String(outputStream.ToArray());

        return '0' + base64;
    }
}
