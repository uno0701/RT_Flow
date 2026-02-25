//! Compare result types — the structured output of the Compare Engine.
//!
//! These types are serialized to JSON and must match the contract defined in
//! `contracts/compare-result.json`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::diff::TokenDiff;

// ---------------------------------------------------------------------------
// DeltaKind
// ---------------------------------------------------------------------------

/// Disposition of a single block after comparison.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeltaKind {
    /// Block exists only in the right (incoming) document.
    Inserted,
    /// Block exists only in the left (base) document.
    Deleted,
    /// Block exists in both documents but its content has changed.
    Modified,
    /// Block exists in both documents but its structural position has changed.
    Moved,
}

// ---------------------------------------------------------------------------
// BlockDelta
// ---------------------------------------------------------------------------

/// Comparison result for one aligned pair (or singleton) of blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDelta {
    /// Stable unique identifier for this delta record (UUIDv4).
    pub id: Uuid,
    /// Disposition of this block pair.
    pub kind: DeltaKind,
    /// UUID of the block in the left (base) document; `None` for insertions.
    pub left_block_id: Option<Uuid>,
    /// UUID of the block in the right (incoming) document; `None` for deletions.
    pub right_block_id: Option<Uuid>,
    /// Zero-based position of this block in the left document's flat block list;
    /// `None` for inserted blocks.
    pub left_ordinal: Option<usize>,
    /// Zero-based position of this block in the right document's flat block list;
    /// `None` for deleted blocks.
    pub right_ordinal: Option<usize>,
    /// Token-level diffs; empty for non-modified deltas.
    pub token_diffs: Vec<TokenDiff>,
    /// Normalised text similarity in [0.0, 1.0] between the two block versions;
    /// `None` for inserted or deleted blocks.
    pub similarity_score: Option<f64>,
    /// For `kind = Moved`: the UUID of the corresponding block in the target
    /// document; `None` otherwise.
    pub move_target_id: Option<Uuid>,
}

// ---------------------------------------------------------------------------
// CompareStats
// ---------------------------------------------------------------------------

/// Aggregate counts summarising the comparison run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareStats {
    /// Total number of blocks in the left (base) document.
    pub blocks_left: usize,
    /// Total number of blocks in the right (incoming) document.
    pub blocks_right: usize,
    /// Number of blocks present only in the right document.
    pub inserted: usize,
    /// Number of blocks present only in the left document.
    pub deleted: usize,
    /// Number of aligned block pairs where text or formatting changed.
    pub modified: usize,
    /// Number of blocks whose structural position changed between documents.
    pub moved: usize,
    /// Number of aligned block pairs that are identical in both documents.
    pub unchanged: usize,
}

// ---------------------------------------------------------------------------
// CompareResult
// ---------------------------------------------------------------------------

/// The top-level output of a single comparison run.
///
/// Serialised to JSON this matches the schema defined in
/// `contracts/compare-result.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareResult {
    /// Stable unique identifier for this comparison run (UUIDv4).
    pub run_id: Uuid,
    /// UUID of the left (base) document.
    pub left_doc_id: Uuid,
    /// UUID of the right (incoming) document.
    pub right_doc_id: Uuid,
    /// Wall-clock duration of the comparison run in milliseconds.
    pub elapsed_ms: u64,
    /// Aggregate block-level counts for this comparison.
    pub stats: CompareStats,
    /// Ordered list of per-block deltas in left-document traversal order.
    pub deltas: Vec<BlockDelta>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diff::DiffKind;

    fn make_result() -> CompareResult {
        CompareResult {
            run_id: Uuid::new_v4(),
            left_doc_id: Uuid::new_v4(),
            right_doc_id: Uuid::new_v4(),
            elapsed_ms: 42,
            stats: CompareStats {
                blocks_left: 3,
                blocks_right: 4,
                inserted: 1,
                deleted: 0,
                modified: 1,
                moved: 0,
                unchanged: 2,
            },
            deltas: vec![
                BlockDelta {
                    id: Uuid::new_v4(),
                    kind: DeltaKind::Modified,
                    left_block_id: Some(Uuid::new_v4()),
                    right_block_id: Some(Uuid::new_v4()),
                    left_ordinal: Some(0),
                    right_ordinal: Some(0),
                    token_diffs: vec![TokenDiff {
                        kind: DiffKind::Equal,
                        left_tokens: vec!["the".to_string()],
                        right_tokens: vec!["the".to_string()],
                        left_offset: 0,
                        right_offset: 0,
                    }],
                    similarity_score: Some(0.9),
                    move_target_id: None,
                },
                BlockDelta {
                    id: Uuid::new_v4(),
                    kind: DeltaKind::Inserted,
                    left_block_id: None,
                    right_block_id: Some(Uuid::new_v4()),
                    left_ordinal: None,
                    right_ordinal: Some(3),
                    token_diffs: vec![],
                    similarity_score: None,
                    move_target_id: None,
                },
            ],
        }
    }

    #[test]
    fn compare_result_round_trips_json() {
        let result = make_result();
        let json = serde_json::to_string(&result).expect("serialize");
        let restored: CompareResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.run_id, result.run_id);
        assert_eq!(restored.elapsed_ms, 42);
        assert_eq!(restored.stats.inserted, 1);
        assert_eq!(restored.deltas.len(), 2);
    }

    #[test]
    fn delta_kind_serializes_to_snake_case() {
        assert_eq!(
            serde_json::to_string(&DeltaKind::Inserted).unwrap(),
            "\"inserted\""
        );
        assert_eq!(
            serde_json::to_string(&DeltaKind::Deleted).unwrap(),
            "\"deleted\""
        );
        assert_eq!(
            serde_json::to_string(&DeltaKind::Modified).unwrap(),
            "\"modified\""
        );
        assert_eq!(
            serde_json::to_string(&DeltaKind::Moved).unwrap(),
            "\"moved\""
        );
    }

    #[test]
    fn optional_fields_serialize_as_null() {
        let delta = BlockDelta {
            id: Uuid::new_v4(),
            kind: DeltaKind::Inserted,
            left_block_id: None,
            right_block_id: Some(Uuid::new_v4()),
            left_ordinal: None,
            right_ordinal: Some(0),
            token_diffs: vec![],
            similarity_score: None,
            move_target_id: None,
        };
        let json = serde_json::to_string(&delta).expect("serialize");
        assert!(json.contains("\"left_block_id\":null"));
        assert!(json.contains("\"left_ordinal\":null"));
        assert!(json.contains("\"similarity_score\":null"));
        assert!(json.contains("\"move_target_id\":null"));
    }

    #[test]
    fn stats_all_zero_is_valid() {
        let stats = CompareStats {
            blocks_left: 0,
            blocks_right: 0,
            inserted: 0,
            deleted: 0,
            modified: 0,
            moved: 0,
            unchanged: 0,
        };
        let json = serde_json::to_string(&stats).expect("serialize");
        assert!(json.contains("\"blocks_left\":0"));
    }

    #[test]
    fn moved_delta_has_move_target() {
        let target_id = Uuid::new_v4();
        let delta = BlockDelta {
            id: Uuid::new_v4(),
            kind: DeltaKind::Moved,
            left_block_id: Some(Uuid::new_v4()),
            right_block_id: Some(Uuid::new_v4()),
            left_ordinal: Some(0),
            right_ordinal: Some(5),
            token_diffs: vec![],
            similarity_score: Some(0.95),
            move_target_id: Some(target_id),
        };
        let json = serde_json::to_string(&delta).expect("serialize");
        assert!(json.contains(&target_id.to_string()));
    }
}
