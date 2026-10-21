using System.Globalization;
using System.Text;

namespace DotnetCompatOracle;

// Writes the fixtures described in ../FIXTURES.md. Every file uses its own fixed seed,
// so a rerun on the same SDK gives the same bytes.
public static class Program
{
    public static int Main(string[] args)
    {
        if (args.Length != 1)
        {
            Console.Error.WriteLine("usage: DotnetCompatOracle <output directory>");
            return 2;
        }

        var outDir = args[0];
        Directory.CreateDirectory(outDir);

        Write(outDir, "hashset.jsonl", HashSetScenarios.Generate(new Random(1)));
        Write(outDir, "dictionary.jsonl", DictionaryScenarios.Generate(new Random(2)));
        Write(outDir, "priority_queue.jsonl", PriorityQueueScenarios.Generate(new Random(3)));
        Write(outDir, "sort.jsonl", SortScenarios.Generate(new Random(4)));
        Write(outDir, "math.jsonl", MathScenarios.Generate(new Random(5)));
        return 0;
    }

    private static void Write(string outDir, string fileName, IEnumerable<Scenario> scenarios)
    {
        var sb = new StringBuilder();
        var count = 0;
        foreach (var s in scenarios)
        {
            sb.Append(s.ToJson()).Append('\n');
            count++;
        }

        var path = Path.Combine(outDir, fileName);
        File.WriteAllText(path, sb.ToString(), new UTF8Encoding(encoderShouldEmitUTF8Identifier: false));
        Console.WriteLine($"{fileName}: {count} scenarios, {new FileInfo(path).Length} bytes");
    }
}

/// <summary>One fixture line: a name and a list of ops, each a JSON array.</summary>
public sealed class Scenario
{
    private readonly List<string> _ops = new();

    public Scenario(string name)
    {
        Name = name;
    }

    public string Name { get; }

    public void Op(params object[] items)
    {
        var sb = new StringBuilder("[");
        for (var i = 0; i < items.Length; i++)
        {
            if (i > 0)
            {
                sb.Append(',');
            }

            Json.Append(sb, items[i]);
        }

        sb.Append(']');
        _ops.Add(sb.ToString());
    }

    public string ToJson()
    {
        var sb = new StringBuilder();
        sb.Append("{\"name\":");
        Json.Append(sb, Name);
        sb.Append(",\"ops\":[");
        sb.AppendJoin(',', _ops);
        sb.Append("]}");
        return sb.ToString();
    }
}

public static class Json
{
    public static void Append(StringBuilder sb, object? value)
    {
        switch (value)
        {
            case null:
                sb.Append("null");
                break;
            case string s:
                sb.Append('"');
                foreach (var c in s)
                {
                    if (c == '"' || c == '\\')
                    {
                        sb.Append('\\');
                    }

                    sb.Append(c);
                }

                sb.Append('"');
                break;
            case bool b:
                sb.Append(b ? "true" : "false");
                break;
            case int i:
                sb.Append(i.ToString(CultureInfo.InvariantCulture));
                break;
            case double d:
                sb.Append(Double(d));
                break;
            case System.Collections.IEnumerable list:
                sb.Append('[');
                var first = true;
                foreach (var item in list)
                {
                    if (!first)
                    {
                        sb.Append(',');
                    }

                    first = false;
                    Append(sb, item);
                }

                sb.Append(']');
                break;
            default:
                throw new ArgumentException($"Unsupported JSON value {value.GetType()}");
        }
    }

    /// <summary>"R" round-trip text. Always has a '.' or exponent so JSON readers keep it a float.</summary>
    public static string Double(double d)
    {
        if (double.IsNaN(d) || double.IsInfinity(d))
        {
            throw new ArgumentException("Non-finite doubles are not written.");
        }

        var s = d.ToString("R", CultureInfo.InvariantCulture);
        if (!s.Contains('.') && !s.Contains('E'))
        {
            s += ".0";
        }

        return s;
    }
}

