using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Wordprocessing;

namespace RT.Document;

/// <summary>
/// Resolves OOXML numbering definitions to structural path strings
/// (e.g., "1.2(a)(iii)") and numbering metadata.
/// </summary>
public class NumberingResolver
{
    // abstractNumId → (ilvl → (format, start))
    private readonly Dictionary<int, Dictionary<int, (string Format, int Start, string LevelText)>> _abstractNums;
    // numId → (abstractNumId, level-overrides)
    private readonly Dictionary<int, (int AbstractNumId, Dictionary<int, int> StartOverrides)> _nums;
    // Current counters: (numId, ilvl) → count
    private readonly Dictionary<(int, int), int> _counters = new();

    public NumberingResolver(WordprocessingDocument doc)
    {
        _abstractNums = new Dictionary<int, Dictionary<int, (string, int, string)>>();
        _nums = new Dictionary<int, (int, Dictionary<int, int>)>();

        var numberingPart = doc.MainDocumentPart?.NumberingDefinitionsPart;
        if (numberingPart?.Numbering == null) return;

        // Load abstract numbering definitions
        foreach (var abstractNum in numberingPart.Numbering.Elements<AbstractNum>())
        {
            var abstractId = abstractNum.AbstractNumberId?.Value ?? 0;
            var levels = new Dictionary<int, (string, int, string)>();

            foreach (var lvl in abstractNum.Elements<Level>())
            {
                var ilvl = lvl.LevelIndex?.Value ?? 0;
                var format = lvl.NumberingFormat?.Val?.Value.ToString() ?? "decimal";
                var start = (int)(lvl.StartNumberingValue?.Val?.Value ?? 1);
                var levelText = lvl.LevelText?.Val?.Value ?? "%1.";
                levels[ilvl] = (format, start, levelText);
            }
            _abstractNums[abstractId] = levels;
        }

        // Load num instances
        foreach (var num in numberingPart.Numbering.Elements<NumberingInstance>())
        {
            var numId = (int)(num.NumberID?.Value ?? 0);
            var abstractNumRef = num.AbstractNumId?.Val?.Value ?? 0;
            var overrides = new Dictionary<int, int>();

            foreach (var lvlOverride in num.Elements<LevelOverride>())
            {
                var ilvl = lvlOverride.LevelIndex?.Value ?? 0;
                var startOverride = lvlOverride.StartOverrideNumberingValue?.Val?.Value;
                if (startOverride.HasValue)
                    overrides[(int)ilvl] = (int)startOverride.Value;
            }
            _nums[numId] = (abstractNumRef, overrides);
        }
    }

    /// <summary>
    /// Get the numbering instance ID and level for a paragraph.
    /// Returns (null, null) for non-numbered paragraphs.
    /// </summary>
    public (int? id, int? level) GetNumberingInfo(Paragraph para)
    {
        var numPr = para.ParagraphProperties?.NumberingProperties;
        if (numPr == null) return (null, null);

        var numId = (int?)numPr.NumberingId?.Val?.Value;
        var ilvl = (int?)numPr.NumberingLevelReference?.Val?.Value;

        if (numId == null || numId == 0) return (null, null);
        return (numId, ilvl ?? 0);
    }

    /// <summary>
    /// Get the structural path for a paragraph (e.g., "1.2(a)(iii)").
    /// Advances the counter for numbered paragraphs.
    /// </summary>
    public string ResolveStructuralPath(Paragraph para)
    {
        var (numId, ilvl) = GetNumberingInfo(para);
        if (numId == null || ilvl == null) return string.Empty;

        var key = (numId.Value, ilvl.Value);

        // Advance the counter for this level, reset deeper levels
        if (!_counters.TryGetValue(key, out var current))
        {
            // Determine start value
            var start = GetStartValue(numId.Value, ilvl.Value);
            _counters[key] = start;
            current = start;
        }
        else
        {
            _counters[key] = current + 1;
            current = _counters[key];
        }

        // Reset all deeper levels for the same numId
        var keysToReset = _counters.Keys
            .Where(k => k.Item1 == numId.Value && k.Item2 > ilvl.Value)
            .ToList();
        foreach (var k in keysToReset)
            _counters.Remove(k);

        // Build path by collecting all ancestor levels
        var pathParts = new List<string>();
        for (var l = 0; l <= ilvl.Value; l++)
        {
            var lKey = (numId.Value, l);
            var count = _counters.TryGetValue(lKey, out var c) ? c : GetStartValue(numId.Value, l);
            var formatted = FormatNumber(count, numId.Value, l);
            pathParts.Add(formatted);
        }

        return string.Join("", pathParts);
    }

    private int GetStartValue(int numId, int ilvl)
    {
        if (_nums.TryGetValue(numId, out var numInfo))
        {
            if (numInfo.StartOverrides.TryGetValue(ilvl, out var startOverride))
                return startOverride;

            if (_abstractNums.TryGetValue(numInfo.AbstractNumId, out var levels))
            {
                if (levels.TryGetValue(ilvl, out var levelInfo))
                    return levelInfo.Start;
            }
        }
        return 1;
    }

    private string FormatNumber(int value, int numId, int ilvl)
    {
        string format = "decimal";
        if (_nums.TryGetValue(numId, out var numInfo))
        {
            if (_abstractNums.TryGetValue(numInfo.AbstractNumId, out var levels))
            {
                if (levels.TryGetValue(ilvl, out var levelInfo))
                    format = levelInfo.Format.ToLowerInvariant();
            }
        }

        return format switch
        {
            "decimal" => ilvl == 0 ? $"{value}." : $"{value}.",
            "lowerletter" or "lowerLetter" => $"({ToLetter(value, false)})",
            "upperletter" or "upperLetter" => $"({ToLetter(value, true)})",
            "lowerroman" or "lowerRoman" => $"({ToRoman(value, false)})",
            "upperroman" or "upperRoman" => $"({ToRoman(value, true)})",
            "bullet" => "",
            _ => $"{value}.",
        };
    }

    private static string ToLetter(int value, bool upper)
    {
        var letters = new List<char>();
        while (value > 0)
        {
            value--;
            letters.Insert(0, (char)('a' + value % 26));
            value /= 26;
        }
        var s = new string(letters.ToArray());
        return upper ? s.ToUpperInvariant() : s;
    }

    private static string ToRoman(int value, bool upper)
    {
        var romanNumerals = new[]
        {
            (1000, "m"), (900, "cm"), (500, "d"), (400, "cd"),
            (100, "c"), (90, "xc"), (50, "l"), (40, "xl"),
            (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")
        };
        var result = "";
        foreach (var (val, numeral) in romanNumerals)
        {
            while (value >= val)
            {
                result += numeral;
                value -= val;
            }
        }
        return upper ? result.ToUpperInvariant() : result;
    }
}
