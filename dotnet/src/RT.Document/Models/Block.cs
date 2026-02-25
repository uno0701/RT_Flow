using System;
using System.Collections.Generic;
using System.Text.Json.Serialization;

namespace RT.Document.Models;

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/// <summary>
/// Structural role of a block within a legal document.
/// Mirrors the Rust <c>BlockType</c> enum; serialised as a snake_case string.
/// </summary>
[JsonConverter(typeof(JsonStringEnumConverter))]
public enum BlockType
{
    [JsonPropertyName("section")]    Section,
    [JsonPropertyName("clause")]     Clause,
    [JsonPropertyName("subclause")]  Subclause,
    [JsonPropertyName("paragraph")]  Paragraph,
    [JsonPropertyName("table")]      Table,
    [JsonPropertyName("table_row")]  TableRow,
    [JsonPropertyName("table_cell")] TableCell,
}

/// <summary>
/// Lexical category of a token.
/// Mirrors the Rust <c>TokenKind</c> enum; serialised as a snake_case string.
/// </summary>
[JsonConverter(typeof(JsonStringEnumConverter))]
public enum TokenKind
{
    [JsonPropertyName("word")]         Word,
    [JsonPropertyName("number")]       Number,
    [JsonPropertyName("punctuation")]  Punctuation,
    [JsonPropertyName("whitespace")]   Whitespace,
    [JsonPropertyName("defined_term")] DefinedTerm,
    [JsonPropertyName("party_ref")]    PartyRef,
    [JsonPropertyName("date_ref")]     DateRef,
}

/// <summary>
/// Nature of a tracked revision.
/// Mirrors the Rust <c>ChangeType</c> enum; serialised as a snake_case string.
/// </summary>
[JsonConverter(typeof(JsonStringEnumConverter))]
public enum ChangeType
{
    [JsonPropertyName("insert")]        Insert,
    [JsonPropertyName("delete")]        Delete,
    [JsonPropertyName("format_change")] FormatChange,
}

/// <summary>
/// Provenance classification of a document ingested into RT_Flow.
/// Mirrors the Rust <c>DocumentType</c> enum; serialised as a snake_case string.
/// </summary>
[JsonConverter(typeof(JsonStringEnumConverter))]
public enum DocumentType
{
    [JsonPropertyName("original")] Original,
    [JsonPropertyName("redline")]  Redline,
    [JsonPropertyName("merged")]   Merged,
    [JsonPropertyName("snapshot")] Snapshot,
}

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/// <summary>
/// Atomic unit of text produced by the tokenizer.
///
/// <c>offset</c> is the byte offset of the token's first character within
/// the parent block's <c>canonical_text</c>.
///
/// Mirrors the Rust <c>Token</c> struct.
/// </summary>
public record Token(
    /// <summary>Raw text as it appears in the document.</summary>
    [property: JsonPropertyName("text")]       string Text,
    /// <summary>Semantic classification of this token.</summary>
    [property: JsonPropertyName("kind")]       TokenKind Kind,
    /// <summary>
    /// Lowercased, whitespace-normalised form used for matching and search.
    /// </summary>
    [property: JsonPropertyName("normalized")] string Normalized,
    /// <summary>Byte offset within the parent block's canonical_text.</summary>
    [property: JsonPropertyName("offset")]     int Offset
);

// ---------------------------------------------------------------------------
// RunFormatting / Run
// ---------------------------------------------------------------------------

/// <summary>
/// Typographic attributes attached to a <see cref="Run"/>.
/// Mirrors the Rust <c>RunFormatting</c> struct.
/// </summary>
public record RunFormatting(
    [property: JsonPropertyName("bold")]          bool Bold,
    [property: JsonPropertyName("italic")]        bool Italic,
    [property: JsonPropertyName("underline")]     bool Underline,
    [property: JsonPropertyName("strikethrough")] bool Strikethrough,
    /// <summary>
    /// Point size, if explicitly set; <c>null</c> means "inherit from style".
    /// </summary>
    [property: JsonPropertyName("font_size")]     float? FontSize,
    /// <summary>
    /// CSS-style hex colour string (e.g. <c>"#FF0000"</c>), if explicitly set.
    /// </summary>
    [property: JsonPropertyName("color")]         string? Color
);

