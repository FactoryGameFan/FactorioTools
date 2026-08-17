using System;
using System.Collections.Generic;
using System.IO;
using System.IO.Compression;
using System.Linq;
using System.Text.Json;
using Knapcode.FactorioTools.Data;

namespace Knapcode.FactorioTools.OilField;

public static class ParseBlueprint
{
    /// <summary>
    /// The blueprint version at which Factorio widened directions from 8-way to 16-way.
    /// GridToBlueprintString.FormatVersion(2, 0, 0, 0). Confirmed against a real 2.1.14
    /// export, whose version is 562954249306113 (2.1.14.1).
    /// </summary>
    private const ulong FirstSixteenWayVersion = 562949953421312UL;

    /// <summary>
    /// Converts a blueprint's direction to the internal 1.1-style four-way
    /// <see cref="Direction"/> (N=0, E=2, S=4, W=6).
    ///
    /// Factorio 2.0 widened directions to 16-way (N=0, E=4, S=8, W=12). The old values are
    /// still legal, so a 2.x east read as 1.1 is not an error, it is a silently rotated
    /// pumpjack. Sniffing the values cannot resolve it either: a blueprint whose directions
    /// are all in {0, 4} is valid under both readings with different meanings. The version
    /// is the only sound signal.
    ///
    /// A missing or zero version is treated as modern, because that is what users paste
    /// today. The trade-off is spelled out in the design doc.
    /// </summary>
    public static Direction ToInternalDirection(Direction direction, ulong version)
    {
        var raw = (int)direction;
        var internalValue = version == 0 || version >= FirstSixteenWayVersion ? raw / 2 : raw;

        if (internalValue != (int)Direction.Up
            && internalValue != (int)Direction.Right
            && internalValue != (int)Direction.Down
            && internalValue != (int)Direction.Left)
        {
            throw new FactorioToolsException(
                $"Blueprint direction {raw} is not one of the four directions a pumpjack can face.",
                badInput: true);
        }

        return (Direction)internalValue;
    }

    public static List<string> ReadBlueprintFile(string fileName)
    {
        return File
            .ReadAllLines(fileName)
            .Select(x => x.Trim())
            .Where(x => x.Length > 0 && !x.StartsWith("#"))
            .ToList();
    }

    public static Blueprint Execute(string blueprintString)
    {
        if (string.IsNullOrEmpty(blueprintString))
        {
            throw new FactorioToolsException("Input blueprint string is empty.", badInput: true);
        }

        if (blueprintString[0] != '0')
        {
            throw new FactorioToolsException("Input blueprint does not have the expected version byte of '0' at the beginning.", badInput: true);
        }

        BlueprintRoot? root;
        bool hadMissingPadding = false;
        bool looksLikeJson = true;
        try
        {
            var base64 = blueprintString.Substring(1); // skip the version byte
            var missingPadding = (4 - (base64.Length % 4)) % 4;
            if (missingPadding > 0)
            {
                base64 += new string('=', missingPadding);
                hadMissingPadding = true;
            }

            var bytes = Convert.FromBase64String(base64);

            using var inputStream = new MemoryStream(bytes);
            using var zlibStream = new ZLibStream(inputStream, CompressionMode.Decompress);
            using var streamReader = new StreamReader(zlibStream);
            var json = streamReader.ReadToEnd();

            looksLikeJson = json.StartsWith('{');

            root = JsonSerializer.Deserialize(
                json,
                typeof(BlueprintRoot),
                BlueprintSerialization.Context) as BlueprintRoot;
        }
        catch (Exception ex) when (ex is FormatException || ex is JsonException || hadMissingPadding || looksLikeJson)
        {
            throw new FactorioToolsException("Input blueprint string could not be fully decoded. Are you sure you copied the whole blueprint?", ex, badInput: true);
        }
        catch (Exception ex)
        {
            throw new FactorioToolsException("Input blueprint string could not be decoded.", ex, badInput: true);
        }

        if (root == null)
        {
            throw new FactorioToolsException("The blueprint JSON deserialized as null.", badInput: true);
        }

        if (root.BlueprintBook is not null)
        {
            throw new FactorioToolsException("The blueprint provided contains a blueprint book, not an individual blueprint.", badInput: true);
        }

        if (root.Blueprint is null)
        {
            throw new FactorioToolsException("No blueprint was found in the deserialized JSON.", badInput: true);
        }

        for (var i = 0; i < root.Blueprint.Entities.Length; i++)
        {
            var entity = root.Blueprint.Entities[i];
            if (entity.Direction.HasValue)
            {
                entity.Direction = ToInternalDirection(entity.Direction.Value, root.Blueprint.Version);
            }
        }

        return root.Blueprint;
    }
}
