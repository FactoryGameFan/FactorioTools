// Records Helpers and Validate for the Rust port (#138), stage 4.
// Writes helpers.jsonl. The format is in rust/planner/FIXTURES.md.
//
// Every case runs on a context the recorder rebuilds from a recorded snapshot, so .NET and
// the Rust test start from the same container state: entities added in the recorded order
// into fresh, compact sets and dictionaries. A real plan's grid can have freed slots that a
// rebuild does not, and the helpers that change the grid would then order things differently.

using System.Reflection;
using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;
using static Knapcode.FactorioTools.OilField.Helpers;
// The core's own alias (CollectionExtensions.cs) is global only inside that project. The
// recorder builds with the default USE_BITARRAY, so it is the wrapper type here too.
using CountedBitArray = Knapcode.FactorioTools.OilField.WrapperCountedBitArray;

static class HelpersRecorder
{
    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static readonly FieldInfo NextIdField = typeof(SquareGrid).GetField("_nextId", BindingFlags.NonPublic | BindingFlags.Instance)
        ?? throw new InvalidOperationException("SquareGrid._nextId not found.");

    /// <summary>How many contexts keep the full candidate list, for reading a mismatch.</summary>
    private const int FullListCases = 4;

    /// <summary>The most provider placements a simulated beacon or pole loop records.</summary>
    private const int MaxSteps = 60;

    private sealed record Ent(int X, int Y, string Kind, int Id, int Extra, int[]? Neighbors);

    private sealed record Term(int Cx, int Cy, int Tx, int Ty, int Dir);

    private sealed class Snapshot
    {
        public required string Name;
        public required int BeaconWidth, BeaconHeight, BeaconSupplyWidth, BeaconSupplyHeight;
        public required int PoleWidth, PoleHeight, PoleSupplyWidth, PoleSupplyHeight;
        public required bool OverlapBeacons, ValidateSolution, AddHeatPipes, AddBeacons;
        public required int Width, Height, NextId;
        public required List<Ent> Entities;
        public required List<Location> Centers;
        public required List<(Location Key, List<Term> Terminals)> CenterToTerminals;
        public required List<(Location Key, List<Term> Terminals)> LocationToTerminals;
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

        public Fnv Mix(double value)
        {
            var bits = BitConverter.DoubleToInt64Bits(value);
            Mix(unchecked((int)(bits & 0xFFFFFFFF)));
            return Mix(unchecked((int)(bits >> 32)));
        }

        public Fnv Mix(Location location) => Mix(location.X).Mix(location.Y);

        public string Hex => _hash.ToString("x16");
    }

    private static int _contextCount;
    private static int _caseCount;

    public static void Record(string outputDir, IReadOnlyList<string> smallList)
    {
        var path = Path.Combine(outputDir, "helpers.jsonl");
        _contextCount = 0;
        _caseCount = 0;
        using (var stream = File.Create(path))
        {
            var fullLists = 0;

            // Finished plans, and the same grids before poles and before beacons and poles, the
            // states the pole and beacon passes start from.
            var planSets = new (string Name, Func<OilFieldOptions> Options, int Stride, int Offset, bool Derive)[]
            {
                ("medium_b_o_u", () => Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: true, heat: false), 4, 0, true),
                ("substation_b", () => Opts(OilFieldOptions.ForSubstation, beacons: true, overlap: false, heat: false), 8, 1, true),
                ("small_u", () => Opts(OilFieldOptions.ForSmallElectricPole, beacons: false, overlap: false, heat: false), 8, 2, true),
                ("big_b", () => Opts(OilFieldOptions.ForBigElectricPole, beacons: true, overlap: false, heat: false), 8, 5, true),
                ("medium_heat_b", () => Opts(OilFieldOptions.ForMediumElectricPole, beacons: true, overlap: true, heat: true), 8, 3, false),
                ("medium_heat", () => Opts(OilFieldOptions.ForMediumElectricPole, beacons: false, overlap: false, heat: true), 8, 6, false),
            };

            foreach (var set in planSets)
            {
                for (var i = set.Offset; i < smallList.Count; i += set.Stride)
                {
                    var name = $"small-{i:D4}_{set.Name}";
                    var result = Planner.Execute(set.Options(), ParseBlueprint.Execute(smallList[i]));
                    var final = Capture(name, result.Context, _ => false);
                    var finalId = WriteSnapshot(stream, final);
                    var full = fullLists++ < FullListCases;

                    RecordFinal(stream, finalId, final, full);

                    if (set.Derive)
                    {
                        var noPoles = Capture(name + "_no-poles", result.Context, e => e is ElectricPoleCenter or ElectricPoleSide);
                        var noPolesId = WriteSnapshot(stream, noPoles);
                        RecordNoPoles(stream, noPolesId, noPoles, full);

                        if (final.AddBeacons)
                        {
                            var bare = Capture(name + "_no-beacons", result.Context, e => e is ElectricPoleCenter or ElectricPoleSide or BeaconCenter or BeaconSide);
                            var bareId = WriteSnapshot(stream, bare);
                            RecordNoBeacons(stream, bareId, bare, full);
                        }
                    }
                }
            }

            // Starting contexts, before any pipe, for the terminal helpers.
            for (var i = 0; i < smallList.Count; i += 4)
            {
                var name = $"small-{i:D4}_initial";
                var context = InitializeContext.Execute(OilFieldOptions.ForMediumElectricPole, ParseBlueprint.Execute(smallList[i]), Array.Empty<AvoidLocation>());
                var snapshot = Capture(name, context, _ => false);
                var id = WriteSnapshot(stream, snapshot);
                RecordInitial(stream, id, snapshot);
            }

            RecordGeometry(stream);
        }

