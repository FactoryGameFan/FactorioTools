// Records FluteSharp's output for the Rust port in rust/flute (#138). See ../FIXTURES.md.
//
// Usage: dotnet run --project rust/flute/oracle -c Release -- <output directory>
// Run from anywhere inside the repo; the blueprint lists are found from the repo root.

using System.Text;
using System.Text.Json;
using Knapcode.FactorioTools.OilField;
using Knapcode.FluteSharp;

var outputDirectory = Path.GetFullPath(args.Length > 0 ? args[0] : "rust/flute/fixtures");
var repoRoot = FindRepoRoot(Directory.GetCurrentDirectory());
var listDirectory = Path.Combine(repoRoot, "test", "FactorioTools.Test", "OilField");

var flute = new FLUTE(LookUpTable.Degree6);
var cases = new List<(string Name, List<Point> Points)>();

// (a) The terminal points the planner hands FLUTE, built the way
// AddPipesConnectedCentersFLUTE.GetFluteTree builds them: every terminal of every center,
// in CenterToTerminals order, straight after InitializeContext.
AddBlueprintCases("small", Path.Combine(listDirectory, "small-list.txt"), stride: 1);
AddBlueprintCases("big", Path.Combine(listDirectory, "big-list.txt"), stride: 2);

// (b) Synthetic inputs, from a fixed seed.
var random = new Random(42);
cases.Add(("empty", new List<Point>()));
for (var n = 1; n <= 10; n++)
{
    for (var i = 0; i < 30; i++)
    {
        cases.Add(($"tiny-{n}-{i}", RandomPoints(n, 0, 4)));
    }

    for (var i = 0; i < 15; i++)
    {
        cases.Add(($"small-random-{n}-{i}", RandomPoints(n, 0, 20)));
    }
}

for (var n = 11; n <= 60; n++)
{
    for (var i = 0; i < 3; i++)
    {
        cases.Add(($"medium-random-{n}-{i}", RandomPoints(n, -10, 40)));
    }

    cases.Add(($"crowded-{n}", RandomPoints(n, 0, 6)));
}

for (var n = 3; n <= 14; n++)
{
    cases.Add(($"horizontal-{n}", Enumerable.Range(0, n).Select(i => new Point(i * 2, 5)).ToList()));
    cases.Add(($"vertical-{n}", Enumerable.Range(0, n).Select(i => new Point(3, i)).ToList()));
    cases.Add(($"diagonal-{n}", Enumerable.Range(0, n).Select(i => new Point(i, i)).ToList()));
    cases.Add(($"same-point-{n}", Enumerable.Range(0, n).Select(_ => new Point(7, 7)).ToList()));
    cases.Add(($"grid-{n}", Enumerable.Range(0, n).Select(i => new Point(i % 3 * 3, i / 3 * 3)).ToList()));
}

Directory.CreateDirectory(outputDirectory);
var outputPath = Path.Combine(outputDirectory, "flute.jsonl");
var tempPath = outputPath + ".tmp";
var errors = 0;
using (var stream = File.Create(tempPath))
{
    foreach (var (name, points) in cases)
    {
        var buffer = new MemoryStream();
        using (var writer = new Utf8JsonWriter(buffer))
        {
            writer.WriteStartObject();
            writer.WriteString("name", name);
            writer.WriteStartArray("points");
            foreach (var p in points)
            {
                writer.WriteStartArray();
                writer.WriteNumberValue(p.X);
                writer.WriteNumberValue(p.Y);
                writer.WriteEndArray();
            }

            writer.WriteEndArray();

            try
            {
                var tree = flute.Execute(points);
                writer.WriteNumber("deg", tree.Deg);
                writer.WriteNumber("length", tree.Length);
                writer.WriteStartArray("branches");
                foreach (var b in tree.Branch)
                {
                    writer.WriteStartArray();
                    writer.WriteNumberValue(b.X);
                    writer.WriteNumberValue(b.Y);
                    writer.WriteNumberValue(b.N);
                    writer.WriteEndArray();
                }

                writer.WriteEndArray();
            }
            catch (Exception ex)
            {
                writer.WriteString("error", ex.GetType().Name);
                errors++;
            }

            writer.WriteEndObject();
        }

        buffer.WriteByte((byte)'\n');
        buffer.Position = 0;
        buffer.CopyTo(stream);
    }
}

File.Move(tempPath, outputPath, overwrite: true);
Console.WriteLine($"flute.jsonl: {cases.Count} cases ({errors} errors), {new FileInfo(outputPath).Length} bytes");
return 0;

void AddBlueprintCases(string prefix, string path, int stride)
{
    var blueprints = ParseBlueprint.ReadBlueprintFile(path);
    for (var i = 0; i < blueprints.Count; i += stride)
    {
        var blueprint = ParseBlueprint.Execute(blueprints[i]);
        var context = InitializeContext.Execute(OilFieldOptions.ForMediumElectricPole, blueprint, Array.Empty<AvoidLocation>());
        var points = new List<Point>();
        foreach (var terminals in context.CenterToTerminals.Values)
        {
            for (var j = 0; j < terminals.Count; j++)
            {
                points.Add(new Point(terminals[j].Terminal.X, terminals[j].Terminal.Y));
            }
        }

        cases.Add(($"{prefix}-{i:D4}", points));
    }
}

List<Point> RandomPoints(int n, int min, int max)
{
    var points = new List<Point>(n);
    for (var i = 0; i < n; i++)
    {
        points.Add(new Point(random.Next(min, max + 1), random.Next(min, max + 1)));
    }

    return points;
}

static string FindRepoRoot(string start)
{
    var directory = new DirectoryInfo(start);
    while (directory is not null)
    {
        if (File.Exists(Path.Combine(directory.FullName, "FactorioTools.sln")))
        {
            return directory.FullName;
        }

        directory = directory.Parent;
    }

    throw new InvalidOperationException($"No FactorioTools.sln above {start}.");
}
