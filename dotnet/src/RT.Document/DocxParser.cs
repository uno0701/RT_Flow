using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;
using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Wordprocessing;
using RT.Document.Models;

// Alias to disambiguate the two 'Run' types
using OxmlRun = DocumentFormat.OpenXml.Wordprocessing.Run;
using ModelRun = RT.Document.Models.Run;

namespace RT.Document;

/// <summary>
/// Parses a DOCX file using the Open XML SDK and produces a flat list of
/// <see cref="Block"/> objects in the canonical RT_Flow block model.
/// </summary>
public class DocxParser
{
    private static readonly Regex NumberingPrefixPattern = new(
        @"^[\s]*(?:\d+(?:\.\d+)*[.\):]?|[a-zA-Z][.\):]?|\([a-zA-Z0-9]+\)|[ivxlcdmIVXLCDM]+[.\):]?)\s+",
        RegexOptions.Compiled);

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// <summary>Parse a DOCX file at the given path and return Block list.</summary>
    public List<Block> Parse(string filePath)
    {
        using var stream = File.OpenRead(filePath);
        return Parse(stream);
    }

    /// <summary>Parse a DOCX from a stream and return Block list.</summary>
    public List<Block> Parse(Stream stream)
    {
        using var doc = WordprocessingDocument.Open(stream, isEditable: false);
        return ExtractBlocks(doc);
    }

    // -----------------------------------------------------------------------
    // Core extraction
    // -----------------------------------------------------------------------

    private List<Block> ExtractBlocks(WordprocessingDocument doc)
    {
        var body = doc.MainDocumentPart?.Document?.Body;
        if (body == null) return new List<Block>();

        var docId = Guid.NewGuid();
        var styles = new StyleResolver(doc);
        var numbering = new NumberingResolver(doc);
        var blocks = new List<Block>();
        var index = 0;

        foreach (var element in body.Elements())
        {
            if (element is Paragraph para)
            {
                // Skip empty paragraphs with no meaningful content
                var text = ExtractParagraphText(para);
                if (string.IsNullOrWhiteSpace(text) && !HasTrackedChanges(para))
                    continue;

                var block = ProcessParagraph(para, index, docId, null, styles, numbering);
                blocks.Add(block);
                index++;
            }
            else if (element is Table table)
            {
                var tableBlocks = ProcessTable(table, index, docId, null, styles, numbering);
                blocks.AddRange(tableBlocks);
                index += tableBlocks.Count;
            }
        }

        return blocks;
    }

    // -----------------------------------------------------------------------
    // Paragraph processing
    // -----------------------------------------------------------------------

    private Block ProcessParagraph(
        Paragraph para,
        int index,
        Guid docId,
        Guid? parentId,
        StyleResolver styles,
        NumberingResolver numbering,
        int depth = 0)
    {
        var id = Guid.NewGuid();
        var runs = ExtractRuns(para);
        var displayText = string.Concat(runs.Select(r => r.Text));
        var canonicalText = NormalizeCanonical(displayText);
        var structuralPath = numbering.ResolveStructuralPath(para);
        var meta = ExtractMeta(para, numbering, styles);
        var blockType = ResolveBlockTypeForParagraph(para, styles);

        // Strip numbering prefix from canonical text
        var canonicalNoPrefix = StripNumberingPrefix(canonicalText);

        var anchorSig = ComputeAnchorSignature(blockType, structuralPath, canonicalNoPrefix);
        var clauseHash = ComputeClauseHash(canonicalNoPrefix);
        var tokens = Tokenizer.Tokenize(canonicalNoPrefix);

        return new Block(
            Id: id,
            DocumentId: docId,
            ParentId: parentId,
            BlockType: blockType,
            Level: depth,
            StructuralPath: structuralPath,
            AnchorSignature: anchorSig,
            ClauseHash: clauseHash,
            CanonicalText: canonicalNoPrefix,
            DisplayText: displayText,
            FormattingMeta: meta,
            PositionIndex: index,
            Tokens: tokens,
            Runs: runs,
            Children: new List<Block>()
        );
    }

