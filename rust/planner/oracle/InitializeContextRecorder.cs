// Records InitializeContext.Execute for the Rust port (#138), stage 3.
// Writes initialize_context.jsonl. The format is in rust/planner/FIXTURES.md.

using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

static class InitializeContextRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private sealed record Case(
        string Name,
        OilFieldOptions Options,
        Blueprint Blueprint,
        AvoidLocation[] Avoid,
        int MinWidth,
        int MinHeight);

    /// <summary>The presets and defaults, so the Rust test checks its own against them.</summary>
    private static readonly (string Name, Func<OilFieldOptions> Options)[] Presets =
    {
        ("Default", () => new OilFieldOptions()),
        ("ForSmallIronElectricPole", () => OilFieldOptions.ForSmallIronElectricPole),
        ("ForSmallElectricPole", () => OilFieldOptions.ForSmallElectricPole),
        ("ForMediumElectricPole", () => OilFieldOptions.ForMediumElectricPole),
        ("ForBigElectricPole", () => OilFieldOptions.ForBigElectricPole),
        ("ForSubstation", () => OilFieldOptions.ForSubstation),
    };

    public static void Record(string outputDir, string repoRoot, IReadOnlyList<string> smallList)
    {
        var bigList = ParseBlueprint.ReadBlueprintFile(Path.Combine(repoRoot, "test", "FactorioTools.Test", "OilField", "big-list.txt"));
        var cases = BuildCases(smallList, bigList);

        var path = Path.Combine(outputDir, "initialize_context.jsonl");
        var errors = 0;
        using (var stream = File.Create(path))
        {
            foreach (var (name, options) in Presets)
            {
                using (var writer = new Utf8JsonWriter(stream, WriterOptions))
                {
                    writer.WriteStartObject();
                    writer.WriteString("kind", "preset");
                    writer.WriteString("name", name);
                    writer.WritePropertyName("options");
                    WriteOptions(writer, options());
                    writer.WriteEndObject();
                }

                stream.WriteByte((byte)'\n');
            }

            foreach (var c in cases)
            {
                using (var writer = new Utf8JsonWriter(stream, WriterOptions))
                {
                    writer.WriteStartObject();
                    writer.WriteString("kind", "case");
                    writer.WriteString("name", c.Name);
                    writer.WritePropertyName("options");
                    WriteOptions(writer, c.Options);
                    WriteBlueprint(writer, c.Blueprint);
                    writer.WriteStartArray("avoid");
                    foreach (var a in c.Avoid)
                    {
                        writer.WriteStartArray();
                        writer.WriteNumberValue(BitConverter.SingleToInt32Bits(a.X));
                        writer.WriteNumberValue(BitConverter.SingleToInt32Bits(a.Y));
                        writer.WriteEndArray();
                    }

                    writer.WriteEndArray();
                    writer.WriteStartArray("min");
                    writer.WriteNumberValue(c.MinWidth);
                    writer.WriteNumberValue(c.MinHeight);
                    writer.WriteEndArray();

                    Context? context = null;
                    FactorioToolsException? error = null;
                    try
                    {
                        context = InitializeContext.Execute(c.Options, c.Blueprint, c.Avoid, c.MinWidth, c.MinHeight);
                    }
                    catch (FactorioToolsException ex)
                    {
                        error = ex;
                    }

                    if (error is not null)
                    {
                        errors++;
                        writer.WriteStartObject("error");
                        writer.WriteString("message", error.Message);
                        writer.WriteBoolean("bad_input", error.BadInput);
                        writer.WriteEndObject();
                    }
                    else
                    {
                        WriteContext(writer, c.Name, context!);
                    }

                    writer.WriteEndObject();
                }

                stream.WriteByte((byte)'\n');
            }
        }

        Console.WriteLine($"initialize_context.jsonl: {Presets.Length} presets, {cases.Count} cases ({errors} errors), {new FileInfo(path).Length} bytes");
    }

    private static List<Case> BuildCases(IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        var cases = new List<Case>();
        var none = Array.Empty<AvoidLocation>();

        // Every small-list blueprint with the medium pole; the other option sets on every
        // third blueprint, offset so each blueprint also meets other presets.
        var sets = new (string Name, Func<OilFieldOptions> Options, int Stride, int Offset)[]
        {
            ("medium", () => OilFieldOptions.ForMediumElectricPole, 1, 0),
            ("small", () => OilFieldOptions.ForSmallElectricPole, 3, 0),
            ("big", () => OilFieldOptions.ForBigElectricPole, 3, 1),
            ("substation", () => OilFieldOptions.ForSubstation, 3, 2),
            ("medium-no-beacons", () => { var o = OilFieldOptions.ForMediumElectricPole; o.AddBeacons = false; return o; }, 3, 1),
            ("medium-non-standard-beacon", () =>
            {
                var o = OilFieldOptions.ForMediumElectricPole;
                o.BeaconWidth = 2;
                o.BeaconHeight = 4;
                o.BeaconSupplyWidth = 4;
                o.BeaconSupplyHeight = 6;
                return o;
            }, 3, 2),
            ("medium-legendary-pole", () => { var o = OilFieldOptions.ForMediumElectricPole; o.ElectricPoleQuality = Quality.Legendary; return o; }, 6, 0),
            ("big-rare-everything", () =>
            {
                var o = OilFieldOptions.ForBigElectricPole;
                o.ElectricPoleQuality = Quality.Rare;
                o.PumpjackQuality = Quality.Epic;
                o.BeaconQuality = Quality.Uncommon;
                o.PumpjackModuleQuality = Quality.Legendary;
                o.BeaconModuleQuality = Quality.Rare;
                return o;
            }, 6, 3),
        };

        foreach (var set in sets)
        {
            for (var i = set.Offset; i < smallList.Count; i += set.Stride)
            {
                cases.Add(new Case($"small-{i:D4}_{set.Name}", set.Options(), ParseBlueprint.Execute(smallList[i]), none, 0, 0));
            }
        }

        // A sample of the big list with the medium pole.
        for (var i = 0; i < bigList.Count; i += 20)
        {
            cases.Add(new Case($"big-{i:D4}_medium", OilFieldOptions.ForMediumElectricPole, ParseBlueprint.Execute(bigList[i]), none, 0, 0));
        }

        // InitializeContextTest's inputs.
        var one = Centers((25.5f, -14.5f));
        var two = Centers((25.5f, -14.5f), (33.5f, -12.5f));
        var allSides = new[]
        {
            new AvoidLocation(29.5f, -24.5f),
            new AvoidLocation(43.5f, -13.5f),
            new AvoidLocation(29.5f, -2.5f),
            new AvoidLocation(15.5f, -13.5f),
        };
        foreach (var (w, h) in new[] { (0, 0), (0, 1), (1, 0), (1, 1), (5, 3) })
        {
            cases.Add(new Case($"test-empty-{w}x{h}", NoBeacons(OilFieldOptions.ForMediumElectricPole), EmptyBlueprint(), none, w, h));
        }

        cases.Add(new Case("test-one-substation-beacons", OilFieldOptions.ForSubstation, one, none, 0, 0));
        cases.Add(new Case("test-two-medium-beacons", OilFieldOptions.ForMediumElectricPole, two, none, 0, 0));
        cases.Add(new Case("test-avoid-less-than-min-y", OilFieldOptions.ForSubstation, two, new[] { new AvoidLocation(29.5f, -21.5f) }, 0, 0));
        cases.Add(new Case("test-avoid-all-sides", OilFieldOptions.ForSubstation, two, allSides, 0, 0));
        cases.Add(new Case("test-larger-min-no-avoid", OilFieldOptions.ForSubstation, two, none, 45, 0));
        cases.Add(new Case("test-larger-min-with-avoid", OilFieldOptions.ForSubstation, two, allSides, 50, 40));
        cases.Add(new Case("test-odd-min-padding", OilFieldOptions.ForMediumElectricPole, two, none, 30, 24));
        cases.Add(new Case("avoid-only", OilFieldOptions.ForMediumElectricPole, Centers(), new[] { new AvoidLocation(3, 4), new AvoidLocation(-2, 7) }, 0, 0));

        // PlannerTest.AllowsLocationsToBeAvoided.
        var avoided = new Blueprint
        {
            Entities = new[]
            {
                new Entity { Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = -3, Y = -5 } },
                new Entity { Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = 4, Y = 5 } },
            },
        };
        cases.Add(new Case("planner-test-avoid", OilFieldOptions.ForMediumElectricPole, avoided, Enumerable.Range(-7, 16).Select(x => new AvoidLocation(x, 0)).ToArray(), 0, 0));

        // Directions, a missing direction, and entities that are not pumpjacks.
        cases.Add(new Case("mixed-entities-and-directions", OilFieldOptions.ForMediumElectricPole, new Blueprint
        {
            Entities = new[]
            {
                new Entity { EntityNumber = 1, Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = 0.5f, Y = 0.5f }, Direction = Direction.Right },
                new Entity { EntityNumber = 2, Name = EntityNames.Vanilla.Pipe, Position = new Position { X = 4.5f, Y = 0.5f } },
                new Entity { EntityNumber = 3, Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = 8.5f, Y = -3.5f }, Direction = Direction.Left, Mirror = true },
                new Entity { EntityNumber = 4, Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = 0.5f, Y = 9.5f } },
                new Entity { EntityNumber = 5, Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = -6.5f, Y = 4.5f }, Direction = Direction.Down },
                new Entity { EntityNumber = 6, Name = EntityNames.Vanilla.Beacon, Position = new Position { X = 20.5f, Y = 20.5f } },
            },
        }, none, 0, 0));

        // Errors.
        cases.Add(new Case("error-too-many-pumpjacks", OilFieldOptions.ForMediumElectricPole,
            Centers(Enumerable.Range(0, 151).Select(i => ((float)(i % 15) * 4, (float)(i / 15) * 4)).ToArray()), none, 0, 0));
        cases.Add(new Case("error-non-integer-width", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f), (5.25f, 0.5f)), none, 0, 0));
        cases.Add(new Case("error-non-integer-height", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f), (5.5f, 4.75f)), none, 0, 0));
        // The outermost pumpjacks line up, so the grid size is whole, but a middle one does not.
        cases.Add(new Case("error-non-integer-entity-x", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f), (4.25f, 8.5f), (9.5f, 0.5f)), none, 0, 0));
        cases.Add(new Case("error-non-integer-entity-y", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f), (4.5f, 4.75f), (9.5f, 9.5f)), none, 0, 0));
        cases.Add(new Case("error-non-integer-avoid-x", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f)), new[] { new AvoidLocation(2.25f, 1) }, 0, 0));
        cases.Add(new Case("error-non-integer-avoid-y", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f)), new[] { new AvoidLocation(2.5f, 1.5f), new AvoidLocation(1.5f, 2.75f) }, 0, 0));
        cases.Add(new Case("error-grid-too-wide", OilFieldOptions.ForMediumElectricPole, Centers((0, 0), (1200, 0)), none, 0, 0));
        cases.Add(new Case("error-grid-area-too-large", OilFieldOptions.ForMediumElectricPole, Centers((0, 0), (600, 600)), none, 0, 0));
        cases.Add(new Case("error-avoid-on-pumpjack", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f)), new[] { new AvoidLocation(0.5f, 0.5f) }, 0, 0));
        cases.Add(new Case("error-avoid-twice", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f)), new[] { new AvoidLocation(4.5f, 0.5f), new AvoidLocation(4.5f, 0.5f) }, 0, 0));
        cases.Add(new Case("error-overlapping-pumpjacks", OilFieldOptions.ForMediumElectricPole, Centers((0.5f, 0.5f), (2.5f, 1.5f)), none, 0, 0));
        cases.Add(new Case("error-no-room-for-terminals", OilFieldOptions.ForMediumElectricPole,
            Centers((0.5f, 0.5f), (1.5f, -2.5f), (3.5f, 1.5f), (-0.5f, 3.5f), (-2.5f, -0.5f)), none, 0, 0));

        return cases;
    }

    private static OilFieldOptions NoBeacons(OilFieldOptions options)
    {
        options.AddBeacons = false;
        return options;
    }

    private static Blueprint EmptyBlueprint()
    {
        return new Blueprint
        {
            Entities = Array.Empty<Entity>(),
            Icons = new[] { new Icon { Index = 1, Signal = new SignalID { Name = EntityNames.Vanilla.Pumpjack, Type = SignalTypes.Vanilla.Item } } },
            Item = ItemNames.Vanilla.Blueprint,
        };
    }

    private static Blueprint Centers(params (float X, float Y)[] centers)
    {
        return new Blueprint
        {
            Entities = centers
                .Select((p, i) => new Entity { EntityNumber = i + 1, Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = p.X, Y = p.Y } })
                .ToArray(),
        };
    }

    private static void WriteOptions(Utf8JsonWriter writer, OilFieldOptions o)
    {
        writer.WriteStartObject();
        writer.WriteBoolean("use_underground_pipes", o.UseUndergroundPipes);
        writer.WriteBoolean("add_beacons", o.AddBeacons);
        writer.WriteBoolean("optimize_pipes", o.OptimizePipes);
        writer.WriteBoolean("overlap_beacons", o.OverlapBeacons);
        writer.WriteBoolean("add_electric_poles", o.AddElectricPoles);
        writer.WriteBoolean("add_heat_pipes", o.AddHeatPipes);
        writer.WriteString("heat_pipe_entity_name", o.HeatPipeEntityName);
        writer.WriteStartArray("pipe_strategies");
        foreach (var s in o.PipeStrategies)
        {
            writer.WriteNumberValue((int)s);
        }

        writer.WriteEndArray();
        writer.WriteStartArray("beacon_strategies");
        foreach (var s in o.BeaconStrategies)
        {
            writer.WriteNumberValue((int)s);
        }

        writer.WriteEndArray();
        writer.WriteString("electric_pole_entity_name", o.ElectricPoleEntityName);
        writer.WriteNumber("electric_pole_supply_width", o.ElectricPoleSupplyWidth);
        writer.WriteNumber("electric_pole_supply_height", o.ElectricPoleSupplyHeight);
        writer.WriteNumber("electric_pole_wire_reach_bits", BitConverter.DoubleToInt64Bits(o.ElectricPoleWireReach));
        writer.WriteNumber("electric_pole_width", o.ElectricPoleWidth);
        writer.WriteNumber("electric_pole_height", o.ElectricPoleHeight);
        writer.WriteString("beacon_entity_name", o.BeaconEntityName);
        writer.WriteNumber("beacon_supply_width", o.BeaconSupplyWidth);
        writer.WriteNumber("beacon_supply_height", o.BeaconSupplyHeight);
        writer.WriteNumber("beacon_width", o.BeaconWidth);
        writer.WriteNumber("beacon_height", o.BeaconHeight);
        writer.WriteBoolean("validate_solution", o.ValidateSolution);
        WriteModules(writer, "pumpjack_modules", o.PumpjackModules);
        WriteModules(writer, "beacon_modules", o.BeaconModules);
        writer.WriteNumber("pumpjack_quality", (int)o.PumpjackQuality);
        writer.WriteNumber("beacon_quality", (int)o.BeaconQuality);
        writer.WriteNumber("electric_pole_quality", (int)o.ElectricPoleQuality);
        writer.WriteNumber("pumpjack_module_quality", (int)o.PumpjackModuleQuality);
        writer.WriteNumber("beacon_module_quality", (int)o.BeaconModuleQuality);
        writer.WriteEndObject();
    }

    private static void WriteModules(Utf8JsonWriter writer, string name, Dictionary<string, int> modules)
    {
        writer.WriteStartArray(name);
        foreach (var (item, count) in modules)
        {
            writer.WriteStartArray();
            writer.WriteStringValue(item);
            writer.WriteNumberValue(count);
            writer.WriteEndArray();
        }

        writer.WriteEndArray();
    }

    /// <summary>The fields InitializeContext reads, plus mirror. Positions are float bits.</summary>
    private static void WriteBlueprint(Utf8JsonWriter writer, Blueprint blueprint)
    {
        writer.WriteStartArray("entities");
        foreach (var e in blueprint.Entities)
        {
            writer.WriteStartArray();
            writer.WriteNumberValue(e.EntityNumber);
            writer.WriteStringValue(e.Name);
            writer.WriteNumberValue(BitConverter.SingleToInt32Bits(e.Position.X));
            writer.WriteNumberValue(BitConverter.SingleToInt32Bits(e.Position.Y));
            if (e.Direction.HasValue)
            {
                writer.WriteNumberValue((int)e.Direction.Value);
            }
            else
            {
                writer.WriteNullValue();
            }

            if (e.Mirror.HasValue)
            {
                writer.WriteBooleanValue(e.Mirror.Value);
            }
            else
            {
                writer.WriteNullValue();
            }

            writer.WriteEndArray();
        }

        writer.WriteEndArray();
    }

    private static void WriteTerminal(Utf8JsonWriter writer, TerminalLocation t)
    {
        writer.WriteStartArray();
        writer.WriteNumberValue(t.Center.X);
        writer.WriteNumberValue(t.Center.Y);
        writer.WriteNumberValue(t.Terminal.X);
        writer.WriteNumberValue(t.Terminal.Y);
        writer.WriteNumberValue((int)t.Direction);
        writer.WriteEndArray();
    }

    private static void WriteTerminalDictionary(Utf8JsonWriter writer, string name, ILocationDictionary<List<TerminalLocation>> dictionary)
    {
        writer.WriteStartArray(name);
        foreach (var pair in dictionary.EnumeratePairs())
        {
            writer.WriteStartArray();
            writer.WriteNumberValue(pair.Key.X);
            writer.WriteNumberValue(pair.Key.Y);
            writer.WriteStartArray();
            foreach (var t in pair.Value)
            {
                WriteTerminal(writer, t);
            }

            writer.WriteEndArray();
            writer.WriteEndArray();
        }

        writer.WriteEndArray();
    }

    private static void WriteContext(Utf8JsonWriter writer, string name, Context context)
    {
        var grid = context.Grid;
        writer.WriteStartObject("context");
        writer.WriteNumber("delta_x_bits", BitConverter.SingleToInt32Bits(context.DeltaX));
        writer.WriteNumber("delta_y_bits", BitConverter.SingleToInt32Bits(context.DeltaY));
        writer.WriteNumber("width", grid.Width);
        writer.WriteNumber("height", grid.Height);
        writer.WriteStartArray("middle");
        writer.WriteNumberValue(grid.Middle.X);
        writer.WriteNumberValue(grid.Middle.Y);
        writer.WriteEndArray();

        writer.WriteStartArray("entities");
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            var entity = grid[location] ?? throw new InvalidOperationException($"No entity at {location} in {name}.");
            writer.WriteStartArray();
            writer.WriteNumberValue(location.X);
            writer.WriteNumberValue(location.Y);
            writer.WriteStringValue(entity.GetType().Name);
            writer.WriteNumberValue(entity.Id);
            switch (entity)
            {
                case PumpjackCenter pumpjack:
                    writer.WriteNumberValue((int)pumpjack.Direction);
                    break;
                case PumpjackSide side:
                    writer.WriteNumberValue(side.Center.Id);
                    break;
                case AvoidEntity:
                    break;
                default:
                    throw new NotSupportedException($"InitializeContext placed an unexpected {entity.GetType().Name} in {name}.");
            }

            writer.WriteEndArray();
        }

        writer.WriteEndArray();

        // The id the next GetId() call returns. Taking it changes nothing recorded above.
        writer.WriteNumber("next_id", grid.GetId());

        writer.WriteStartArray("centers");
        foreach (var center in context.Centers)
        {
            writer.WriteStartArray();
            writer.WriteNumberValue(center.X);
            writer.WriteNumberValue(center.Y);
            writer.WriteEndArray();
        }

        writer.WriteEndArray();

        WriteTerminalDictionary(writer, "center_to_terminals", context.CenterToTerminals);

        writer.WriteStartArray("center_to_original_direction");
        foreach (var pair in context.CenterToOriginalDirection.EnumeratePairs())
        {
            writer.WriteStartArray();
            writer.WriteNumberValue(pair.Key.X);
            writer.WriteNumberValue(pair.Key.Y);
            writer.WriteNumberValue((int)pair.Value);
            writer.WriteEndArray();
        }

        writer.WriteEndArray();

        WriteTerminalDictionary(writer, "location_to_terminals", context.LocationToTerminals);

        // Sparse: most cells border no pumpjack side.
        writer.WriteNumber("adjacent_count_length", context.LocationToAdjacentCount.Length);
        writer.WriteStartArray("adjacent_counts");
        for (var i = 0; i < context.LocationToAdjacentCount.Length; i++)
        {
            if (context.LocationToAdjacentCount[i] != 0)
            {
                writer.WriteStartArray();
                writer.WriteNumberValue(i);
                writer.WriteNumberValue(context.LocationToAdjacentCount[i]);
                writer.WriteEndArray();
            }
        }

        writer.WriteEndArray();

        writer.WriteNumber("electric_pole_supply_width_with_quality", context.ElectricPoleSupplyWidthWithQuality);
        writer.WriteNumber("electric_pole_supply_height_with_quality", context.ElectricPoleSupplyHeightWithQuality);
        writer.WriteNumber("electric_pole_wire_reach_with_quality_bits", BitConverter.DoubleToInt64Bits(context.ElectricPoleWireReachWithQuality));
        writer.WriteNumber("electric_pole_wire_reach_squared_with_quality_bits", BitConverter.DoubleToInt64Bits(context.ElectricPoleWireReachSquaredWithQuality));
        writer.WriteEndObject();
    }
}