/// <summary>
/// A contiguous span of text that shares a single set of formatting attributes.
/// Analogous to a DOCX <c>&lt;w:r&gt;</c> element.
/// Mirrors the Rust <c>Run</c> struct.
/// </summary>
public record Run(
    [property: JsonPropertyName("text")]       string Text,
    [property: JsonPropertyName("formatting")] RunFormatting Formatting
);

// ---------------------------------------------------------------------------
// TrackedChange
// ---------------------------------------------------------------------------

/// <summary>
/// A single tracked revision record attached to a block.
/// Mirrors the Rust <c>TrackedChange</c> struct.
/// </summary>
public record TrackedChange(
    /// <summary>Display name of the author who made this change.</summary>
    [property: JsonPropertyName("author")]      string Author,
    /// <summary>UTC timestamp when the change was recorded.</summary>
    [property: JsonPropertyName("date")]        DateTime Date,
    /// <summary>Whether content was inserted, deleted, or only reformatted.</summary>
    [property: JsonPropertyName("change_type")] ChangeType ChangeType,
    /// <summary>
    /// Original text before the change (for <c>Delete</c> and
    /// <c>FormatChange</c>); <c>null</c> for pure insertions.
    /// </summary>
    [property: JsonPropertyName("original")]    string? Original
);

// ---------------------------------------------------------------------------
// FormattingMeta
// ---------------------------------------------------------------------------

/// <summary>
/// Document-level and paragraph-level formatting metadata.
/// Stored as a JSON blob in the database; not used for hashing.
/// Mirrors the Rust <c>FormattingMeta</c> struct.
/// </summary>
public record FormattingMeta(
    /// <summary>
    /// Named paragraph/character style (e.g. <c>"Heading 1"</c>,
    /// <c>"Body Text"</c>).
    /// </summary>
    [property: JsonPropertyName("style_name")]      string? StyleName,
    /// <summary>Word-processor list numbering identifier.</summary>
    [property: JsonPropertyName("numbering_id")]    int? NumberingId,
    /// <summary>Nesting depth within the numbering scheme (0-based).</summary>
    [property: JsonPropertyName("numbering_level")] int? NumberingLevel,
    /// <summary>
    /// <c>true</c> when this block carries redline (tracked-changes) markup.
    /// </summary>
    [property: JsonPropertyName("is_redline")]      bool IsRedline,
    /// <summary>The specific tracked-change record, if present.</summary>
    [property: JsonPropertyName("tracked_change")]  TrackedChange? TrackedChange
);

// ---------------------------------------------------------------------------
// Block
// ---------------------------------------------------------------------------

