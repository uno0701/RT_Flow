/**
 * TypeScript interfaces mirroring the Rust block types defined in
 * crates/rt-core/src/block.rs.
 *
 * Field names use snake_case to match serde's JSON output so that
 * JSON.parse / JSON.stringify round-trips cleanly without a mapping layer.
 */

// ---------------------------------------------------------------------------
// Enumerations
// ---------------------------------------------------------------------------

/**
 * Structural role of a block within a legal document.
 * Mirrors the Rust `BlockType` enum (serialised as snake_case strings).
 */
export type BlockType =
  | 'section'
  | 'clause'
  | 'subclause'
  | 'paragraph'
  | 'table'
  | 'table_row'
  | 'table_cell';

/**
 * Semantic category of a single token.
 * Mirrors the Rust `TokenKind` enum (serialised as snake_case strings).
 */
export type TokenKind =
  | 'word'
  | 'number'
  | 'punctuation'
  | 'whitespace'
  | 'defined_term'
  | 'party_ref'
  | 'date_ref';

/**
 * Nature of a tracked revision.
 * Mirrors the Rust `ChangeType` enum (serialised as snake_case strings).
 */
export type ChangeType = 'insert' | 'delete' | 'format_change';

/**
 * Provenance classification of a document ingested into RT_Flow.
 * Mirrors the Rust `DocumentType` enum (serialised as snake_case strings).
 */
export type DocumentType = 'original' | 'redline' | 'merged' | 'snapshot';

// ---------------------------------------------------------------------------
// Token
// ---------------------------------------------------------------------------

/**
 * Atomic unit of text produced by the tokenizer.
 *
 * `offset` is the byte offset of the token's first character within the
 * parent block's `canonical_text`.
 *
 * Mirrors the Rust `Token` struct.
 */
export interface Token {
  /** Raw text as it appears in the document. */
  text: string;
  /** Semantic classification of this token. */
  kind: TokenKind;
  /** Lowercased, whitespace-normalised form used for matching and search. */
  normalized: string;
  /** Byte offset within the parent block's `canonical_text`. */
  offset: number;
}

// ---------------------------------------------------------------------------
// RunFormatting / Run
// ---------------------------------------------------------------------------

/**
 * Typographic attributes attached to a `Run`.
 * Mirrors the Rust `RunFormatting` struct.
 */
export interface RunFormatting {
  bold: boolean;
  italic: boolean;
  underline: boolean;
  strikethrough: boolean;
  /**
   * Point size, if explicitly set.
   * `null` means "inherit from style".
   */
  font_size: number | null;
  /**
   * CSS-style hex colour string (e.g. `"#FF0000"`), if explicitly set.
   * `null` when not set.
   */
  color: string | null;
}

/**
 * A contiguous span of text that shares a single set of formatting attributes.
 * Analogous to a DOCX `<w:r>` element.
 * Mirrors the Rust `Run` struct.
 */
export interface Run {
  text: string;
  formatting: RunFormatting;
}

// ---------------------------------------------------------------------------
// TrackedChange
// ---------------------------------------------------------------------------

/**
 * A single tracked revision record attached to a block.
 * Mirrors the Rust `TrackedChange` struct.
 */
export interface TrackedChange {
  /** Display name of the author who made this change. */
  author: string;
  /** ISO 8601 UTC timestamp when the change was recorded. */
  date: string;
  /** Whether content was inserted, deleted, or only reformatted. */
  change_type: ChangeType;
  /**
   * Original text before the change (for `delete` and `format_change`).
   * `null` for pure insertions.
   */
  original: string | null;
}

// ---------------------------------------------------------------------------
// FormattingMeta
// ---------------------------------------------------------------------------

/**
 * Document-level and paragraph-level formatting metadata.
 * Stored as a JSON blob in the database; not used for hashing.
 * Mirrors the Rust `FormattingMeta` struct.
 */
export interface FormattingMeta {
  /**
   * Named paragraph/character style (e.g. `"Heading 1"`, `"Body Text"`).
   * `null` when not set.
   */
  style_name: string | null;
  /**
   * Word-processor list numbering identifier.
   * `null` when not a list item.
   */
  numbering_id: number | null;
  /**
   * Nesting depth within the numbering scheme (0-based).
   * `null` when not a list item.
   */
  numbering_level: number | null;
  /** `true` when this block carries redline (tracked-changes) markup. */
  is_redline: boolean;
  /** The specific tracked-change record, if present; `null` otherwise. */
  tracked_change: TrackedChange | null;
}

// ---------------------------------------------------------------------------
// Block
// ---------------------------------------------------------------------------

/**
 * Core structural unit of the RT_Flow document model.
 *
 * Blocks form a tree: each `Block` may own zero or more child blocks in
 * `children`. The `parent_id` field mirrors the tree relationship via UUIDs
 * so that a flat list can be reconstituted into a tree without carrying nested
 * objects.
 *
 * Mirrors the Rust `Block` struct.
 */
export interface Block {
  /** Stable unique identifier (UUIDv4). */
  id: string;
  /** UUID of the owning document. */
  document_id: string;
  /** UUID of the parent block; `null` for root blocks. */
  parent_id: string | null;
  /** Structural role within the document. */
  block_type: BlockType;
  /** Nesting depth (root = 0). */
  level: number;
  /**
   * Human-readable structural address, e.g. `"1.2(a)(iii)"`.
   * Used as part of the anchor signature computation.
   */
  structural_path: string;
  /**
   * SHA-256-based primary anchor: stable identity key for comparison and
   * merging. Computed from `block_type`, `structural_path`, and the first
   * 128 characters of `canonical_text`.
   */
  anchor_signature: string;
  /**
   * SHA-256 of `canonical_text` — detects any textual change in the block.
   */
  clause_hash: string;
  /** Whitespace-normalised text used for hashing and diffing. */
  canonical_text: string;
  /**
   * Original text preserving typographic fidelity (capitalisation,
   * non-breaking spaces, special characters, etc.).
   */
  display_text: string;
  /** Paragraph/character formatting metadata; not used for hashing. */
  formatting_meta: FormattingMeta;
  /**
   * Zero-based insertion order among siblings sharing the same `parent_id`.
   */
  position_index: number;
  /** Token stream derived from `canonical_text`. */
  tokens: Token[];
  /**
   * Run stream derived from `display_text` (preserves formatting spans).
   */
  runs: Run[];
  /** Direct children in document order. */
  children: Block[];
}

// ---------------------------------------------------------------------------
// Document
// ---------------------------------------------------------------------------

/**
 * Top-level document record — the root of the block tree.
 * Mirrors the Rust `Document` struct.
 */
export interface Document {
  /** Stable UUID for this document. */
  id: string;
  /** Human-readable document name (e.g. filename without extension). */
  name: string;
  /**
   * Filesystem or object-storage path from which this document was ingested.
   * `null` for programmatically created documents.
   */
  source_path: string | null;
  /** Provenance classification of this document. */
  doc_type: DocumentType;
  /** Semver string identifying the block-model schema (e.g. `"1.0.0"`). */
  schema_version: string;
  /** Semver string identifying the text-normalisation algorithm. */
  normalization_version: string;
  /** Semver string identifying the clause-hashing contract. */
  hash_contract_version: string;
  /** ISO 8601 UTC timestamp when the document was ingested. */
  ingested_at: string;
  /**
   * Arbitrary key/value metadata (e.g. parties, jurisdiction, matter ID).
   * `null` when no metadata was supplied at ingestion time.
   */
  metadata: Record<string, unknown> | null;
}