/// <summary>The capacities .NET's HashHelpers steps through, plus their neighbors.</summary>
public static class Sizes
{
    public static readonly int[] AroundPrimes =
    {
        1, 2, 3, 4, 5, 6, 7, 8, 10, 11, 12, 16, 17, 18, 22, 23, 24, 29, 30, 36, 37, 38,
        46, 47, 48, 58, 59, 60, 70, 71, 72, 88, 89, 90, 106, 107, 108, 130, 131, 132,
    };

    public static readonly int[] Capacities = { -1, 0, 1, 2, 3, 5, 7, 8, 11, 17, 30, 37, 100 };
}

public static class HashSetScenarios
{
    public static IEnumerable<Scenario> Generate(Random rng)
    {
        foreach (var s in EdgeCases())
        {
            yield return s;
        }

        // Fill, remove some, copy, then add: the copy constructor keeps free slots only
        // when capacities line up, so walk the sizes around each prime step.
        foreach (var n in Sizes.AroundPrimes)
        {
            foreach (var removeKind in new[] { "one", "quarter", "half", "all_but_one", "all" })
            {
                foreach (var capacity in new[] { -1, n * 2 + 1 })
                {
                    yield return FillRemoveCopy(rng, n, removeKind, capacity, copyKind: "copy");
                }
            }

            yield return FillRemoveCopy(rng, n, "half", -1, copyKind: "copy_list");
        }

        // Random operation mixes.
        var keyRanges = new[] { 4, 8, 16, 32, 64, 200 };
        var opCounts = new[] { 10, 30, 80, 200, 300 };
        var index = 0;
        foreach (var keyRange in keyRanges)
        {
            foreach (var opCount in opCounts)
            {
                for (var rep = 0; rep < (opCount >= 200 ? 2 : 4); rep++)
                {
                    var capacity = Sizes.Capacities[rng.Next(Sizes.Capacities.Length)];
                    yield return RandomMix(rng, $"random_{index++}_k{keyRange}_n{opCount}", keyRange, opCount, capacity);
                }
            }
        }
    }

    private static HashSet<int> New(int capacity) => capacity < 0 ? new HashSet<int>() : new HashSet<int>(capacity);

    private static IEnumerable<Scenario> EdgeCases()
    {
        var s = new Scenario("empty");
        var set = New(-1);
        s.Op("new", -1);
        Order(s, set);
        s.Op("copy");
        set = new HashSet<int>(set);
        Order(s, set);
        yield return s;

        s = new Scenario("single");
        set = New(-1);
        s.Op("new", -1);
        s.Op("add", 5, set.Add(5));
        s.Op("add", 5, set.Add(5));
        s.Op("contains", 5, set.Contains(5));
        s.Op("contains", 6, set.Contains(6));
        Order(s, set);
        s.Op("remove", 5, set.Remove(5));
        s.Op("remove", 5, set.Remove(5));
        Order(s, set);
        yield return s;

        s = new Scenario("remove_all_then_add");
        set = New(-1);
        s.Op("new", -1);
        for (var i = 0; i < 10; i++)
        {
            s.Op("add", i, set.Add(i));
        }

        for (var i = 0; i < 10; i++)
        {
            s.Op("remove", i, set.Remove(i));
        }

        Order(s, set);
        for (var i = 20; i < 25; i++)
        {
            s.Op("add", i, set.Add(i));
        }

        Order(s, set);
        yield return s;

        s = new Scenario("clear_then_add");
        set = New(4);
        s.Op("new", 4);
        for (var i = 0; i < 12; i++)
        {
            s.Op("add", i * 3, set.Add(i * 3));
        }

        s.Op("remove", 6, set.Remove(6));
        s.Op("clear");
        set.Clear();
        Order(s, set);
        for (var i = 0; i < 5; i++)
        {
            s.Op("add", 100 - i, set.Add(100 - i));
        }

        Order(s, set);
        yield return s;

        s = new Scenario("copy_with_holes_then_add");
        set = New(-1);
        s.Op("new", -1);
        for (var i = 0; i < 7; i++)
        {
            s.Op("add", i, set.Add(i));
        }

        s.Op("remove", 1, set.Remove(1));
        s.Op("remove", 4, set.Remove(4));
        Order(s, set);
        s.Op("copy");
        set = new HashSet<int>(set);
        Order(s, set);
        s.Op("add", 10, set.Add(10));
        s.Op("add", 11, set.Add(11));
        s.Op("add", 12, set.Add(12));
        Order(s, set);
        yield return s;

        s = new Scenario("union_with_self_and_except_with_self");
        set = New(-1);
        s.Op("new", -1);
        foreach (var k in new[] { 3, 1, 4, 1, 5, 9, 2, 6 })
        {
            s.Op("add", k, set.Add(k));
        }

        s.Op("union", set.ToList());
        set.UnionWith(set.ToList());
        Order(s, set);
        s.Op("except", set.ToList());
        set.ExceptWith(set.ToList());
        Order(s, set);
        yield return s;
    }

