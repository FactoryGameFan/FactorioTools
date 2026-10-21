// Records RotateOptimize for the Rust port (#138), stage 8.
// Writes rotate_optimize.jsonl. The format is in rust/planner/FIXTURES.md.
//
// AddPipes.GetSolutionGroups runs each pipe strategy on fresh copies of the terminal maps, then
// OptimizeAndAddSolutions copies the maps again and calls RotateOptimize.Execute on a copy of
// the strategy's pipes. This does the same through the public API. The recorded input pipes are
// a compact rebuild (the strategy's pipes added in order to a fresh set): HashSet's copy
// constructor can keep freed slots, which a rebuild from a list cannot reproduce, and later
// adds would fill them in a different order. Each case also runs on the planner's real copy and
// records whether that gave the same result, so the fixture says how often the hidden state
// mattered.

using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

static class RotateOptimizeRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static int _caseCount;
    private static int _errorCount;
    private static int _changedCount;
    private static int _realCopyDiffers;
    private static int _strategyFailures;

    public static void Record(string outputDir, IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        _caseCount = 0;
        _errorCount = 0;
        _changedCount = 0;
        _realCopyDiffers = 0;
        _strategyFailures = 0;

        var path = Path.Combine(outputDir, "rotate_optimize.jsonl");
        var grids = new Dictionary<string, int>(StringComparer.Ordinal);
        using (var stream = File.Create(path))
        {
            for (var i = 0; i < smallList.Count; i++)
            {
                var blueprint = ParseBlueprint.Execute(smallList[i]);
                RecordBlueprint(stream, grids, $"small-{i:D4}", blueprint, OilFieldOptions.ForMediumElectricPole, polesFirst: false);

                // Planner.cs's poles-first path: poles go in before the pipes, and terminals
                // under a pole are dropped. RotateOptimize then walks a grid that holds poles.
                if (i % 3 == 0)
                {
                    RecordBlueprint(stream, grids, $"small-{i:D4}-poles-first-small", blueprint, OilFieldOptions.ForSmallElectricPole, polesFirst: true);
                }

                if (i % 3 == 1)
                {
                    RecordBlueprint(stream, grids, $"small-{i:D4}-poles-first-big", blueprint, OilFieldOptions.ForBigElectricPole, polesFirst: true);
                }
            }

            for (var i = 0; i < bigList.Count; i += 20)
            {
                var blueprint = ParseBlueprint.Execute(bigList[i]);
                RecordBlueprint(stream, grids, $"big-{i:D4}", blueprint, OilFieldOptions.ForMediumElectricPole, polesFirst: false);
            }

            // Every one-pumpjack blueprint in the big list: AddPipes's single-terminal path,
            // where RotateOptimize returns at once.
            var singles = 0;
            for (var i = 0; i < bigList.Count && singles < 3; i++)
            {
                var blueprint = ParseBlueprint.Execute(bigList[i]);
                var options = OilFieldOptions.ForMediumElectricPole;
                options.PipeStrategies = OilFieldOptions.AllPipeStrategies.ToList();
                var context = InitializeContext.Execute(options, blueprint, Array.Empty<AvoidLocation>());
                if (context.CenterToTerminals.Count != 1)
                {
                    continue;
                }

                singles++;
                var terminal = context.CenterToTerminals.EnumeratePairs().Single().Value[0];
                Helpers.EliminateOtherTerminals(context, terminal);
                var pipes = context.GetSingleLocationSet(terminal.Terminal);
                RecordCall(stream, grids, $"big-{i:D4}-single", "single", context, pipes);
            }
        }

        Console.WriteLine(
            $"rotate_optimize.jsonl: {_caseCount} cases ({_changedCount} changed which cells are pipes, {_errorCount} errors, " +
            $"{_realCopyDiffers} differ on the planner's real pipe copy, {_strategyFailures} strategy runs failed and were skipped), " +
            $"{grids.Count} grids, {new FileInfo(path).Length} bytes");
    }

    private static void RecordBlueprint(Stream stream, Dictionary<string, int> grids, string name, Blueprint blueprint, OilFieldOptions options, bool polesFirst)
    {
        options.PipeStrategies = OilFieldOptions.AllPipeStrategies.ToList();
        Context context;
        try
        {
            context = InitializeContext.Execute(options, blueprint, Array.Empty<AvoidLocation>());
        }
        catch (FactorioToolsException)
        {
            return;
        }

        if (context.CenterToTerminals.Count < 2)
        {
            return;
        }

        if (polesFirst)
        {
            var poles = AddElectricPoles.Execute(context, EmptyLocationSet.Instance, allowRetries: false);
            if (poles is null)
            {
                return;
            }

            // Copied from Planner.Execute: remove terminals overlapping with the added poles.
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

        var originalCenterToTerminals = context.CenterToTerminals;
        var originalLocationToTerminals = context.LocationToTerminals;

        foreach (var strategy in context.Options.PipeStrategies)
        {
            context.CenterToTerminals = Copy(context, originalCenterToTerminals);
            context.LocationToTerminals = Copy(context, originalLocationToTerminals);

            ILocationSet pipes;
            try
            {
                switch (strategy)
                {
                    case PipeStrategy.FbeOriginal:
                    case PipeStrategy.Fbe:
                        {
                            var result = AddPipesFbe.Execute(context, strategy);
                            if (result.Exception is not null)
                            {
                                _strategyFailures++;
                                continue;
                            }

                            pipes = result.Data!.Pipes;
                        }
                        break;

                    default:
                        {
                            var centerToConnectedCenters = AddPipesConnectedCenters.GetConnectedPumpjacks(context, strategy);
                            var result = AddPipesConnectedCenters.FindTrunksAndConnect(context, centerToConnectedCenters);
                            if (result.Exception is not null)
                            {
                                _strategyFailures++;
                                continue;
                            }

                            pipes = result.Data!;
                        }
                        break;
                }
            }
            catch (FactorioToolsException)
            {
                _strategyFailures++;
                continue;
            }

            RecordCall(stream, grids, $"{name}-{strategy}", strategy.ToString(), context, pipes);
        }

        context.CenterToTerminals = originalCenterToTerminals;
        context.LocationToTerminals = originalLocationToTerminals;
    }

    /// <summary>OptimizeAndAddSolutions's call, twice: on a compact rebuild of the pipes (recorded)
    /// and on the planner's own copy (compared).</summary>
    private static void RecordCall(Stream stream, Dictionary<string, int> grids, string name, string strategy, Context context, ILocationSet pipes)
    {
        var strategyCenterToTerminals = context.CenterToTerminals;
        var strategyLocationToTerminals = context.LocationToTerminals;

        // The compact rebuild the Rust test can reproduce.
        var single = pipes is SingleLocationSet;
        ILocationSet input;
        if (single)
        {
            input = context.GetLocationSet(pipes);
        }
        else
        {
            input = context.GetLocationSet(pipes.Count, allowEnumerate: true);
            foreach (var pipe in pipes.EnumerateItems())
            {
                input.Add(pipe);
            }
        }

        var inputPipes = input.EnumerateItems().ToList();
        var inputCenterToTerminals = Copy(context, strategyCenterToTerminals);
        var inputLocationToTerminals = Copy(context, strategyLocationToTerminals);

        var gridId = WriteGrid(stream, grids, context.Grid);

        context.CenterToTerminals = Copy(context, strategyCenterToTerminals);
        context.LocationToTerminals = Copy(context, strategyLocationToTerminals);
        var compact = Run(context, input);

        context.CenterToTerminals = Copy(context, strategyCenterToTerminals);
        context.LocationToTerminals = Copy(context, strategyLocationToTerminals);
        var real = Run(context, context.GetLocationSet(pipes));

        var realMatches = compact.Equals(real);
        if (!realMatches)
        {
            _realCopyDiffers++;
        }

        if (compact.Error is not null)
        {
            _errorCount++;
        }
        else if (!new HashSet<Location>(compact.Pipes).SetEquals(inputPipes))
        {
            _changedCount++;
        }

        _caseCount++;
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "case");
            w.WriteString("name", name);
            w.WriteString("strategy", strategy);
            w.WriteNumber("grid", gridId);
            w.WriteString("pipes_type", single ? "single" : "hash");
            WriteLocations(w, "pipes", inputPipes);
            WriteTerminals(w, "center_to_terminals", inputCenterToTerminals);
            WriteTerminals(w, "location_to_terminals", inputLocationToTerminals);
            w.WriteStartObject("out");
            if (compact.Error is not null)
            {
                w.WriteString("error", compact.Error);
            }
            else
            {
                w.WriteNull("error");
            }

            WriteLocations(w, "pipes", compact.Pipes);
            w.WriteString("center_to_terminals", compact.CenterToTerminals);
            w.WriteString("location_to_terminals", compact.LocationToTerminals);
            w.WriteEndObject();
            w.WriteBoolean("real_copy_matches", realMatches);
        });

        context.CenterToTerminals = strategyCenterToTerminals;
        context.LocationToTerminals = strategyLocationToTerminals;
    }

    private record Outcome(string? Error, List<Location> Pipes, string CenterToTerminals, string LocationToTerminals)
    {
        public virtual bool Equals(Outcome? other)
        {
            return other is not null
                && Error == other.Error
                && Pipes.SequenceEqual(other.Pipes)
                && CenterToTerminals == other.CenterToTerminals
                && LocationToTerminals == other.LocationToTerminals;
        }

        public override int GetHashCode() => 0;
    }

    private static Outcome Run(Context context, ILocationSet pipes)
    {
        string? error = null;
        try
        {
            RotateOptimize.Execute(context, pipes);
        }
        catch (Exception ex) when (ex is FactorioToolsException or InvalidOperationException or KeyNotFoundException)
        {
            error = $"{ex.GetType().Name}: {ex.Message}";
        }

        return new Outcome(
            error,
            pipes.EnumerateItems().ToList(),
            TerminalsText(context.CenterToTerminals),
            TerminalsText(context.LocationToTerminals));
    }

    /// <summary>ToDictionary(context, x => x.Key, x => x.Value.ToList()), as AddPipes copies the maps.</summary>
    private static ILocationDictionary<List<TerminalLocation>> Copy(Context context, ILocationDictionary<List<TerminalLocation>> source)
    {
        var copy = context.GetLocationDictionary<List<TerminalLocation>>();
        foreach (var pair in source.EnumeratePairs())
        {
            copy.Add(pair.Key, pair.Value.ToList());
        }

        return copy;
    }

    /// <summary>A terminal map in enumeration order, as text: "kx,ky:cx,cy,tx,ty,dir;...|".
    /// The Rust test builds the same text.</summary>
    private static string TerminalsText(ILocationDictionary<List<TerminalLocation>> dictionary)
    {
        var builder = new System.Text.StringBuilder();
        foreach (var pair in dictionary.EnumeratePairs())
        {
            builder.Append(pair.Key.X).Append(',').Append(pair.Key.Y).Append(':');
            foreach (var t in pair.Value)
            {
                builder.Append(t.Center.X).Append(',').Append(t.Center.Y).Append(',')
                    .Append(t.Terminal.X).Append(',').Append(t.Terminal.Y).Append(',')
                    .Append((int)t.Direction).Append(';');
            }

            builder.Append('|');
        }

        return builder.ToString();
    }

    /// <summary>The grid as RotateOptimize sees it: which cells hold an entity, and which of those
    /// are pipes. Written once per distinct grid.</summary>
    private static int WriteGrid(Stream stream, Dictionary<string, int> grids, SquareGrid grid)
    {
        var cells = new List<int>();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            cells.Add(location.X);
            cells.Add(location.Y);
            cells.Add(grid[location] is Pipe ? 1 : 0);
        }

        var key = $"{grid.Width}x{grid.Height}:{string.Join(",", cells)}";
        if (grids.TryGetValue(key, out var id))
        {
            return id;
        }

        id = grids.Count;
        grids.Add(key, id);
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "grid");
            w.WriteNumber("id", id);
            w.WriteNumber("width", grid.Width);
            w.WriteNumber("height", grid.Height);
            w.WriteStartArray("cells");
            foreach (var value in cells)
            {
                w.WriteNumberValue(value);
            }

            w.WriteEndArray();
        });

        return id;
    }

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

    private static void WriteLine(Stream stream, Action<Utf8JsonWriter> body)
    {
        using (var writer = new Utf8JsonWriter(stream, WriterOptions))
        {
            writer.WriteStartObject();
            body(writer);
            writer.WriteEndObject();
        }

        stream.WriteByte((byte)'\n');
    }
}
