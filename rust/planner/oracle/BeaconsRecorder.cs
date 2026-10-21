// Records PlanBeacons for the Rust port (#138), stage 9.
// Writes beacons.jsonl. The format is in rust/planner/FIXTURES.md.
//
// Each case is a real PlanBeacons.Execute call. AddPipes.GetSolution calls it with the pipe set
// PlanUndergroundPipes has already thinned, which is exactly the Pipe, Terminal and
// UndergroundPipe cells a finished plan's grid ends up with, on the planner's grid of
// pumpjacks (and avoid cells, and on the poles-first path, poles). So the recorder plans a
// blueprint, keeps those entities as the case's grid and the pipe cells as its pipe set, and
// calls PlanBeacons.Execute on a context rebuilt from that snapshot - the same rebuild the Rust
// test does, so both start from identical container state. On Aquilo the heat pipes passed in
// come from AddHeatPipes.Route on its own rebuild of the same snapshot, as GetSolution routes
// them before planning beacons.
//
// A built-in check confirms the inputs are the real ones: for every case without heat pipes,
// a selected plan's beacon strategy must give that plan's beacon and effect counts, and every
// beacon on the finished grid must be one of its beacons. Cases that fail it are named
// "_input-unverified" (see the comment in RecordPlan).

using System.Reflection;
using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