    private BlockType ResolveBlockTypeForParagraph(Paragraph para, StyleResolver styles)
    {
        var styleId = para.ParagraphProperties?.ParagraphStyleId?.Val?.Value;
        return styles.ResolveBlockType(styleId);
    }

    // -----------------------------------------------------------------------
    // Table processing
    // -----------------------------------------------------------------------

    private List<Block> ProcessTable(
        Table table,
        int startIndex,
        Guid docId,
        Guid? parentId,
        StyleResolver styles,
        NumberingResolver numbering,
        int depth = 0)
    {
        var blocks = new List<Block>();
        var tableId = Guid.NewGuid();
        var tableIndex = startIndex;

        // Create the Table block itself
        var tableBlock = new Block(
            Id: tableId,
            DocumentId: docId,
            ParentId: parentId,
            BlockType: BlockType.Table,
            Level: depth,
            StructuralPath: string.Empty,
            AnchorSignature: ComputeAnchorSignature(BlockType.Table, string.Empty, $"table:{tableIndex}"),
            ClauseHash: ComputeClauseHash(string.Empty),
            CanonicalText: string.Empty,
            DisplayText: string.Empty,
            FormattingMeta: new FormattingMeta(null, null, null, false, null),
            PositionIndex: tableIndex,
            Tokens: new List<Token>(),
            Runs: new List<ModelRun>(),
            Children: new List<Block>()
        );
        blocks.Add(tableBlock);

        var childIndex = 0;
        foreach (var row in table.Elements<TableRow>())
        {
            var rowId = Guid.NewGuid();
            var rowBlock = new Block(
                Id: rowId,
                DocumentId: docId,
                ParentId: tableId,
                BlockType: BlockType.TableRow,
                Level: depth + 1,
                StructuralPath: string.Empty,
                AnchorSignature: ComputeAnchorSignature(BlockType.TableRow, string.Empty, $"row:{childIndex}"),
                ClauseHash: ComputeClauseHash(string.Empty),
                CanonicalText: string.Empty,
                DisplayText: string.Empty,
                FormattingMeta: new FormattingMeta(null, null, null, false, null),
                PositionIndex: childIndex,
                Tokens: new List<Token>(),
                Runs: new List<ModelRun>(),
                Children: new List<Block>()
            );
            blocks.Add(rowBlock);

            var cellIndex = 0;
            foreach (var cell in row.Elements<TableCell>())
            {
                var cellId = Guid.NewGuid();
                // Extract all text from cell paragraphs
                var cellParagraphs = cell.Elements<Paragraph>().ToList();
                var cellRuns = cellParagraphs.SelectMany(ExtractRuns).ToList();
                var cellDisplayText = string.Concat(cellRuns.Select(r => r.Text));
                var cellCanonical = NormalizeCanonical(cellDisplayText);
                var cellTokens = Tokenizer.Tokenize(cellCanonical);

                var cellBlock = new Block(
                    Id: cellId,
                    DocumentId: docId,
                    ParentId: rowId,
                    BlockType: BlockType.TableCell,
                    Level: depth + 2,
                    StructuralPath: string.Empty,
                    AnchorSignature: ComputeAnchorSignature(BlockType.TableCell, string.Empty, cellCanonical),
                    ClauseHash: ComputeClauseHash(cellCanonical),
                    CanonicalText: cellCanonical,
                    DisplayText: cellDisplayText,
                    FormattingMeta: new FormattingMeta(null, null, null, false, null),
                    PositionIndex: cellIndex,
                    Tokens: cellTokens,
                    Runs: cellRuns,
                    Children: new List<Block>()
                );
                blocks.Add(cellBlock);
                cellIndex++;
            }
            childIndex++;
        }

        return blocks;
    }

