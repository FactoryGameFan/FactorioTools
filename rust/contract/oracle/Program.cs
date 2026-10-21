// Records the C# JSON contract for the Rust port (#138). Writes contract.jsonl: one line per
// call of PlanOrchestrator.PlanJson or NormalizeJson with the exact request text and the
// exact response text (or the exception the call threw), plus a few direct
// GridToBlueprintString calls with avoid entities, which only the CLI reaches.
// Usage: ContractOracle <output directory>

using System.IO.Compression;
using System.Text;
using System.Text.Json;
using System.Text.Json.Nodes;
using Knapcode.FactorioTools.Contract;
using Knapcode.FactorioTools.Data;
using Knapcode.FactorioTools.OilField;

var outputDir = Path.GetFullPath(args.Length > 0 ? args[0] : ".");
var repoRoot = FindRepoRoot(outputDir) ?? FindRepoRoot(Directory.GetCurrentDirectory())
    ?? throw new InvalidOperationException("Could not find the repository root (FactorioTools.sln).");
var oilFieldTestDir = Path.Combine(repoRoot, "test", "FactorioTools.Test", "OilField");
var smallList = ParseBlueprint.ReadBlueprintFile(Path.Combine(oilFieldTestDir, "small-list.txt"));
var bigList = ParseBlueprint.ReadBlueprintFile(Path.Combine(oilFieldTestDir, "big-list.txt"));

Directory.CreateDirectory(outputDir);
var path = Path.Combine(outputDir, "contract.jsonl");
var temp = path + ".tmp";
var counts = new SortedDictionary<string, int>(StringComparer.Ordinal);

