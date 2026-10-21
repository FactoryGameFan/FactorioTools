// Records PlanUndergroundPipes.Execute and AddPipeEntities.Execute for the Rust port (#138),
// stage 5. Writes pipes.jsonl. The format is in rust/planner/FIXTURES.md.
//
// The planner calls both inside AddPipes with the pipe set of the plan it picked, which no
// public API hands out. So each case rebuilds that set from a finished plan: every Pipe,
// Terminal and UndergroundPipe cell of the final grid, plus the cells each underground pair
// skips over. The case then runs both steps on a fresh starting Context that carries the
// plan's selected terminals, the way AddPipes runs them, and records the inputs and the
// results. The recorder also checks that the rebuilt run lands the same pipes on the grid as
// the real plan did, so the inputs are known to be real ones.

using System.Reflection;
using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

static class PipesRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static readonly FieldInfo NextIdField = typeof(SquareGrid).GetField("_nextId", BindingFlags.NonPublic | BindingFlags.Instance)
        ?? throw new InvalidOperationException("SquareGrid._nextId not found.");

    /// <summary>How a case runs the two steps.</summary>
    private enum Mode
    {
        /// <summary>As AddPipes does: the plan's selected terminals, onto the context's grid.</summary>
        Selected,

        /// <summary>Every candidate terminal, onto a clone, with allowMultipleTerminals.</summary>
        MultipleOnClone,

        /// <summary>Every candidate terminal without allowMultipleTerminals, which throws.</summary>
        MultipleError,
    }

    private sealed record Case(string Name, OilFieldOptions Options, string Blueprint, bool PlanUnderground, Mode Mode);

    public static void Record(string outputDir, string repoRoot, IReadOnlyList<string> smallList)
    {
        var bigList = ParseBlueprint.ReadBlueprintFile(Path.Combine(repoRoot, "test", "FactorioTools.Test", "OilField", "big-list.txt"));
        var cases = BuildCases(smallList, bigList);

        var path = Path.Combine(outputDir, "pipes.jsonl");
        var written = 0;
        var skipped = 0;
        var rebuiltMismatches = 0;
        var errors = 0;
        using (var stream = File.Create(path))
        {
            foreach (var c in cases)
            {
                var plan = Planner.Execute(c.Options, ParseBlueprint.Execute(c.Blueprint));
                var context = InitializeContext.Execute(c.Options, ParseBlueprint.Execute(c.Blueprint), Array.Empty<AvoidLocation>());
                if (context.Grid.Width != plan.Context.Grid.Width || context.Grid.Height != plan.Context.Grid.Height)
                {
                    // The planner re-ran InitializeContext with other inputs (the poles-first
                    // retry), so the plan's cells do not line up with a fresh context.
                    skipped++;
                    continue;
                }

                if (c.Mode == Mode.Selected)
                {
                    context.CenterToTerminals = Copy(context, plan.Context.CenterToTerminals);
                    context.LocationToTerminals = Copy(context, plan.Context.LocationToTerminals);
                }

                var pipes = RebuildPipeSet(context, plan.Context.Grid);

                using (var writer = new Utf8JsonWriter(stream, WriterOptions))
                {
                    writer.WriteStartObject();
                    writer.WriteString("name", c.Name);
                    writer.WriteBoolean("validate", c.Options.ValidateSolution);
                    writer.WriteNumber("width", context.Grid.Width);
                    writer.WriteNumber("height", context.Grid.Height);
                    writer.WriteNumber("next_id", (int)NextIdField.GetValue(context.Grid)!);
                    WriteEntities(writer, "grid", context.Grid, context.Grid.EntityLocations.EnumerateItems());
                    WriteTerminalDictionary(writer, "center_to_terminals", context.CenterToTerminals);
                    WriteTerminalDictionary(writer, "location_to_terminals", context.LocationToTerminals);
                    WriteLocations(writer, "pipes", pipes.EnumerateItems());
                    writer.WriteBoolean("plan_underground", c.PlanUnderground);
                    writer.WriteString("mode", c.Mode switch
                    {
                        Mode.Selected => "selected",
                        Mode.MultipleOnClone => "multiple_on_clone",
                        _ => "multiple_error",
                    });

                    ILocationDictionary<Direction>? underground = null;
                    if (c.PlanUnderground)
                    {
                        underground = PlanUndergroundPipes.Execute(context, pipes);
                        writer.WriteStartArray("underground");
                        foreach (var pair in underground.EnumeratePairs())
                        {
                            writer.WriteStartArray();
                            writer.WriteNumberValue(pair.Key.X);
                            writer.WriteNumberValue(pair.Key.Y);
                            writer.WriteNumberValue((int)pair.Value);
                            writer.WriteEndArray();
                        }

                        writer.WriteEndArray();
                        WriteLocations(writer, "pipes_after", pipes.EnumerateItems());
                    }

                    if (c.Mode == Mode.Selected)
                    {
                        WriteNoOverlapChecks(writer, context, plan.Context.Grid, pipes, underground);
                    }

                    var grid = c.Mode == Mode.MultipleOnClone ? new PipeGrid(context.Grid) : context.Grid;
                    var before = grid.EntityLocations.EnumerateItems().ToList();
                    try
                    {
                        AddPipeEntities.Execute(context, grid, pipes, underground, allowMultipleTerminals: c.Mode == Mode.MultipleOnClone);

                        // The grid's entity set only grows here, so the starting entities keep
                        // their order and the new ones follow. Record just the new ones.
                        var after = grid.EntityLocations.EnumerateItems().ToList();
                        if (!after.Take(before.Count).SequenceEqual(before))
                        {
                            throw new InvalidOperationException($"{c.Name}: the starting entities changed order.");
                        }

                        WriteEntities(writer, "added", grid, after.Skip(before.Count));
                        writer.WriteNumber("next_id_after", (int)NextIdField.GetValue(grid)!);

                        if (c.Mode == Mode.Selected && c.PlanUnderground == c.Options.UseUndergroundPipes && !SamePipes(grid, plan.Context.Grid))
                        {
                            rebuiltMismatches++;
                            Console.Error.WriteLine($"pipes.jsonl: {c.Name}: the rebuilt run placed different pipes than the plan.");
                        }
                    }
                    catch (FactorioToolsException ex)
                    {
                        errors++;
                        writer.WriteString("error", ex.Message);
                    }

                    writer.WriteEndObject();
                }

                stream.WriteByte((byte)'\n');
                written++;
            }
        }

        Console.WriteLine($"pipes.jsonl: {written} cases ({errors} errors, {skipped} skipped, {rebuiltMismatches} rebuilt-plan mismatches), {new FileInfo(path).Length} bytes");
        if (rebuiltMismatches > 0)
        {
            throw new InvalidOperationException("Some rebuilt pipe sets do not reproduce their plan; see above.");
        }
    }

    /// <summary>
    /// Validate.NoOverlappingEntities on this case's pipes: with no beacons, with the plan's own
    /// beacons (both pass), and three broken inputs that must fail - a beacon on a pipe, the same
    /// beacon twice, and an extra pipe on a pumpjack center. Each result is null or the message.
    /// The check clones the grid, so the context is unchanged afterwards.
    /// </summary>
    private static void WriteNoOverlapChecks(Utf8JsonWriter writer, Context context, SquareGrid planGrid, ILocationSet pipes, ILocationDictionary<Direction>? underground)
    {
        var planBeacons = planGrid.EntityLocations.EnumerateItems()
            .Where(l => planGrid[l] is BeaconCenter)
            .ToList();
        // A pipe whose whole beacon footprint is inside the grid: AddProviderToGrid places cells
        // from the top-left with no bounds check, and an edge pipe would index off the grid.
        var firstPipe = pipes.EnumerateItems().First(l =>
            l.X - ((context.Options.BeaconWidth - 1) / 2) >= 0
            && l.Y - ((context.Options.BeaconHeight - 1) / 2) >= 0
            && l.X + (context.Options.BeaconWidth / 2) < context.Grid.Width
            && l.Y + (context.Options.BeaconHeight / 2) < context.Grid.Height);
        var pipesOnPumpjack = context.GetLocationSet(pipes);
        pipesOnPumpjack.Add(context.Centers[0]);

        var checks = new List<(string Label, ILocationSet Pipes, List<Location>? Beacons)>
        {
            ("no_beacons", pipes, null),
            ("plan_beacons", pipes, planBeacons),
            ("beacon_on_pipe", pipes, new List<Location> { firstPipe }),
            ("duplicate_beacon", pipes, planBeacons.Count > 0 ? new List<Location> { planBeacons[0], planBeacons[0] } : null),
            ("pipe_on_pumpjack", pipesOnPumpjack, null),
        };

        writer.WriteStartArray("no_overlap");
        foreach (var check in checks)
        {
            if (check.Label == "duplicate_beacon" && check.Beacons is null)
            {
                continue;
            }

            writer.WriteStartObject();
            writer.WriteString("label", check.Label);
            WriteLocations(writer, "pipes", check.Pipes.EnumerateItems());
            if (check.Beacons is not null)
            {
                WriteLocations(writer, "beacons", check.Beacons);
            }

            List<BeaconSolution>? solutions = check.Beacons is null
                ? null
                : new List<BeaconSolution> { new BeaconSolution(BeaconStrategy.Snug, check.Beacons, 0, new List<int>()) };
            try
            {
                Validate.NoOverlappingEntities(context, check.Pipes, underground, solutions);
                writer.WriteNull("error");
            }
            catch (FactorioToolsException ex)
            {
                writer.WriteString("error", ex.Message);
            }

            writer.WriteEndObject();
        }

        writer.WriteEndArray();
    }

    private static List<Case> BuildCases(IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        var cases = new List<Case>();

        // The PlannerOnAllBlueprintsAndOptions option sets that grids.jsonl uses: the medium
        // pole with undergrounds on every blueprint, the others on every third.
        var sets = new (string Name, Func<OilFieldOptions> Options, bool Beacons, bool Overlap, bool Underground, int Stride, int Offset)[]
        {
            ("medium-electric-pole_b_o_u", () => OilFieldOptions.ForMediumElectricPole, true, true, true, 1, 0),
            ("small-electric-pole_u", () => OilFieldOptions.ForSmallElectricPole, false, false, true, 3, 0),
            ("big-electric-pole_b", () => OilFieldOptions.ForBigElectricPole, true, false, false, 3, 1),
            ("substation_b_o", () => OilFieldOptions.ForSubstation, true, true, false, 3, 2),
        };

        foreach (var set in sets)
        {
            for (var i = set.Offset; i < smallList.Count; i += set.Stride)
            {
                var options = set.Options();
                options.AddBeacons = set.Beacons;
                options.OverlapBeacons = set.Overlap;
                options.UseUndergroundPipes = set.Underground;
                options.ValidateSolution = true;
                cases.Add(new Case($"small-{i:D4}_{set.Name}", options, smallList[i], set.Underground, Mode.Selected));

                // Plans made without undergrounds still give real pipe sets to plan them on.
                if (!set.Underground)
                {
                    cases.Add(new Case($"small-{i:D4}_{set.Name}_forced-u", options, smallList[i], true, Mode.Selected));
                }
            }
        }

        // Every candidate terminal at once, as RotateOptimize and the connected-centers
        // strategy pass them, onto a clone. Plus the error AddPipes guards against.
        for (var i = 0; i < smallList.Count; i += 2)
        {
            var options = OilFieldOptions.ForMediumElectricPole;
            options.ValidateSolution = true;
            cases.Add(new Case($"small-{i:D4}_medium_multiple-terminals", options, smallList[i], true, Mode.MultipleOnClone));
        }

        for (var i = 1; i < smallList.Count; i += 12)
        {
            var options = OilFieldOptions.ForMediumElectricPole;
            options.ValidateSolution = false;
            cases.Add(new Case($"small-{i:D4}_medium_multiple-terminals-error", options, smallList[i], false, Mode.MultipleError));
        }

        // A sample of the big list, without validation, as the site plans.
        for (var i = 0; i < bigList.Count; i += 40)
        {
            cases.Add(new Case($"big-{i:D4}_medium", OilFieldOptions.ForMediumElectricPole, bigList[i], true, Mode.Selected));
        }

        return cases;
    }

    /// <summary>
    /// The plan's pipe set before PlanUndergroundPipes: every pipe cell of the final grid, in
    /// entity order, with each underground pair's skipped cells added right after its
    /// left or upper end.
    /// </summary>
    private static ILocationSet RebuildPipeSet(Context context, SquareGrid grid)
    {
        var pipes = context.GetLocationSet(allowEnumerate: true);
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            if (grid[location] is not Pipe)
            {
                continue;
            }

            pipes.Add(location);
            if (grid[location] is UndergroundPipe { Direction: Direction.Left or Direction.Up } start)
            {
                var step = start.Direction == Direction.Left ? new Location(1, 0) : new Location(0, 1);
                var end = start.Direction == Direction.Left ? Direction.Right : Direction.Down;
                var current = location.Translate(step);
                while (grid[current] is not UndergroundPipe u || u.Direction != end)
                {
                    pipes.Add(current);
                    current = current.Translate(step);
                }
            }
        }

        return pipes;
    }

    /// <summary>Whether two grids hold the same pipe entities at the same cells.</summary>
    private static bool SamePipes(SquareGrid a, SquareGrid b)
    {
        static Dictionary<Location, string> Pipes(SquareGrid grid)
        {
            var result = new Dictionary<Location, string>();
            foreach (var location in grid.EntityLocations.EnumerateItems())
            {
                switch (grid[location])
                {
                    case UndergroundPipe u:
                        result.Add(location, $"UndergroundPipe {u.Direction}");
                        break;
                    case Pipe p:
                        result.Add(location, p.GetType().Name);
                        break;
                }
            }

            return result;
        }

        var pa = Pipes(a);
        var pb = Pipes(b);
        return pa.Count == pb.Count && pa.All(x => pb.TryGetValue(x.Key, out var v) && v == x.Value);
    }

    private static ILocationDictionary<List<TerminalLocation>> Copy(Context context, ILocationDictionary<List<TerminalLocation>> source)
    {
        var copy = context.GetLocationDictionary<List<TerminalLocation>>();
        foreach (var pair in source.EnumeratePairs())
        {
            copy.Add(pair.Key, pair.Value.ToList());
        }

        return copy;
    }

    private static void WriteLocations(Utf8JsonWriter writer, string name, IEnumerable<Location> locations)
    {
        writer.WriteStartArray(name);
        foreach (var location in locations)
        {
            writer.WriteNumberValue(location.X);
            writer.WriteNumberValue(location.Y);
        }

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

    /// <summary>Entities as `[x, y, kind, id, ...extra]`, the grids.jsonl encoding.</summary>
    private static void WriteEntities(Utf8JsonWriter writer, string name, SquareGrid grid, IEnumerable<Location> locations)
    {
        writer.WriteStartArray(name);
        foreach (var location in locations)
        {
            var entity = grid[location] ?? throw new InvalidOperationException($"No entity at {location}.");
            writer.WriteStartArray();
            writer.WriteNumberValue(location.X);
            writer.WriteNumberValue(location.Y);
            writer.WriteStringValue(entity.GetType().Name);
            writer.WriteNumberValue(entity.Id);
            switch (entity)
            {
                case UndergroundPipe underground:
                    writer.WriteNumberValue((int)underground.Direction);
                    break;
                case Terminal:
                case Pipe:
                case AvoidEntity:
                case TemporaryEntity:
                    break;
                case PumpjackCenter pumpjack:
                    writer.WriteNumberValue((int)pumpjack.Direction);
                    break;
                case PumpjackSide side:
                    writer.WriteNumberValue(side.Center.Id);
                    break;
                default:
                    throw new NotSupportedException($"pipes.jsonl has no encoding for {entity.GetType().Name}.");
            }

            writer.WriteEndArray();
        }

        writer.WriteEndArray();
    }
}
