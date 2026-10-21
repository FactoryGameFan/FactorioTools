// Records calls to the planner's graph and line algorithms (stage 2 of #138):
// A*, Dijkstra, breadth-first search, Prim's MST and Bresenham's line.
// Format: rust/planner/FIXTURES.md, "algorithms.jsonl".

using System.Text.Encodings.Web;
using System.Text.Json;
using DelaunatorSharp;
using Knapcode.FactorioTools.OilField;

static class AlgorithmCases
{
    /// <summary>Every Nth big-list blueprint is recorded too, to keep the file small.</summary>
    private const int BigListStride = 20;

    /// <summary>Full LocationToPrevious contents are written for this many Dijkstra calls per
    /// blueprint; the rest carry only a count and a hash of the same contents.</summary>
    private const int FullPreviousPerBlueprint = 1;

    private static readonly JsonWriterOptions WriterOptions = new()
    {
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    private static readonly (bool PreferNoTurns, int XWeight, int YWeight)[] Weights =
    {
        (true, 1, 1),
        (false, 1, 1),
        (true, 2, 1),
        (true, 1, 2),
    };

    public static void Write(string outputDir, IReadOnlyList<string> smallList, IReadOnlyList<string> bigList)
    {
        var path = Path.Combine(outputDir, "algorithms.jsonl");
        var counts = new SortedDictionary<string, int>(StringComparer.Ordinal);
        using (var stream = File.Create(path))
        {
            for (var i = 0; i < smallList.Count; i++)
            {
                WriteBlueprint(stream, $"small-{i:D4}", smallList[i], seed: i, withPlan: true, counts);
            }

            for (var i = 0; i < bigList.Count; i += BigListStride)
            {
                WriteBlueprint(stream, $"big-{i:D4}", bigList[i], seed: 100_000 + i, withPlan: false, counts);
            }

            var bresenham = new Random(424242);
            for (var i = 0; i < 400; i++)
            {
                var a = new Location(bresenham.Next(-5, 60), bresenham.Next(-5, 60));
                var b = new Location(bresenham.Next(-5, 60), bresenham.Next(-5, 60));
                WriteLine(stream, w =>
                {
                    w.WriteString("kind", "bresenham");
                    WritePoint(w, "a", a);
                    WritePoint(w, "b", b);
                    WritePoints(w, "path", BresenhamsLine.GetPath(a, b));
                });
                Count(counts, "bresenham");
            }
        }

        Console.WriteLine($"algorithms.jsonl: {new FileInfo(path).Length} bytes; calls: " + string.Join(", ", counts.Select(x => $"{x.Key}={x.Value}")));
    }

    private static void WriteBlueprint(Stream stream, string id, string blueprintString, int seed, bool withPlan, IDictionary<string, int> counts)
    {
        var random = new Random(seed);
        var blueprint = ParseBlueprint.Execute(blueprintString);
        var context = InitializeContext.Execute(OilFieldOptions.ForMediumElectricPole, blueprint, Array.Empty<AvoidLocation>());

        WriteLine(stream, w =>
        {
            w.WriteString("kind", "grid");
            w.WriteString("id", id);
            w.WriteNumber("width", context.Grid.Width);
            w.WriteNumber("height", context.Grid.Height);
            WritePoints(w, "occupied", context.Grid.EntityLocations.EnumerateItems());
        });

        var terminals = context.CenterToTerminals.Values
            .SelectMany(list => list.Select(t => t.Terminal))
            .Distinct()
            .ToList();
        var occupied = context.Centers[0];
        var previousWritten = 0;

        // A* on the initial grid: varied weights, one goal set per call.
        for (var k = 0; k < 8; k++)
        {
            var start = terminals[random.Next(terminals.Count)];
            var goals = RandomGoals(context, random, terminals, start, maxGoals: 6);
            var (preferNoTurns, xWeight, yWeight) = Weights[k % Weights.Length];
            WriteAStar(stream, context, context.Grid, "grid", id, start, goals, "hash", preferNoTurns, xWeight, yWeight, prefix: null);
            Count(counts, "astar");
        }

        // A* edge cases: the start is a goal, a single-location goal set, an unreachable goal
        // (a pumpjack center, which is never empty), and a caller's list to append to.
        {
            var start = terminals[0];
            var goals = context.GetLocationSet(allowEnumerate: true);
            goals.Add(start);
            goals.Add(terminals[^1]);
            WriteAStar(stream, context, context.Grid, "grid", id, start, goals, "hash", true, 1, 1, prefix: null);

            var single = context.GetSingleLocationSet(terminals[^1]);
            WriteAStar(stream, context, context.Grid, "grid", id, start, single, "single", true, 1, 1, prefix: null);

            var unreachable = context.GetLocationSet(allowEnumerate: true);
            unreachable.Add(occupied);
            WriteAStar(stream, context, context.Grid, "grid", id, start, unreachable, "hash", true, 1, 1, prefix: new List<Location> { new(-5, -5) });

            var withPrefix = RandomGoals(context, random, terminals, start, maxGoals: 3);
            WriteAStar(stream, context, context.Grid, "grid", id, start, withPrefix, "hash", true, 2, 1, prefix: new List<Location> { new(-5, -5), new(-6, -6) });
            Count(counts, "astar", 4);
        }

        // Dijkstra on the initial grid.
        foreach (var (stopOnFirstGoal, allowGoalEnumerate) in new[] { (false, true), (true, true), (false, false) })
        {
            var start = terminals[random.Next(terminals.Count)];
            var goals = RandomGoals(context, random, terminals, start, maxGoals: 6);
            WriteDijkstra(stream, context, context.Grid, "grid", id, start, goals, stopOnFirstGoal, allowGoalEnumerate, previousWritten++ < FullPreviousPerBlueprint);
            Count(counts, "dijkstra");
        }

        // Breadth-first search, which always walks context.Grid.
        for (var k = 0; k < 4; k++)
        {
            var start = terminals[random.Next(terminals.Count)];
            var goal = terminals[random.Next(terminals.Count)];
            WriteBfs(stream, context, id, start, goal);
        }

        WriteBfs(stream, context, id, terminals[0], terminals[0]);
        WriteBfs(stream, context, id, terminals[0], occupied);
        Count(counts, "bfs", 6);

        // Prim's MST on the graph AddPipesConnectedCentersDT.ExecuteWithDelaunayMst builds, and
        // on a complete graph over a few centers with small costs, so ties are common.
        var delaunayGraph = BuildDelaunayGraph(context);
        if (delaunayGraph is not null)
        {
            var first = FirstMinByDistanceToMiddle(context);
            var mst = WritePrims(stream, context, delaunayGraph, first, digraph: false);
            WritePrims(stream, context, delaunayGraph, first, digraph: true);
            CheckMatchesPlanner(context, mst, id);
            Count(counts, "prims", 2);
        }

        var someCenters = context.Centers.Take(8).ToList();
        if (someCenters.Count >= 2)
        {
            var complete = context.GetLocationDictionary<ILocationDictionary<int>>(someCenters.Count);
            foreach (var center in someCenters)
            {
                complete.Add(center, context.GetLocationDictionary<int>());
            }

            for (var a = 0; a < someCenters.Count; a++)
            {
                for (var b = a + 1; b < someCenters.Count; b++)
                {
                    var cost = random.Next(1, 4);
                    complete[someCenters[a]][someCenters[b]] = cost;
                    complete[someCenters[b]][someCenters[a]] = cost;
                }
            }

            WritePrims(stream, context, complete, someCenters[random.Next(someCenters.Count)], digraph: false);
            WritePrims(stream, context, complete, someCenters[0], digraph: true);
            Count(counts, "prims", 2);
        }

        if (!withPlan)
        {
            return;
        }

        // The finished plan's pipes, walked through an ExistingPipeGrid as RotateOptimize and
        // Validate do.
        var plan = Planner.Execute(OilFieldOptions.ForMediumElectricPole, ParseBlueprint.Execute(blueprintString));
        var planContext = plan.Context;
        var pipes = planContext.GetLocationSet(allowEnumerate: true);
        var planTerminals = new List<Location>();
        foreach (var location in planContext.Grid.EntityLocations.EnumerateItems())
        {
            var entity = planContext.Grid[location];
            if (entity is Pipe)
            {
                pipes.Add(location);
            }

            if (entity is Terminal)
            {
                planTerminals.Add(location);
            }
        }

        var pipesId = id + "-pipes";
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "pipes");
            w.WriteString("id", pipesId);
            w.WriteNumber("width", planContext.Grid.Width);
            w.WriteNumber("height", planContext.Grid.Height);
            WritePoints(w, "pipes", pipes.EnumerateItems());
        });

        var existing = new ExistingPipeGrid(planContext.Grid, pipes);
        var pipeList = pipes.EnumerateItems().ToList();
        for (var k = 0; k < 4; k++)
        {
            var start = pipeList[random.Next(pipeList.Count)];
            var goals = RandomGoals(planContext, random, planTerminals, start, maxGoals: 4);
            var (preferNoTurns, xWeight, yWeight) = Weights[k % Weights.Length];
            WriteAStar(stream, planContext, existing, "pipes", pipesId, start, goals, "hash", preferNoTurns, xWeight, yWeight, prefix: null);
            Count(counts, "astar_existing");
        }

        foreach (var stopOnFirstGoal in new[] { false, true })
        {
            var goals = planContext.GetLocationSet(allowEnumerate: true);
            foreach (var terminal in planTerminals)
            {
                goals.Add(terminal);
            }

            var start = goals.EnumerateItems().First();
            goals.Remove(start);
            WriteDijkstra(stream, planContext, existing, "pipes", pipesId, start, goals, stopOnFirstGoal, allowGoalEnumerate: true, writeFullPrevious: false);
            Count(counts, "dijkstra_existing");
        }
    }

    private static ILocationSet RandomGoals(Context context, Random random, List<Location> candidates, Location start, int maxGoals)
    {
        var goals = context.GetLocationSet(allowEnumerate: true);
        var count = 1 + random.Next(maxGoals);
        for (var i = 0; i < count; i++)
        {
            var goal = candidates[random.Next(candidates.Count)];
            if (goal != start)
            {
                goals.Add(goal);
            }
        }

        if (goals.Count == 0)
        {
            // Every pick was the start (a one-pumpjack field has few candidates). Fall back to
            // any other candidate, or to the start itself, which A* answers immediately.
            goals.Add(candidates.FirstOrDefault(c => c != start, start));
        }

        return goals;
    }

    private static void WriteAStar(Stream stream, Context context, SquareGrid grid, string gridKind, string gridId, Location start, ILocationSet goals, string goalsKind, bool preferNoTurns, int xWeight, int yWeight, List<Location>? prefix)
    {
        var goalItems = goals.EnumerateItems().ToList();
        var outputList = prefix is null ? null : new List<Location>(prefix);
        var result = AStar.GetShortestPath(context, grid, start, goals, preferNoTurns, xWeight, yWeight, outputList);
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "astar");
            w.WriteString(gridKind, gridId);
            WritePoint(w, "start", start);
            WritePoints(w, "goals", goalItems);
            w.WriteString("goalsKind", goalsKind);
            w.WriteBoolean("preferNoTurns", preferNoTurns);
            w.WriteNumber("xWeight", xWeight);
            w.WriteNumber("yWeight", yWeight);
            if (prefix is null)
            {
                w.WriteNull("prefix");
            }
            else
            {
                WritePoints(w, "prefix", prefix);
            }

            w.WriteBoolean("success", result.Success);
            WritePoint(w, "reached", result.ReachedGoal);
            if (result.Success)
            {
                WritePoints(w, "path", result.Path);
            }
            else
            {
                w.WriteNull("path");
            }
        });
    }

    private static void WriteDijkstra(Stream stream, Context context, SquareGrid grid, string gridKind, string gridId, Location start, ILocationSet goals, bool stopOnFirstGoal, bool allowGoalEnumerate, bool writeFullPrevious)
    {
        var goalItems = goals.EnumerateItems().ToList();
        var result = Dijkstras.GetShortestPaths(context, grid, start, goals, stopOnFirstGoal, allowGoalEnumerate);

        // LocationToPrevious values are LocationBitSets, which cannot be enumerated. Every
        // previous cell is a grid neighbor, so probe the four in-bounds neighbors instead and
        // check that accounts for the whole count.
        var previous = new List<int>();
        foreach (var location in result.LocationToPrevious.Keys)
        {
            var set = result.LocationToPrevious[location];
            var mask = 0;
            var neighbors = new[]
            {
                location.Translate(1, 0),
                location.Translate(0, -1),
                location.Translate(-1, 0),
                location.Translate(0, 1),
            };
            for (var i = 0; i < neighbors.Length; i++)
            {
                var n = neighbors[i];
                if (n.X >= 0 && n.Y >= 0 && n.X < context.Grid.Width && n.Y < context.Grid.Height && set.Contains(n))
                {
                    mask |= 1 << i;
                }
            }

            if (System.Numerics.BitOperations.PopCount((uint)mask) != set.Count)
            {
                throw new InvalidOperationException($"A LocationToPrevious set at {location} in {gridId} holds a cell that is not a neighbor.");
            }

            previous.Add(location.X);
            previous.Add(location.Y);
            previous.Add(mask);
        }

        WriteLine(stream, w =>
        {
            w.WriteString("kind", "dijkstra");
            w.WriteString(gridKind, gridId);
            WritePoint(w, "start", start);
            WritePoints(w, "goals", goalItems);
            w.WriteBoolean("stopOnFirstGoal", stopOnFirstGoal);
            w.WriteBoolean("allowGoalEnumerate", allowGoalEnumerate);
            w.WriteNumber("reachedCount", result.ReachedGoals.Count);
            if (allowGoalEnumerate)
            {
                WritePoints(w, "reached", result.ReachedGoals.EnumerateItems());
            }
            else
            {
                w.WriteStartArray("reachedContains");
                foreach (var goal in goalItems)
                {
                    w.WriteBooleanValue(result.ReachedGoals.Contains(goal));
                }

                w.WriteEndArray();
            }

            w.WriteNumber("previousCount", result.LocationToPrevious.Count);
            w.WriteNumber("previousHash", Fnv(previous));
            if (writeFullPrevious)
            {
                w.WriteStartArray("previous");
                foreach (var value in previous)
                {
                    w.WriteNumberValue(value);
                }

                w.WriteEndArray();
            }
        });
    }

    private static void WriteBfs(Stream stream, Context context, string gridId, Location start, Location goal)
    {
        var path = BreadthFirstFinder.GetShortestPath(context, start, goal);
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "bfs");
            w.WriteString("grid", gridId);
            WritePoint(w, "start", start);
            WritePoint(w, "goal", goal);
            if (path is null)
            {
                w.WriteNull("path");
            }
            else
            {
                WritePoints(w, "path", path);
            }
        });
    }

    private static ILocationDictionary<ILocationSet> WritePrims(Stream stream, Context context, ILocationDictionary<ILocationDictionary<int>> graph, Location first, bool digraph)
    {
        var mst = Prims.GetMinimumSpanningTree(context, graph, first, digraph);
        WriteLine(stream, w =>
        {
            w.WriteString("kind", "prims");
            w.WriteStartArray("graph");
            foreach (var pair in graph.EnumeratePairs())
            {
                w.WriteStartArray();
                w.WriteNumberValue(pair.Key.X);
                w.WriteNumberValue(pair.Key.Y);
                w.WriteStartArray();
                foreach (var edge in pair.Value.EnumeratePairs())
                {
                    w.WriteNumberValue(edge.Key.X);
                    w.WriteNumberValue(edge.Key.Y);
                    w.WriteNumberValue(edge.Value);
                }

                w.WriteEndArray();
                w.WriteEndArray();
            }

            w.WriteEndArray();
            WritePoint(w, "first", first);
            w.WriteBoolean("digraph", digraph);
            w.WriteStartArray("mst");
            foreach (var pair in mst.EnumeratePairs())
            {
                w.WriteStartArray();
                w.WriteNumberValue(pair.Key.X);
                w.WriteNumberValue(pair.Key.Y);
                w.WriteStartArray();
                foreach (var node in pair.Value.EnumerateItems())
                {
                    w.WriteNumberValue(node.X);
                    w.WriteNumberValue(node.Y);
                }

                w.WriteEndArray();
                w.WriteEndArray();
            }

            w.WriteEndArray();
        });

        return mst;
    }

    /// <summary>
    /// The graph ExecuteWithDelaunayMst builds, copied step for step (CollectionExtensions is
    /// internal). Null when Delaunator cannot triangulate the centers.
    /// </summary>
    private static ILocationDictionary<ILocationDictionary<int>>? BuildDelaunayGraph(Context context)
    {
        var centers = context.Centers;
        var points = new IPoint[centers.Count];
        for (var i = 0; i < centers.Count; i++)
        {
            points[i] = new Point(centers[i].X, centers[i].Y);
        }

        Delaunator delaunator;
        try
        {
            delaunator = new Delaunator(points);
        }
        catch (Exception)
        {
            return null;
        }

        var graph = context.GetLocationDictionary<ILocationDictionary<int>>(centers.Count);
        foreach (var center in centers)
        {
            graph.Add(center, context.GetLocationDictionary<int>());
        }

        for (var e = 0; e < delaunator.Triangles.Length; e++)
        {
            if (e > delaunator.Halfedges[e])
            {
                var p = centers[delaunator.Triangles[e]];
                var q = centers[delaunator.Triangles[e % 3 == 2 ? e - 2 : e + 1]];
                var cost = p.GetEuclideanDistanceSquared(q);
                graph[p][q] = cost;
                graph[q][p] = cost;
            }
        }

        return graph;
    }

    /// <summary>CollectionExtensions.MinBy: the first of equal minimums wins.</summary>
    private static Location FirstMinByDistanceToMiddle(Context context)
    {
        var middle = context.Grid.Middle;
        var best = context.Centers[0];
        var bestKey = middle.GetEuclideanDistanceSquared(best);
        foreach (var center in context.Centers.Skip(1))
        {
            var key = middle.GetEuclideanDistanceSquared(center);
            if (key < bestKey)
            {
                best = center;
                bestKey = key;
            }
        }

        return best;
    }

    /// <summary>Guards the copied graph-building above against drifting from the planner's.</summary>
    private static void CheckMatchesPlanner(Context context, ILocationDictionary<ILocationSet> mst, string id)
    {
        var planner = AddPipesConnectedCentersDT.ExecuteWithDelaunayMst(context, context.Centers);
        var a = string.Join(";", mst.EnumeratePairs().Select(p => $"{p.Key.X},{p.Key.Y}:" + string.Join(" ", p.Value.EnumerateItems().Select(l => $"{l.X},{l.Y}"))));
        var b = string.Join(";", planner.EnumeratePairs().Select(p => $"{p.Key.X},{p.Key.Y}:" + string.Join(" ", p.Value.EnumerateItems().Select(l => $"{l.X},{l.Y}"))));
        if (a != b)
        {
            throw new InvalidOperationException($"The recorded Delaunay MST for {id} does not match AddPipesConnectedCentersDT.ExecuteWithDelaunayMst.");
        }
    }

    /// <summary>FNV-1a over the little-endian bytes of each int.</summary>
    private static ulong Fnv(List<int> values)
    {
        var hash = 14695981039346656037UL;
        foreach (var value in values)
        {
            var v = unchecked((uint)value);
            for (var b = 0; b < 4; b++)
            {
                hash ^= (v >> (8 * b)) & 0xFF;
                hash *= 1099511628211UL;
            }
        }

        return hash;
    }

    private static void Count(IDictionary<string, int> counts, string kind, int n = 1)
    {
        counts[kind] = counts.TryGetValue(kind, out var c) ? c + n : n;
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

    private static void WritePoint(Utf8JsonWriter w, string name, Location location)
    {
        w.WriteStartArray(name);
        w.WriteNumberValue(location.X);
        w.WriteNumberValue(location.Y);
        w.WriteEndArray();
    }

    /// <summary>Points as one flat array: [x0, y0, x1, y1, ...].</summary>
    private static void WritePoints(Utf8JsonWriter w, string name, IEnumerable<Location> locations)
    {
        w.WriteStartArray(name);
        foreach (var location in locations)
        {
            w.WriteNumberValue(location.X);
            w.WriteNumberValue(location.Y);
        }

        w.WriteEndArray();
    }
}