using (var writer = new StreamWriter(temp, append: false, new UTF8Encoding(false)) { NewLine = "\n" })
{
    void Plan(string name, string request) => Write(writer, counts, name, "plan", request, () => PlanOrchestrator.PlanJson(request));
    void Normalize(string name, string request) => Write(writer, counts, name, "normalize", request, () => PlanOrchestrator.NormalizeJson(request));

    string Serialize(OilFieldPlanRequest request) => JsonSerializer.Serialize(request, ContractJson.Options);

    // 1. The smallest request: only a blueprint, so every option takes its default.
    for (var i = 0; i < smallList.Count; i++)
    {
        Plan($"small-{i:D4}_default", new JsonObject { ["blueprint"] = smallList[i] }.ToJsonString());
    }

    // 2. Full option objects, the way the site sends them.
    var variants = new (string Name, int Step, int Offset, Action<OilFieldPlanRequest> Apply)[]
    {
        ("small-pole", 3, 0, r => CopyOptions(OilFieldOptions.ForSmallElectricPole, r)),
        ("big-pole", 3, 1, r => CopyOptions(OilFieldOptions.ForBigElectricPole, r)),
        ("substation", 3, 2, r => CopyOptions(OilFieldOptions.ForSubstation, r)),
        ("no-beacons-no-underground", 4, 1, r => { r.AddBeacons = false; r.UseUndergroundPipes = false; }),
        ("no-overlap", 5, 2, r => r.OverlapBeacons = false),
        ("heat", 6, 0, r => { r.AddHeatPipes = true; r.AddBeacons = false; }),
        ("heat-beacons", 6, 3, r => r.AddHeatPipes = true),
        ("quality", 7, 1, r =>
        {
            r.PumpjackQuality = Quality.Legendary;
            r.BeaconQuality = Quality.Rare;
            r.ElectricPoleQuality = Quality.Uncommon;
            r.PumpjackModuleQuality = Quality.Epic;
            r.BeaconModuleQuality = Quality.Legendary;
        }),
        ("fbe-offset", 8, 2, r => r.AddFbeOffset = true),
        ("validate", 9, 4, r => r.ValidateSolution = true),
        ("modules", 10, 5, r =>
        {
            r.PumpjackModules = new Dictionary<string, int> { ["speed-module-3"] = 1, ["efficiency-module-3"] = 1 };
            r.BeaconModules = new Dictionary<string, int>();
        }),
        ("all-strategies", 11, 6, r =>
        {
            r.PipeStrategies = OilFieldOptions.AllPipeStrategies.ToList();
            r.BeaconStrategies = OilFieldOptions.AllBeaconStrategies.ToList();
        }),
        ("no-poles-no-optimize", 12, 7, r => { r.AddElectricPoles = false; r.OptimizePipes = false; }),
    };

    foreach (var (variantName, step, offset, apply) in variants)
    {
        for (var i = offset; i < smallList.Count; i += step)
        {
            var request = new OilFieldPlanRequest { Blueprint = smallList[i] };
            apply(request);
            Plan($"small-{i:D4}_{variantName}", Serialize(request));
        }
    }

    // 3. A big-list sample with defaults.
    for (var i = 0; i < bigList.Count; i += 50)
    {
        Plan($"big-{i:D4}_default", new JsonObject { ["blueprint"] = bigList[i] }.ToJsonString());
    }

    // 4. How requests are read: case-insensitive names, numbers in strings, enum spellings,
    // unknown and repeated properties.
    var bp = JsonSerializer.Serialize(smallList[1]);
    Plan("read_pascal-case-names", $"{{\"Blueprint\":{bp},\"AddBeacons\":false,\"ElectricPoleEntityName\":\"small-electric-pole\",\"ElectricPoleSupplyWidth\":5,\"ElectricPoleSupplyHeight\":5,\"ElectricPoleWireReach\":7.5}}");
    Plan("read_upper-case-names", $"{{\"BLUEPRINT\":{bp},\"USEUNDERGROUNDPIPES\":false}}");
    Plan("read_numbers-in-strings", $"{{\"blueprint\":{bp},\"electricPoleSupplyWidth\":\"5\",\"electricPoleSupplyHeight\":\"5\",\"electricPoleWireReach\":\"7.5\",\"beaconWidth\":\"3\"}}");
    Plan("read_enum-lowercase", $"{{\"blueprint\":{bp},\"pipeStrategies\":[\"fbe\",\"connectedcentersflute\"],\"beaconStrategies\":[\"snug\"]}}");
    Plan("read_enum-integers", $"{{\"blueprint\":{bp},\"pipeStrategies\":[1,4],\"beaconStrategies\":[2],\"pumpjackQuality\":5}}");
    Plan("read_quality-undefined-integer", $"{{\"blueprint\":{bp},\"pumpjackQuality\":42,\"beaconQuality\":\"Rare\"}}");
    Plan("read_unknown-property", $"{{\"blueprint\":{bp},\"somethingElse\":{{\"a\":[1,2,3]}},\"another\":null}}");
    Plan("read_repeated-property", $"{{\"blueprint\":{bp},\"addBeacons\":true,\"addBeacons\":false}}");
    Plan("read_wire-reach-integer", $"{{\"blueprint\":{bp},\"electricPoleWireReach\":10}}");
    Plan("read_wire-reach-fraction", $"{{\"blueprint\":{bp},\"electricPoleWireReach\":10.25}}");
    Plan("read_whitespace", $"  {{ \"blueprint\" : {bp} ,\n\t\"addBeacons\" : false }}  ");
    Plan("read_escaped-name", $"{{\"blueprint\":{bp},\"heatPipeEntityName\":\"heat\\u002Dpipe <&'+>\\u00e9\",\"addHeatPipes\":true,\"addBeacons\":false}}");
    Plan("read_empty-strategy-lists", $"{{\"blueprint\":{bp},\"beaconStrategies\":[]}}");
    Plan("read_module-dictionary-order", $"{{\"blueprint\":{bp},\"beaconModules\":{{\"speed-module-3\":1,\"productivity-module-3\":1,\"speed-module-3\":3}}}}");

    // 5. Normalize.
    for (var i = 0; i < smallList.Count; i++)
    {
        Normalize($"small-{i:D4}", new JsonObject { ["blueprint"] = smallList[i] }.ToJsonString());
    }

    for (var i = 0; i < bigList.Count; i += 25)
    {
        Normalize($"big-{i:D4}", new JsonObject { ["blueprint"] = bigList[i] }.ToJsonString());
    }

    // 6. Bad input. Most become an error envelope; some throw out of the call.
    var book = Encode("{\"blueprint_book\":{\"blueprints\":[],\"item\":\"blueprint-book\",\"label\":\"x\",\"active_index\":0,\"version\":0}}");
    var noPumpjacks = Encode("{\"blueprint\":{\"icons\":[],\"entities\":[{\"entity_number\":1,\"name\":\"pipe\",\"position\":{\"x\":0.5,\"y\":0.5}}],\"item\":\"blueprint\",\"version\":0}}");
    var diagonal = Encode("{\"blueprint\":{\"icons\":[],\"entities\":[{\"entity_number\":1,\"name\":\"pumpjack\",\"position\":{\"x\":0.5,\"y\":0.5},\"direction\":2}],\"item\":\"blueprint\",\"version\":562949955518464}}");
    var notJson = Encode("this is not json");
    var nullRoot = Encode("null");
    var noBlueprint = Encode("{}");
    var badBlueprints = new (string Name, string Value)[]
    {
        ("empty", ""),
        ("not-a-blueprint", "not-a-blueprint"),
        ("wrong-version-byte", "1eJyMj70OwjAMhN"),
        ("bad-base64", "0!!!!"),
        ("not-zlib", "0" + Convert.ToBase64String(Encoding.UTF8.GetBytes("hello world"))),
        ("not-json", notJson),
        ("null-root", nullRoot),
        ("no-blueprint", noBlueprint),
        ("book", book),
        ("no-pumpjacks", noPumpjacks),
        ("diagonal-pumpjack", diagonal),
    };

    foreach (var (name, value) in badBlueprints)
    {
        var quoted = JsonSerializer.Serialize(value);
        Plan($"bad_{name}", $"{{\"blueprint\":{quoted}}}");
        Normalize($"bad_{name}", $"{{\"blueprint\":{quoted}}}");
    }

    var badRequests = new (string Name, string Value)[]
    {
        ("invalid-json", "{"),
        ("json-null", "null"),
        ("json-array", "[]"),
        ("missing-blueprint", "{}"),
        ("null-blueprint", "{\"blueprint\":null}"),
        ("bool-in-string", $"{{\"blueprint\":{bp},\"addBeacons\":\"false\"}}"),
        ("unknown-quality", $"{{\"blueprint\":{bp},\"pumpjackQuality\":\"Mythic\"}}"),
        ("fraction-for-int", $"{{\"blueprint\":{bp},\"beaconWidth\":3.5}}"),
        ("trailing-comma", $"{{\"blueprint\":{bp},}}"),
        ("empty-pipe-strategies", $"{{\"blueprint\":{bp},\"pipeStrategies\":[]}}"),
    };

    foreach (var (name, value) in badRequests)
    {
        Plan($"bad-request_{name}", value);
        Normalize($"bad-request_{name}", value);
    }

    // 7. GridToBlueprintString with avoid entities, which the contract never asks for.
    for (var i = 0; i < 6; i++)
    {
        var blueprint = ParseBlueprint.Execute(smallList[i]);
        var pumpjack = blueprint.Entities.First(e => e.Name == EntityNames.Vanilla.Pumpjack);
        var avoid = new List<AvoidLocation>
        {
            new(pumpjack.Position.X + 5, pumpjack.Position.Y + 5),
            new(pumpjack.Position.X - 6, pumpjack.Position.Y + 1),
        };
        var options = OilFieldOptions.ForMediumElectricPole;
        var record = new JsonObject
        {
            ["name"] = $"grid_small-{i:D4}_avoid",
            ["op"] = "grid_to_blueprint_string",
            ["blueprint"] = smallList[i],
            ["avoid"] = new JsonArray(avoid.Select(a => (JsonNode)new JsonArray(a.X, a.Y)).ToArray()),
            ["add_fbe_offset"] = i % 2 == 1,
        };
        try
        {
            var result = Planner.Execute(options, blueprint, avoid);
            var text = GridToBlueprintString.Execute(result.Context, addFbeOffset: i % 2 == 1, addAvoidEntities: true);
            record["outcome"] = "ok";
            record["json"] = Decode(text);
        }
        catch (Exception ex)
        {
            record["outcome"] = "threw";
            record["exception"] = ex.GetType().Name;
            record["message"] = ex.Message;
        }

        writer.WriteLine(record.ToJsonString());
        counts["grid_to_blueprint_string"] = (counts.TryGetValue("grid_to_blueprint_string", out var gridCount) ? gridCount : 0) + 1;
    }
}