/// <summary>
/// Core structural unit of the RT_Flow document model.
///
/// Blocks form a tree: each <see cref="Block"/> may own zero or more child
/// blocks in <see cref="Children"/>. The <see cref="ParentId"/> field mirrors
/// the tree relationship via UUIDs so that a flat list can be reconstituted
/// into a tree without carrying nested objects.
///
/// Mirrors the Rust <c>Block</c> struct.
/// </summary>
public record Block(
    /// <summary>Stable unique identifier (UUIDv4).</summary>
    [property: JsonPropertyName("id")]               Guid Id,
    /// <summary>Identifier of the owning document.</summary>
    [property: JsonPropertyName("document_id")]      Guid DocumentId,
    /// <summary>Identifier of the parent block; <c>null</c> for root blocks.</summary>
    [property: JsonPropertyName("parent_id")]        Guid? ParentId,
    /// <summary>Structural role within the document.</summary>
    [property: JsonPropertyName("block_type")]       BlockType BlockType,
    /// <summary>Nesting depth (root = 0).</summary>
    [property: JsonPropertyName("level")]            int Level,
    /// <summary>
    /// Human-readable structural address, e.g. <c>"1.2(a)(iii)"</c>.
    /// </summary>
    [property: JsonPropertyName("structural_path")]  string StructuralPath,
    /// <summary>
    /// SHA-256-based primary anchor: stable identity key for comparison and
    /// merging. Computed from <c>block_type</c>, <c>structural_path</c>, and
    /// the first 128 characters of <c>canonical_text</c>.
    /// </summary>
    [property: JsonPropertyName("anchor_signature")] string AnchorSignature,
    /// <summary>
    /// SHA-256 of <c>canonical_text</c> — detects any textual change.
    /// </summary>
    [property: JsonPropertyName("clause_hash")]      string ClauseHash,
    /// <summary>
    /// Whitespace-normalised text used for hashing and diffing.
    /// </summary>
    [property: JsonPropertyName("canonical_text")]   string CanonicalText,
    /// <summary>
    /// Original text preserving typographic fidelity (capitalisation,
    /// non-breaking spaces, special characters, etc.).
    /// </summary>
    [property: JsonPropertyName("display_text")]     string DisplayText,
    /// <summary>
    /// Paragraph/character formatting metadata; not used for hashing.
    /// </summary>
    [property: JsonPropertyName("formatting_meta")]  FormattingMeta FormattingMeta,
    /// <summary>
    /// Zero-based insertion order among siblings sharing the same
    /// <see cref="ParentId"/>.
    /// </summary>
    [property: JsonPropertyName("position_index")]   int PositionIndex,
    /// <summary>Token stream derived from <c>canonical_text</c>.</summary>
    [property: JsonPropertyName("tokens")]           List<Token> Tokens,
    /// <summary>
    /// Run stream derived from <c>display_text</c> (preserves formatting spans).
    /// </summary>
    [property: JsonPropertyName("runs")]             List<Run> Runs,
    /// <summary>Direct children in document order.</summary>
    [property: JsonPropertyName("children")]         List<Block> Children
);

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/// <summary>
/// Top-level document record — the root of the block tree.
/// Mirrors the Rust <c>Document</c> struct.
/// </summary>
public record Document(
    [property: JsonPropertyName("id")]                     Guid Id,
    /// <summary>
    /// Human-readable document name (e.g. filename without extension).
    /// </summary>
    [property: JsonPropertyName("name")]                   string Name,
    /// <summary>
    /// Filesystem or object-storage path from which this document was
    /// ingested; <c>null</c> for programmatically created documents.
    /// </summary>
    [property: JsonPropertyName("source_path")]            string? SourcePath,
    /// <summary>Provenance classification of this document.</summary>
    [property: JsonPropertyName("doc_type")]               DocumentType DocType,
    /// <summary>
    /// Semver string identifying the block-model schema (e.g. <c>"1.0.0"</c>).
    /// </summary>
    [property: JsonPropertyName("schema_version")]         string SchemaVersion,
    /// <summary>
    /// Semver string identifying the text-normalisation algorithm.
    /// </summary>
    [property: JsonPropertyName("normalization_version")]  string NormalizationVersion,
    /// <summary>
    /// Semver string identifying the clause-hashing contract.
    /// </summary>
    [property: JsonPropertyName("hash_contract_version")] string HashContractVersion,
    /// <summary>UTC timestamp when the document was ingested.</summary>
    [property: JsonPropertyName("ingested_at")]            DateTime IngestedAt,
    /// <summary>
    /// Arbitrary key/value metadata (e.g. parties, jurisdiction, matter ID);
    /// serialised as a free-form JSON object.
    /// </summary>
    [property: JsonPropertyName("metadata")]               System.Text.Json.JsonElement? Metadata
);