    // -----------------------------------------------------------------------
    // Run extraction and formatting
    // -----------------------------------------------------------------------

    /// <summary>
    /// Extract all runs from a paragraph, including runs inside tracked-change
    /// elements (w:ins / w:del).
    /// </summary>
    private List<ModelRun> ExtractRuns(Paragraph para)
    {
        var result = new List<ModelRun>();

        foreach (var child in para.ChildElements)
        {
            ExtractRunsFromElement(child, result);
        }

        return result;
    }

    private void ExtractRunsFromElement(OpenXmlElement element, List<ModelRun> result)
    {
        switch (element)
        {
            case OxmlRun run:
            {
                var text = run.GetRunText();
                if (!string.IsNullOrEmpty(text))
                {
                    var fmt = GetFormatting(run.RunProperties);
                    result.Add(new ModelRun(text, fmt));
                }
                break;
            }
            case Inserted ins:
            {
                foreach (var r in ins.Elements<OxmlRun>())
                    ExtractRunsFromElement(r, result);
                break;
            }
            case Deleted del:
            {
                // For deleted text, include original content from w:delText
                foreach (var r in del.Elements<DeletedRun>())
                {
                    var delText = string.Concat(r.Elements<DeletedText>().Select(dt => dt.Text ?? ""));
                    if (!string.IsNullOrEmpty(delText))
                    {
                        var fmt = GetFormatting(r.GetFirstChild<RunProperties>());
                        result.Add(new ModelRun(delText, fmt));
                    }
                }
                break;
            }
            default:
            {
                // Recurse into complex field instructions, SDTs, etc.
                foreach (var child in element.ChildElements)
                    ExtractRunsFromElement(child, result);
                break;
            }
        }
    }

    private RunFormatting GetFormatting(RunProperties? props)
    {
        if (props == null)
            return new RunFormatting(false, false, false, false, null, null);

        var bold = props.Bold != null && props.Bold.Val?.Value != false;
        var italic = props.Italic != null && props.Italic.Val?.Value != false;
        var underline = props.Underline != null &&
                        props.Underline.Val?.Value != UnderlineValues.None;
        var strikethrough = props.Strike != null && props.Strike.Val?.Value != false;

        float? fontSize = null;
        if (props.FontSize?.Val?.Value is { } sz)
        {
            // Half-points → points
            if (float.TryParse(sz, out var szPt))
                fontSize = szPt / 2f;
        }

        string? color = null;
        var colorVal = props.Color?.Val?.Value;
        if (!string.IsNullOrEmpty(colorVal) && colorVal != "auto")
            color = $"#{colorVal}";

        return new RunFormatting(bold, italic, underline, strikethrough, fontSize, color);
    }

    // -----------------------------------------------------------------------
    // Tracked changes
    // -----------------------------------------------------------------------

    private static bool HasTrackedChanges(Paragraph para)
    {
        return para.Descendants<Inserted>().Any() || para.Descendants<Deleted>().Any();
    }

    private static TrackedChange? ExtractTrackedChange(Paragraph para)
    {
        // Check for insertion
        var ins = para.Descendants<Inserted>().FirstOrDefault();
        if (ins != null)
        {
            var author = ins.Author?.Value ?? "Unknown";
            var dateVal = ins.Date?.Value;
            var date = dateVal.HasValue ? dateVal.Value : DateTime.UtcNow;
            return new TrackedChange(author, date, ChangeType.Insert, null);
        }

        // Check for deletion
        var del = para.Descendants<Deleted>().FirstOrDefault();
        if (del != null)
        {
            var author = del.Author?.Value ?? "Unknown";
            var dateVal = del.Date?.Value;
            var date = dateVal.HasValue ? dateVal.Value : DateTime.UtcNow;
            var originalText = string.Concat(
                del.Descendants<DeletedText>().Select(dt => dt.Text ?? ""));
            return new TrackedChange(author, date, ChangeType.Delete,
                string.IsNullOrEmpty(originalText) ? null : originalText);
        }

        return null;
    }

