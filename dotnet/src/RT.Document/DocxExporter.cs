using DocumentFormat.OpenXml;
using DocumentFormat.OpenXml.Packaging;
using DocumentFormat.OpenXml.Wordprocessing;
using RT.Document.Models;

// Alias to disambiguate 'Run' and 'Document' types (namespace RT.Document shadows WordprocessingDocument types)
using OxmlRun = DocumentFormat.OpenXml.Wordprocessing.Run;
using ModelRun = RT.Document.Models.Run;
using WpDocument = DocumentFormat.OpenXml.Wordprocessing.Document;

namespace RT.Document;

/// <summary>
/// Exports a list of <see cref="Block"/> objects back to a DOCX file,
/// preserving run formatting and serialising tracked changes as OOXML
/// w:ins / w:del markup.
/// </summary>
public class DocxExporter
{
    // Revision ID counter (OOXML requires unique IDs per revision mark)
    private int _revId = 1;

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// <summary>Export Block list to a DOCX file at the given path.</summary>
    public void Export(List<Block> blocks, string outputPath)
    {
        using var stream = File.Open(outputPath, FileMode.Create, FileAccess.ReadWrite);
        Export(blocks, stream);
    }

    /// <summary>Export Block list to a writable stream.</summary>
    public void Export(List<Block> blocks, Stream output)
    {
        using var doc = WordprocessingDocument.Create(output, WordprocessingDocumentType.Document);

        // Create minimal required parts
        var mainPart = doc.AddMainDocumentPart();
        mainPart.Document = new WpDocument(new Body());

        AddNumberingPart(mainPart, blocks);
        AddStylesPart(mainPart);

        var body = mainPart.Document.Body!;

        // Index child blocks for quick lookup
        var rowBlocks = blocks
            .Where(b => b.BlockType == BlockType.TableRow)
            .GroupBy(b => b.ParentId)
            .Where(g => g.Key.HasValue)
            .ToDictionary(g => g.Key!.Value, g => g.OrderBy(b => b.PositionIndex).ToList());

        var cellBlocks = blocks
            .Where(b => b.BlockType == BlockType.TableCell)
            .GroupBy(b => b.ParentId)
            .Where(g => g.Key.HasValue)
            .ToDictionary(g => g.Key!.Value, g => g.OrderBy(b => b.PositionIndex).ToList());

        // Process only top-level blocks (no parent)
        var topLevel = blocks
            .Where(b => b.ParentId == null
                     && b.BlockType != BlockType.TableRow
                     && b.BlockType != BlockType.TableCell)
            .OrderBy(b => b.PositionIndex)
            .ToList();

        foreach (var block in topLevel)
        {
            if (block.BlockType == BlockType.Table)
            {
                var table = BuildTable(block, rowBlocks, cellBlocks);
                body.Append(table);
            }
            else
            {
                var para = BuildParagraph(block);
                body.Append(para);
            }
        }

        // OOXML requires at least one paragraph in the body
        if (!body.Elements<Paragraph>().Any() && !body.Elements<Table>().Any())
            body.Append(new Paragraph());

        // Append section properties
        body.Append(new SectionProperties());

        mainPart.Document.Save();
    }

    // -----------------------------------------------------------------------
    // Paragraph builder
    // -----------------------------------------------------------------------

    private Paragraph BuildParagraph(Block block)
    {
        var para = new Paragraph();
        var pPr = new ParagraphProperties();

        // Apply style
        var styleName = block.FormattingMeta.StyleName;
        if (!string.IsNullOrEmpty(styleName))
        {
            var styleId = styleName.Replace(" ", "");
            pPr.Append(new ParagraphStyleId { Val = styleId });
        }

        // Apply numbering if present
        if (block.FormattingMeta.NumberingId.HasValue && block.FormattingMeta.NumberingLevel.HasValue)
        {
            var numPr = new NumberingProperties(
                new NumberingLevelReference { Val = block.FormattingMeta.NumberingLevel.Value },
                new NumberingId { Val = block.FormattingMeta.NumberingId.Value }
            );
            pPr.Append(numPr);
        }

        if (pPr.HasChildren)
            para.Append(pPr);

        // Handle tracked changes
        var trackedChange = block.FormattingMeta.TrackedChange;
        var isInsert = trackedChange?.ChangeType == ChangeType.Insert;
        var isDelete = trackedChange?.ChangeType == ChangeType.Delete;

        if (trackedChange != null && (isInsert || isDelete))
        {
            var oxmlRuns = BuildOxmlRuns(block.Runs);
            if (isInsert)
            {
                var ins = new Inserted
                {
                    Author = trackedChange.Author,
                    Date = trackedChange.Date,
                    Id = (_revId++).ToString()
                };
                foreach (var r in oxmlRuns) ins.Append(r);
                para.Append(ins);
            }
            else
            {
                var del = new Deleted
                {
                    Author = trackedChange.Author,
                    Date = trackedChange.Date,
                    Id = (_revId++).ToString()
                };
                foreach (var r in oxmlRuns)
                {
                    var delRun = new DeletedRun();
                    if (r.RunProperties != null)
                        delRun.Append(r.RunProperties.CloneNode(true));
                    foreach (var t in r.Elements<Text>())
                    {
                        delRun.Append(new DeletedText(t.Text ?? "") { Space = SpaceProcessingModeValues.Preserve });
                    }
                    del.Append(delRun);
                }
                para.Append(del);
            }
        }
        else
        {
            foreach (var run in BuildOxmlRuns(block.Runs))
                para.Append(run);
        }

        return para;
    }