    private static Scenario FillRemoveCopy(Random rng, int n, string removeKind, int capacity, string copyKind)
    {
        var s = new Scenario($"fill_remove_copy_n{n}_{removeKind}_cap{capacity}_{copyKind}");
        var set = New(capacity);
        s.Op("new", capacity);
        var keys = Enumerable.Range(0, n).Select(i => i * 7 + rng.Next(3)).Distinct().ToList();
        foreach (var k in keys)
        {
            s.Op("add", k, set.Add(k));
        }

        var removeCount = removeKind switch
        {
            "one" => 1,
            "quarter" => Math.Max(1, keys.Count / 4),
            "half" => Math.Max(1, keys.Count / 2),
            "all_but_one" => Math.Max(0, keys.Count - 1),
            "all" => keys.Count,
            _ => throw new ArgumentException(removeKind),
        };

        var shuffled = keys.OrderBy(_ => rng.Next()).ToList();
        for (var i = 0; i < removeCount && i < shuffled.Count; i++)
        {
            s.Op("remove", shuffled[i], set.Remove(shuffled[i]));
        }

        Order(s, set);
        s.Op(copyKind);
        set = copyKind == "copy" ? new HashSet<int>(set) : new HashSet<int>(set.ToList());
        Order(s, set);

        var addCount = n / 2 + 2;
        for (var i = 0; i < addCount; i++)
        {
            var k = 10_000 + i * 13;
            s.Op("add", k, set.Add(k));
        }

        Order(s, set);
        return s;
    }

