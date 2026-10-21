// Records what DelaunatorSharp returns for the point sets the planner hands it, plus
// synthetic sets chosen to hit ties, so the Rust port in rust/delaunator-net can be
// checked without .NET. See ../FIXTURES.md.

using System.Text;
using System.Text.Json;
using DelaunatorSharp;
using Knapcode.FactorioTools.OilField;

if (args.Length != 1)
{
    Console.Error.WriteLine("usage: DelaunatorOracle <output directory>");
    return 2;
}

var outputDir = Path.GetFullPath(args[0]);
Directory.CreateDirectory(outputDir);
var repoRoot = FindRepoRoot(outputDir) ?? FindRepoRoot(Directory.GetCurrentDirectory())
    ?? throw new InvalidOperationException("Could not find the repository root.");
var listDir = Path.Combine(repoRoot, "test", "FactorioTools.Test", "OilField");

var cases = new List<(string Name, int[][] Points)>();

// 1. The pumpjack centers as the planner computes them, in context.Centers order. The
//    connected-centers strategies (AddPipes.3.ConnectedCenters.DT.cs) and FBE
//    (PointsToLines(context.Centers, sort: false)) both triangulate this array as is.
// 2. The same centers sorted by X then Y, as PointsToLines(..., sort: true) does, when
//    PointsToLines would actually reach Delaunator (more than 2 points, not collinear).
foreach (var (prefix, file) in new[] { ("small", "small-list.txt"), ("big", "big-list.txt") })
{
    var blueprints = ParseBlueprint.ReadBlueprintFile(Path.Combine(listDir, file));
    for (var i = 0; i < blueprints.Count; i++)
    {
        List<Location> centers;
        try
        {
            var blueprint = ParseBlueprint.Execute(blueprints[i]);
            var context = InitializeContext.Execute(OilFieldOptions.ForMediumElectricPole, blueprint, Array.Empty<AvoidLocation>());
            centers = context.Centers;
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"{prefix}-{i:D4}: skipped, {ex.GetType().Name}: {ex.Message}");
            continue;
        }

        cases.Add(($"{prefix}-{i:D4}-centers", ToArray(centers)));

        var sorted = centers.ToList();
        sorted.Sort((a, b) =>
        {
            var c = a.X.CompareTo(b.X);
            return c != 0 ? c : a.Y.CompareTo(b.Y);
        });
        if (sorted.Count > 2 && !Helpers.AreLocationsCollinear(sorted))
        {
            cases.Add(($"{prefix}-{i:D4}-sorted", ToArray(sorted)));
        }
    }
}

// 3. Synthetic sets.
cases.Add(("too-few-0", Array.Empty<int[]>()));
cases.Add(("too-few-1", new[] { new[] { 3, 4 } }));
cases.Add(("too-few-2", new[] { new[] { 0, 0 }, new[] { 5, 5 } }));
cases.Add(("collinear-horizontal-3", Line(3, 1, 0)));
cases.Add(("collinear-vertical-5", Line(5, 0, 1)));
cases.Add(("collinear-diagonal-30", Line(30, 1, 1)));
cases.Add(("collinear-spaced-12", Line(12, 3, 0)));
cases.Add(("identical-5", Enumerable.Repeat(new[] { 7, 7 }, 5).ToArray()));
cases.Add(("duplicates-square", new[] { new[] { 0, 0 }, new[] { 4, 0 }, new[] { 0, 0 }, new[] { 4, 4 }, new[] { 0, 4 }, new[] { 4, 4 }, new[] { 2, 2 } }));
cases.Add(("right-triangle-circumcenter-is-a-point", new[] { new[] { 0, 0 }, new[] { 2, 0 }, new[] { 0, 2 }, new[] { 1, 1 }, new[] { 2, 2 } }));
cases.Add(("circle-r5", CirclePoints(5)));
cases.Add(("circle-r25", CirclePoints(25)));
cases.Add(("circle-r65", CirclePoints(65)));

