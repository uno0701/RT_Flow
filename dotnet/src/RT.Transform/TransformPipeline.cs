using System.Security.Cryptography;
using System.Text;
using System.Text.Json;
using System.Text.Json.Serialization;
using System.Text.RegularExpressions;
using RT.Document;
using RT.Document.Models;

namespace RT.Transform;

/// <summary>
/// Full DOCX → Block[] JSON transform pipeline.
/// Encapsulates parsing, normalization, hash computation, and JSON serialization.
/// </summary>
public class TransformPipeline
{
    private static readonly JsonSerializerOptions JsonOptions = new()
    {
        WriteIndented = false,
        DefaultIgnoreCondition = JsonIgnoreCondition.Never,
        Converters = { new JsonStringEnumConverter() },
    };

    // Matches common legal numbering prefixes: "1.", "(a)", "1.2(a)(iii)", etc.
    private static readonly Regex NumberingPrefixPattern = new(
        @"^[\s]*(?:\d+(?:\.\d+)*[.\):]?|[a-zA-Z][.\):]?|\([a-zA-Z0-9]+\)|[ivxlcdmIVXLCDM]+[.\):]?)\s+",
        RegexOptions.Compiled);

    // -----------------------------------------------------------------------
    // Public pipeline entry point
    // -----------------------------------------------------------------------

    /// <summary>
    /// Full pipeline: DOCX file path → Block[] JSON string.
    /// </summary>
    public string Transform(string docxPath)
    {
        var parser = new DocxParser();
        var blocks = parser.Parse(docxPath);
        return JsonSerializer.Serialize(blocks, JsonOptions);
    }

    /// <summary>
    /// Full pipeline from stream: DOCX stream → Block[] JSON string.
    /// </summary>
    public string Transform(Stream docxStream)
    {
        var parser = new DocxParser();
        var blocks = parser.Parse(docxStream);
        return JsonSerializer.Serialize(blocks, JsonOptions);
    }

    // -----------------------------------------------------------------------
    // Static text-processing helpers (used by pipeline and tests)
    // -----------------------------------------------------------------------

    /// <summary>
    /// Normalize canonical text: collapse whitespace, strip numbering prefix.
    /// Mirrors the Rust <c>normalize_canonical</c> function.
    /// </summary>
    public static string NormalizeCanonical(string text)
    {
        if (string.IsNullOrEmpty(text)) return string.Empty;

        // Step 1: collapse all whitespace runs to a single space and trim
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
        var collapsed = sb.ToString().Trim();

        // Step 2: strip leading numbering prefix
        var match = NumberingPrefixPattern.Match(collapsed);
        return match.Success ? collapsed[match.Length..].TrimStart() : collapsed;
    }

    /// <summary>
    /// Compute SHA-256 hex digest of a UTF-8 string.
    /// Mirrors the Rust <c>sha256_hex</c> function.
    /// </summary>
    public static string Sha256Hex(string input)
    {
        var bytes = Encoding.UTF8.GetBytes(input);
        var hash = SHA256.HashData(bytes);
        return Convert.ToHexString(hash).ToLowerInvariant();
    }

    /// <summary>
    /// Compute the anchor signature: SHA-256( type | path | first128chars ).
    /// Mirrors the Rust <c>compute_anchor_signature</c> function.
    /// </summary>
    public static string ComputeAnchorSignature(BlockType type, string path, string text)
    {
        var first128 = text.Length > 128 ? text[..128] : text;
        var typeStr = BlockTypeToString(type);
        var input = $"{typeStr}|{path}|{first128}";
        return Sha256Hex(input);
    }

    /// <summary>
    /// Compute the clause hash: SHA-256( canonical_text ).
    /// Mirrors the Rust <c>compute_clause_hash</c> function.
    /// </summary>
    public static string ComputeClauseHash(string canonicalText)
    {
        return Sha256Hex(canonicalText);
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

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
