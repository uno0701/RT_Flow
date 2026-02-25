use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::anchor::compute_anchor_signature;
use crate::hash::compute_clause_hash;

// ---------------------------------------------------------------------------
// BlockType
// ---------------------------------------------------------------------------

/// Structural role of a block within a legal document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Section,
    Clause,
    Subclause,
    Paragraph,
    Table,
    TableRow,
    TableCell,
}

// ---------------------------------------------------------------------------
// TokenKind / Token
// ---------------------------------------------------------------------------

/// Semantic category of a single token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenKind {
    Word,
    Number,
    Punctuation,
    Whitespace,
    /// A term explicitly defined within the document (e.g. "Borrower").
    DefinedTerm,
    /// Reference to a named party (e.g. "the Lender").
    PartyRef,
    /// A date expression (e.g. "January 1, 2025").
    DateRef,
}

/// Atomic unit of text produced by the tokenizer.
///
/// `offset` is the byte offset of the token's first character within the
/// parent block's `canonical_text`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Token {
    /// Raw text as it appears in the document.
    pub text: String,
    /// Semantic classification of this token.
    pub kind: TokenKind,
    /// Lowercased, whitespace-normalised form used for matching and search.
    pub normalized: String,
    /// Byte offset within the parent block's `canonical_text`.
    pub offset: usize,
}

// ---------------------------------------------------------------------------
// RunFormatting / Run
// ---------------------------------------------------------------------------

/// Typographic attributes attached to a [`Run`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunFormatting {
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
    pub strikethrough: bool,
    /// Point size, if explicitly set; `None` means "inherit from style".
    pub font_size: Option<f32>,
    /// CSS-style hex colour string (e.g. `"#FF0000"`), if explicitly set.
    pub color: Option<String>,
}

impl Default for RunFormatting {
    fn default() -> Self {
        Self {
            bold: false,
            italic: false,
            underline: false,
            strikethrough: false,
            font_size: None,
            color: None,
        }
    }
}

/// A contiguous span of text that shares a single set of formatting attributes.
///
/// Analogous to a DOCX `<w:r>` element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Run {
    pub text: String,
    pub formatting: RunFormatting,
}

// ---------------------------------------------------------------------------
// ChangeType / TrackedChange
// ---------------------------------------------------------------------------

/// Nature of a tracked revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChangeType {
    Insert,
    Delete,
    FormatChange,
}

/// A single tracked revision record attached to a block.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackedChange {
    /// Display name of the author who made this change.
    pub author: String,
    /// UTC timestamp when the change was recorded.
    pub date: DateTime<Utc>,
    /// Whether content was inserted, deleted, or only reformatted.
    pub change_type: ChangeType,
    /// Original text before the change (for `Delete` and `FormatChange`).
    pub original: Option<String>,
}

// ---------------------------------------------------------------------------
// FormattingMeta
// ---------------------------------------------------------------------------

/// Document-level and paragraph-level formatting metadata.
///
/// Stored as a JSON blob in the database; not used for hashing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormattingMeta {
    /// Named paragraph/character style (e.g. `"Heading 1"`, `"Body Text"`).
    pub style_name: Option<String>,
    /// Word-processor list numbering identifier.
    pub numbering_id: Option<i32>,
    /// Nesting depth within the numbering scheme (0-based).
    pub numbering_level: Option<i32>,
    /// `true` when this block carries redline (tracked-changes) markup.
    pub is_redline: bool,
    /// The specific tracked-change record, if present.
    pub tracked_change: Option<TrackedChange>,
}

impl Default for FormattingMeta {
    fn default() -> Self {
        Self {
            style_name: None,
            numbering_id: None,
            numbering_level: None,
            is_redline: false,
            tracked_change: None,
        }
    }
}

// ---------------------------------------------------------------------------
// DocumentType / Document
// ---------------------------------------------------------------------------

/// Provenance classification of a document ingested into RT_Flow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentType {
    /// The original, unmodified counterpart.
    Original,
    /// A document containing tracked changes / redlines.
    Redline,
    /// The result of a merge operation.
    Merged,
    /// A point-in-time snapshot preserved for audit purposes.
    Snapshot,
}

/// Top-level document record — the root of the block tree.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: Uuid,
    /// Human-readable document name (e.g. filename without extension).
    pub name: String,
    /// Filesystem or object-storage path from which this document was ingested.
    pub source_path: Option<String>,
    pub doc_type: DocumentType,
    /// Semver string identifying the block-model schema (e.g. `"1.0.0"`).
    pub schema_version: String,
    /// Semver string identifying the text-normalization algorithm.
    pub normalization_version: String,
    /// Semver string identifying the clause-hashing contract.
    pub hash_contract_version: String,
    /// UTC timestamp when the document was ingested.
    pub ingested_at: DateTime<Utc>,
    /// Arbitrary key/value metadata (e.g. parties, jurisdiction, matter ID).
    pub metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Block
