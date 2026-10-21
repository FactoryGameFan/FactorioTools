// Records AddPipesFbe.Execute for the Rust port (#138), stage 6.
// Writes fbe_pipes.jsonl. The format is in rust/planner/FIXTURES.md.

using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

static class FbePipesRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private sealed record Case(string Name, OilFieldOptions Options, Blueprint Blueprint, AvoidLocation[] Avoid, bool PolesFirst, bool Walled = false);

    public static void Record(string outputDir, IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        var cases = new List<Case>();
        for (var i = 0; i < smallList.Count; i++)
        {
            var blueprint = ParseBlueprint.Execute(smallList[i]);

            // The grid's size depends on AddBeacons and the pole's footprint, and the middle
            // (which FBE measures distances from) moves with it, so cover each shape.
            cases.Add(new Case($"small-{i:D4}_medium_b", Opts(OilFieldOptions.ForMediumElectricPole, beacons: true), blueprint, [], false));
            cases.Add(new Case($"small-{i:D4}_medium", Opts(OilFieldOptions.ForMediumElectricPole, beacons: false), blueprint, [], false));
            if (i % 2 == 0)
            {
                cases.Add(new Case($"small-{i:D4}_substation_b", Opts(OilFieldOptions.ForSubstation, beacons: true), blueprint, [], false));
            }
            else
            {
                cases.Add(new Case($"small-{i:D4}_big", Opts(OilFieldOptions.ForBigElectricPole, beacons: false), blueprint, [], false));
            }

            // Poles placed before pipes (the planner's retry mode): the poles block cells and
            // remove the terminals they cover.
            if (i % 3 == 0)
            {
                cases.Add(new Case($"small-{i:D4}_medium_poles-first", Opts(OilFieldOptions.ForMediumElectricPole, beacons: false), blueprint, [], true));
            }

            // Not a real planner input: the last pumpjack's terminals are walled in, so FBE
            // cannot reach them. This drives the failure paths (no path between terminals,
            // the FbeOriginal fallback, the thrown "should have" checks), which real inputs
            // rarely reach.
            if (i % 4 == 0)
            {
                cases.Add(new Case($"small-{i:D4}_medium_walled", Opts(OilFieldOptions.ForMediumElectricPole, beacons: false), blueprint, [], false, Walled: true));
            }
        }

        for (var i = 0; i < bigList.Count; i += 20)
        {
            cases.Add(new Case($"big-{i:D4}_medium_b", Opts(OilFieldOptions.ForMediumElectricPole, beacons: true), ParseBlueprint.Execute(bigList[i]), [], false));
        }

        cases.Add(AvoidCase());

        var path = Path.Combine(outputDir, "fbe_pipes.jsonl");
        var count = 0;
        var errors = 0;
        var fallbacks = 0;
        using (var stream = File.Create(path))
        {
            foreach (var c in cases)
            {
                foreach (var strategy in new[] { PipeStrategy.FbeOriginal, PipeStrategy.Fbe })
                {
                    var context = Prepare(c);
                    if (context is null)
                    {
                        continue;
                    }

                    var outcome = Write(stream, $"{c.Name}_{strategy}", strategy, context);
                    count++;
                    if (outcome == Outcome.Error)
                    {
                        errors++;
                    }
                    else if (outcome == Outcome.Fallback)
                    {
                        fallbacks++;
                    }
                }
            }
        }

        Console.WriteLine($"fbe_pipes.jsonl: {count} cases ({errors} errors, {fallbacks} FbeOriginal fallbacks to Fbe), {new FileInfo(path).Length} bytes");
    }

    private static OilFieldOptions Opts(OilFieldOptions options, bool beacons)
    {
        options.AddBeacons = beacons;
        options.ValidateSolution = true;
        return options;
    }

    private static Case AvoidCase()
    {
        // From PlannerTest.AllowsLocationsToBeAvoided.
        var options = OilFieldOptions.ForMediumElectricPole;
        options.ValidateSolution = true;
        options.AddBeacons = true;
        var blueprint = new Blueprint
        {
            Entities = new[]
            {
                new Entity { Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = -3, Y = -5 } },
                new Entity { Name = EntityNames.Vanilla.Pumpjack, Position = new Position { X = 4, Y = 5 } },
            },
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
            Version = 0,
        };
        var avoid = Enumerable.Range(-7, 16).Select(x => new AvoidLocation(x, 0)).ToArray();
        return new Case("PlannerTest.AllowsLocationsToBeAvoided", options, blueprint, avoid, false);
    }

    /// <summary>
    /// The context AddPipes.GetSolutionGroups hands AddPipesFbe.Execute: the starting context
    /// with the terminal maps cloned (AddPipes.0.cs ~:443). Null when the planner would not call
    /// FBE (one pumpjack is handled without a strategy) or the poles-first placement fails.
    /// </summary>
    private static Context? Prepare(Case c)
    {
        var context = InitializeContext.Execute(c.Options, c.Blueprint, c.Avoid);
        if (c.PolesFirst)
        {
            var poles = AddElectricPoles.Execute(context, EmptyLocationSet.Instance, allowRetries: false);
            if (poles is null)
            {
                return null;
            }

            // Planner.cs: remove terminals overlapping with the added poles.
            foreach (var terminals in context.CenterToTerminals.Values)
            {
                for (int i = 0; i < terminals.Count; i++)
                {
                    var terminal = terminals[i];
                    if (context.Grid.IsEmpty(terminal.Terminal))
                    {
                        continue;
                    }

                    terminals.RemoveAt(i);
                    context.LocationToTerminals[terminal.Terminal].Remove(terminal);
                    i--;
                }
            }
        }

        if (context.CenterToTerminals.Count <= 1)
        {
            return null;
        }

        if (c.Walled)
        {
            var center = context.Centers[context.Centers.Count - 1];
            foreach (var terminal in context.CenterToTerminals[center])
            {
                foreach (var (dx, dy) in new[] { (1, 0), (0, -1), (-1, 0), (0, 1) })
                {
                    var cell = terminal.Terminal.Translate(dx, dy);
                    if (context.Grid.IsInBounds(cell)
                        && context.Grid.IsEmpty(cell)
                        && !context.LocationToTerminals.ContainsKey(cell))
                    {
                        context.Grid.AddEntity(cell, new TemporaryEntity(context.Grid.GetId()));
                    }
                }
            }
        }

        context.CenterToTerminals = Clone(context, context.CenterToTerminals);
        context.LocationToTerminals = Clone(context, context.LocationToTerminals);
        return context;
    }

    /// <summary>CollectionExtensions.ToDictionary (internal), as GetSolutionGroups uses it.</summary>
    private static ILocationDictionary<List<TerminalLocation>> Clone(Context context, ILocationDictionary<List<TerminalLocation>> original)
    {
        var pairs = original.EnumeratePairs();
        var dictionary = context.GetLocationDictionary<List<TerminalLocation>>(pairs.Count);
        foreach (var pair in pairs)
        {
            dictionary.Add(pair.Key, pair.Value.ToList());
        }

        return dictionary;
    }

    private enum Outcome { Data, Fallback, Error }

    private static Outcome Write(Stream stream, string name, PipeStrategy strategy, Context context)
    {
        using var w = new Utf8JsonWriter(stream, WriterOptions);
        w.WriteStartObject();
        w.WriteString("name", name);
        w.WriteNumber("strategy", (int)strategy);
        w.WriteNumber("width", context.Grid.Width);
        w.WriteNumber("height", context.Grid.Height);
        WritePoint(w, "middle", context.Grid.Middle);
        WriteLocations(w, "occupied", context.Grid.EntityLocations.EnumerateItems());
        WriteLocations(w, "centers", context.Centers);
        WriteTerminals(w, "center_to_terminals", context.CenterToTerminals);
        WriteTerminals(w, "location_to_terminals", context.LocationToTerminals);

        Outcome outcome;
        Result<AddPipesFbe.FbeResult>? result = null;
        Exception? thrown = null;
        try
        {
            result = AddPipesFbe.Execute(context, strategy);
        }
        catch (Exception ex)
        {
            thrown = ex;
        }

        var exception = thrown ?? result!.Exception;
        if (exception is not null)
        {
            outcome = Outcome.Error;
            w.WriteStartObject("error");
            w.WriteString("type", exception.GetType().Name);
            w.WriteString("message", exception.Message);
            w.WriteBoolean("thrown", thrown is not null);
            if (exception is FactorioToolsException fte)
            {
                w.WriteBoolean("bad_input", fte.BadInput);
            }

            if (exception is NoPathBetweenTerminalsException noPath)
            {
                w.WriteStartArray("no_path");
                w.WriteNumberValue(noPath.LocationA.X);
                w.WriteNumberValue(noPath.LocationA.Y);
                w.WriteNumberValue(noPath.LocationB.X);
                w.WriteNumberValue(noPath.LocationB.Y);
                w.WriteEndArray();
            }

            w.WriteEndObject();
        }
        else
        {
            var (pipes, finalStrategy) = result!.Data!;
            outcome = finalStrategy != strategy ? Outcome.Fallback : Outcome.Data;
            w.WriteNumber("final_strategy", (int)finalStrategy);
            WriteLocations(w, "pipes", pipes.EnumerateItems());
        }

        // The terminal maps afterwards: EliminateOtherTerminals on success, untouched on failure.
        WriteTerminals(w, "center_to_terminals_after", context.CenterToTerminals);
        WriteTerminals(w, "location_to_terminals_after", context.LocationToTerminals);

        w.WriteEndObject();
        w.Flush();
        stream.WriteByte((byte)'\n');
        return outcome;
    }

    private static void WritePoint(Utf8JsonWriter w, string name, Location l)
    {
        w.WriteStartArray(name);
        w.WriteNumberValue(l.X);
        w.WriteNumberValue(l.Y);
        w.WriteEndArray();
    }

    /// <summary>A flat [x0, y0, x1, y1, ...] array, in the given order.</summary>
    private static void WriteLocations(Utf8JsonWriter w, string name, IEnumerable<Location> locations)
    {
        w.WriteStartArray(name);
        foreach (var l in locations)
        {
            w.WriteNumberValue(l.X);
            w.WriteNumberValue(l.Y);
        }

        w.WriteEndArray();
    }

    /// <summary>[[key x, key y, [center x, center y, terminal x, terminal y, direction, ...]], ...] in enumeration order.</summary>
    private static void WriteTerminals(Utf8JsonWriter w, string name, ILocationDictionary<List<TerminalLocation>> dictionary)
    {
        w.WriteStartArray(name);
        foreach (var pair in dictionary.EnumeratePairs())
        {
            w.WriteStartArray();
            w.WriteNumberValue(pair.Key.X);
            w.WriteNumberValue(pair.Key.Y);
            w.WriteStartArray();
            foreach (var t in pair.Value)
            {
                w.WriteNumberValue(t.Center.X);
                w.WriteNumberValue(t.Center.Y);
                w.WriteNumberValue(t.Terminal.X);
                w.WriteNumberValue(t.Terminal.Y);
                w.WriteNumberValue((int)t.Direction);
            }

            w.WriteEndArray();
            w.WriteEndArray();
        }

        w.WriteEndArray();
    }
}