        Console.WriteLine($"helpers.jsonl: {_contextCount} contexts, {_caseCount} cases, {new FileInfo(path).Length} bytes");
    }

    private static OilFieldOptions Opts(OilFieldOptions options, bool beacons, bool overlap, bool heat)
    {
        options.AddBeacons = beacons;
        options.OverlapBeacons = overlap;
        options.AddHeatPipes = heat;
        options.ValidateSolution = true;
        return options;
    }

    // ---------------------------------------------------------------------------------------
    // Snapshots

    private static Snapshot Capture(string name, Context context, Func<GridEntity, bool> exclude)
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

        // Excluded poles are gone, so drop them from the remaining poles' neighbor lists. Only
        // poles are ever excluded together with all other poles, so this leaves no pole.
        return new Snapshot
        {
            Name = name,
            BeaconWidth = context.Options.BeaconWidth,
            BeaconHeight = context.Options.BeaconHeight,
            BeaconSupplyWidth = context.Options.BeaconSupplyWidth,
            BeaconSupplyHeight = context.Options.BeaconSupplyHeight,
            PoleWidth = context.Options.ElectricPoleWidth,
            PoleHeight = context.Options.ElectricPoleHeight,
            PoleSupplyWidth = context.ElectricPoleSupplyWidthWithQuality,
            PoleSupplyHeight = context.ElectricPoleSupplyHeightWithQuality,
            OverlapBeacons = context.Options.OverlapBeacons,
            ValidateSolution = context.Options.ValidateSolution,
            AddHeatPipes = context.Options.AddHeatPipes,
            AddBeacons = context.Options.AddBeacons,
            Width = grid.Width,
            Height = grid.Height,
            NextId = (int)NextIdField.GetValue(grid)!,
            Entities = entities,
            Centers = context.Centers.ToList(),
            CenterToTerminals = context.CenterToTerminals.EnumeratePairs().Select(p => (p.Key, p.Value.Select(ToTerm).ToList())).ToList(),
            LocationToTerminals = context.LocationToTerminals.EnumeratePairs().Select(p => (p.Key, p.Value.Select(ToTerm).ToList())).ToList(),
            HeatPipes = context.HeatPipes?.EnumerateItems().ToList(),
        };
    }

    private static Term ToTerm(TerminalLocation t) => new(t.Center.X, t.Center.Y, t.Terminal.X, t.Terminal.Y, (int)t.Direction);

    private static Ent ToEnt(Location location, GridEntity entity)
    {
        return entity switch
        {
            UndergroundPipe u => new Ent(location.X, location.Y, "UndergroundPipe", entity.Id, (int)u.Direction, null),
            Terminal => new Ent(location.X, location.Y, "Terminal", entity.Id, 0, null),
            Pipe => new Ent(location.X, location.Y, "Pipe", entity.Id, 0, null),
            HeatPipe => new Ent(location.X, location.Y, "HeatPipe", entity.Id, 0, null),
            BeaconCenter => new Ent(location.X, location.Y, "BeaconCenter", entity.Id, 0, null),
            TemporaryEntity => new Ent(location.X, location.Y, "TemporaryEntity", entity.Id, 0, null),
            AvoidEntity => new Ent(location.X, location.Y, "AvoidEntity", entity.Id, 0, null),
            PumpjackCenter p => new Ent(location.X, location.Y, "PumpjackCenter", entity.Id, (int)p.Direction, null),
            PumpjackSide s => new Ent(location.X, location.Y, "PumpjackSide", entity.Id, s.Center.Id, null),
            BeaconSide s => new Ent(location.X, location.Y, "BeaconSide", entity.Id, s.Center.Id, null),
            ElectricPoleSide s => new Ent(location.X, location.Y, "ElectricPoleSide", entity.Id, s.Center.Id, null),
            ElectricPoleCenter p => new Ent(location.X, location.Y, "ElectricPoleCenter", entity.Id, 0, p.Neighbors.ToArray()),
            _ => throw new NotSupportedException(entity.GetType().Name),
        };
    }

    /// <summary>A fresh context holding the snapshot, built the way the Rust test builds it.</summary>
    private static Context Rebuild(Snapshot s)
    {
        var options = new OilFieldOptions
        {
            BeaconWidth = s.BeaconWidth,
            BeaconHeight = s.BeaconHeight,
            BeaconSupplyWidth = s.BeaconSupplyWidth,
            BeaconSupplyHeight = s.BeaconSupplyHeight,
            ElectricPoleWidth = s.PoleWidth,
            ElectricPoleHeight = s.PoleHeight,
            OverlapBeacons = s.OverlapBeacons,
            ValidateSolution = s.ValidateSolution,
            AddHeatPipes = s.AddHeatPipes,
            AddBeacons = s.AddBeacons,
        };

        var grid = new PipeGrid(s.Width, s.Height);
        var byId = new Dictionary<int, GridEntity>();
        foreach (var e in s.Entities)
        {
            GridEntity? entity = e.Kind switch
            {
                "PumpjackCenter" => new PumpjackCenter(e.Id) { Direction = (Direction)e.Extra },
                "BeaconCenter" => new BeaconCenter(e.Id),
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
                "PumpjackCenter" or "BeaconCenter" or "ElectricPoleCenter" => byId[e.Id],
                "PumpjackSide" => new PumpjackSide(e.Id, (PumpjackCenter)byId[e.Extra]),
                "BeaconSide" => new BeaconSide(e.Id, (BeaconCenter)byId[e.Extra]),
                "ElectricPoleSide" => new ElectricPoleSide(e.Id, (ElectricPoleCenter)byId[e.Extra]),
                "Pipe" => new Pipe(e.Id),
                "Terminal" => new Terminal(e.Id),
                "UndergroundPipe" => new UndergroundPipe(e.Id, (Direction)e.Extra),
                "HeatPipe" => new HeatPipe(e.Id),
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
        };

        // The two terminal dictionaries share TerminalLocation objects, as the planner's do.
        var terminals = new Dictionary<Term, TerminalLocation>();
        TerminalLocation Get(Term t)
        {
            if (!terminals.TryGetValue(t, out var terminal))
            {
                terminal = new TerminalLocation(new Location(t.Cx, t.Cy), new Location(t.Tx, t.Ty), (Direction)t.Dir);
                terminals.Add(t, terminal);
            }

            return terminal;
        }

        foreach (var (key, list) in s.CenterToTerminals)
        {
            context.CenterToTerminals.Add(key, list.Select(Get).ToList());
        }

        foreach (var (key, list) in s.LocationToTerminals)
        {
            context.LocationToTerminals.Add(key, list.Select(Get).ToList());
        }

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
            w.WriteStartObject("options");
            w.WriteNumber("beacon_width", s.BeaconWidth);
            w.WriteNumber("beacon_height", s.BeaconHeight);
            w.WriteNumber("beacon_supply_width", s.BeaconSupplyWidth);
            w.WriteNumber("beacon_supply_height", s.BeaconSupplyHeight);
            w.WriteNumber("electric_pole_width", s.PoleWidth);
            w.WriteNumber("electric_pole_height", s.PoleHeight);
            w.WriteBoolean("overlap_beacons", s.OverlapBeacons);
            w.WriteBoolean("validate_solution", s.ValidateSolution);
            w.WriteBoolean("add_heat_pipes", s.AddHeatPipes);
            w.WriteBoolean("add_beacons", s.AddBeacons);
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
                    case "BeaconSide":
                    case "ElectricPoleSide":
                    case "UndergroundPipe":
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
            WriteTerminals(w, "center_to_terminals", s.CenterToTerminals);
            WriteTerminals(w, "location_to_terminals", s.LocationToTerminals);
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

    private static void WriteTerminals(Utf8JsonWriter w, string name, List<(Location Key, List<Term> Terminals)> dictionary)
    {
        w.WriteStartArray(name);
        foreach (var (key, list) in dictionary)
        {
            w.WriteStartArray();
            w.WriteNumberValue(key.X);
            w.WriteNumberValue(key.Y);
            w.WriteStartArray();
            foreach (var t in list)
            {
                w.WriteNumberValue(t.Cx);
                w.WriteNumberValue(t.Cy);
                w.WriteNumberValue(t.Tx);
                w.WriteNumberValue(t.Ty);
                w.WriteNumberValue(t.Dir);
            }

            w.WriteEndArray();
            w.WriteEndArray();
        }

        w.WriteEndArray();
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

    private static void Case(Stream stream, string op, int? ctx, Action<Utf8JsonWriter> inputs, Action<Utf8JsonWriter> outputs)
    {
        _caseCount++;
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "case");
            w.WriteString("op", op);
            if (ctx is not null)
            {
                w.WriteNumber("ctx", ctx.Value);
            }

            inputs(w);
            w.WriteStartObject("out");
            outputs(w);
            w.WriteEndObject();
        });
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

    private static void WritePoint(Utf8JsonWriter w, string name, Location l)
    {
        w.WriteStartArray(name);
        w.WriteNumberValue(l.X);
        w.WriteNumberValue(l.Y);
        w.WriteEndArray();
    }

    private static string Bits(CountedBitArray bits)
    {
        var chars = new char[bits.Count];
        for (var i = 0; i < bits.Count; i++)
        {
            chars[i] = bits[i] ? '1' : '0';
        }

        return new string(chars);
    }

    private static void WriteSetDictionary(Utf8JsonWriter w, string name, ILocationDictionary<ILocationSet> dictionary)
    {
        w.WriteStartArray(name);
        foreach (var (key, set) in dictionary.EnumeratePairs())
        {
            w.WriteStartArray();
            w.WriteNumberValue(key.X);
            w.WriteNumberValue(key.Y);
            w.WriteStartArray();
            foreach (var l in set.EnumerateItems())
            {
                w.WriteNumberValue(l.X);
                w.WriteNumberValue(l.Y);
            }

            w.WriteEndArray();
            w.WriteEndArray();
        }

        w.WriteEndArray();
    }

    // ---------------------------------------------------------------------------------------
    // Hashes, defined the same way in rust/planner/tests/helpers.rs

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

    private static string HashCandidates<TInfo>(ILocationDictionary<TInfo> dictionary) where TInfo : CandidateInfo
    {
        var h = new Fnv().Mix(dictionary.Count);
        foreach (var (location, info) in dictionary.EnumeratePairs())
        {
            h.Mix(location).Mix(info.Covered.Count).Mix(info.Covered.TrueCount);
            for (var i = 0; i < info.Covered.Count; i++)
            {
                h.Mix(info.Covered[i] ? 1 : 0);
            }

            h.Mix(info.EntityDistance);
        }

        return h.Hex;
    }

    private static string HashKeys<T>(ILocationDictionary<T> dictionary)
    {
        var h = new Fnv().Mix(dictionary.Count);
        foreach (var location in dictionary.Keys)
        {
            h.Mix(location);
        }

        return h.Hex;
    }

    private static string HashCoveredToCandidates<TInfo>(Dictionary<int, ILocationDictionary<TInfo>> coveredToCandidates)
    {
        var h = new Fnv().Mix(coveredToCandidates.Count);
        for (var i = 0; i < coveredToCandidates.Count; i++)
        {
            var d = coveredToCandidates[i];
            h.Mix(i).Mix(d.Count);
            foreach (var location in d.Keys)
            {
                h.Mix(location);
            }
        }

        return h.Hex;
    }

    private static string HashBatches<TInfo>(SortedBatches<TInfo> batches)
    {
        var h = new Fnv().Mix(batches.Queue.Count).Mix(batches.Lookup.Count);
        foreach (var (key, d) in batches.Lookup)
        {
            h.Mix(key).Mix(d.Count);
            foreach (var location in d.Keys)
            {
                h.Mix(location);
            }
        }

        return h.Hex;
    }

    private static string HashTerminals(ILocationDictionary<List<TerminalLocation>> dictionary)
    {
        var h = new Fnv().Mix(dictionary.Count);
        foreach (var (key, list) in dictionary.EnumeratePairs())
        {
            h.Mix(key).Mix(list.Count);
            foreach (var t in list)
            {
                h.Mix(t.Center).Mix(t.Terminal).Mix((int)t.Direction);
            }
        }

        return h.Hex;
    }

    // ---------------------------------------------------------------------------------------
    // Mutations: changes applied to a rebuilt context before an operation, recorded under
    // "mut" so the Rust test applies the same ones.

    private sealed class Mutation
    {
        public List<Location> RemoveCells = new();
        public List<Location> AddPoles = new();
        public List<Location> RemoveHeatPipes = new();
        public bool? OverlapBeacons;

        public bool IsEmpty => RemoveCells.Count == 0 && AddPoles.Count == 0 && RemoveHeatPipes.Count == 0 && OverlapBeacons is null;

        public void Apply(Context context)
        {
            if (OverlapBeacons is not null)
            {
                context.Options.OverlapBeacons = OverlapBeacons.Value;
            }

            foreach (var l in RemoveCells)
            {
                context.Grid.RemoveEntity(l);
            }

            foreach (var l in AddPoles)
            {
                AddProviderToGrid(
                    context.Grid,
                    l,
                    new ElectricPoleCenter(context.Grid.GetId()),
                    c => new ElectricPoleSide(context.Grid.GetId(), c),
                    context.Options.ElectricPoleWidth,
                    context.Options.ElectricPoleHeight);
            }

            foreach (var l in RemoveHeatPipes)
            {
                context.HeatPipes!.Remove(l);
            }
        }

        public void Write(Utf8JsonWriter w)
        {
            if (IsEmpty)
            {
                return;
            }

            w.WriteStartObject("mut");
            WriteLocations(w, "remove_cells", RemoveCells);
            WriteLocations(w, "add_poles", AddPoles);
            WriteLocations(w, "remove_heat_pipes", RemoveHeatPipes);
            if (OverlapBeacons is not null)
            {
                w.WriteBoolean("overlap_beacons", OverlapBeacons.Value);
            }

            w.WriteEndObject();
        }
    }

    private static readonly Mutation None = new();

    private static void WriteResult(Utf8JsonWriter w, Action action)
    {
        try
        {
            action();
            w.WriteBoolean("ok", true);
        }
        catch (FactorioToolsException ex)
        {
            w.WriteString("error", ex.Message);
        }
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

    /// <summary>The first spot, scanning from the bottom right, where a pole fits fully inside
    /// the grid. Used to add a pole that is likely to cover nothing.</summary>
    private static Location? SpareSpot(Context context, int width, int height)
    {
        for (var y = context.Grid.Height - 2; y >= 1; y--)
        {
            for (var x = context.Grid.Width - 2; x >= 1; x--)
            {
                var l = new Location(x, y);
                if (IsProviderInBounds(context.Grid, width, height, l) && DoesProviderFit(context.Grid, width, height, l))
                {
                    return l;
                }
            }
        }

        return null;
    }

    // ---------------------------------------------------------------------------------------
    // Cases on a finished plan

    private static void RecordFinal(Stream stream, int id, Snapshot s, bool full)
    {
        var context = Rebuild(s);
        var grid = context.Grid;
        var poles = CellsOf(grid, e => e is ElectricPoleCenter);
        var beacons = CellsOf(grid, e => e is BeaconCenter);
        var pipes = CellsOf(grid, e => e is Pipe);
        var undergrounds = CellsOf(grid, e => e is UndergroundPipe);
        var heatPipes = CellsOf(grid, e => e is HeatPipe);

        Case(stream, "powered", id, _ => { }, w =>
        {
            var (powered, hasBeacons) = GetPoweredEntities(Rebuild(s));
            WriteRecipients(w, powered);
            w.WriteBoolean("has_beacons", hasBeacons);
        });

        RecordCoverage(stream, id, s, None);
        if (poles.Count > 0)
        {
            // Without one pole, coverage may fail.
            var m = new Mutation();
            m.RemoveCells.AddRange(ProviderCells(poles[poles.Count / 2], s.PoleWidth, s.PoleHeight));
            RecordCoverage(stream, id, s, m);
        }

        RecordBeaconCoverage(stream, id, s, None);

        // Existing poles as providers: kept when used, and a spare pole that is likely unused.
        RecordCandidates(stream, id, s, None, "pole", "pole", removeUnused: true, full, simulate: false);
        var spare = SpareSpot(context, s.PoleWidth, s.PoleHeight);
        if (spare is not null)
        {
            var m = new Mutation();
            m.AddPoles.Add(spare.Value);
            RecordCandidates(stream, id, s, m, "pole", "pole", removeUnused: true, full, simulate: false);
        }

        if (s.AddBeacons)
        {
            RecordCandidates(stream, id, s, None, "beacon", "plain", removeUnused: true, full, simulate: false);
        }

        // Grid changes.
        if (beacons.Count > 0)
        {
            RecordMutateGrid(stream, id, s, "remove_entity", beacons[0], s.BeaconWidth, s.BeaconHeight, null);
        }

        if (poles.Count > 0)
        {
            RecordMutateGrid(stream, id, s, "remove_entity", poles[0], s.PoleWidth, s.PoleHeight, null);
        }

        RecordMutateGrid(stream, id, s, "remove_pumpjack", s.Centers[0], 0, 0, null);
        if (spare is not null)
        {
            RecordMutateGrid(stream, id, s, "add_pole", spare.Value, 0, 0, null);
            RecordMutateGrid(stream, id, s, "add_beacons", spare.Value, 0, 0, new List<Location> { spare.Value });
        }

        // A beacon on a pipe fails part way, after placing what fit before it. Its cells must
        // stay on the grid, or the C# throws IndexOutOfRangeException instead.
        var blocked = pipes.FirstOrDefault(p => p.X - 1 - ((s.BeaconWidth - 1) / 2) >= 0
            && p.X - 1 + (s.BeaconWidth / 2) < s.Width
            && p.Y - ((s.BeaconHeight - 1) / 2) >= 0
            && p.Y + (s.BeaconHeight / 2) < s.Height);
        if (pipes.Count > 0 && blocked != default)
        {
            RecordMutateGrid(stream, id, s, "add_beacons", blocked, 0, 0, new List<Location> { new Location(blocked.X - 1, blocked.Y) });
        }

        // Every cell of the grid, so only on every other finished plan to keep the file small.
        if (id % 2 == 0)
        {
            RecordFit(stream, id, s);
        }

        // Validate.
        RecordValidate(stream, id, s, None, "pipes_connected", w => w.WriteNumber("drop", -1));
        if (pipes.Count > 2)
        {
            RecordValidate(stream, id, s, None, "pipes_connected", w => w.WriteNumber("drop", pipes.Count / 2));
        }

        RecordValidate(stream, id, s, None, "underground_are_pipes", w => w.WriteNumber("drop", -1));
        if (undergrounds.Count > 0)
        {
            RecordValidate(stream, id, s, None, "underground_are_pipes", w => w.WriteNumber("drop", 0));
        }

        foreach (var variant in new[] { "same", "minus_one", "bit_same" })
        {
            RecordValidate(stream, id, s, None, "pipes_do_not_match", w => w.WriteString("variant", variant));
        }

        if (s.AddBeacons)
        {
            RecordValidate(stream, id, s, None, "beacons_do_not_overlap", _ => { });
            var m = new Mutation { OverlapBeacons = false };
            RecordValidate(stream, id, s, m, "beacons_do_not_overlap", _ => { });
        }

        RecordValidate(stream, id, s, None, "no_existing_beacons", w => w.WriteString("variant", "empty"));
        RecordValidate(stream, id, s, None, "no_existing_beacons", w => w.WriteString("variant", "grid"));

        RecordValidate(stream, id, s, None, "unheated", _ => { });
        RecordValidate(stream, id, s, None, "no_unheated_beacons", _ => { });
        RecordValidate(stream, id, s, None, "heat_connected", _ => { });
        if (heatPipes.Count > 0)
        {
            var m = new Mutation();
            m.RemoveCells.Add(heatPipes[0]);
            RecordValidate(stream, id, s, m, "unheated", _ => { });

            var all = new Mutation();
            all.RemoveCells.AddRange(heatPipes);
            RecordValidate(stream, id, s, all, "no_unheated_beacons", _ => { });

            var gap = new Mutation();
            gap.RemoveHeatPipes.Add(s.HeatPipes![s.HeatPipes.Count / 2]);
            RecordValidate(stream, id, s, gap, "heat_connected", _ => { });
        }

        RecordValidate(stream, id, s, None, "all_power", _ => { });
        if (poles.Count > 0)
        {
            var m = new Mutation();
            m.RemoveCells.AddRange(ProviderCells(poles[0], s.PoleWidth, s.PoleHeight));
            RecordValidate(stream, id, s, m, "all_power", _ => { });
        }

        RecordGetPath(stream, id, s);
        RecordLinesOnEmpty(stream, id, s, pipes);

        // Pole centers as AddElectricPoles gives them to PointsToLines.
        if (poles.Count > 0)
        {
            RecordLines(stream, poles, sort: true);
        }

        RecordTurns(stream, pipes);
    }

    private static IEnumerable<Location> ProviderCells(Location center, int width, int height)
    {
        for (var x = center.X - ((width - 1) / 2); x <= center.X + (width / 2); x++)
        {
            for (var y = center.Y - ((height - 1) / 2); y <= center.Y + (height / 2); y++)
            {
                yield return new Location(x, y);
            }
        }
    }

    private static void WriteRecipients(Utf8JsonWriter w, List<ProviderRecipient> recipients)
    {
        w.WriteStartArray("recipients");
        foreach (var r in recipients)
        {
            w.WriteNumberValue(r.Center.X);
            w.WriteNumberValue(r.Center.Y);
            w.WriteNumberValue(r.Width);
            w.WriteNumberValue(r.Height);
        }

        w.WriteEndArray();
    }

    private static void RecordCoverage(Stream stream, int id, Snapshot s, Mutation m)
    {
        Case(stream, "pole_coverage", id, m.Write, w =>
        {
            var context = Rebuild(s);
            m.Apply(context);
            var (powered, _) = GetPoweredEntities(context);
            var poles = CellsOf(context.Grid, e => e is ElectricPoleCenter);
            try
            {
                var (p2c, c2p) = GetElectricPoleCoverage(context, powered, poles);
                WriteSetDictionary(w, "p2c", p2c);
                WriteSetDictionary(w, "c2p", c2p);
            }
            catch (FactorioToolsException ex)
            {
                w.WriteString("error", ex.Message);
            }
        });
    }

    private static void RecordBeaconCoverage(Stream stream, int id, Snapshot s, Mutation m)
    {
        Case(stream, "beacon_coverage", id, m.Write, w =>
        {
            var context = Rebuild(s);
            m.Apply(context);
            var beacons = CellsOf(context.Grid, e => e is BeaconCenter);
            var p2c = GetProviderCenterToCoveredCenters(
                context,
                context.Options.BeaconWidth,
                context.Options.BeaconHeight,
                context.Options.BeaconSupplyWidth,
                context.Options.BeaconSupplyHeight,
                beacons,
                includePumpjacks: true,
                includeBeacons: false);
            WriteSetDictionary(w, "p2c", p2c);
            WriteSetDictionary(w, "c2p", GetCoveredCenterToProviderCenters(context, p2c));
        });
    }

    private sealed class PlainFactory : ICandidateFactory<CandidateInfo>
    {
        public CandidateInfo Create(CountedBitArray covered) => new(covered);
    }

    private sealed class PoleFactory : ICandidateFactory<ElectricPoleCandidateInfo>
    {
        public ElectricPoleCandidateInfo Create(CountedBitArray covered) => new(covered);
    }

    private static List<ProviderRecipient> Recipients(Context context, string provider)
    {
        // The pole pass powers pumpjacks and beacons (GetPoweredEntities); the beacon pass
        // affects pumpjacks, in Centers order (PlanBeaconsSnug).
        return provider == "pole"
            ? GetPoweredEntities(context).PoweredEntities
            : context.Centers.Select(c => new ProviderRecipient(c, PumpjackWidth, PumpjackHeight)).ToList();
    }

    private static void RecordCandidates(Stream stream, int id, Snapshot s, Mutation m, string provider, string info, bool removeUnused, bool full, bool simulate)
    {
        Case(
            stream,
            "candidates",
            id,
            w =>
            {
                m.Write(w);
                w.WriteString("provider", provider);
                w.WriteString("info", info);
                w.WriteBoolean("remove_unused", removeUnused);
                w.WriteBoolean("simulate", simulate);
            },
            w =>
            {
                if (info == "pole")
                {
                    CandidatesOut<ElectricPoleCandidateInfo>(w, s, m, provider, new PoleFactory(), removeUnused, full, simulate);
                }
                else
                {
                    CandidatesOut<CandidateInfo>(w, s, m, provider, new PlainFactory(), removeUnused, full, simulate);
                }
            });
    }

    private static (Context Context, List<ProviderRecipient> Recipients, ILocationDictionary<TInfo> CandidateToInfo, CountedBitArray Covered, object Providers) RunCandidates<TInfo>(
        Snapshot s, Mutation m, string provider, ICandidateFactory<TInfo> factory, bool removeUnused)
        where TInfo : CandidateInfo
    {
        var context = Rebuild(s);
        m.Apply(context);
        var recipients = Recipients(context, provider);
        if (provider == "pole")
        {
            var (c, covered, providers) = GetElectricPoleCandidateToCovered(context, recipients, factory, removeUnused);
            return (context, recipients, c, covered, providers);
        }
        else
        {
            var (c, covered, providers) = GetBeaconCandidateToCovered(context, recipients, factory, removeUnused);
            return (context, recipients, c, covered, providers);
        }
    }

    private static void CandidatesOut<TInfo>(Utf8JsonWriter w, Snapshot s, Mutation m, string provider, ICandidateFactory<TInfo> factory, bool removeUnused, bool full, bool simulate)
        where TInfo : CandidateInfo
    {
        var (context, recipients, candidateToInfo, covered, providersObject) = RunCandidates(s, m, provider, factory, removeUnused);

        WriteRecipients(w, recipients);
        w.WriteNumber("count", candidateToInfo.Count);
        w.WriteString("hash", HashCandidates(candidateToInfo));
        w.WriteString("covered", Bits(covered));
        w.WriteStartArray("providers");
        foreach (var (location, entity) in ProviderPairs(providersObject))
        {
            w.WriteNumberValue(location.X);
            w.WriteNumberValue(location.Y);
            w.WriteNumberValue(entity.Id);
        }

        w.WriteEndArray();
        w.WriteString("grid", HashGrid(context.Grid));

        if (full)
        {
            w.WriteStartArray("list");
            foreach (var (location, i) in candidateToInfo.EnumeratePairs())
            {
                w.WriteStartArray();
                w.WriteNumberValue(location.X);
                w.WriteNumberValue(location.Y);
                w.WriteStringValue(Bits(i.Covered));
                w.WriteEndArray();
            }

            w.WriteEndArray();
        }

        // GetEntityDistance for every candidate, then GetCoveredToCandidates.
        var distances = new Fnv();
        foreach (var (location, i) in candidateToInfo.EnumeratePairs())
        {
            distances.Mix(GetEntityDistance(recipients, location, i.Covered));
        }

        w.WriteString("entity_distance", distances.Hex);
        w.WriteString("covered_to_candidates", HashCoveredToCandidates(GetCoveredToCandidates(context, candidateToInfo, covered)));

        if (!simulate)
        {
            return;
        }

        if (provider == "pole" && candidateToInfo is ILocationDictionary<ElectricPoleCandidateInfo> poleCandidates)
        {
            // CandidateCoversMoreEntities on the first candidate: nothing covered yet (passes),
            // then everything it covers already covered (throws).
            var (first, firstInfo) = poleCandidates.EnumeratePairs().FirstOrDefault();
            if (firstInfo is not null)
            {
                w.WriteStartArray("covers_more");
                foreach (var coveredSoFar in new[] { new CountedBitArray(recipients.Count), new CountedBitArray(firstInfo.Covered) })
                {
                    try
                    {
                        Validate.CandidateCoversMoreEntities(context, recipients, coveredSoFar, first, firstInfo);
                        w.WriteStringValue("ok");
                    }
                    catch (FactorioToolsException ex)
                    {
                        w.WriteStringValue(ex.Message);
                    }
                }

                w.WriteEndArray();
            }

            SimulateAllow(w, "allow", s, m, separate: false);
            SimulateAllow(w, "allow_separate", s, m, separate: true);
        }
        else if (provider == "beacon")
        {
            SimulatePrevent(w, s, m);
            SimulateOverlap(w, s, m);
        }
    }

    private static IEnumerable<(Location, GridEntity)> ProviderPairs(object providers)
    {
        return providers switch
        {
            ILocationDictionary<ElectricPoleCenter> p => p.EnumeratePairs().Select(x => (x.Key, (GridEntity)x.Value)),
            ILocationDictionary<BeaconCenter> p => p.EnumeratePairs().Select(x => (x.Key, (GridEntity)x.Value)),
            _ => throw new NotSupportedException(),
        };
    }

    /// <summary>
    /// The pole pass's loop (AddElectricPoles.AddElectricPolesAroundEntities) without its
    /// priority subsets and candidate ranking: candidates batched by covered count, the first
    /// candidate of the top batch placed each time with AddProviderAndAllowMultipleProviders.
    /// With <paramref name="separate"/>, the scoped dictionary is a copy of every candidate
    /// instead of a batch, as the priority subsets are in the real pass.
    /// </summary>
    private static void SimulateAllow(Utf8JsonWriter w, string name, Snapshot s, Mutation m, bool separate)
    {
        var (context, recipients, all, covered, _) = RunCandidates(s, m, "pole", new PoleFactory(), removeUnused: true);
        foreach (var (location, info) in all.EnumeratePairs())
        {
            info.EntityDistance = GetEntityDistance(recipients, location, info.Covered);
        }

        var coveredToCandidates = GetCoveredToCandidates(context, all, covered);
        var coveredToLocationToInfo = new Dictionary<int, ILocationDictionary<ElectricPoleCandidateInfo>>();
        foreach (var (location, info) in all.EnumeratePairs())
        {
            if (!coveredToLocationToInfo.TryGetValue(info.Covered.TrueCount, out var locationToInfo))
            {
                locationToInfo = context.GetLocationDictionary<ElectricPoleCandidateInfo>();
                coveredToLocationToInfo.Add(info.Covered.TrueCount, locationToInfo);
            }

            locationToInfo.Add(location, info);
        }

        var batches = new SortedBatches<ElectricPoleCandidateInfo>(coveredToLocationToInfo, ascending: false);

        ILocationDictionary<ElectricPoleCandidateInfo>? scoped = null;
        if (separate)
        {
            scoped = context.GetLocationDictionary<ElectricPoleCandidateInfo>();
            foreach (var (location, info) in all.EnumeratePairs())
            {
                scoped.Add(location, info);
            }
        }

        w.WriteStartArray(name);
        var steps = 0;
        while (covered.Any(false) && steps < MaxSteps)
        {
            if (scoped is null)
            {
                if (separate || batches.Queue.Count == 0)
                {
                    break;
                }

                scoped = batches.Queue.Peek();
                if (scoped.Count == 0)
                {
                    scoped = null;
                    batches.Queue.Dequeue();
                    continue;
                }
            }

            var (candidate, info) = scoped.EnumeratePairs().First();
            if (!all.ContainsKey(candidate))
            {
                scoped.Remove(candidate);
                if (scoped.Count == 0)
                {
                    scoped = null;
                }

                continue;
            }

            string? error = null;
            try
            {
                AddProviderAndAllowMultipleProviders(
                    context,
                    candidate,
                    info,
                    context.Options.ElectricPoleWidth,
                    context.Options.ElectricPoleHeight,
                    recipients,
                    covered,
                    coveredToCandidates,
                    all,
                    scoped,
                    batches);
            }
            catch (FactorioToolsException ex)
            {
                error = ex.Message;
            }

            steps++;
            w.WriteStartArray();
            w.WriteNumberValue(candidate.X);
            w.WriteNumberValue(candidate.Y);
            w.WriteStringValue(error ?? Bits(covered));
            w.WriteStringValue(HashCandidates(all));
            w.WriteStringValue(HashCoveredToCandidates(coveredToCandidates));
            w.WriteStringValue(HashBatches(batches));
            w.WriteNumberValue(scoped.Count);
            w.WriteEndArray();

            if (error is not null)
            {
                break;
            }

            if (scoped.Count == 0)
            {
                scoped = null;
            }
        }

        w.WriteEndArray();
    }

    /// <summary>
    /// The beacon pass without overlap (PlanBeaconsSnug with OverlapBeacons off), simplified:
    /// the first candidate that still covers something new is placed each time with
    /// AddProviderAndPreventMultipleProviders. Then one candidate that covers nothing new,
    /// which throws.
    /// </summary>
    private static void SimulatePrevent(Utf8JsonWriter w, Snapshot s, Mutation m)
    {
        var (context, recipients, candidateToInfo, covered, _) = RunCandidates(s, m, "beacon", new PlainFactory(), removeUnused: false);
        var coveredToCandidates = GetCoveredToCandidates(context, candidateToInfo, covered);

        w.WriteStartArray("prevent");
        var steps = 0;
        while (steps < MaxSteps)
        {
            Location? pick = null;
            CandidateInfo? pickInfo = null;
            foreach (var (location, info) in candidateToInfo.EnumeratePairs())
            {
                var newly = new CountedBitArray(covered);
                newly.Not();
                newly.And(info.Covered);
                if (newly.TrueCount > 0)
                {
                    pick = location;
                    pickInfo = info;
                    break;
                }
            }

            if (pick is null)
            {
                break;
            }

            AddProviderAndPreventMultipleProviders(
                context,
                pick.Value,
                pickInfo!,
                context.Options.BeaconWidth,
                context.Options.BeaconHeight,
                recipients,
                covered,
                coveredToCandidates,
                candidateToInfo);
            steps++;

            w.WriteStartArray();
            w.WriteNumberValue(pick.Value.X);
            w.WriteNumberValue(pick.Value.Y);
            w.WriteStringValue(Bits(covered));
            w.WriteStringValue(HashKeys(candidateToInfo));
            w.WriteStringValue(HashCoveredToCandidates(coveredToCandidates));
            w.WriteEndArray();
        }

        w.WriteEndArray();

        var (rest, restInfo) = candidateToInfo.EnumeratePairs().FirstOrDefault();
        if (restInfo is not null)
        {
            try
            {
                AddProviderAndPreventMultipleProviders(
                    context,
                    rest,
                    restInfo,
                    context.Options.BeaconWidth,
                    context.Options.BeaconHeight,
                    recipients,
                    covered,
                    coveredToCandidates,
                    candidateToInfo);
                w.WriteString("prevent_last", "ok");
            }
            catch (FactorioToolsException ex)
            {
                w.WriteString("prevent_last", ex.Message);
            }
        }
    }

    /// <summary>
    /// The three RemoveOverlappingCandidates overloads, each on its own fresh copy of the beacon
    /// candidates, at the first few candidate spots in turn.
    /// </summary>
    private static void SimulateOverlap(Utf8JsonWriter w, Snapshot s, Mutation m)
    {
        var (context, _, candidateToInfo, covered, _) = RunCandidates(s, m, "beacon", new PlainFactory(), removeUnused: false);
        var centers = candidateToInfo.Keys.Take(4).ToList();
        var width = context.Options.BeaconWidth;
        var height = context.Options.BeaconHeight;

        ILocationDictionary<CandidateInfo> Copy()
        {
            var copy = context.GetLocationDictionary<CandidateInfo>();
            foreach (var (location, info) in candidateToInfo.EnumeratePairs())
            {
                copy.Add(location, info);
            }

            return copy;
        }

        w.WriteStartArray("overlap");

        var one = Copy();
        var two = Copy();
        var twoCovered = GetCoveredToCandidates(context, two, covered);
        var three = Copy();
        var threeScoped = Copy();
        var four = Copy();
        var fourScoped = Copy();
        var fourCovered = GetCoveredToCandidates(context, four, covered);
        foreach (var center in centers)
        {
            RemoveOverlappingCandidates(context.Grid, center, width, height, one);
            RemoveOverlappingCandidates(context.Grid, center, width, height, two, twoCovered);
            RemoveOverlappingCandidates(context.Grid, center, width, height, three, threeScoped, null!);
            RemoveOverlappingCandidates(context.Grid, center, width, height, four, fourScoped, fourCovered);

            w.WriteStartArray();
            w.WriteNumberValue(center.X);
            w.WriteNumberValue(center.Y);
            w.WriteStringValue(HashKeys(one));
            w.WriteStringValue(HashKeys(two));
            w.WriteStringValue(HashCoveredToCandidates(twoCovered));
            w.WriteStringValue(HashKeys(three));
            w.WriteStringValue(HashKeys(threeScoped));
            w.WriteStringValue(HashKeys(four));
            w.WriteStringValue(HashKeys(fourScoped));
            w.WriteStringValue(HashCoveredToCandidates(fourCovered));
            w.WriteEndArray();
        }

        w.WriteEndArray();
    }

    private static void RecordMutateGrid(Stream stream, int id, Snapshot s, string action, Location center, int width, int height, List<Location>? centers)
    {
        Case(
            stream,
            "mutate_grid",
            id,
            w =>
            {
                w.WriteString("action", action);
                WritePoint(w, "center", center);
                w.WriteStartArray("size");
                w.WriteNumberValue(width);
                w.WriteNumberValue(height);
                w.WriteEndArray();
                if (centers is not null)
                {
                    WriteLocations(w, "centers", centers);
                }
            },
            w =>
            {
                var context = Rebuild(s);
                var grid = context.Grid;
                try
                {
                    switch (action)
                    {
                        case "remove_entity":
                            RemoveEntity(grid, center, width, height);
                            break;
                        case "remove_pumpjack":
                            RemovePumpjack(grid, center);
                            break;
                        case "add_pole":
                            AddProviderToGrid(
                                grid,
                                center,
                                new ElectricPoleCenter(grid.GetId()),
                                c => new ElectricPoleSide(grid.GetId(), c),
                                context.Options.ElectricPoleWidth,
                                context.Options.ElectricPoleHeight);
                            break;
                        case "add_beacons":
                            AddBeaconsToGrid(grid, context.Options, centers!);
                            break;
                    }
                }
                catch (FactorioToolsException ex)
                {
                    w.WriteString("error", ex.Message);
                }

                w.WriteNumber("count", grid.EntityLocations.Count);
                w.WriteString("grid", HashGrid(grid));
            });
    }

    private static void RecordFit(Stream stream, int id, Snapshot s)
    {
        foreach (var (width, height) in new[] { (s.PoleWidth, s.PoleHeight), (s.BeaconWidth, s.BeaconHeight) })
        {
            Case(
                stream,
                "fit",
                id,
                w =>
                {
                    w.WriteStartArray("size");
                    w.WriteNumberValue(width);
                    w.WriteNumberValue(height);
                    w.WriteEndArray();
                },
                w =>
                {
                    var context = Rebuild(s);
                    var grid = context.Grid;
                    var fit = new System.Text.StringBuilder();
                    var bounds = new System.Text.StringBuilder();
                    for (var y = 0; y < grid.Height; y++)
                    {
                        for (var x = 0; x < grid.Width; x++)
                        {
                            var l = new Location(x, y);
                            var inside = x - ((width - 1) / 2) >= 0 && y - ((height - 1) / 2) >= 0 && x + (width / 2) < grid.Width && y + (height / 2) < grid.Height;
                            fit.Append(!inside ? '-' : DoesProviderFit(grid, width, height, l) ? '1' : '0');
                            bounds.Append(IsProviderInBounds(grid, width, height, l) ? '1' : '0');
                        }
                    }

                    w.WriteString("fit", fit.ToString());
                    w.WriteString("bounds", bounds.ToString());
                });
        }
    }

    private static void RecordValidate(Stream stream, int id, Snapshot s, Mutation m, string check, Action<Utf8JsonWriter> inputs)
    {
        // The inputs are read back from the JSON the Rust test sees, so the same values drive
        // both sides.
        var inputBuffer = new MemoryStream();
        using (var iw = new Utf8JsonWriter(inputBuffer))
        {
            iw.WriteStartObject();
            inputs(iw);
            iw.WriteEndObject();
        }

        var input = JsonDocument.Parse(inputBuffer.ToArray()).RootElement;

        Case(
            stream,
            "validate",
            id,
            w =>
            {
                m.Write(w);
                w.WriteString("check", check);
                inputs(w);
            },
            w =>
            {
                var context = Rebuild(s);
                m.Apply(context);
                var grid = context.Grid;
                var pipes = CellsOf(grid, e => e is Pipe);
                switch (check)
                {
                    case "pipes_connected":
                    {
                        var drop = input.GetProperty("drop").GetInt32();
                        var set = context.GetLocationSet(allowEnumerate: true);
                        for (var i = 0; i < pipes.Count; i++)
                        {
                            if (i != drop)
                            {
                                set.Add(pipes[i]);
                            }
                        }

                        WriteResult(w, () => Validate.PipesAreConnected(context, set));
                        break;
                    }
                    case "underground_are_pipes":
                    {
                        var drop = input.GetProperty("drop").GetInt32();
                        var undergrounds = CellsOf(grid, e => e is UndergroundPipe);
                        var directions = context.GetLocationDictionary<Direction>();
                        foreach (var u in undergrounds)
                        {
                            directions.Add(u, ((UndergroundPipe)grid[u]!).Direction);
                        }

                        var set = context.GetLocationSet(allowEnumerate: true);
                        foreach (var p in pipes)
                        {
                            if (drop < 0 || p != undergrounds[drop])
                            {
                                set.Add(p);
                            }
                        }

                        WriteResult(w, () => Validate.UndergroundPipesArePipes(context, set, directions));
                        break;
                    }
                    case "pipes_do_not_match":
                    {
                        var variant = input.GetProperty("variant").GetString();
                        var bits = variant == "bit_same";
                        var one = context.GetLocationSet(allowEnumerate: !bits);
                        var two = context.GetLocationSet(allowEnumerate: !bits);
                        foreach (var p in pipes)
                        {
                            one.Add(p);
                            two.Add(p);
                        }

                        if (variant == "minus_one" && pipes.Count > 0)
                        {
                            two.Remove(pipes[0]);
                        }

                        WriteResult(w, () => Validate.PipesDoNotMatch(context, one, two));
                        break;
                    }
                    case "beacons_do_not_overlap":
                    {
                        var beacons = CellsOf(grid, e => e is BeaconCenter);
                        var solutions = new List<BeaconSolution> { new(BeaconStrategy.Snug, beacons, 0, new List<int>()) };
                        WriteResult(w, () => Validate.BeaconsDoNotOverlap(context, solutions));
                        break;
                    }
                    case "no_existing_beacons":
                    {
                        var existing = context.GetLocationDictionary<BeaconCenter>();
                        if (input.GetProperty("variant").GetString() == "grid")
                        {
                            foreach (var b in CellsOf(grid, e => e is BeaconCenter))
                            {
                                existing.Add(b, (BeaconCenter)grid[b]!);
                            }
                        }

                        WriteResult(w, () => Validate.NoExistingBeacons(context, existing));
                        break;
                    }
                    case "unheated":
                    {
                        Validate.CountUnheatedTargets(context, out var pumpjacks, out var unheatedPipes);
                        w.WriteStartArray("counts");
                        w.WriteNumberValue(pumpjacks);
                        w.WriteNumberValue(unheatedPipes);
                        w.WriteEndArray();
                        break;
                    }
                    case "no_unheated_beacons":
                        WriteResult(w, () => Validate.NoUnheatedBeacons(context));
                        break;
                    case "heat_connected":
                        WriteResult(w, () => Validate.HeatPipesAreConnected(context));
                        break;
                    case "all_power":
                        WriteResult(w, () => Validate.AllEntitiesHavePower(context));
                        break;
                    default:
                        throw new NotSupportedException(check);
                }
            });
    }

    /// <summary>A breadth-first tree over empty cells, as A* and Dijkstra record it, and the
    /// path GetPath reads back from its last cell.</summary>
    private static void RecordGetPath(Stream stream, int id, Snapshot s)
    {
        var context = Rebuild(s);
        var grid = context.Grid;
        Location? start = null;
        for (var y = 0; y < grid.Height && start is null; y++)
        {
            for (var x = 0; x < grid.Width && start is null; x++)
            {
                if (grid.IsEmpty(new Location(x, y)))
                {
                    start = new Location(x, y);
                }
            }
        }

        if (start is null)
        {
            return;
        }

        var cameFrom = context.GetLocationDictionary<Location>();
        cameFrom.Add(start.Value, start.Value);
        var queue = new Queue<Location>();
        queue.Enqueue(start.Value);
        var last = start.Value;
        Span<Location> adjacent = new Location[4];
        while (queue.Count > 0 && cameFrom.Count < 150)
        {
            var current = queue.Dequeue();
            last = current;
            grid.GetAdjacent(adjacent, current);
            foreach (var next in adjacent)
            {
                if (next.IsValid && grid.IsEmpty(next) && !cameFrom.ContainsKey(next))
                {
                    cameFrom.Add(next, current);
                    queue.Enqueue(next);
                }
            }
        }

        Case(
            stream,
            "path",
            null,
            w =>
            {
                w.WriteStartArray("came_from");
                foreach (var (location, from) in cameFrom.EnumeratePairs())
                {
                    w.WriteNumberValue(location.X);
                    w.WriteNumberValue(location.Y);
                    w.WriteNumberValue(from.X);
                    w.WriteNumberValue(from.Y);
                }

                w.WriteEndArray();
                WritePoint(w, "start", start.Value);
                WritePoint(w, "goal", last);
            },
            w => WriteLocations(w, "path", GetPath(cameFrom, start.Value, last)));
    }

    private static void RecordLinesOnEmpty(Stream stream, int id, Snapshot s, List<Location> pipes)
    {
        var context = Rebuild(s);
        var grid = context.Grid;
        var pairs = new List<(Location, Location)>();

        // Straight runs between terminal spots, which may cross entities, and runs from each
        // pipe to the grid edge.
        var terminals = s.LocationToTerminals.Select(p => p.Key).ToList();
        for (var i = 0; i < terminals.Count && pairs.Count < 12; i++)
        {
            for (var j = i + 1; j < terminals.Count && pairs.Count < 12; j++)
            {
                if (terminals[i].X == terminals[j].X || terminals[i].Y == terminals[j].Y)
                {
                    pairs.Add((terminals[i], terminals[j]));
                }
            }
        }

        foreach (var p in pipes.Take(4))
        {
            pairs.Add((new Location(p.X, 0), new Location(p.X, grid.Height - 1)));
            pairs.Add((new Location(0, p.Y), p));
        }

        if (terminals.Count >= 2)
        {
            pairs.Add((new Location(0, 0), new Location(1, 1)));
        }

        foreach (var (a, b) in pairs)
        {
            Case(
                stream,
                "line_on_empty",
                id,
                w =>
                {
                    WritePoint(w, "a", a);
                    WritePoint(w, "b", b);
                },
                w =>
                {
                    try
                    {
                        var line = MakeStraightLineOnEmpty(Rebuild(s).Grid, a, b);
                        if (line is null)
                        {
                            w.WriteBoolean("none", true);
                        }
                        else
                        {
                            WriteLocations(w, "points", line);
                        }
                    }
                    catch (ArgumentException ex)
                    {
                        w.WriteString("error", $"ArgumentException: {ex.Message}");
                    }
                });
        }
    }

    private static void RecordLines(Stream stream, IReadOnlyList<Location> points, bool sort)
    {
        Case(
            stream,
            "lines",
            null,
            w =>
            {
                WriteLocations(w, "points", points);
                w.WriteBoolean("sort", sort);
            },
            w =>
            {
                try
                {
                    var lines = PointsToLines(points, sort);
                    w.WriteStartArray("lines");
                    foreach (var line in lines)
                    {
                        w.WriteNumberValue(line.A.X);
                        w.WriteNumberValue(line.A.Y);
                        w.WriteNumberValue(line.B.X);
                        w.WriteNumberValue(line.B.Y);
                    }

                    w.WriteEndArray();
                }
                catch (Exception ex) when (ex is not FactorioToolsException)
                {
                    w.WriteString("error_type", ex.GetType().Name);
                    w.WriteString("error", ex.Message);
                }
            });
    }

    private static void RecordTurns(Stream stream, IReadOnlyList<Location> points)
    {
        Case(stream, "turns", null, w => WriteLocations(w, "points", points), w => w.WriteNumber("result", CountTurns(points.ToList())));
        Case(stream, "collinear", null, w => WriteLocations(w, "points", points), w => w.WriteBoolean("result", AreLocationsCollinear(points)));
    }

    // ---------------------------------------------------------------------------------------
    // Cases on the states before poles and before beacons

    private static void RecordNoPoles(Stream stream, int id, Snapshot s, bool full)
    {
        RecordCandidates(stream, id, s, None, "pole", "pole", removeUnused: true, full, simulate: true);
        RecordCandidates(stream, id, s, None, "pole", "plain", removeUnused: false, full, simulate: false);
    }

    private static void RecordNoBeacons(Stream stream, int id, Snapshot s, bool full)
    {
        RecordCandidates(stream, id, s, None, "beacon", "plain", removeUnused: false, full, simulate: true);
        RecordCandidates(stream, id, s, None, "pole", "pole", removeUnused: true, full, simulate: false);
        RecordBeaconCoverage(stream, id, s, None);
    }

    // ---------------------------------------------------------------------------------------
    // Cases on a starting context

    private static void RecordInitial(Stream stream, int id, Snapshot s)
    {
        Case(stream, "terminals", id, _ => { }, w =>
        {
            var context = Rebuild(s);
            var c2t = GetCenterToTerminals(context, context.Grid, context.Centers);
            var l2t = GetLocationToTerminals(context, c2t);
            w.WriteString("c2t", HashTerminals(c2t));
            w.WriteString("l2t", HashTerminals(l2t));

            // EliminateOtherTerminals, choosing each pumpjack's last option in turn on a fresh
            // context.
            w.WriteStartArray("eliminate");
            foreach (var center in s.Centers.Take(6))
            {
                var fresh = Rebuild(s);
                var options = fresh.CenterToTerminals[center];
                var selected = options[options.Count - 1];
                EliminateOtherTerminals(fresh, selected);
                w.WriteStartArray();
                w.WriteNumberValue(center.X);
                w.WriteNumberValue(center.Y);
                w.WriteStringValue(HashTerminals(fresh.CenterToTerminals));
                w.WriteStringValue(HashTerminals(fresh.LocationToTerminals));
                w.WriteEndArray();
            }

            w.WriteEndArray();
        });

        RecordLines(stream, s.Centers, sort: true);
        RecordLines(stream, s.Centers, sort: false);
        for (var n = 0; n <= Math.Min(4, s.Centers.Count); n++)
        {
            RecordLines(stream, s.Centers.Take(n).ToList(), sort: true);
        }

        RecordTurns(stream, s.Centers);
        RecordTurns(stream, s.Centers.Take(3).ToList());
        foreach (var t in s.LocationToTerminals.Take(6))
        {
            var a = t.Key;
            RecordStraightLine(stream, a, new Location(a.X, a.Y + 5));
            RecordStraightLine(stream, a, new Location(a.X - 3, a.Y));
            RecordStraightLine(stream, a, new Location(a.X + 1, a.Y + 1));
        }
    }

    private static void RecordStraightLine(Stream stream, Location a, Location b)
    {
        Case(
            stream,
            "line",
            null,
            w =>
            {
                WritePoint(w, "a", a);
                WritePoint(w, "b", b);
            },
            w =>
            {
                try
                {
                    WriteLocations(w, "points", MakeStraightLine(a, b));
                }
                catch (ArgumentException ex)
                {
                    w.WriteString("error", $"ArgumentException: {ex.Message}");
                }
            });
    }

    // ---------------------------------------------------------------------------------------
    // Synthetic geometry

    private static void RecordGeometry(Stream stream)
    {
        var random = new Random(4);
        var sets = new List<List<Location>>
        {
            new(),
            new() { new Location(5, 5) },
            new() { new Location(5, 5), new Location(5, 5) },
            new() { new Location(5, 5), new Location(5, 5), new Location(5, 5) },
            new() { new Location(0, 0), new Location(1, 1), new Location(2, 2), new Location(3, 3) },
            new() { new Location(0, 0), new Location(0, 4), new Location(0, 2) },
            new() { new Location(2, 0), new Location(0, 0), new Location(1, 0), new Location(1, 1) },
            new() { new Location(0, 0), new Location(2, 1), new Location(4, 2), new Location(6, 4) },
        };

        for (var i = 0; i < 40; i++)
        {
            var count = random.Next(3, 14);
            var set = new List<Location>();
            for (var j = 0; j < count; j++)
            {
                set.Add(new Location(random.Next(0, 12), random.Next(0, 12)));
            }

            sets.Add(set);
        }

        foreach (var set in sets)
        {
            RecordLines(stream, set, sort: true);
            RecordLines(stream, set, sort: false);
            RecordTurns(stream, set);
        }
    }
}
