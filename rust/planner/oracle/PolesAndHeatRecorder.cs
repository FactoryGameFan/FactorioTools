// Records AddElectricPoles and AddHeatPipes for the Rust port (#138), stage 10.
// Writes poles_and_heat.jsonl. The format is in rust/planner/FIXTURES.md.
//
// Like HelpersRecorder, every case runs on a context the recorder rebuilds from a recorded
// snapshot, so .NET and the Rust test start from the same container state: entities added in
// the recorded order, into fresh sets and dictionaries, with the recorded next entity id.
// The snapshots are the states the planner hands these steps, taken from finished plans:
//
// - before poles: the finished grid without its poles (AddElectricPoles runs last, and with
//   AddLast it only adds poles), checked by replanning the poles and comparing with the plan;
// - poles first: only the pumpjacks, with every terminal avoided, as Planner.cs does on its
//   retry path;
// - before heat: the grid without poles, beacons and heat pipes (AddHeatPipes.Execute), or
//   with only the pumpjacks and the plan's pipe tiles passed in (AddHeatPipes.Route);
// - a routed heat network plus the plan's beacons (AddHeatPipes.ExtendToBeacons).

using System.Reflection;
using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;
using static Knapcode.FactorioTools.OilField.Helpers;

static class PolesAndHeatRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static readonly FieldInfo NextIdField = typeof(SquareGrid).GetField("_nextId", BindingFlags.NonPublic | BindingFlags.Instance)
        ?? throw new InvalidOperationException("SquareGrid._nextId not found.");

    /// <summary>An entity: location, kind code (see KindCode), id, and per-kind state.</summary>
    private sealed record Ent(int X, int Y, int Kind, int Id, int Extra, int[]? Neighbors);

    private sealed class Snapshot
    {
        public required string Name;
        public required int BeaconWidth, BeaconHeight, PoleWidth, PoleHeight;
        public required int PoleSupplyWidth, PoleSupplyHeight;
        public required double WireReach, WireReachSquared;
        public required bool ValidateSolution, AddHeatPipes;
        public required int Width, Height, NextId;
        public required List<Ent> Entities;
        public required List<Location> Centers;
        public required List<Location>? HeatPipes;
    }

    private sealed class Fnv
    {
        private ulong _hash = 1469598103934665603UL;

        public Fnv Mix(int value)
        {
            var v = unchecked((uint)value);
            for (var b = 0; b < 4; b++)
            {
                _hash ^= (v >> (8 * b)) & 0xFF;
                _hash *= 1099511628211UL;
            }

            return this;
        }

        public Fnv Mix(Location location) => Mix(location.X).Mix(location.Y);

        public string Hex => _hash.ToString("x16");
    }

    private static int _contextCount;
    private static int _caseCount;
    private static int _planPolesMatched;
    private static int _planPolesCompared;

    public static void Record(string outputDir, IReadOnlyList<string> smallList)
    {
        var path = Path.Combine(outputDir, "poles_and_heat.jsonl");
        _contextCount = 0;
        _caseCount = 0;
        _planPolesMatched = 0;
        _planPolesCompared = 0;

        using (var stream = File.Create(path))
        {
            RecordPoles(stream, smallList);
            RecordHeat(stream, smallList);
        }

        Console.WriteLine($"poles_and_heat.jsonl: {_contextCount} contexts, {_caseCount} cases, {new FileInfo(path).Length} bytes, replanned poles match the plan in {_planPolesMatched} of {_planPolesCompared}");
    }

    private static OilFieldOptions Opts(OilFieldOptions options, bool beacons, bool overlap, bool heat, Quality poleQuality = Quality.Normal)
    {
        options.AddBeacons = beacons;
        options.OverlapBeacons = overlap;
        options.AddHeatPipes = heat;
        options.ElectricPoleQuality = poleQuality;
        options.ValidateSolution = true;
        return options;
    }

    // ---------------------------------------------------------------------------------------
    // AddElectricPoles

    private static void RecordPoles(Stream stream, IReadOnlyList<string> smallList)
    {
        var sets = new (string Name, Func<OilFieldOptions> Options, int Stride, int Offset)[]
        {
            ("medium_b_o_u", () => Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: true, heat: false), 2, 0),
            ("small_u", () => Opts(OilFieldOptions.ForSmallElectricPole, beacons: false, overlap: false, heat: false), 4, 1),
            ("big_b", () => Opts(OilFieldOptions.ForBigElectricPole, beacons: true, overlap: false, heat: false), 4, 2),
            ("substation_b_o", () => Opts(OilFieldOptions.ForSubstation, beacons: true, overlap: true, heat: false), 4, 3),
            ("medium_rare_b", () => Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: false, heat: false, Quality.Rare), 6, 1),
            ("small_legendary", () => Opts(OilFieldOptions.ForSmallElectricPole, beacons: false, overlap: false, heat: false, Quality.Legendary), 6, 4),
            ("medium_heat_b", () => Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: true, heat: true), 6, 5),
        };

        foreach (var set in sets)
        {
            for (var i = set.Offset; i < smallList.Count; i += set.Stride)
            {
                var name = $"small-{i:D4}_{set.Name}";
                var options = set.Options();
                var blueprint = ParseBlueprint.Execute(smallList[i]);
                var result = Planner.Execute(options, blueprint);
                var context = result.Context;
                var planPoles = CellsOf(context.Grid, e => e is ElectricPoleCenter);

                // Before poles: the plan without them, as AddLast hands it to AddElectricPoles.
                var noPoles = Capture(name + "_no-poles", context, e => e is ElectricPoleCenter or ElectricPoleSide);
                var noPolesId = WriteSnapshot(stream, noPoles);
                var replanned = RecordPoleCase(stream, noPolesId, noPoles, avoid: new List<Location>(), allowRetries: false, planPoles);
                _planPolesCompared++;
                if (replanned is not null && replanned.SequenceEqual(planPoles))
                {
                    _planPolesMatched++;
                }

                // Poles first, avoiding every terminal: only the pumpjacks (and avoided cells).
                if (i % 3 == 0)
                {
                    var initial = InitializeContext.Execute(options, blueprint, Array.Empty<AvoidLocation>());
                    var terminals = new List<Location>();
                    foreach (var list in initial.CenterToTerminals.Values)
                    {
                        foreach (var terminal in list)
                        {
                            terminals.Add(terminal.Terminal);
                        }
                    }

                    var bare = Capture(name + "_pumpjacks", context, e => e is not (PumpjackCenter or PumpjackSide or AvoidEntity));
                    var bareId = WriteSnapshot(stream, bare);
                    RecordPoleCase(stream, bareId, bare, terminals, allowRetries: false, planPoles: null);
                    RecordPoleCase(stream, bareId, bare, terminals, allowRetries: true, planPoles: null);
                }

                // Crowded grids, so the cover fails and the retry strategies run.
                if (i % 4 == 1)
                {
                    foreach (var fraction in new[] { 0.5, 0.8 })
                    {
                        var random = new Random(1000 + i * 10 + (int)(fraction * 10));
                        var empty = new List<Location>();
                        for (var y = 0; y < noPoles.Height; y++)
                        {
                            for (var x = 0; x < noPoles.Width; x++)
                            {
                                var location = new Location(x, y);
                                if (context.Grid.IsEmpty(location) || context.Grid[location] is ElectricPoleCenter or ElectricPoleSide)
                                {
                                    empty.Add(location);
                                }
                            }
                        }

                        var avoid = empty.Where(_ => random.NextDouble() < fraction).ToList();
                        RecordPoleCase(stream, noPolesId, noPoles, avoid, allowRetries: true, planPoles: null);
                        RecordPoleCase(stream, noPolesId, noPoles, avoid, allowRetries: false, planPoles: null);
                    }
                }
            }
        }

        // Blocked entities: avoid every free cell around one pumpjack, one beacon, or both, so
        // no pole can power it. Without retries that fails; with them it walks the retry
        // strategies down to removing the unpowered entities.
        for (var i = 0; i < smallList.Count; i += 5)
        {
            var options = Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: true, heat: false);
            var result = Planner.Execute(options, ParseBlueprint.Execute(smallList[i]));
            var context = result.Context;
            var noPoles = Capture($"small-{i:D4}_medium_b_o_blocked", context, e => e is ElectricPoleCenter or ElectricPoleSide);
            var id = WriteSnapshot(stream, noPoles);
            var pumpjack = context.Centers[i % context.Centers.Count];
            var beaconCells = CellsOf(context.Grid, e => e is BeaconCenter);
            var targets = new List<(string Name, List<Location> Centers)> { ("pumpjack", new List<Location> { pumpjack }) };
            if (beaconCells.Count > 0)
            {
                var beacon = beaconCells[i % beaconCells.Count];
                targets.Add(("beacon", new List<Location> { beacon }));
                targets.Add(("both", new List<Location> { pumpjack, beacon }));
            }

            foreach (var (_, centers) in targets)
            {
                var radius = (noPoles.PoleSupplyWidth / 2) + 3;
                var avoid = new List<Location>();
                for (var y = 0; y < noPoles.Height; y++)
                {
                    for (var x = 0; x < noPoles.Width; x++)
                    {
                        var location = new Location(x, y);
                        var free = context.Grid.IsEmpty(location) || context.Grid[location] is ElectricPoleCenter or ElectricPoleSide;
                        if (free && centers.Any(c => Math.Abs(c.X - x) <= radius && Math.Abs(c.Y - y) <= radius))
                        {
                            avoid.Add(location);
                        }
                    }
                }

                RecordPoleCase(stream, id, noPoles, avoid, allowRetries: false, planPoles: null);
                RecordPoleCase(stream, id, noPoles, avoid, allowRetries: true, planPoles: null);
            }
        }

        // The sample, with and without beacons.
        foreach (var beacons in new[] { false, true })
        {
            var options = Opts(OilFieldOptions.ForMediumElectricPole, beacons, overlap: true, heat: false);
            var result = Planner.Execute(options, Planner.ExecuteSample().Context.InputBlueprint);
            var planPoles = CellsOf(result.Context.Grid, e => e is ElectricPoleCenter);
            var noPoles = Capture($"sample_{(beacons ? "b" : "plain")}_no-poles", result.Context, e => e is ElectricPoleCenter or ElectricPoleSide);
            var id = WriteSnapshot(stream, noPoles);
            RecordPoleCase(stream, id, noPoles, new List<Location>(), allowRetries: false, planPoles);
        }
    }

    /// <summary>Runs AddElectricPoles on a rebuilt context. Returns the resulting pole centers in
    /// grid order, or null when it returned null or threw.</summary>
    private static List<Location>? RecordPoleCase(Stream stream, int ctx, Snapshot s, List<Location> avoid, bool allowRetries, List<Location>? planPoles)
    {
        var context = Rebuild(s);
        ILocationSet avoidSet;
        if (avoid.Count == 0)
        {
            avoidSet = EmptyLocationSet.Instance;
        }
        else
        {
            avoidSet = context.GetLocationSet(allowEnumerate: true);
            foreach (var location in avoid)
            {
                avoidSet.Add(location);
            }
        }

        string? error = null;
        string? exception = null;
        ILocationSet? returned = null;
        try
        {
            returned = AddElectricPoles.Execute(context, avoidSet, allowRetries);
        }
        catch (FactorioToolsException e)
        {
            error = e.Message;
        }
        catch (Exception e) when (e is not OutOfMemoryException)
        {
            exception = e.GetType().Name;
        }

        var poles = CellsOf(context.Grid, e => e is ElectricPoleCenter);
        Case(stream, "poles", ctx, w =>
        {
            WriteLocations(w, "avoid", avoid);
            w.WriteBoolean("allow_retries", allowRetries);
            if (planPoles is not null)
            {
                WriteLocations(w, "plan_poles", planPoles);
            }
        }, w =>
        {
            WriteError(w, error, exception);
            if (returned is null)
            {
                w.WriteNull("returned");
            }
            else
            {
                w.WriteStartObject("returned");
                w.WriteNumber("count", returned.Count);
                WriteLocations(w, "members", poles.Where(returned.Contains));
                w.WriteEndObject();
            }

            WritePoles(w, context.Grid);
            w.WriteString("grid", HashGrid(context.Grid));
        });

        return error is null && exception is null && returned is not null ? poles : null;
    }

    // ---------------------------------------------------------------------------------------
    // AddHeatPipes

    private static void RecordHeat(Stream stream, IReadOnlyList<string> smallList)
    {
        var runs = new List<(string Name, Func<PlannerResult> Run)>();
        for (var i = 0; i < smallList.Count; i += 2)
        {
            var blueprintString = smallList[i];
            runs.Add(($"small-{i:D4}_medium_heat", () => Planner.Execute(Opts(OilFieldOptions.ForMediumElectricPole, beacons: false, overlap: false, heat: true), ParseBlueprint.Execute(blueprintString))));
        }

        for (var i = 1; i < smallList.Count; i += 3)
        {
            var blueprintString = smallList[i];
            runs.Add(($"small-{i:D4}_medium_heat_b_o", () => Planner.Execute(Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: true, heat: true), ParseBlueprint.Execute(blueprintString))));
        }

        for (var i = 2; i < smallList.Count; i += 6)
        {
            var blueprintString = smallList[i];
            runs.Add(($"small-{i:D4}_big_heat_b", () => Planner.Execute(Opts(OilFieldOptions.ForBigElectricPole, beacons: true, overlap: false, heat: true), ParseBlueprint.Execute(blueprintString))));
        }

        // PlannerTest's two heat snapshots.
        runs.Add(("PlannerTest.AddsHeatPipesForAquilo", () => Planner.Execute(Opts(OilFieldOptions.ForMediumElectricPole, beacons: false, overlap: true, heat: true), ParseBlueprint.Execute(smallList[0]))));
        runs.Add(("PlannerTest.AddsHeatPipesAndBeaconsTogetherForAquilo", () => Planner.Execute(Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: true, heat: true), ParseBlueprint.Execute(smallList[0]))));

        foreach (var (name, run) in runs)
        {
            var result = run();
            var context = result.Context;

            // Execute: the pipes are on the grid, no heat yet.
            var beforeHeat = Capture(name + "_pipes", context, e => e is ElectricPoleCenter or ElectricPoleSide or BeaconCenter or BeaconSide or HeatPipe, keepHeatPipes: false);
            var beforeHeatId = WriteSnapshot(stream, beforeHeat);
            {
                var rebuilt = Rebuild(beforeHeat);
                string? error = null, exception = null;
                try
                {
                    AddHeatPipes.Execute(rebuilt);
                }
                catch (FactorioToolsException e)
                {
                    error = e.Message;
                }
                catch (Exception e) when (e is not OutOfMemoryException)
                {
                    exception = e.GetType().Name;
                }

                Case(stream, "heat_execute", beforeHeatId, _ => { }, w =>
                {
                    WriteError(w, error, exception);
                    if (rebuilt.HeatPipes is null)
                    {
                        w.WriteNull("heat_pipes");
                    }
                    else
                    {
                        WriteLocations(w, "heat_pipes", rebuilt.HeatPipes.EnumerateItems());
                    }

                    w.WriteString("grid", HashGrid(rebuilt.Grid));
                });
            }

            // Route: only the pumpjacks on the grid, the plan's pipe tiles passed in.
            var pipeTiles = CellsOf(context.Grid, e => e is Pipe);
            var pumpjacks = Capture(name + "_pumpjacks", context, e => e is not (PumpjackCenter or PumpjackSide or AvoidEntity), keepHeatPipes: false);
            var pumpjacksId = WriteSnapshot(stream, pumpjacks);
            {
                var rebuilt = Rebuild(pumpjacks);
                var tiles = rebuilt.GetLocationSet(allowEnumerate: true);
                foreach (var location in pipeTiles)
                {
                    tiles.Add(location);
                }

                string? error = null, exception = null;
                ILocationSet? chosen = null, uncoveredPipes = null, uncoveredCenters = null;
                try
                {
                    chosen = AddHeatPipes.Route(rebuilt, tiles, out uncoveredPipes, out uncoveredCenters);
                }
                catch (FactorioToolsException e)
                {
                    error = e.Message;
                }
                catch (Exception e) when (e is not OutOfMemoryException)
                {
                    exception = e.GetType().Name;
                }

                Case(stream, "heat_route", pumpjacksId, w => WriteLocations(w, "pipe_tiles", pipeTiles), w =>
                {
                    WriteError(w, error, exception);
                    WriteLocations(w, "chosen", chosen?.EnumerateItems() ?? Array.Empty<Location>());
                    WriteLocations(w, "uncovered_pipes", uncoveredPipes?.EnumerateItems() ?? Array.Empty<Location>());
                    WriteLocations(w, "uncovered_centers", uncoveredCenters?.EnumerateItems() ?? Array.Empty<Location>());
                    w.WriteString("grid", HashGrid(rebuilt.Grid));
                });
            }

            // ExtendToBeacons: route and place the heat network, add the plan's beacons where
            // they still fit, and extend the network to them.
            var beacons = CellsOf(context.Grid, e => e is BeaconCenter);
            if (beacons.Count > 0)
            {
                var staged = Rebuild(beforeHeat);
                staged.Options.AddHeatPipes = true;
                AddHeatPipes.Execute(staged);
                var placed = new List<Location>();
                foreach (var center in beacons)
                {
                    if (DoesProviderFit(staged.Grid, staged.Options.BeaconWidth, staged.Options.BeaconHeight, center))
                    {
                        AddBeaconsToGrid(staged.Grid, staged.Options, new[] { center });
                        placed.Add(center);
                    }
                }

                var withBeacons = Capture(name + "_beacons", staged, _ => false, keepHeatPipes: true);
                var withBeaconsId = WriteSnapshot(stream, withBeacons);
                var rebuilt = Rebuild(withBeacons);

                string? error = null, exception = null;
                ILocationSet? heated = null;
                try
                {
                    heated = AddHeatPipes.ExtendToBeacons(rebuilt, placed);
                }
                catch (FactorioToolsException e)
                {
                    error = e.Message;
                }
                catch (Exception e) when (e is not OutOfMemoryException)
                {
                    exception = e.GetType().Name;
                }

                Case(stream, "heat_extend", withBeaconsId, w => WriteLocations(w, "beacons", placed), w =>
                {
                    WriteError(w, error, exception);
                    WriteLocations(w, "heated", heated?.EnumerateItems() ?? Array.Empty<Location>());
                    WriteLocations(w, "heat_pipes", rebuilt.HeatPipes?.EnumerateItems() ?? Array.Empty<Location>());
                    w.WriteString("grid", HashGrid(rebuilt.Grid));
                });
            }
        }
    }

    // ---------------------------------------------------------------------------------------
    // Snapshots

    private static Snapshot Capture(string name, Context context, Func<GridEntity, bool> exclude, bool keepHeatPipes = false)
    {
        var grid = context.Grid;
        var entities = new List<Ent>();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            var entity = grid[location]!;
            if (exclude(entity))
            {
                continue;
            }

            entities.Add(ToEnt(location, entity));
        }

        // Excluded poles are always excluded together with every other pole, so no recorded
        // pole is left pointing at one that is gone.
        return new Snapshot
        {
            Name = name,
            BeaconWidth = context.Options.BeaconWidth,
            BeaconHeight = context.Options.BeaconHeight,
            PoleWidth = context.Options.ElectricPoleWidth,
            PoleHeight = context.Options.ElectricPoleHeight,
            PoleSupplyWidth = context.ElectricPoleSupplyWidthWithQuality,
            PoleSupplyHeight = context.ElectricPoleSupplyHeightWithQuality,
            WireReach = context.ElectricPoleWireReachWithQuality,
            WireReachSquared = context.ElectricPoleWireReachSquaredWithQuality,
            ValidateSolution = context.Options.ValidateSolution,
            AddHeatPipes = context.Options.AddHeatPipes,
            Width = grid.Width,
            Height = grid.Height,
            NextId = (int)NextIdField.GetValue(grid)!,
            Entities = entities,
            Centers = context.Centers.ToList(),
            HeatPipes = keepHeatPipes ? context.HeatPipes?.EnumerateItems().ToList() : null,
        };
    }

    private static Ent ToEnt(Location location, GridEntity entity)
    {
        var code = KindCode(entity);
        return entity switch
        {
            UndergroundPipe u => new Ent(location.X, location.Y, code, entity.Id, (int)u.Direction, null),
            PumpjackCenter p => new Ent(location.X, location.Y, code, entity.Id, (int)p.Direction, null),
            PumpjackSide s => new Ent(location.X, location.Y, code, entity.Id, s.Center.Id, null),
            BeaconSide s => new Ent(location.X, location.Y, code, entity.Id, s.Center.Id, null),
            ElectricPoleSide s => new Ent(location.X, location.Y, code, entity.Id, s.Center.Id, null),
            ElectricPoleCenter p => new Ent(location.X, location.Y, code, entity.Id, 0, p.Neighbors.ToArray()),
            _ => new Ent(location.X, location.Y, code, entity.Id, 0, null),
        };
    }

    /// <summary>A fresh context holding the snapshot, built the way the Rust test builds it.</summary>
    private static Context Rebuild(Snapshot s)
    {
        var options = new OilFieldOptions
        {
            BeaconWidth = s.BeaconWidth,
            BeaconHeight = s.BeaconHeight,
            ElectricPoleWidth = s.PoleWidth,
            ElectricPoleHeight = s.PoleHeight,
            ValidateSolution = s.ValidateSolution,
            AddHeatPipes = s.AddHeatPipes,
        };

        var grid = new PipeGrid(s.Width, s.Height);
        var byId = new Dictionary<int, GridEntity>();
        foreach (var e in s.Entities)
        {
            GridEntity? entity = e.Kind switch
            {
                0 => new PumpjackCenter(e.Id) { Direction = (Direction)e.Extra },
                2 => new BeaconCenter(e.Id),
                4 => new ElectricPoleCenter(e.Id),
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
                0 or 2 or 4 => byId[e.Id],
                1 => new PumpjackSide(e.Id, (PumpjackCenter)byId[e.Extra]),
                3 => new BeaconSide(e.Id, (BeaconCenter)byId[e.Extra]),
                5 => new ElectricPoleSide(e.Id, (ElectricPoleCenter)byId[e.Extra]),
                6 => new Pipe(e.Id),
                7 => new Terminal(e.Id),
                8 => new UndergroundPipe(e.Id, (Direction)e.Extra),
                9 => new HeatPipe(e.Id),
                10 => new TemporaryEntity(e.Id),
                11 => new AvoidEntity(e.Id),
                _ => throw new NotSupportedException(e.Kind.ToString()),
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

        var context = new Context
        {
            Options = options,
            InputBlueprint = new Blueprint { Entities = Array.Empty<Entity>(), Icons = Array.Empty<Icon>(), Item = ItemNames.Vanilla.Blueprint },
            DeltaX = 0,
            DeltaY = 0,
            Grid = grid,
            Centers = s.Centers.ToList(),
            CenterToTerminals = new LocationHashDictionary<List<TerminalLocation>>(),
            CenterToOriginalDirection = new LocationHashDictionary<Direction>(),
            LocationToTerminals = new LocationHashDictionary<List<TerminalLocation>>(),
            LocationToAdjacentCount = new int[s.Width * s.Height],
            SharedInstances = new SharedInstances(grid),
            ElectricPoleSupplyWidthWithQuality = s.PoleSupplyWidth,
            ElectricPoleSupplyHeightWithQuality = s.PoleSupplyHeight,
            ElectricPoleWireReachWithQuality = s.WireReach,
            ElectricPoleWireReachSquaredWithQuality = s.WireReachSquared,
        };

        if (s.HeatPipes is not null)
        {
            var heatPipes = context.GetLocationSet(allowEnumerate: true);
            foreach (var location in s.HeatPipes)
            {
                heatPipes.Add(location);
            }

            context.HeatPipes = heatPipes;
        }

        return context;
    }

    private static int WriteSnapshot(Stream stream, Snapshot s)
    {
        var id = _contextCount++;
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "context");
            w.WriteNumber("id", id);
            w.WriteString("name", s.Name);
            w.WriteStartArray("sizes");
            w.WriteNumberValue(s.BeaconWidth);
            w.WriteNumberValue(s.BeaconHeight);
            w.WriteNumberValue(s.PoleWidth);
            w.WriteNumberValue(s.PoleHeight);
            w.WriteNumberValue(s.PoleSupplyWidth);
            w.WriteNumberValue(s.PoleSupplyHeight);
            w.WriteEndArray();
            w.WriteStartArray("wire_reach");
            w.WriteNumberValue(s.WireReach);
            w.WriteNumberValue(s.WireReachSquared);
            w.WriteEndArray();
            w.WriteBoolean("validate_solution", s.ValidateSolution);
            w.WriteBoolean("add_heat_pipes", s.AddHeatPipes);
            w.WriteNumber("width", s.Width);
            w.WriteNumber("height", s.Height);
            w.WriteNumber("next_id", s.NextId);
            w.WriteStartArray("entities");
            foreach (var e in s.Entities)
            {
                w.WriteStartArray();
                w.WriteNumberValue(e.X);
                w.WriteNumberValue(e.Y);
                w.WriteNumberValue(e.Kind);
                w.WriteNumberValue(e.Id);
                switch (e.Kind)
                {
                    case 0:
                    case 1:
                    case 3:
                    case 5:
                    case 8:
                        w.WriteNumberValue(e.Extra);
                        break;
                    case 4:
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
            if (s.HeatPipes is null)
            {
                w.WriteNull("heat_pipes");
            }
            else
            {
                WriteLocations(w, "heat_pipes", s.HeatPipes);
            }
        });

        return id;
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

    private static void Case(Stream stream, string op, int ctx, Action<Utf8JsonWriter> inputs, Action<Utf8JsonWriter> outputs)
    {
        _caseCount++;
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "case");
            w.WriteString("op", op);
            w.WriteNumber("ctx", ctx);
            inputs(w);
            w.WriteStartObject("out");
            outputs(w);
            w.WriteEndObject();
        });
    }

    private static void WriteError(Utf8JsonWriter w, string? error, string? exception)
    {
        if (error is null)
        {
            w.WriteNull("error");
        }
        else
        {
            w.WriteString("error", error);
        }

        if (exception is not null)
        {
            w.WriteString("exception", exception);
        }
    }

    /// <summary>A flat [x0, y0, x1, y1, ...] array.</summary>
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

    /// <summary>Every pole, in grid order, as [x, y, id, [neighbor ids in order]]: what the
    /// grid hash covers, spelled out so a mismatch is readable.</summary>
    private static void WritePoles(Utf8JsonWriter w, SquareGrid grid)
    {
        w.WriteStartArray("poles");
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            if (grid[location] is ElectricPoleCenter pole)
            {
                w.WriteStartArray();
                w.WriteNumberValue(location.X);
                w.WriteNumberValue(location.Y);
                w.WriteNumberValue(pole.Id);
                w.WriteStartArray();
                foreach (var n in pole.Neighbors)
                {
                    w.WriteNumberValue(n);
                }

                w.WriteEndArray();
                w.WriteEndArray();
            }
        }

        w.WriteEndArray();
    }

    private static List<Location> CellsOf(SquareGrid grid, Func<GridEntity, bool> match)
    {
        var cells = new List<Location>();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            if (match(grid[location]!))
            {
                cells.Add(location);
            }
        }

        return cells;
    }

    // ---------------------------------------------------------------------------------------
    // The grid hash, defined the same way in rust/planner/tests/helpers.rs and
    // rust/planner/tests/poles_and_heat.rs

    private static int KindCode(GridEntity e) => e switch
    {
        PumpjackCenter => 0,
        PumpjackSide => 1,
        BeaconCenter => 2,
        BeaconSide => 3,
        ElectricPoleCenter => 4,
        ElectricPoleSide => 5,
        UndergroundPipe => 8,
        Terminal => 7,
        Pipe => 6,
        HeatPipe => 9,
        TemporaryEntity => 10,
        AvoidEntity => 11,
        _ => throw new NotSupportedException(e.GetType().Name),
    };

    /// <summary>Every entity in enumeration order, with its id and per-kind state, then the
    /// next id. Reading the next id takes it, so this is the last thing done to a grid.</summary>
    private static string HashGrid(SquareGrid grid)
    {
        var h = new Fnv();
        foreach (var location in grid.EntityLocations.EnumerateItems())
        {
            var e = grid[location]!;
            h.Mix(location).Mix(KindCode(e)).Mix(e.Id);
            switch (e)
            {
                case PumpjackCenter p: h.Mix((int)p.Direction); break;
                case PumpjackSide s: h.Mix(s.Center.Id); break;
                case BeaconSide s: h.Mix(s.Center.Id); break;
                case ElectricPoleSide s: h.Mix(s.Center.Id); break;
                case UndergroundPipe u: h.Mix((int)u.Direction); break;
                case ElectricPoleCenter pole:
                    h.Mix(pole.Neighbors.Count);
                    foreach (var n in pole.Neighbors)
                    {
                        h.Mix(n);
                    }

                    break;
            }
        }

        h.Mix(grid.GetId());
        return h.Hex;
    }
}
