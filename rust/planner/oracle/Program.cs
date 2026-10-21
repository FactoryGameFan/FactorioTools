// Records the C# planner's behavior as fixtures for the Rust port (#138).
// Usage: PlannerOracle <output directory> [--check-snapshots]
// Writes grids.jsonl (stage 1) and algorithms.jsonl (stage 2, see Algorithms.cs).
// Format and how to regenerate: rust/planner/FIXTURES.md.

using System.Text;
using System.Text.Encodings.Web;
using System.Text.Json;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

if (args.Length < 1)
{
    Console.Error.WriteLine("usage: PlannerOracle <output directory> [--check-snapshots]");
    return 2;
}

var outputDir = Path.GetFullPath(args[0]);
var checkSnapshots = args.Contains("--check-snapshots");
var repoRoot = FindRepoRoot(outputDir) ?? FindRepoRoot(Directory.GetCurrentDirectory())
    ?? throw new InvalidOperationException("Could not find the repo root (FactorioTools.sln) above the output directory or the current directory.");
var oilFieldTestDir = Path.Combine(repoRoot, "test", "FactorioTools.Test", "OilField");
var smallList = ParseBlueprint.ReadBlueprintFile(Path.Combine(oilFieldTestDir, "small-list.txt"));

Directory.CreateDirectory(outputDir);

var cases = GridCases.Build(smallList);
var gridsPath = Path.Combine(outputDir, "grids.jsonl");
var snapshotMismatches = 0;
var snapshotMatches = 0;
var kindCounts = new SortedDictionary<string, int>(StringComparer.Ordinal);

using (var stream = File.Create(gridsPath))
{
    foreach (var gridCase in cases)
    {
        var result = gridCase.Run();
        var grid = result.Context.Grid;
        var builder = new StringBuilder();
        grid.ToString(builder, spacing: 1);

        // StringBuilder.AppendLine writes Environment.NewLine. Record LF everywhere so the
        // fixture does not depend on the OS the recorder ran on (the Verify snapshots are
        // LF too, via .gitattributes).
        var text = builder.ToString().Replace("\r\n", "\n");

        GridCases.Write(stream, gridCase.Name, grid, text, kindCounts);

        if (checkSnapshots && gridCase.SnapshotFile is not null)
        {
            var snapshotPath = Path.Combine(oilFieldTestDir, gridCase.SnapshotFile);
            var snapshot = File.ReadAllText(snapshotPath).Replace("\r\n", "\n"); // strips the BOM
            var gridStart = snapshot.IndexOf("\n\n", StringComparison.Ordinal);
            var snapshotGrid = gridStart < 0 ? null : snapshot.Substring(gridStart + 2);
            if (snapshotGrid == text)
            {
                snapshotMatches++;
            }
            else
            {
                snapshotMismatches++;
                Console.Error.WriteLine($"grid differs from snapshot: {gridCase.Name} vs {gridCase.SnapshotFile}");
            }
        }
    }
}

Console.WriteLine($"grids.jsonl: {cases.Count} cases, {new FileInfo(gridsPath).Length} bytes");

var bigList = ParseBlueprint.ReadBlueprintFile(Path.Combine(oilFieldTestDir, "big-list.txt"));
AlgorithmCases.Write(outputDir, smallList, bigList);
Console.WriteLine("entity kinds: " + string.Join(", ", kindCounts.Select(x => $"{x.Key}={x.Value}")));
InitializeContextRecorder.Record(outputDir, repoRoot, smallList);
HelpersRecorder.Record(outputDir, smallList);
if (checkSnapshots)
{
    Console.WriteLine($"snapshots: {snapshotMatches} match, {snapshotMismatches} differ");
    if (snapshotMismatches > 0)
    {
        return 1;
    }
}

return 0;

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