static class BeaconsRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static readonly FieldInfo NextIdField = typeof(SquareGrid).GetField("_nextId", BindingFlags.NonPublic | BindingFlags.Instance)
        ?? throw new InvalidOperationException("SquareGrid._nextId not found.");

    private sealed record Ent(int X, int Y, string Kind, int Id, int Extra, int[]? Neighbors);

    private sealed class Snapshot
    {
        public required OilFieldOptions Options;
        public required int PoleSupplyWidth, PoleSupplyHeight;
        public required int Width, Height, NextId;
        public required List<Ent> Entities;
        public required List<Location> Centers;
        public required int CenterToTerminalsCount;
    }

    private static int _caseCount;
    private static int _checked;
    private static int _checkMismatches;
    private static int _polesFirst;

    public static void Record(string outputDir, IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        var path = Path.Combine(outputDir, "beacons.jsonl");
        _caseCount = 0;
        _checked = 0;
        _checkMismatches = 0;
        _polesFirst = 0;

        var all = OilFieldOptions.AllBeaconStrategies.ToList();
        var sets = new (string Name, Func<OilFieldOptions> Options, int Stride, int Offset, bool KeepPoles)[]
        {
            ("medium_b_o_u", () => Opts(OilFieldOptions.ForMediumElectricPole, overlap: true, underground: true, heat: false, all), 1, 0, false),
            ("big_b", () => Opts(OilFieldOptions.ForBigElectricPole, overlap: false, underground: false, heat: false, all), 2, 1, false),
            ("substation_b_o", () => Opts(OilFieldOptions.ForSubstation, overlap: true, underground: false, heat: false, all), 3, 2, false),
            ("small_b_u", () => Opts(OilFieldOptions.ForSmallElectricPole, overlap: false, underground: true, heat: false, all), 3, 0, false),
            ("nonstandard_b_o_u", () => NonStandard(overlap: true), 2, 0, false),
            ("nonstandard_b_u", () => NonStandard(overlap: false), 3, 1, false),
            ("medium_heat_b_o", () => Opts(OilFieldOptions.ForMediumElectricPole, overlap: true, underground: true, heat: true, all), 3, 0, false),
            ("medium_heat_b", () => Opts(OilFieldOptions.ForMediumElectricPole, overlap: false, underground: true, heat: true, all), 4, 2, false),
            ("medium_b_o_u_poles", () => Opts(OilFieldOptions.ForMediumElectricPole, overlap: true, underground: true, heat: false, all), 4, 3, true),
            ("big_b_poles", () => Opts(OilFieldOptions.ForBigElectricPole, overlap: false, underground: false, heat: false, all), 6, 5, true),
        };

        using (var stream = File.Create(path))
        {
            foreach (var set in sets)
            {
                for (var i = set.Offset; i < smallList.Count; i += set.Stride)
                {
                    RecordPlan(stream, $"small-{i:D4}_{set.Name}", set.Options(), ParseBlueprint.Execute(smallList[i]), Array.Empty<AvoidLocation>(), set.KeepPoles, strategies: null);
                }
            }

            for (var i = 0; i < bigList.Count; i += 50)
            {
                RecordPlan(stream, $"big-{i:D4}_medium_b_o_u", Opts(OilFieldOptions.ForMediumElectricPole, overlap: true, underground: true, heat: false, all), ParseBlueprint.Execute(bigList[i]), Array.Empty<AvoidLocation>(), keepPoles: false, strategies: null);
            }

            // PlannerTest.AllowsLocationsToBeAvoided, for AvoidEntity cells.
            var avoidOptions = Opts(OilFieldOptions.ForMediumElectricPole, overlap: true, underground: true, heat: false, all);
            RecordPlan(stream, "PlannerTest.AllowsLocationsToBeAvoided", avoidOptions, AvoidBlueprint(), Enumerable.Range(-7, 16).Select(x => new AvoidLocation(x, 0)).ToArray(), keepPoles: false, strategies: null);

            // The strategy list: order, repeats (each runs once), the default list, and none.
            var lists = new (string Name, List<BeaconStrategy> Strategies)[]
            {
                ("order", new List<BeaconStrategy> { BeaconStrategy.Snug, BeaconStrategy.Fbe, BeaconStrategy.Snug, BeaconStrategy.FbeOriginal, BeaconStrategy.Fbe }),
                ("default", OilFieldOptions.DefaultBeaconStrategies.ToList()),
                ("none", new List<BeaconStrategy>()),
            };
            foreach (var (name, strategies) in lists)
            {
                foreach (var i in new[] { 4, 17, 33 })
                {
                    RecordPlan(stream, $"small-{i:D4}_medium_b_o_u_strategies-{name}", Opts(OilFieldOptions.ForMediumElectricPole, overlap: true, underground: true, heat: false, all), ParseBlueprint.Execute(smallList[i]), Array.Empty<AvoidLocation>(), keepPoles: false, strategies);
                }
            }
        }

        Console.WriteLine($"beacons.jsonl: {_caseCount} cases, {new FileInfo(path).Length} bytes; selected-plan check: {_checked} checked, {_checkMismatches} differ, {_polesFirst} poles-first");
    }

    private static OilFieldOptions Opts(OilFieldOptions options, bool overlap, bool underground, bool heat, List<BeaconStrategy> strategies)
    {
        options.AddBeacons = true;
        options.OverlapBeacons = overlap;
        options.UseUndergroundPipes = underground;
        options.AddHeatPipes = heat;
        options.ValidateSolution = true;
        options.BeaconStrategies = strategies.ToList();
        return options;
    }

    /// <summary>NonStandardBeacon/Theory.cs: a 2x4 beacon with a 4x6 supply area.</summary>
    private static OilFieldOptions NonStandard(bool overlap)
    {
        var options = Opts(OilFieldOptions.ForMediumElectricPole, overlap, underground: true, heat: false, OilFieldOptions.AllBeaconStrategies.ToList());
        options.BeaconWidth = 2;
        options.BeaconHeight = 4;
        options.BeaconSupplyWidth = 4;
        options.BeaconSupplyHeight = 6;
        return options;
    }

    private static Blueprint AvoidBlueprint() => new()
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
                Signal = new SignalID { Name = EntityNames.Vanilla.Pumpjack, Type = SignalTypes.Vanilla.Item },
            },
        },
        Item = ItemNames.Vanilla.Blueprint,
        Version = 0,
    };

    private static void RecordPlan(Stream stream, string name, OilFieldOptions options, Blueprint blueprint, AvoidLocation[] avoid, bool keepPoles, List<BeaconStrategy>? strategies)
    {
        var result = Planner.Execute(options, blueprint, avoid);
        var grid = result.Context.Grid;

        var pipes = new List<Location>();
        var beacons = new HashSet<Location>();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            switch (grid[location])
            {
                case Pipe:
                    pipes.Add(location);
                    break;
                case BeaconCenter:
                    beacons.Add(location);
                    break;
            }
        }

        // A plan whose poles went on first (the planner's poles-first retry) planned its beacons
        // with the poles already on the grid. Its beacons only come back with the poles kept, so
        // a mismatch is retried that way and the case recorded with its poles.
        var checkable = !options.AddHeatPipes && strategies is null
            && result.Summary.SelectedPlans.Count > 0 && result.Summary.SelectedPlans[0].BeaconStrategy is not null;
        var check = checkable && !keepPoles;
        var (snapshot, heatPipes, context, solutions, error) = Run(result, options, keepPoles, strategies);
        if (check && solutions is not null && !Matches(result, beacons, solutions))
        {
            var retry = Run(result, options, keepPoles: true, strategies);
            if (retry.Solutions is not null && Matches(result, beacons, retry.Solutions))
            {
                (snapshot, heatPipes, context, solutions, error) = retry;
                name += "_poles-first";
                keepPoles = true;
                _polesFirst++;
            }
        }

        if (snapshot is null)
        {
            // GetSolution does not plan beacons for a layout it cannot heat.
            return;
        }

        // The inputs are the planner's own only if they give back the plan's beacons. A plan that
        // went through the poles-first retry with beacons on placed its poles before planning
        // beacons and then ran pole placement again, which can move poles; no snapshot of the
        // finished plan has the poles its beacon pass saw. Such a case is still a valid input
        // for comparing .NET and Rust, so it is kept and named as unverified.
        if (check)
        {
            _checked++;
            if (solutions is null || !Matches(result, beacons, solutions))
            {
                _checkMismatches++;
                name += "_input-unverified";
                Console.Error.WriteLine($"beacons differ from the plan's: {name}");
            }
        }

        _caseCount++;
        WriteLine(stream, w =>
        {
            w.WriteString("name", name);
            WriteSnapshot(w, snapshot);
            WriteLocations(w, "pipes", pipes);
            if (heatPipes is null)
            {
                w.WriteNull("heat_pipes");
            }
            else
            {
                WriteLocations(w, "heat_pipes", heatPipes);
            }

            if (error is not null)
            {
                w.WriteString("error", error);
            }
            else
            {
                w.WriteStartArray("solutions");
                foreach (var s in solutions!)
                {
                    w.WriteStartObject();
                    w.WriteNumber("strategy", (int)s.Strategy);
                    WriteLocations(w, "beacons", s.Beacons);
                    w.WriteNumber("effects", s.Effects);
                    w.WriteStartArray("counts");
                    foreach (var c in s.EffectsGivenCounts)
                    {
                        w.WriteNumberValue(c);
                    }

                    w.WriteEndArray();
                    w.WriteEndObject();
                }

                w.WriteEndArray();
            }

            WriteLocations(w, "after_entity_locations", context.Grid.EntityLocations.EnumerateItems());
            w.WriteNumber("after_next_id", (int)NextIdField.GetValue(context.Grid)!);
        });
    }

    /// <summary>Whether the solutions reproduce the plan: some selected plan's beacon strategy
    /// gives exactly that plan's beacon and effect counts, and every beacon left on the finished
    /// grid is one of that solution's. (A later step can still drop a beacon from the grid, such
    /// as one no pole can power, so the grid holds a subset.)</summary>
    private static bool Matches(PlannerResult result, HashSet<Location> beacons, List<BeaconSolution> solutions)
    {
        foreach (var plan in result.Summary.SelectedPlans)
        {
            var solution = solutions.SingleOrDefault(s => s.Strategy == plan.BeaconStrategy);
            if (solution is not null
                && solution.Beacons.Count == plan.BeaconCount
                && solution.Effects == plan.BeaconEffectCount
                && beacons.IsSubsetOf(solution.Beacons))
            {
                return true;
            }
        }

        return false;
    }

    private static (Snapshot? Snapshot, List<Location>? HeatPipes, Context? Context, List<BeaconSolution>? Solutions, string? Error) Run(
        PlannerResult result, OilFieldOptions options, bool keepPoles, List<BeaconStrategy>? strategies)
    {
        var pipes = Pipes(result.Context.Grid);
        var snapshot = Capture(result.Context, keepPoles);
        if (strategies is not null)
        {
            snapshot.Options.BeaconStrategies = strategies.ToList();
        }

        List<Location>? heatPipes = null;
        if (options.AddHeatPipes)
        {
            var routeContext = Rebuild(snapshot);
            var routed = AddHeatPipes.Route(routeContext, ToSet(routeContext, pipes), out var uncoveredPipes, out var uncoveredCenters);
            if (uncoveredPipes.Count > 0 || uncoveredCenters.Count > 0)
            {
                return (null, null, null, null, null);
            }

            heatPipes = routed.EnumerateItems().ToList();
        }

        var context = Rebuild(snapshot);
        try
        {
            var solutions = PlanBeacons.Execute(context, ToSet(context, pipes), heatPipes is null ? null : ToSet(context, heatPipes));
            return (snapshot, heatPipes, context, solutions, null);
        }
        catch (FactorioToolsException ex)
        {
            return (snapshot, heatPipes, context, null, ex.Message);
        }
    }

    /// <summary>The Pipe, Terminal and UndergroundPipe cells, in EntityLocations order.</summary>
    private static List<Location> Pipes(SquareGrid grid)
    {
        var pipes = new List<Location>();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            if (grid[location] is Pipe)
            {
                pipes.Add(location);
            }
        }

        return pipes;
    }

    private static ILocationSet ToSet(Context context, IEnumerable<Location> locations)
    {
        var set = context.GetLocationSet(allowEnumerate: true);
        foreach (var location in locations)
        {
            set.Add(location);
        }

        return set;
    }

    // ---------------------------------------------------------------------------------------
    // Snapshots

    /// <summary>The finished grid without pipes, heat pipes, beacons and (unless kept) poles: the
    /// grid PlanBeacons sees, minus the pipes it adds itself.</summary>
    private static Snapshot Capture(Context context, bool keepPoles)
    {
        var grid = context.Grid;
        var entities = new List<Ent>();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            var entity = grid[location]!;
            var keep = entity switch
            {
                Pipe or HeatPipe or BeaconCenter or BeaconSide => false,
                ElectricPoleCenter or ElectricPoleSide => keepPoles,
                _ => true,
            };
            if (keep)
            {
                entities.Add(ToEnt(location, entity));
            }
        }

        var o = context.Options;
        var options = new OilFieldOptions
        {
            BeaconWidth = o.BeaconWidth,
            BeaconHeight = o.BeaconHeight,
            BeaconSupplyWidth = o.BeaconSupplyWidth,
            BeaconSupplyHeight = o.BeaconSupplyHeight,
            ElectricPoleWidth = o.ElectricPoleWidth,
            ElectricPoleHeight = o.ElectricPoleHeight,
            OverlapBeacons = o.OverlapBeacons,
            ValidateSolution = o.ValidateSolution,
            AddHeatPipes = o.AddHeatPipes,
            AddBeacons = o.AddBeacons,
            BeaconStrategies = o.BeaconStrategies.ToList(),
        };

        return new Snapshot
        {
            Options = options,
            PoleSupplyWidth = context.ElectricPoleSupplyWidthWithQuality,
            PoleSupplyHeight = context.ElectricPoleSupplyHeightWithQuality,
            Width = grid.Width,
            Height = grid.Height,
            NextId = (int)NextIdField.GetValue(grid)!,
            Entities = entities,
            Centers = context.Centers.ToList(),
            CenterToTerminalsCount = context.CenterToTerminals.Count,
        };
    }

    private static Ent ToEnt(Location location, GridEntity entity)
    {
        return entity switch
        {
            TemporaryEntity => new Ent(location.X, location.Y, "TemporaryEntity", entity.Id, 0, null),
            AvoidEntity => new Ent(location.X, location.Y, "AvoidEntity", entity.Id, 0, null),
            PumpjackCenter p => new Ent(location.X, location.Y, "PumpjackCenter", entity.Id, (int)p.Direction, null),
            PumpjackSide s => new Ent(location.X, location.Y, "PumpjackSide", entity.Id, s.Center.Id, null),
            ElectricPoleSide s => new Ent(location.X, location.Y, "ElectricPoleSide", entity.Id, s.Center.Id, null),
            ElectricPoleCenter p => new Ent(location.X, location.Y, "ElectricPoleCenter", entity.Id, 0, p.Neighbors.ToArray()),
            _ => throw new NotSupportedException(entity.GetType().Name),
        };
    }

    /// <summary>A fresh context holding the snapshot, built the way the Rust test builds it.
    /// Only what PlanBeacons and AddHeatPipes.Route read is filled in.</summary>
    private static Context Rebuild(Snapshot s)
    {
        var grid = new PipeGrid(s.Width, s.Height);
        var byId = new Dictionary<int, GridEntity>();
        foreach (var e in s.Entities)
        {
            GridEntity? entity = e.Kind switch
            {
                "PumpjackCenter" => new PumpjackCenter(e.Id) { Direction = (Direction)e.Extra },
                "ElectricPoleCenter" => new ElectricPoleCenter(e.Id),
                _ => null,
            };
            if (entity is not null)
            {
                byId.Add(e.Id, entity);
            }
        }

        foreach (var e in s.Entities)
        {
            GridEntity entity = e.Kind switch
            {
                "PumpjackCenter" or "ElectricPoleCenter" => byId[e.Id],
                "PumpjackSide" => new PumpjackSide(e.Id, (PumpjackCenter)byId[e.Extra]),
                "ElectricPoleSide" => new ElectricPoleSide(e.Id, (ElectricPoleCenter)byId[e.Extra]),
                "TemporaryEntity" => new TemporaryEntity(e.Id),
                "AvoidEntity" => new AvoidEntity(e.Id),
                _ => throw new NotSupportedException(e.Kind),
            };
            grid.AddEntity(new Location(e.X, e.Y), entity);
        }

        foreach (var e in s.Entities)
        {
            if (e.Neighbors is not null)
            {
                var pole = (ElectricPoleCenter)byId[e.Id];
                foreach (var id in e.Neighbors)
                {
                    pole.Neighbors.Add(id);
                }
            }
        }

        NextIdField.SetValue(grid, s.NextId);

        var o = s.Options;
        var options = new OilFieldOptions
        {
            BeaconWidth = o.BeaconWidth,
            BeaconHeight = o.BeaconHeight,
            BeaconSupplyWidth = o.BeaconSupplyWidth,
            BeaconSupplyHeight = o.BeaconSupplyHeight,
            ElectricPoleWidth = o.ElectricPoleWidth,
            ElectricPoleHeight = o.ElectricPoleHeight,
            OverlapBeacons = o.OverlapBeacons,
            ValidateSolution = o.ValidateSolution,
            AddHeatPipes = o.AddHeatPipes,
            AddBeacons = o.AddBeacons,
            BeaconStrategies = o.BeaconStrategies.ToList(),
        };

        // Snug sizes a list by CenterToTerminals.Count; nothing else reads it here.
        var centerToTerminals = new LocationHashDictionary<List<TerminalLocation>>();
        foreach (var center in s.Centers.Take(s.CenterToTerminalsCount))
        {
            centerToTerminals.Add(center, new List<TerminalLocation>());
        }

        return new Context
        {
            Options = options,
            InputBlueprint = new Blueprint { Entities = Array.Empty<Entity>(), Icons = Array.Empty<Icon>(), Item = ItemNames.Vanilla.Blueprint },
            DeltaX = 0,
            DeltaY = 0,
            Grid = grid,
            Centers = s.Centers.ToList(),
            CenterToTerminals = centerToTerminals,
            CenterToOriginalDirection = new LocationHashDictionary<Direction>(),
            LocationToTerminals = new LocationHashDictionary<List<TerminalLocation>>(),
            LocationToAdjacentCount = new int[s.Width * s.Height],
            SharedInstances = new SharedInstances(grid),
            ElectricPoleSupplyWidthWithQuality = s.PoleSupplyWidth,
            ElectricPoleSupplyHeightWithQuality = s.PoleSupplyHeight,
        };
    }

    private static void WriteSnapshot(Utf8JsonWriter w, Snapshot s)
    {
        var o = s.Options;
        w.WriteStartObject("options");
        w.WriteNumber("beacon_width", o.BeaconWidth);
        w.WriteNumber("beacon_height", o.BeaconHeight);
        w.WriteNumber("beacon_supply_width", o.BeaconSupplyWidth);
        w.WriteNumber("beacon_supply_height", o.BeaconSupplyHeight);
        w.WriteNumber("electric_pole_width", o.ElectricPoleWidth);
        w.WriteNumber("electric_pole_height", o.ElectricPoleHeight);
        w.WriteBoolean("overlap_beacons", o.OverlapBeacons);
        w.WriteBoolean("validate_solution", o.ValidateSolution);
        w.WriteBoolean("add_heat_pipes", o.AddHeatPipes);
        w.WriteBoolean("add_beacons", o.AddBeacons);
        w.WriteStartArray("beacon_strategies");
        foreach (var strategy in o.BeaconStrategies)
        {
            w.WriteNumberValue((int)strategy);
        }

        w.WriteEndArray();
        w.WriteEndObject();
        w.WriteStartArray("pole_supply");
        w.WriteNumberValue(s.PoleSupplyWidth);
        w.WriteNumberValue(s.PoleSupplyHeight);
        w.WriteEndArray();
        w.WriteNumber("width", s.Width);
        w.WriteNumber("height", s.Height);
        w.WriteNumber("next_id", s.NextId);
        w.WriteStartArray("entities");
        foreach (var e in s.Entities)
        {
            w.WriteStartArray();
            w.WriteNumberValue(e.X);
            w.WriteNumberValue(e.Y);
            w.WriteStringValue(e.Kind);
            w.WriteNumberValue(e.Id);
            switch (e.Kind)
            {
                case "PumpjackCenter":
                case "PumpjackSide":
                case "ElectricPoleSide":
                    w.WriteNumberValue(e.Extra);
                    break;
                case "ElectricPoleCenter":
                    w.WriteStartArray();
                    foreach (var n in e.Neighbors!)
                    {
                        w.WriteNumberValue(n);
                    }

                    w.WriteEndArray();
                    break;
            }

            w.WriteEndArray();
        }

        w.WriteEndArray();
        WriteLocations(w, "centers", s.Centers);
        w.WriteNumber("center_to_terminals_count", s.CenterToTerminalsCount);
    }

    // ---------------------------------------------------------------------------------------
    // Writing helpers

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

    /// <summary>A flat <c>[x0, y0, x1, y1, ...]</c> array.</summary>
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
}