    private List<OxmlRun> BuildOxmlRuns(IEnumerable<ModelRun> modelRuns)
    {
        var result = new List<OxmlRun>();
        foreach (var modelRun in modelRuns)
        {
            var run = new OxmlRun();
            var rPr = BuildRunProperties(modelRun.Formatting);
            if (rPr.HasChildren)
                run.Append(rPr);

            run.Append(new Text(modelRun.Text) { Space = SpaceProcessingModeValues.Preserve });
            result.Add(run);
        }
        return result;
    }

    private static RunProperties BuildRunProperties(RunFormatting fmt)
    {
        var rPr = new RunProperties();

        if (fmt.Bold)          rPr.Append(new Bold());
        if (fmt.Italic)        rPr.Append(new Italic());
        if (fmt.Strikethrough) rPr.Append(new Strike());

        if (fmt.Underline)
            rPr.Append(new Underline { Val = UnderlineValues.Single });

        if (fmt.FontSize.HasValue)
        {
            var halfPts = (int)(fmt.FontSize.Value * 2);
            rPr.Append(new FontSize { Val = halfPts.ToString() });
        }

        if (!string.IsNullOrEmpty(fmt.Color))
        {
            var hex = fmt.Color.TrimStart('#');
            rPr.Append(new Color { Val = hex });
        }

        return rPr;
    }

    // -----------------------------------------------------------------------
    // Table builder
    // -----------------------------------------------------------------------

    private Table BuildTable(
        Block tableBlock,
        Dictionary<Guid, List<Block>> rowBlocks,
        Dictionary<Guid, List<Block>> cellBlocks)
    {
        var table = new Table();

        var tblPr = new TableProperties(
            new TableStyle { Val = "TableGrid" },
            new TableWidth { Width = "0", Type = TableWidthUnitValues.Auto }
        );
        table.Append(tblPr);

        if (!rowBlocks.TryGetValue(tableBlock.Id, out var rows))
            return table;

        foreach (var rowBlock in rows)
        {
            var row = new TableRow();

            if (!cellBlocks.TryGetValue(rowBlock.Id, out var cells))
            {
                table.Append(row);
                continue;
            }

            foreach (var cellBlock in cells)
            {
                var cell = new TableCell();
                var cellPara = BuildParagraph(cellBlock);
                cell.Append(cellPara);
                row.Append(cell);
            }

            table.Append(row);
        }

        return table;
    }

    // -----------------------------------------------------------------------
    // Supporting parts
    // -----------------------------------------------------------------------

    private static void AddStylesPart(MainDocumentPart mainPart)
    {
        var stylesPart = mainPart.AddNewPart<StyleDefinitionsPart>();
        var styles = new Styles();

        styles.Append(CreateStyle("Normal", "Normal", StyleValues.Paragraph, isDefault: true));
        for (var i = 1; i <= 6; i++)
            styles.Append(CreateStyle($"Heading{i}", $"Heading {i}", StyleValues.Paragraph));

        styles.Append(CreateStyle("ListParagraph", "List Paragraph", StyleValues.Paragraph));
        stylesPart.Styles = styles;
    }

    private static Style CreateStyle(string styleId, string styleName, StyleValues type, bool isDefault = false)
    {
        var style = new Style
        {
            Type = type,
            StyleId = styleId,
        };
        if (isDefault) style.Default = true;
        style.Append(new StyleName { Val = styleName });
        return style;
    }

    private static void AddNumberingPart(MainDocumentPart mainPart, List<Block> blocks)
    {
        var numIds = blocks
            .Where(b => b.FormattingMeta.NumberingId.HasValue)
            .Select(b => b.FormattingMeta.NumberingId!.Value)
            .Distinct()
            .ToList();

        if (numIds.Count == 0) return;

        var numberingPart = mainPart.AddNewPart<NumberingDefinitionsPart>();
        var numbering = new Numbering();

        foreach (var numId in numIds)
        {
            var abstractNum = new AbstractNum { AbstractNumberId = numId };
            abstractNum.Append(new AbstractNumDefinitionName { Val = $"abstract{numId}" });

            for (var ilvl = 0; ilvl < 9; ilvl++)
            {
                var level = new Level { LevelIndex = ilvl };
                level.Append(new StartNumberingValue { Val = 1 });
                level.Append(new NumberingFormat { Val = NumberFormatValues.Decimal });
                level.Append(new LevelText { Val = $"%{ilvl + 1}." });
                level.Append(new LevelJustification { Val = LevelJustificationValues.Left });
                abstractNum.Append(level);
            }

            numbering.Append(abstractNum);

            var numInstance = new NumberingInstance { NumberID = numId };
            numInstance.Append(new AbstractNumId { Val = numId });
            numbering.Append(numInstance);
        }

        numberingPart.Numbering = numbering;
    }
}