    // -----------------------------------------------------------------------
    // Metadata extraction
    // -----------------------------------------------------------------------

    private FormattingMeta ExtractMeta(
        Paragraph para,
        NumberingResolver numbering,
        StyleResolver styles)
    {
        var styleId = para.ParagraphProperties?.ParagraphStyleId?.Val?.Value;
        var styleName = styles.GetStyleName(styleId);
        var (numId, numLevel) = numbering.GetNumberingInfo(para);
        var trackedChange = ExtractTrackedChange(para);
        var isRedline = trackedChange != null;

        return new FormattingMeta(
            StyleName: styleName,
            NumberingId: numId,
            NumberingLevel: numLevel,
            IsRedline: isRedline,
            TrackedChange: trackedChange
        );
    }

    // -----------------------------------------------------------------------
    // Text helpers
    // -----------------------------------------------------------------------

    private static string ExtractParagraphText(Paragraph para)
    {
        var sb = new StringBuilder();
        foreach (var run in para.Descendants<OxmlRun>())
            sb.Append(run.GetRunText());
        foreach (var del in para.Descendants<DeletedRun>())
            foreach (var dt in del.Elements<DeletedText>())
                sb.Append(dt.Text ?? "");
        return sb.ToString();
    }

    public static string NormalizeCanonical(string text)
    {
        if (string.IsNullOrEmpty(text)) return string.Empty;

        // Collapse whitespace (tabs, newlines, multiple spaces → single space)
        var sb = new StringBuilder(text.Length);
        var prevWs = false;
        foreach (var c in text)
        {
            if (char.IsWhiteSpace(c))
            {
                if (!prevWs) sb.Append(' ');
                prevWs = true;
            }
            else
            {
                sb.Append(c);
                prevWs = false;
            }
        }
        return sb.ToString().Trim();
    }

    private static string StripNumberingPrefix(string text)
    {
        var match = NumberingPrefixPattern.Match(text);
        return match.Success ? text[match.Length..].TrimStart() : text;
    }

    // -----------------------------------------------------------------------
    // Hashing (mirrors Rust implementation)
    // -----------------------------------------------------------------------

    public static string ComputeAnchorSignature(BlockType type, string path, string text)
    {
        var first128 = text.Length > 128 ? text[..128] : text;
        var input = $"{BlockTypeToString(type)}|{path}|{first128}";
        return Sha256Hex(input);
    }

    public static string ComputeClauseHash(string canonicalText)
    {
        return Sha256Hex(canonicalText);
    }

    public static string Sha256Hex(string input)
    {
        var bytes = Encoding.UTF8.GetBytes(input);
        var hash = SHA256.HashData(bytes);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }

    private static string BlockTypeToString(BlockType type) => type switch
    {
        BlockType.Section    => "section",
        BlockType.Clause     => "clause",
        BlockType.Subclause  => "subclause",
        BlockType.Paragraph  => "paragraph",
        BlockType.Table      => "table",
        BlockType.TableRow   => "table_row",
        BlockType.TableCell  => "table_cell",
        _                    => "paragraph",
    };
}

/// <summary>
/// Extension methods for Open XML run text extraction.
/// </summary>
internal static class OpenXmlExtensions
{
    /// <summary>
    /// Get the concatenated text content of a Run, handling w:t with xml:space="preserve".
    /// </summary>
    public static string GetRunText(this OxmlRun run)
    {
        var sb = new StringBuilder();
        foreach (var child in run.ChildElements)
        {
            if (child is Text t)
                sb.Append(t.Text ?? "");
            else if (child is Break br)
            {
                var breakType = br.Type?.Value;
                if (breakType == BreakValues.TextWrapping || breakType == null)
                    sb.Append('\n');
            }
        }
        return sb.ToString();
    }
}