    private static Scenario RandomMix(Random rng, string name, int keyRange, int opCount, int capacity)
    {
        var s = new Scenario(name);
        var set = New(capacity);
        s.Op("new", capacity);
        for (var i = 0; i < opCount; i++)
        {
            var roll = rng.Next(100);
            if (roll < 35)
            {
                var k = rng.Next(keyRange);
                s.Op("add", k, set.Add(k));
            }
            else if (roll < 55)
            {
                var k = rng.Next(keyRange);
                s.Op("remove", k, set.Remove(k));
            }
            else if (roll < 62)
            {
                var k = rng.Next(keyRange);
                s.Op("contains", k, set.Contains(k));
            }
            else if (roll < 66)
            {
                var ks = RandomKeys(rng, keyRange);
                s.Op("union", ks);
                set.UnionWith(ks);
            }
            else if (roll < 69)
            {
                var ks = RandomKeys(rng, keyRange);
                s.Op("union_set", ks);
                set.UnionWith(new HashSet<int>(ks));
            }
            else if (roll < 73)
            {
                var ks = RandomKeys(rng, keyRange);
                s.Op("except", ks);
                set.ExceptWith(ks);
            }
            else if (roll < 75)
            {
                var ks = RandomKeys(rng, keyRange);
                s.Op("intersect", ks);
                set.IntersectWith(ks);
            }
            else if (roll < 77)
            {
                var ks = RandomKeys(rng, keyRange);
                s.Op("intersect_set", ks);
                set.IntersectWith(new HashSet<int>(ks));
            }
            else if (roll < 80)
            {
                var ks = RandomKeys(rng, keyRange);
                s.Op("overlaps", ks, set.Overlaps(ks));
            }
            else if (roll < 82)
            {
                var ks = rng.Next(2) == 0 ? set.ToList() : RandomKeys(rng, keyRange);
                s.Op("set_equals", ks, set.SetEquals(ks));
            }
            else if (roll < 88)
            {
                s.Op("copy");
                set = new HashSet<int>(set);
            }
            else if (roll < 90)
            {
                s.Op("copy_list");
                set = new HashSet<int>(set.ToList());
            }
            else if (roll < 91)
            {
                s.Op("clear");
                set.Clear();
            }
            else if (roll < 93)
            {
                var n = rng.Next(keyRange * 2);
                s.Op("ensure_capacity", n, set.EnsureCapacity(n));
            }
            else if (roll < 94)
            {
                s.Op("trim");
                set.TrimExcess();
            }
            else
            {
                Order(s, set);
            }
        }

        Order(s, set);
        return s;
    }

    private static List<int> RandomKeys(Random rng, int keyRange)
    {
        var n = rng.Next(0, Math.Min(keyRange, 12) + 1);
        var list = new List<int>(n);
        for (var i = 0; i < n; i++)
        {
            list.Add(rng.Next(keyRange));
        }

        return list;
    }

    private static void Order(Scenario s, HashSet<int> set)
    {
        s.Op("order", set.ToList());
    }
}

public static class DictionaryScenarios
{
    public static IEnumerable<Scenario> Generate(Random rng)
    {
        foreach (var s in EdgeCases())
        {
            yield return s;
        }

        foreach (var n in Sizes.AroundPrimes)
        {
            foreach (var removeKind in new[] { "one", "half", "all_but_one", "all" })
            {
                foreach (var capacity in new[] { -1, n })
                {
                    yield return FillRemoveCopy(rng, n, removeKind, capacity);
                }
            }
        }

        var keyRanges = new[] { 4, 8, 16, 32, 64, 200 };
        var opCounts = new[] { 10, 30, 80, 200, 300 };
        var index = 0;
        foreach (var keyRange in keyRanges)
        {
            foreach (var opCount in opCounts)
            {
                for (var rep = 0; rep < (opCount >= 200 ? 2 : 4); rep++)
                {
                    var capacity = Sizes.Capacities[rng.Next(Sizes.Capacities.Length)];
                    yield return RandomMix(rng, $"random_{index++}_k{keyRange}_n{opCount}", keyRange, opCount, capacity);
                }
            }
        }
    }

    private static Dictionary<int, int> New(int capacity) => capacity < 0 ? new Dictionary<int, int>() : new Dictionary<int, int>(capacity);