// ---------------------------------------------------------------------------

/// Core structural unit of the RT_Flow document model.
///
/// Blocks form a tree: each `Block` may own zero or more child `Block`s in
/// `children`. The `parent_id` field mirrors the tree relationship using UUIDs
/// so that a flat list of blocks can be reconstituted into a tree without
/// carrying nested `Block` objects.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    /// Stable unique identifier (UUIDv4).
    pub id: Uuid,
    /// Identifier of the owning [`Document`].
    pub document_id: Uuid,
    /// Identifier of the parent block, or `None` for root blocks.
    pub parent_id: Option<Uuid>,
    /// Structural role within the document.
    pub block_type: BlockType,
    /// Nesting depth (root = 0).
    pub level: i32,
    /// Human-readable structural address, e.g. `"1.2(a)(iii)"`.
    pub structural_path: String,
    /// SHA-256-based primary anchor: stable identity key for comparison and
    /// merging. Computed from `block_type`, `structural_path`, and the first
    /// 128 characters of `canonical_text`.
    pub anchor_signature: String,
    /// SHA-256 of `canonical_text` — detects any textual change in the block.
    pub clause_hash: String,
    /// Whitespace-normalised text used for hashing and diffing.
    pub canonical_text: String,
    /// Original text preserving typographic fidelity (capitalisation,
    /// non-breaking spaces, special characters, etc.).
    pub display_text: String,
    /// Paragraph/character formatting metadata; not used for hashing.
    pub formatting_meta: FormattingMeta,
    /// Zero-based insertion order among siblings with the same `parent_id`.
    pub position_index: i32,
    /// Token stream derived from `canonical_text`.
    pub tokens: Vec<Token>,
    /// Run stream derived from `display_text` (preserves formatting spans).
    pub runs: Vec<Run>,
    /// Direct children in document order.
    pub children: Vec<Block>,
}

impl BlockType {
    /// Return the canonical snake_case string representation of this variant.
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockType::Section => "section",
            BlockType::Clause => "clause",
            BlockType::Subclause => "subclause",
            BlockType::Paragraph => "paragraph",
            BlockType::Table => "table",
            BlockType::TableRow => "table_row",
            BlockType::TableCell => "table_cell",
        }
    }
}

impl std::fmt::Display for BlockType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<&str> for BlockType {
    fn from(s: &str) -> Self {
        match s {
            "section" => BlockType::Section,
            "clause" => BlockType::Clause,
            "subclause" => BlockType::Subclause,
            "paragraph" => BlockType::Paragraph,
            "table" => BlockType::Table,
            "table_row" => BlockType::TableRow,
            "table_cell" => BlockType::TableCell,
            _ => BlockType::Paragraph, // graceful fallback
        }
    }
}

impl TokenKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            TokenKind::Word => "word",
            TokenKind::Number => "number",
            TokenKind::Punctuation => "punctuation",
            TokenKind::Whitespace => "whitespace",
            TokenKind::DefinedTerm => "defined_term",
            TokenKind::PartyRef => "party_ref",
            TokenKind::DateRef => "date_ref",
        }
    }
}

impl From<&str> for TokenKind {
    fn from(s: &str) -> Self {
        match s {
            "number" => TokenKind::Number,
            "punctuation" => TokenKind::Punctuation,
            "whitespace" => TokenKind::Whitespace,
            "defined_term" => TokenKind::DefinedTerm,
            "party_ref" => TokenKind::PartyRef,
            "date_ref" => TokenKind::DateRef,
            _ => TokenKind::Word, // graceful fallback
        }
    }
}

impl ChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            ChangeType::Insert => "insert",
            ChangeType::Delete => "delete",
            ChangeType::FormatChange => "format_change",
        }
    }
}

impl From<&str> for ChangeType {
    fn from(s: &str) -> Self {
        match s {
            "delete" => ChangeType::Delete,
            "format_change" => ChangeType::FormatChange,
            _ => ChangeType::Insert,
        }
    }
}

impl DocumentType {
    pub fn as_str(&self) -> &'static str {
        match self {
            DocumentType::Original => "original",
            DocumentType::Redline => "redline",
            DocumentType::Merged => "merged",
            DocumentType::Snapshot => "snapshot",
        }
    }
}

impl From<&str> for DocumentType {
    fn from(s: &str) -> Self {
        match s {
            "redline" => DocumentType::Redline,
            "merged" => DocumentType::Merged,
            "snapshot" => DocumentType::Snapshot,
            _ => DocumentType::Original,
        }
    }
}