/// <summary>One planner run whose final grid is recorded.</summary>
/// <param name="SnapshotFile">The committed Verify snapshot with the same inputs, relative to
/// test/FactorioTools.Test/OilField, if there is one. Used only by --check-snapshots.</param>
sealed record GridCase(string Name, Func<PlannerResult> Run, string? SnapshotFile);

static class GridCases
{
    /// <summary>
    /// The option sets, each copied from a PlannerOnAllBlueprintsAndOptions theory so every
    /// case has a committed snapshot to check against. Between them they place every pole
    /// preset, beacons with and without overlap, and pipes with and without undergrounds.
    /// The first set runs on every blueprint. To keep the file small, each of the others runs
    /// on every third blueprint (Stride 3, starting at Offset), so every blueprint is also
    /// planned with exactly one other pole preset.
    /// </summary>
    private static readonly (string Pole, Func<OilFieldOptions> Options, bool Beacons, bool Overlap, bool Underground, int Stride, int Offset)[] OptionSets =
    {
        ("medium-electric-pole", () => OilFieldOptions.ForMediumElectricPole, true, true, true, 1, 0),
        ("small-electric-pole", () => OilFieldOptions.ForSmallElectricPole, false, false, true, 3, 0),
        ("big-electric-pole", () => OilFieldOptions.ForBigElectricPole, true, false, false, 3, 1),
        ("substation", () => OilFieldOptions.ForSubstation, true, true, false, 3, 2),
    };

    /// <summary>Heat pipes run on every fourth blueprint, for the same reason.</summary>
    private const int HeatStride = 4;