    private static IEnumerable<Scenario> EdgeCases()
    {
        var s = new Scenario("empty");
        var d = New(-1);
        s.Op("new", -1);
        Pairs(s, d);
        s.Op("copy");
        d = new Dictionary<int, int>(d);
        Pairs(s, d);
        yield return s;

        s = new Scenario("overwrite_keeps_position");
        d = New(-1);
        s.Op("new", -1);
        for (var i = 0; i < 6; i++)
        {
            s.Op("set", i, i * 10);
            d[i] = i * 10;
        }

        s.Op("set", 2, 99);
        d[2] = 99;
        s.Op("try_add", 3, 77, d.TryAdd(3, 77));
        s.Op("try_add", 30, 77, d.TryAdd(30, 77));
        Pairs(s, d);
        yield return s;

        s = new Scenario("copy_with_holes_then_add");
        d = New(-1);
        s.Op("new", -1);
        for (var i = 0; i < 7; i++)
        {
            s.Op("add", i, -i);
            d.Add(i, -i);
        }

        s.Op("remove", 1, d.Remove(1));
        s.Op("remove", 4, d.Remove(4));
        Order(s, d);
        s.Op("copy");
        d = new Dictionary<int, int>(d);
        Order(s, d);
        s.Op("add", 10, 1);
        d.Add(10, 1);
        s.Op("add", 11, 2);
        d.Add(11, 2);
        s.Op("add", 12, 3);
        d.Add(12, 3);
        Pairs(s, d);
        yield return s;

        s = new Scenario("clear_then_add");
        d = New(3);
        s.Op("new", 3);
        for (var i = 0; i < 9; i++)
        {
            s.Op("set", i * 5, i);
            d[i * 5] = i;
        }

        s.Op("remove", 10, d.Remove(10));
        s.Op("clear");
        d.Clear();
        for (var i = 0; i < 4; i++)
        {
            s.Op("set", 50 - i, i);
            d[50 - i] = i;
        }

        Pairs(s, d);
        yield return s;
    }

    private static Scenario FillRemoveCopy(Random rng, int n, string removeKind, int capacity)
    {
        var s = new Scenario($"fill_remove_copy_n{n}_{removeKind}_cap{capacity}");
        var d = New(capacity);
        s.Op("new", capacity);
        var keys = Enumerable.Range(0, n).Select(i => i * 5 + rng.Next(2)).Distinct().ToList();
        foreach (var k in keys)
        {
            var v = rng.Next(1000);
            s.Op("add", k, v);
            d.Add(k, v);
        }

        var removeCount = removeKind switch
        {
            "one" => 1,
            "half" => Math.Max(1, keys.Count / 2),
            "all_but_one" => Math.Max(0, keys.Count - 1),
            "all" => keys.Count,
            _ => throw new ArgumentException(removeKind),
        };

        var shuffled = keys.OrderBy(_ => rng.Next()).ToList();
        for (var i = 0; i < removeCount && i < shuffled.Count; i++)
        {
            s.Op("remove", shuffled[i], d.Remove(shuffled[i]));
        }

        Order(s, d);
        s.Op("copy");
        d = new Dictionary<int, int>(d);
        Order(s, d);
        for (var i = 0; i < n / 2 + 2; i++)
        {
            var k = 10_000 + i * 11;
            s.Op("set", k, i);
            d[k] = i;
        }

        Pairs(s, d);
        return s;
    }

    private static Scenario RandomMix(Random rng, string name, int keyRange, int opCount, int capacity)
    {
        var s = new Scenario(name);
        var d = New(capacity);
        s.Op("new", capacity);
        for (var i = 0; i < opCount; i++)
        {
            var roll = rng.Next(100);
            var k = rng.Next(keyRange);
            if (roll < 25)
            {
                var v = rng.Next(1000);
                s.Op("set", k, v);
                d[k] = v;
            }
            else if (roll < 35)
            {
                if (!d.ContainsKey(k))
                {
                    var v = rng.Next(1000);
                    s.Op("add", k, v);
                    d.Add(k, v);
                }
            }
            else if (roll < 45)
            {
                var v = rng.Next(1000);
                s.Op("try_add", k, v, d.TryAdd(k, v));
            }
            else if (roll < 62)
            {
                s.Op("remove", k, d.Remove(k));
            }
            else if (roll < 67)
            {
                s.Op("contains", k, d.ContainsKey(k));
            }
            else if (roll < 75)
            {
                var found = d.TryGetValue(k, out var v);
                s.Op("get", k, found, v);
            }
            else if (roll < 81)
            {
                s.Op("copy");
                d = new Dictionary<int, int>(d);
            }
            else if (roll < 82)
            {
                s.Op("clear");
                d.Clear();
            }
            else if (roll < 84)
            {
                var n = rng.Next(keyRange * 2);
                s.Op("ensure_capacity", n, d.EnsureCapacity(n));
            }
            else if (roll < 85)
            {
                s.Op("trim");
                d.TrimExcess();
            }
            else if (roll < 92)
            {
                Order(s, d);
            }
            else
            {
                Pairs(s, d);
            }
        }

        Pairs(s, d);
        return s;
    }

