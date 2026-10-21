// Records the connected-centers pipe strategies for the Rust port (#138), stage 7.
// Writes connected_centers.jsonl. The format is in rust/planner/FIXTURES.md.
//
// For each case it builds the planner's starting Context (InitializeContext.Execute), then
// for each connected-centers strategy does what AddPipes.GetSolutionGroups does: give the
// context fresh copies of its terminal maps, call AddPipesConnectedCenters.GetConnectedPumpjacks,
// then AddPipesConnectedCenters.FindTrunksAndConnect, and record the graph, the pipes (or the
// error) and the terminal maps those calls leave behind. No C# under src/ is changed.

using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

static class ConnectedCentersRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static readonly PipeStrategy[] Strategies =
    {
        PipeStrategy.ConnectedCentersDelaunay,
        PipeStrategy.ConnectedCentersDelaunayMst,
        PipeStrategy.ConnectedCentersFlute,
    };

    private sealed record Case(string Name, string Preset, bool ValidateSolution, Blueprint Blueprint, AvoidLocation[] Avoid);

    public static void Record(string outputDir, IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        var cases = BuildCases(smallList, bigList);
        var path = Path.Combine(outputDir, "connected_centers.jsonl");
        var written = 0;
        var skipped = 0;
        var runs = 0;
        var returnedErrors = 0;
        var thrownErrors = 0;

        using (var stream = File.Create(path))
        {
            foreach (var c in cases)
            {
                Context context;
                try
                {
                    context = InitializeContext.Execute(Options(c), c.Blueprint, c.Avoid);
                }
                catch (FactorioToolsException)
                {
                    // Bad inputs (e.g. an avoided cell under a pumpjack) are stage 3's to check.
                    skipped++;
                    continue;
                }

                using (var writer = new Utf8JsonWriter(stream, WriterOptions))
                {
                    writer.WriteStartObject();
                    writer.WriteString("name", c.Name);
                    writer.WriteString("preset", c.Preset);
                    writer.WriteBoolean("validate_solution", c.ValidateSolution);
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

                    // The starting state, so the Rust test can tell an InitializeContext
                    // difference apart from a connected-centers one.
                    WriteLocations(writer, "centers", context.Centers);
                    WriteTerminalDictionary(writer, "center_to_terminals", context.CenterToTerminals);
                    WriteTerminalDictionary(writer, "location_to_terminals", context.LocationToTerminals);

                    var originalCenterToTerminals = context.CenterToTerminals;
                    var originalLocationToTerminals = context.LocationToTerminals;

                    writer.WriteStartArray("runs");
                    foreach (var strategy in Strategies)
                    {
                        runs++;

                        // AddPipes.GetSolutionGroups: fresh dictionaries and lists, the same
                        // TerminalLocation objects, in the original enumeration order.
                        context.CenterToTerminals = CopyTerminals(context, originalCenterToTerminals);
                        context.LocationToTerminals = CopyTerminals(context, originalLocationToTerminals);

                        writer.WriteStartObject();
                        writer.WriteNumber("strategy", (int)strategy);

                        ILocationDictionary<ILocationSet>? graph = null;
                        try
                        {
                            graph = AddPipesConnectedCenters.GetConnectedPumpjacks(context, strategy);
                        }
                        catch (Exception ex)
                        {
                            thrownErrors++;
                            WriteError(writer, "graph_error", ex);
                        }

                        if (graph is not null)
                        {
                            writer.WriteStartArray("graph");
                            foreach (var pair in graph.EnumeratePairs())
                            {
                                writer.WriteStartArray();
                                writer.WriteNumberValue(pair.Key.X);
                                writer.WriteNumberValue(pair.Key.Y);
                                WriteFlat(writer, pair.Value.EnumerateItems());
                                writer.WriteEndArray();
                            }

                            writer.WriteEndArray();

                            Result<ILocationSet>? result = null;
                            try
                            {
                                result = AddPipesConnectedCenters.FindTrunksAndConnect(context, graph);
                            }
                            catch (Exception ex)
                            {
                                thrownErrors++;
                                WriteError(writer, "thrown_error", ex);
                            }

                            if (result is not null)
                            {
                                if (result.Exception is not null)
                                {
                                    returnedErrors++;
                                    WriteError(writer, "returned_error", result.Exception);
                                }
                                else
                                {
                                    writer.WritePropertyName("pipes");
                                    WriteFlat(writer, result.Data!.EnumerateItems());
                                }
                            }

                            WriteTerminalDictionary(writer, "center_to_terminals_after", context.CenterToTerminals);
                            WriteTerminalDictionary(writer, "location_to_terminals_after", context.LocationToTerminals);
                        }

                        writer.WriteEndObject();
                    }

                    writer.WriteEndArray();
                    writer.WriteEndObject();
                }

                stream.WriteByte((byte)'\n');
                written++;
            }
        }

        Console.WriteLine($"connected_centers.jsonl: {written} cases ({skipped} skipped as bad input), {runs} strategy runs, {returnedErrors} returned and {thrownErrors} thrown errors, {new FileInfo(path).Length} bytes");
    }

    private static List<Case> BuildCases(IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        var cases = new List<Case>();
        var none = Array.Empty<AvoidLocation>();

        for (var i = 0; i < smallList.Count; i++)
        {
            var blueprint = ParseBlueprint.Execute(smallList[i]);

            // The PlannerOnAllBlueprintsAndOptions theory validates every solution, which also
            // runs the bidirectional-graph check in GetConnectedPumpjacks.
            cases.Add(new Case($"small-{i:D4}_medium", "ForMediumElectricPole", true, blueprint, none));

            // Pole presets change the grid's padding, so the same field plans on another grid.
            var other = (i % 3) switch
            {
                0 => "ForSmallElectricPole",
                1 => "ForBigElectricPole",
                _ => "ForSubstation",
            };
            cases.Add(new Case($"small-{i:D4}_{other}", other, false, blueprint, none));

            // Avoided cells near the pumpjacks bend the A* paths and block trunks.
            if (i % 3 == 0)
            {
                cases.Add(new Case($"small-{i:D4}_medium_avoid", "ForMediumElectricPole", true, blueprint, AvoidNearPumpjacks(blueprint, seed: i)));
            }

            // A ring of avoided cells around one pumpjack cuts it off, so A* finds no path to
            // it. Which pumpjack decides whether the error is thrown or returned in a Result.
            if (i % 2 == 0 && Ring(blueprint, pumpjackIndex: i / 2) is { } ring)
            {
                cases.Add(new Case($"small-{i:D4}_medium_ring", "ForMediumElectricPole", true, blueprint, ring));
            }
        }

        for (var i = 0; i < bigList.Count; i += 8)
        {
            var blueprint = ParseBlueprint.Execute(bigList[i]);
            cases.Add(new Case($"big-{i:D4}_medium", "ForMediumElectricPole", true, blueprint, none));
        }

        // PlannerTest.AllowsLocationsToBeAvoided: a wall of avoided cells between the two
        // pumpjacks.
        var wallBlueprint = new Blueprint
        {
            Entities = new[]
            {
                new Entity { EntityNumber = 1, Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = -3, Y = -5 } },
                new Entity { EntityNumber = 2, Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = 4, Y = 5 } },
            },
        };
        var wall = Enumerable.Range(-7, 16).Select(x => new AvoidLocation(x, 0)).ToArray();
        cases.Add(new Case("PlannerTest.AllowsLocationsToBeAvoided", "ForMediumElectricPole", true, wallBlueprint, wall));

        cases.Add(new Case("ring-two-pumpjacks", "ForMediumElectricPole", true, wallBlueprint, Ring(wallBlueprint, pumpjackIndex: 0)!));

        return cases;
    }

    private static OilFieldOptions Options(Case c)
    {
        var options = c.Preset switch
        {
            "ForSmallElectricPole" => OilFieldOptions.ForSmallElectricPole,
            "ForMediumElectricPole" => OilFieldOptions.ForMediumElectricPole,
            "ForBigElectricPole" => OilFieldOptions.ForBigElectricPole,
            "ForSubstation" => OilFieldOptions.ForSubstation,
            _ => throw new NotSupportedException(c.Preset),
        };
        options.ValidateSolution = c.ValidateSolution;
        return options;
    }

    /// <summary>
    /// Up to 8 cells two or three tiles from a pumpjack's center, picked with a fixed seed,
    /// skipping any cell inside a pumpjack. Offsets are whole numbers, so each lands on a cell.
    /// </summary>
    private static AvoidLocation[] AvoidNearPumpjacks(Blueprint blueprint, int seed)
    {
        var pumpjacks = blueprint.Entities.Where(e => e.Name == EntityNames.Vanilla.Pumpjack).Select(e => e.Position).ToList();
        var candidates = new List<(float X, float Y)>();
        foreach (var p in pumpjacks)
        {
            for (var dx = -3; dx <= 3; dx++)
            {
                for (var dy = -3; dy <= 3; dy++)
                {
                    if (Math.Max(Math.Abs(dx), Math.Abs(dy)) < 2)
                    {
                        continue;
                    }

                    var x = p.X + dx;
                    var y = p.Y + dy;
                    if (pumpjacks.Any(q => Math.Abs(q.X - x) <= 1 && Math.Abs(q.Y - y) <= 1))
                    {
                        continue;
                    }

                    if (!candidates.Contains((x, y)))
                    {
                        candidates.Add((x, y));
                    }
                }
            }
        }

        var random = new Random(seed);
        var picked = new List<AvoidLocation>();
        while (picked.Count < 8 && candidates.Count > 0)
        {
            var index = random.Next(candidates.Count);
            picked.Add(new AvoidLocation(candidates[index].X, candidates[index].Y));
            candidates.RemoveAt(index);
        }

        return picked.ToArray();
    }

    /// <summary>
    /// The cells exactly three tiles (Chebyshev) from one pumpjack's center, which encloses
    /// its terminals. Null when the ring would cross another pumpjack.
    /// </summary>
    private static AvoidLocation[]? Ring(Blueprint blueprint, int pumpjackIndex)
    {
        var pumpjacks = blueprint.Entities.Where(e => e.Name == EntityNames.Vanilla.Pumpjack).Select(e => e.Position).ToList();
        if (pumpjacks.Count < 2)
        {
            return null;
        }

        var center = pumpjacks[pumpjackIndex % pumpjacks.Count];
        var ring = new List<AvoidLocation>();
        for (var dx = -3; dx <= 3; dx++)
        {
            for (var dy = -3; dy <= 3; dy++)
            {
                if (Math.Max(Math.Abs(dx), Math.Abs(dy)) != 3)
                {
                    continue;
                }

                var x = center.X + dx;
                var y = center.Y + dy;
                if (pumpjacks.Any(q => Math.Abs(q.X - x) <= 1 && Math.Abs(q.Y - y) <= 1))
                {
                    return null;
                }

                ring.Add(new AvoidLocation(x, y));
            }
        }

        return ring.ToArray();
    }

    private static ILocationDictionary<List<TerminalLocation>> CopyTerminals(Context context, ILocationDictionary<List<TerminalLocation>> original)
    {
        var copy = context.GetLocationDictionary<List<TerminalLocation>>(original.Count);
        foreach (var pair in original.EnumeratePairs())
        {
            copy.Add(pair.Key, new List<TerminalLocation>(pair.Value));
        }

        return copy;
    }

    private static void WriteError(Utf8JsonWriter writer, string name, Exception ex)
    {
        writer.WriteStartObject(name);
        writer.WriteString("type", ex.GetType().Name);
        writer.WriteString("message", ex.Message);
        if (ex is NoPathBetweenTerminalsException noPath)
        {
            writer.WriteStartArray("a");
            writer.WriteNumberValue(noPath.LocationA.X);
            writer.WriteNumberValue(noPath.LocationA.Y);
            writer.WriteEndArray();
            writer.WriteStartArray("b");
            writer.WriteNumberValue(noPath.LocationB.X);
            writer.WriteNumberValue(noPath.LocationB.Y);
            writer.WriteEndArray();
        }

        writer.WriteEndObject();
    }

    private static void WriteFlat(Utf8JsonWriter writer, IEnumerable<Location> locations)
    {
        writer.WriteStartArray();
        foreach (var l in locations)
        {
            writer.WriteNumberValue(l.X);
            writer.WriteNumberValue(l.Y);
        }

        writer.WriteEndArray();
    }

    private static void WriteLocations(Utf8JsonWriter writer, string name, IEnumerable<Location> locations)
    {
        writer.WritePropertyName(name);
        WriteFlat(writer, locations);
    }

    /// <summary>[entity_number, name, x_bits, y_bits, direction or null, mirror or null], as in initialize_context.jsonl.</summary>
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

    /// <summary>[[key_x, key_y, [[center_x, center_y, terminal_x, terminal_y, direction], ...]], ...].</summary>
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
                writer.WriteStartArray();
                writer.WriteNumberValue(t.Center.X);
                writer.WriteNumberValue(t.Center.Y);
                writer.WriteNumberValue(t.Terminal.X);
                writer.WriteNumberValue(t.Terminal.Y);
                writer.WriteNumberValue((int)t.Direction);
                writer.WriteEndArray();
            }

            writer.WriteEndArray();
            writer.WriteEndArray();
        }

        writer.WriteEndArray();
    }
}