    public static List<GridCase> Build(IReadOnlyList<string> smallList)
    {
        var cases = new List<GridCase>();

        foreach (var set in OptionSets)
        {
            for (var i = set.Offset; i < smallList.Count; i += set.Stride)
            {
                var blueprintString = smallList[i];
                var suffix = $"{set.Pole}{(set.Beacons ? "_b" : "")}{(set.Overlap ? "_o" : "")}{(set.Underground ? "_u" : "")}";
                cases.Add(new GridCase(
                    $"small-{i:D4}_{suffix}",
                    () =>
                    {
                        var options = set.Options();
                        options.AddBeacons = set.Beacons;
                        options.OverlapBeacons = set.Overlap;
                        options.UseUndergroundPipes = set.Underground;
                        options.ValidateSolution = true;
                        return Planner.Execute(options, ParseBlueprint.Execute(blueprintString));
                    },
                    $"PlannerWithAllOptions/Theory.E_{i:D4}_{suffix}.verified.txt"));
            }
        }

        // Heat pipes (Aquilo), with the options PlannerTest's small-list heat theory uses. No
        // snapshot exists for these.
        for (var i = 0; i < smallList.Count; i += HeatStride)
        {
            var blueprintString = smallList[i];
            cases.Add(new GridCase(
                $"small-{i:D4}_medium-electric-pole_heat",
                () =>
                {
                    var options = OilFieldOptions.ForMediumElectricPole;
                    options.ValidateSolution = true;
                    options.AddHeatPipes = true;
                    options.AddBeacons = false;
                    return Planner.Execute(options, ParseBlueprint.Execute(blueprintString));
                },
                null));
        }

        // PlannerTest cases with snapshots, copied input for input.
        cases.Add(new GridCase("PlannerTest.ExecuteSample", Planner.ExecuteSample, "PlannerTest.ExecuteSample.verified.txt"));

        cases.Add(new GridCase(
            "PlannerTest.AddsHeatPipesForAquilo",
            () =>
            {
                var options = OilFieldOptions.ForMediumElectricPole;
                options.ValidateSolution = true;
                options.AddHeatPipes = true;
                options.AddBeacons = false;
                return Planner.Execute(options, ParseBlueprint.Execute(smallList[0]));
            },
            "PlannerTest.AddsHeatPipesForAquilo.verified.txt"));

        cases.Add(new GridCase(
            "PlannerTest.AddsHeatPipesAndBeaconsTogetherForAquilo",
            () =>
            {
                var options = OilFieldOptions.ForMediumElectricPole;
                options.ValidateSolution = true;
                options.AddHeatPipes = true;
                options.AddBeacons = true;
                return Planner.Execute(options, ParseBlueprint.Execute(smallList[0]));
            },
            "PlannerTest.AddsHeatPipesAndBeaconsTogetherForAquilo.verified.txt"));

        cases.Add(new GridCase(
            "PlannerTest.AllowsLocationsToBeAvoided",
            () =>
            {
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
                            },
                        },
                    },
                    Item = ItemNames.Vanilla.Blueprint,
                    Version = 0,
                };
                var avoid = Enumerable.Range(-7, 16).Select(x => new AvoidLocation(x, 0)).ToArray();
                return Planner.Execute(options, blueprint, avoid);
            },
            "PlannerTest.AllowsLocationsToBeAvoided.verified.txt"));

        return cases;
    }

    private static readonly JsonWriterOptions WriterOptions = new()
    {
        // Keeps '+', '<' and '>' readable in the grid text. Control characters such as the
        // line feeds are still escaped, so each case stays on one line.
        Encoder = JavaScriptEncoder.UnsafeRelaxedJsonEscaping,
    };

    public static void Write(Stream stream, string name, SquareGrid grid, string text, IDictionary<string, int> kindCounts)
    {
        using (var writer = new Utf8JsonWriter(stream, WriterOptions))
        {
            writer.WriteStartObject();
            writer.WriteString("name", name);
            writer.WriteNumber("width", grid.Width);
            writer.WriteNumber("height", grid.Height);
            writer.WriteStartArray("middle");
            writer.WriteNumberValue(grid.Middle.X);
            writer.WriteNumberValue(grid.Middle.Y);
            writer.WriteEndArray();

            writer.WriteStartArray("entities");
            foreach (var location in grid.EntityLocations.EnumerateItems())
            {
                var entity = grid[location] ?? throw new InvalidOperationException($"No entity at {location} in {name}.");
                var kind = entity.GetType().Name;
                kindCounts[kind] = kindCounts.TryGetValue(kind, out var count) ? count + 1 : 1;

                writer.WriteStartArray();
                writer.WriteNumberValue(location.X);
                writer.WriteNumberValue(location.Y);
                writer.WriteStringValue(kind);
                writer.WriteNumberValue(entity.Id);
                WriteExtra(writer, entity);
                writer.WriteEndArray();
            }

            writer.WriteEndArray();
            writer.WriteString("text", text);
            writer.WriteEndObject();
        }

        stream.WriteByte((byte)'\n');
    }

    /// <summary>The per-kind fields listed in rust/planner/FIXTURES.md.</summary>
    private static void WriteExtra(Utf8JsonWriter writer, GridEntity entity)
    {
        switch (entity)
        {
            // Check the subclasses of Pipe first: Terminal and UndergroundPipe are Pipes.
            case UndergroundPipe underground:
                writer.WriteNumberValue((int)underground.Direction);
                break;
            case Terminal:
            case Pipe:
            case HeatPipe:
            case BeaconCenter:
            case TemporaryEntity:
            case AvoidEntity:
                break;
            case PumpjackCenter pumpjack:
                writer.WriteNumberValue((int)pumpjack.Direction);
                break;
            case PumpjackSide side:
                writer.WriteNumberValue(side.Center.Id);
                break;
            case BeaconSide side:
                writer.WriteNumberValue(side.Center.Id);
                break;
            case ElectricPoleSide side:
                writer.WriteNumberValue(side.Center.Id);
                break;
            case ElectricPoleCenter pole:
                writer.WriteStartArray();
                foreach (var id in pole.Neighbors)
                {
                    writer.WriteNumberValue(id);
                }

                writer.WriteEndArray();
                break;
            default:
                throw new NotSupportedException($"No fixture format for grid entity {entity.GetType().Name}. Add it to rust/planner/FIXTURES.md first.");
        }
    }
}