    private static void Order(Scenario s, Dictionary<int, int> d)
    {
        s.Op("order", d.Keys.ToList());
    }

    private static void Pairs(Scenario s, Dictionary<int, int> d)
    {
        s.Op("pairs", d.Select(p => new[] { p.Key, p.Value }).ToList());
    }
}

public static class PriorityQueueScenarios
{
    private static readonly double[] DoublePriorities = { -1, 0, 0.5, 1, 1.0001, 2, 2.0001, 3, 3.0001, 4, 7.5 };

    public static IEnumerable<Scenario> Generate(Random rng)
    {
        // A*-like: costs grow by 1 with a heuristic and an occasional 0.0001 turn penalty.
        for (var rep = 0; rep < 20; rep++)
        {
            yield return AStarLike(rng, rep);
        }

        var opCounts = new[] { 5, 20, 60, 150, 300 };
        var index = 0;
        foreach (var kind in new[] { "double", "int" })
        {
            foreach (var opCount in opCounts)
            {
                for (var rep = 0; rep < 12; rep++)
                {
                    yield return RandomMix(rng, $"random_{index++}_{kind}_n{opCount}", kind, opCount);
                }
            }
        }
    }

    private static Scenario AStarLike(Random rng, int rep)
    {
        var s = new Scenario($"astar_like_{rep}");
        var q = new PriorityQueue<int, double>();
        s.Op("new", "double");
        var next = 0;
        q.Enqueue(next, 0);
        s.Op("enqueue", next++, 0.0);
        var steps = 30 + rng.Next(120);
        for (var i = 0; i < steps && q.Count > 0; i++)
        {
            q.TryPeek(out _, out var cost);
            var e = q.Dequeue();
            s.Op("dequeue", e);
            var neighbors = rng.Next(5);
            for (var j = 0; j < neighbors; j++)
            {
                var p = Math.Floor(cost) + 1 + rng.Next(3) + (rng.Next(3) == 0 ? 0.0001 : 0);
                q.Enqueue(next, p);
                s.Op("enqueue", next++, p);
            }
        }

        Drain(s, q);
        return s;
    }

    private static Scenario RandomMix(Random rng, string name, string kind, int opCount)
    {
        var s = new Scenario(name);
        s.Op("new", kind);
        if (kind == "double")
        {
            var q = new PriorityQueue<int, double>();
            Run(rng, s, opCount, q, () => DoublePriorities[rng.Next(DoublePriorities.Length)], p => p);
            Drain(s, q);
        }
        else
        {
            var q = new PriorityQueue<int, int>();
            Run(rng, s, opCount, q, () => rng.Next(6), p => p);
            Drain(s, q);
        }

        return s;
    }

    private static void Run<TPriority>(Random rng, Scenario s, int opCount, PriorityQueue<int, TPriority> q, Func<TPriority> nextPriority, Func<TPriority, object> box)
    {
        for (var i = 0; i < opCount; i++)
        {
            var roll = rng.Next(100);
            if (roll < 50 || q.Count == 0 && roll < 80)
            {
                var e = rng.Next(40);
                var p = nextPriority();
                q.Enqueue(e, p);
                s.Op("enqueue", e, box(p));
            }
            else if (roll < 70)
            {
                if (q.Count > 0)
                {
                    s.Op("dequeue", q.Dequeue());
                }
            }
            else if (roll < 78)
            {
                if (q.Count > 0)
                {
                    s.Op("peek", q.Peek());
                }
            }
            else if (roll < 83)
            {
                s.Op("count", q.Count);
            }
            else if (roll < 85)
            {
                s.Op("clear");
                q.Clear();
            }
            else
            {
                var e = rng.Next(40);
                var found = q.Remove(e, out var removedElement, out var removedPriority);
                s.Op("remove", e, found, found ? removedElement : 0, box(removedPriority!));
            }
        }
    }

