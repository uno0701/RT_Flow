using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Wordprocessing;
using RT.Document.Models;

namespace RT.Document;

/// <summary>
/// Resolves Word paragraph/character style IDs to RT_Flow BlockType values
/// and provides style metadata for FormattingMeta.
/// </summary>
public class StyleResolver
{
    private readonly Dictionary<string, Style> _styles;

    public StyleResolver(WordprocessingDocument doc)
    {
        _styles = new Dictionary<string, Style>(StringComparer.OrdinalIgnoreCase);

        var stylesDoc = doc.MainDocumentPart?.StyleDefinitionsPart;
        if (stylesDoc?.Styles == null) return;

        foreach (var style in stylesDoc.Styles.Elements<Style>())
        {
            var styleId = style.StyleId?.Value;
            if (!string.IsNullOrEmpty(styleId))
                _styles[styleId] = style;
        }
    }

    /// <summary>
    /// Map a Word style ID to a canonical BlockType.
    /// </summary>
    public BlockType ResolveBlockType(string? styleId)
    {
        if (string.IsNullOrEmpty(styleId))
            return BlockType.Paragraph;

        // Normalize: remove spaces, lowercase
        var normalized = styleId.Replace(" ", "").ToLowerInvariant();

        // Heading styles → Section
        if (normalized.StartsWith("heading") || normalized == "title" || normalized == "subtitle")
            return BlockType.Section;

        // List paragraph styles → Clause or Subclause
        if (normalized is "listparagraph" or "listbullet" or "listnumber" or
            "listbullet2" or "listbullet3" or "listnumber2" or "listnumber3" or
            "listcontinue" or "listcontinue2" or "listcontinue3")
        {
            // Deeper list styles map to Subclause
            if (normalized.EndsWith("2") || normalized.EndsWith("3"))
                return BlockType.Subclause;
            return BlockType.Clause;
        }

        // Check the actual style name from the styles dictionary
        if (_styles.TryGetValue(styleId, out var style))
        {
            var name = (style.StyleName?.Val?.Value ?? "").ToLowerInvariant().Replace(" ", "");

            if (name.StartsWith("heading"))
                return BlockType.Section;

            if (name is "listparagraph" or "listbullet" or "listnumber" or
                "listcontinue" or "bodytext" or "bodytext2" or "bodytext3")
            {
                if (name.EndsWith("2") || name.EndsWith("3"))
                    return BlockType.Subclause;
                return BlockType.Clause;
            }
        }

        return BlockType.Paragraph;
    }

    /// <summary>
    /// Get the human-readable style name for metadata (e.g., "Heading 1", "Body Text").
    /// </summary>
    public string? GetStyleName(string? styleId)
    {
        if (string.IsNullOrEmpty(styleId)) return null;
        if (_styles.TryGetValue(styleId, out var style))
            return style.StyleName?.Val?.Value;
        return styleId;
    }

    /// <summary>
    /// Returns true when the style indicates a heading or section-level element.
    /// </summary>
    public bool IsHeading(string? styleId)
    {
        if (string.IsNullOrEmpty(styleId)) return false;
        var normalized = styleId.Replace(" ", "").ToLowerInvariant();
        if (normalized.StartsWith("heading") || normalized == "title" || normalized == "subtitle")
            return true;
        if (_styles.TryGetValue(styleId, out var style))
        {
            var name = (style.StyleName?.Val?.Value ?? "").ToLowerInvariant().Replace(" ", "");
            return name.StartsWith("heading") || name == "title" || name == "subtitle";
        }
        return false;
    }

    /// <summary>
    /// Get the heading level (1–9) for a style, or null if not a heading.
    /// </summary>
    public int? GetHeadingLevel(string? styleId)
    {
        if (string.IsNullOrEmpty(styleId)) return null;

        // Try style name from dictionary first
        string? name = null;
        if (_styles.TryGetValue(styleId, out var style))
            name = style.StyleName?.Val?.Value ?? styleId;
        else
            name = styleId;

        // "Heading 1" → 1, "heading2" → 2, etc.
        var normalized = name.Replace(" ", "").ToLowerInvariant();
        if (normalized.StartsWith("heading") && normalized.Length > 7)
        {
            if (int.TryParse(normalized["heading".Length..], out var level))
                return level;
        }
        return null;
    }
}