for (var k = 2; k <= 12; k++)
{
    cases.Add(($"lattice-{k}x{k}", Lattice(k, k, 1)));
    cases.Add(($"lattice-{k}x{k}-spacing3", Lattice(k, k, 3)));
}

cases.Add(("lattice-3x9", Lattice(3, 9, 1)));
cases.Add(("lattice-9x3", Lattice(9, 3, 1)));
cases.Add(("lattice-2x20", Lattice(2, 20, 1)));

var random = new Random(1);
for (var k = 3; k <= 10; k++)
{
    for (var r = 0; r < 5; r++)
    {
        var points = Lattice(k, k, 3);
        random.Shuffle(points);
        cases.Add(($"lattice-{k}x{k}-shuffled-{r}", points));
    }
}

foreach (var range in new[] { 6, 12, 40, 100 })
{
    for (var r = 0; r < 100; r++)
    {
        var n = random.Next(3, 61);
        var points = new int[n][];
        for (var p = 0; p < n; p++)
        {
            points[p] = new[] { random.Next(range), random.Next(range) };
        }

        cases.Add(($"random-range{range}-{r:D3}-n{n}", points));
    }
}

var path = Path.Combine(outputDir, "delaunator.jsonl");
using (var stream = File.Create(path))
{
    var newline = Encoding.UTF8.GetBytes("\n");
    foreach (var (name, points) in cases)
    {
        using (var writer = new Utf8JsonWriter(stream))
        {
            writer.WriteStartObject();
            writer.WriteString("name", name);
            writer.WriteStartArray("points");
            foreach (var p in points)
            {
                writer.WriteStartArray();
                writer.WriteNumberValue(p[0]);
                writer.WriteNumberValue(p[1]);
                writer.WriteEndArray();
            }

            writer.WriteEndArray();

            try
            {
                var delaunator = new Delaunator(points.Select(p => (IPoint)new DelaunatorSharp.Point(p[0], p[1])).ToArray());
                WriteInts(writer, "triangles", delaunator.Triangles);
                WriteInts(writer, "halfedges", delaunator.Halfedges);
            }
            catch (Exception ex)
            {
                writer.WriteString("error", ex.GetType().Name);
            }

            writer.WriteEndObject();
        }

        stream.Write(newline);
    }
}

Console.WriteLine($"delaunator.jsonl: {cases.Count} cases, {new FileInfo(path).Length} bytes");
return 0;

static void WriteInts(Utf8JsonWriter writer, string name, int[] values)
{
    writer.WriteStartArray(name);
    foreach (var v in values)
    {
        writer.WriteNumberValue(v);
    }

    writer.WriteEndArray();
}

static int[][] ToArray(IReadOnlyList<Location> locations) => locations.Select(l => new[] { l.X, l.Y }).ToArray();

static int[][] Line(int n, int dx, int dy) => Enumerable.Range(0, n).Select(i => new[] { i * dx, i * dy }).ToArray();

static int[][] Lattice(int w, int h, int spacing)
{
    var points = new List<int[]>();
    for (var y = 0; y < h; y++)
    {
        for (var x = 0; x < w; x++)
        {
            points.Add(new[] { x * spacing, y * spacing });
        }
    }

    return points.ToArray();
}

// Every integer point on the circle x^2 + y^2 = r^2, shifted to be non-negative.
static int[][] CirclePoints(int r)
{
    var points = new List<int[]>();
    for (var x = -r; x <= r; x++)
    {
        for (var y = -r; y <= r; y++)
        {
            if (x * x + y * y == r * r)
            {
                points.Add(new[] { x + r, y + r });
            }
        }
    }

    return points.ToArray();
}

static string? FindRepoRoot(string start)
{
    for (var dir = new DirectoryInfo(start); dir is not null; dir = dir.Parent)
    {
        if (File.Exists(Path.Combine(dir.FullName, "FactorioTools.sln")))
        {
            return dir.FullName;
        }
    }

    return null;
}