impl Block {
    /// Construct a new `Block`, auto-generating its `id` and computing both
    /// `anchor_signature` and `clause_hash` from the supplied text.
    ///
    /// `tokens`, `runs`, `children`, and `formatting_meta` are initialised to
    /// empty / default values; callers may populate them afterwards.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        block_type: BlockType,
        structural_path: impl Into<String>,
        canonical_text: impl Into<String>,
        display_text: impl Into<String>,
        parent_id: Option<Uuid>,
        document_id: Uuid,
        position_index: i32,
    ) -> Self {
        let structural_path = structural_path.into();
        let canonical_text = canonical_text.into();
        let display_text = display_text.into();

        let anchor_signature =
            compute_anchor_signature(&block_type, &structural_path, &canonical_text);
        let clause_hash = compute_clause_hash(&canonical_text);

        Self {
            id: Uuid::new_v4(),
            document_id,
            parent_id,
            block_type,
            level: 0,
            structural_path,
            anchor_signature,
            clause_hash,
            canonical_text,
            display_text,
            formatting_meta: FormattingMeta::default(),
            position_index,
            tokens: Vec::new(),
            runs: Vec::new(),
            children: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_doc_id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn block_new_generates_unique_ids() {
        let doc = make_doc_id();
        let b1 = Block::new(BlockType::Clause, "1.1", "text", "Text", None, doc, 0);
        let b2 = Block::new(BlockType::Clause, "1.1", "text", "Text", None, doc, 0);
        assert_ne!(b1.id, b2.id);
    }

    #[test]
    fn block_new_computes_hashes() {
        let doc = make_doc_id();
        let b = Block::new(
            BlockType::Clause,
            "2.1(a)",
            "the borrower shall repay",
            "The Borrower shall repay",
            None,
            doc,
            0,
        );
        // Both hashes must be 64-character lowercase hex strings.
        assert_eq!(b.anchor_signature.len(), 64);
        assert_eq!(b.clause_hash.len(), 64);
        // clause_hash must equal direct computation.
        assert_eq!(
            b.clause_hash,
            crate::hash::compute_clause_hash("the borrower shall repay")
        );
    }

    #[test]
    fn block_defaults_are_empty() {
        let doc = make_doc_id();
        let b = Block::new(BlockType::Paragraph, "3.2", "x", "x", None, doc, 0);
        assert!(b.tokens.is_empty());
        assert!(b.runs.is_empty());
        assert!(b.children.is_empty());
        assert!(!b.formatting_meta.is_redline);
        assert_eq!(b.level, 0);
    }

    #[test]
    fn run_formatting_default_all_false() {
        let rf = RunFormatting::default();
        assert!(!rf.bold);
        assert!(!rf.italic);
        assert!(!rf.underline);
        assert!(!rf.strikethrough);
        assert!(rf.font_size.is_none());
        assert!(rf.color.is_none());
    }

    #[test]
    fn formatting_meta_default_all_none() {
        let fm = FormattingMeta::default();
        assert!(fm.style_name.is_none());
        assert!(fm.numbering_id.is_none());
        assert!(fm.numbering_level.is_none());
        assert!(!fm.is_redline);
        assert!(fm.tracked_change.is_none());
    }

    #[test]
    fn block_type_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&BlockType::TableRow).unwrap(),
            "\"table_row\""
        );
        assert_eq!(
            serde_json::to_string(&BlockType::TableCell).unwrap(),
            "\"table_cell\""
        );
        assert_eq!(
            serde_json::to_string(&BlockType::Section).unwrap(),
            "\"section\""
        );
        assert_eq!(
            serde_json::to_string(&BlockType::Subclause).unwrap(),
            "\"subclause\""
        );
    }

    #[test]
    fn block_round_trips_json() {
        let doc = make_doc_id();
        let mut b = Block::new(
            BlockType::Section,
            "1",
            "general provisions",
            "General Provisions",
            None,
            doc,
            0,
        );
        b.level = 1;
        let json = serde_json::to_string(&b).expect("serialize");
        let b2: Block = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(b.id, b2.id);
        assert_eq!(b.clause_hash, b2.clause_hash);
        assert_eq!(b.anchor_signature, b2.anchor_signature);
        assert_eq!(b.canonical_text, b2.canonical_text);
        assert_eq!(b.level, b2.level);
    }

    #[test]
    fn block_with_parent_id() {
        let doc = make_doc_id();
        let parent = Block::new(BlockType::Section, "1", "section one", "Section One", None, doc, 0);
        let child = Block::new(
            BlockType::Clause,
            "1.1",
            "first clause",
            "First Clause",
            Some(parent.id),
            doc,
            0,
        );
        assert_eq!(child.parent_id, Some(parent.id));
        assert_eq!(child.document_id, doc);
    }

    #[test]
    fn document_type_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&DocumentType::Original).unwrap(),
            "\"original\""
        );
        assert_eq!(
            serde_json::to_string(&DocumentType::Redline).unwrap(),
            "\"redline\""
        );
        assert_eq!(
            serde_json::to_string(&DocumentType::Merged).unwrap(),
            "\"merged\""
        );
        assert_eq!(
            serde_json::to_string(&DocumentType::Snapshot).unwrap(),
            "\"snapshot\""
        );
    }
}