    private static void Drain<TPriority>(Scenario s, PriorityQueue<int, TPriority> q)
    {
        var items = new List<object[]>();
        while (q.TryDequeue(out var e, out var p))
        {
            items.Add(new object[] { e, p! });
        }

        s.Op("drain", items);
    }
}

public static class SortScenarios
{
    private readonly record struct Item(int Key, int Id);

    private sealed class KeyComparer : IComparer<Item>
    {
        public int Compare(Item a, Item b) => a.Key.CompareTo(b.Key);
    }

    private static readonly Comparison<Item> ByKey = (a, b) => a.Key.CompareTo(b.Key);

    public static IEnumerable<Scenario> Generate(Random rng)
    {
        var lengths = Enumerable.Range(0, 41).Concat(new[] { 47, 48, 50, 63, 64, 65, 100, 127, 128, 129, 200, 255, 256, 300 }).ToList();
        var patterns = new[] { "random_r1", "random_r2", "random_r3", "random_r5", "random_r10", "random_rn", "sorted", "reversed", "equal", "organ_pipe", "sawtooth" };
        var index = 0;
        foreach (var n in lengths)
        {
            foreach (var pattern in patterns)
            {
                var keys = Keys(rng, n, pattern);
                var s = new Scenario($"sort_{index++}_n{n}_{pattern}");
                if (n <= 64)
                {
                    // Short inputs: record every API, in case they ever diverge.
                    ListSort(s, keys);
                    ArraySort(s, keys);
                    SpanSort(s, keys);
                    ListSortRange(rng, s, keys);
                }
                else
                {
                    switch (index % 4)
                    {
                        case 0: ListSort(s, keys); break;
                        case 1: ArraySort(s, keys); break;
                        case 2: SpanSort(s, keys); break;
                        default: ListSortRange(rng, s, keys); break;
                    }
                }

                yield return s;
            }
        }

        // McIlroy's adversary drives introsort past its depth limit into heapsort.
        foreach (var n in new[] { 20, 50, 100, 200, 300 })
        {
            var keys = Adversary(n);
            var s = new Scenario($"sort_adversary_n{n}");
            ListSort(s, keys);
            ArraySort(s, keys);
            SpanSort(s, keys);
            ListSortRange(rng, s, keys);
            yield return s;

            // The same input with ties: collapse keys into a few buckets.
            var tied = keys.Select(k => k / 7).ToArray();
            s = new Scenario($"sort_adversary_tied_n{n}");
            ListSort(s, tied);
            ArraySort(s, tied);
            yield return s;
        }
    }

    private static int[] Keys(Random rng, int n, string pattern)
    {
        var keys = new int[n];
        for (var i = 0; i < n; i++)
        {
            keys[i] = pattern switch
            {
                "random_r1" => 0,
                "random_r2" => rng.Next(2),
                "random_r3" => rng.Next(3),
                "random_r5" => rng.Next(5),
                "random_r10" => rng.Next(10),
                "random_rn" => rng.Next(Math.Max(1, n)),
                "sorted" => i / 2,
                "reversed" => (n - i) / 2,
                "equal" => 7,
                "organ_pipe" => Math.Min(i, n - i),
                "sawtooth" => i % 5,
                _ => throw new ArgumentException(pattern),
            };
        }

        return keys;
    }

