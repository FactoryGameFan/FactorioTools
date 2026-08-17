using System.Text.Json;

namespace Knapcode.FactorioTools.OilField;

public class CleanBlueprintTest : BasePlannerTest
{
    [Theory]
    [MemberData(nameof(BigListIndexTestData))]
    public void BigListBlueprintsAreNormalized(int blueprintIndex)
    {
        VerifySameBlueprint(BigListBlueprintStrings[blueprintIndex]);
    }

    [Theory]
    [MemberData(nameof(SmallListIndexTestData))]
    public void SmallListBlueprintsAreNormalized(int blueprintIndex)
    {
        VerifySameBlueprint(SmallListBlueprintStrings[blueprintIndex]);
    }

    private static void VerifySameBlueprint(string input)
    {
        var blueprint = ParseBlueprint.Execute(input);
     
        var clean = CleanBlueprint.Execute(blueprint);

        // CleanBlueprint builds a fresh blueprint and does not carry the version over, because
        // the version is stamped on serialize instead - see GridToBlueprintString.SerializeBlueprint.
        // This test is about the corpus entities already being normalized, so compare on equal terms.
        clean.Version = blueprint.Version;

        Assert.Equal(JsonSerializer.Serialize(blueprint), JsonSerializer.Serialize(clean));
    }
}