File.Move(temp, path, overwrite: true);
Console.WriteLine($"contract.jsonl: {string.Join(", ", counts.Select(x => $"{x.Key}={x.Value}"))}, {new FileInfo(path).Length} bytes");
return 0;

static void Write(StreamWriter writer, SortedDictionary<string, int> counts, string name, string op, string request, Func<string> call)
{
    var record = new JsonObject
    {
        ["name"] = name,
        ["op"] = op,
        ["request"] = request,
    };
    try
    {
        record["outcome"] = "ok";
        record["response"] = call();
    }
    catch (Exception ex)
    {
        record["outcome"] = "threw";
        record["exception"] = ex.GetType().Name;
        record["message"] = ex.Message;
    }

    writer.WriteLine(record.ToJsonString());
    var key = $"{op}:{record["outcome"]}";
    counts[key] = (counts.TryGetValue(key, out var count) ? count : 0) + 1;
}

static void CopyOptions(OilFieldOptions source, OilFieldPlanRequest target)
{
    target.ElectricPoleEntityName = source.ElectricPoleEntityName;
    target.ElectricPoleSupplyWidth = source.ElectricPoleSupplyWidth;
    target.ElectricPoleSupplyHeight = source.ElectricPoleSupplyHeight;
    target.ElectricPoleWireReach = source.ElectricPoleWireReach;
    target.ElectricPoleWidth = source.ElectricPoleWidth;
    target.ElectricPoleHeight = source.ElectricPoleHeight;
}

static string Encode(string json)
{
    using var output = new MemoryStream();
    using (var zlib = new ZLibStream(output, CompressionLevel.Optimal))
    {
        var bytes = Encoding.UTF8.GetBytes(json);
        zlib.Write(bytes, 0, bytes.Length);
    }

    return "0" + Convert.ToBase64String(output.ToArray());
}

static string Decode(string blueprintString)
{
    var bytes = Convert.FromBase64String(blueprintString.Substring(1));
    using var input = new ZLibStream(new MemoryStream(bytes), CompressionMode.Decompress);
    using var reader = new StreamReader(input, Encoding.UTF8);
    return reader.ReadToEnd();
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