    private static int[] Adversary(int n)
    {
        var gas = n;
        var val = Enumerable.Repeat(gas, n).ToArray();
        var nsolid = 0;
        var candidate = 0;
        var ids = Enumerable.Range(0, n).ToArray();
        Array.Sort(ids, (x, y) =>
        {
            if (val[x] == gas && val[y] == gas)
            {
                if (x == candidate)
                {
                    val[x] = nsolid++;
                }
                else
                {
                    val[y] = nsolid++;
                }
            }

            if (val[x] == gas)
            {
                candidate = x;
            }
            else if (val[y] == gas)
            {
                candidate = y;
            }

            return val[x].CompareTo(val[y]);
        });
        return val;
    }

    private static List<Item> Items(int[] keys) => keys.Select((k, i) => new Item(k, i)).ToList();

    private static List<int[]> Pairs(List<Item> items) => items.Select(x => new[] { x.Key, x.Id }).ToList();

    private static void ListSort(Scenario s, int[] keys)
    {
        var items = Items(keys);
        var input = Pairs(items);
        items.Sort(ByKey);
        s.Op("list_sort", input, items.Select(x => x.Id).ToList());
    }

    private static void ArraySort(Scenario s, int[] keys)
    {
        var items = Items(keys).ToArray();
        var input = Pairs(items.ToList());
        Array.Sort(items, ByKey);
        s.Op("array_sort", input, items.Select(x => x.Id).ToList());
    }

    private static void SpanSort(Scenario s, int[] keys)
    {
        var items = Items(keys).ToArray();
        var input = Pairs(items.ToList());
        MemoryExtensions.Sort(items.AsSpan(), ByKey);
        s.Op("span_sort", input, items.Select(x => x.Id).ToList());
    }

    private static void ListSortRange(Random rng, Scenario s, int[] keys)
    {
        var items = Items(keys);
        var input = Pairs(items);
        var index = items.Count == 0 ? 0 : rng.Next(items.Count);
        var count = items.Count - index == 0 ? 0 : rng.Next(items.Count - index + 1);
        items.Sort(index, count, new KeyComparer());
        s.Op("list_sort_range", input, index, count, items.Select(x => x.Id).ToList());
    }
}

public static class MathScenarios
{
    public static IEnumerable<Scenario> Generate(Random rng)
    {
        var values = new List<double>();
        for (var k = -20; k <= 20; k++)
        {
            values.Add(k + 0.5);
            values.Add(k);
            values.Add(k + 0.25);
            values.Add(k + 0.75);
            values.Add(BitDecrement(k + 0.5));
            values.Add(BitIncrement(k + 0.5));
        }

        // Centroid-style averages, the case AddPipes.1.FBE.cs:801 rounds.
        for (var count = 1; count <= 12; count++)
        {
            for (var sum = -30; sum <= 60; sum++)
            {
                values.Add((double)sum / count);
            }
        }

        for (var i = 0; i < 300; i++)
        {
            values.Add((rng.NextDouble() - 0.5) * 2000);
        }

        values.Add(0.0);
        values.Add(-0.0);
        values.Add(1e9 + 0.5);
        values.Add(-1e9 - 0.5);
        values.Add(2147483646.5);
        values.Add(-2147483647.5);

        var round = new Scenario("round");
        var roundInt = new Scenario("round_int");
        var toInt = new Scenario("to_int");
        var ceiling = new Scenario("ceiling");
        var floor = new Scenario("floor");
        foreach (var x in values)
        {
            var r = Math.Round(x, 0);
            round.Op("round", x, r);
            if (r >= int.MinValue && r <= int.MaxValue)
            {
                roundInt.Op("round_int", x, (int)Math.Round(x, 0));
            }

            if (x > int.MinValue - 1.0 && x < int.MaxValue + 1.0)
            {
                toInt.Op("to_int", x, (int)x);
            }

            ceiling.Op("ceiling", x, Math.Ceiling(x));
            floor.Op("floor", x, Math.Floor(x));
        }

        yield return round;
        yield return roundInt;
        yield return toInt;
        yield return ceiling;
        yield return floor;
    }

    private static double BitDecrement(double x) => Math.BitDecrement(x);

    private static double BitIncrement(double x) => Math.BitIncrement(x);
}
