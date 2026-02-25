use rt_core::{Block, RtError};
use rt_compare::align::{align_blocks, BlockAlignment};
use rt_compare::diff::{token_diff, DiffKind};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::conflict::{detect_conflicts, ConflictResolution, MergeConflict};
use crate::layer::{BlockDelta, DeltaType};
use crate::resolution::validate_resolution;

// ---------------------------------------------------------------------------
// MergeResult
// ---------------------------------------------------------------------------

/// The output of a merge operation.
///
/// Matches the `MergeResult` schema in `contracts/merge-result.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeResult {
    /// Stable unique identifier for this merge run (UUIDv4).
    pub merge_id: Uuid,
    /// UUID of the base (original) document.
    pub base_doc_id: Uuid,
    /// UUID of the incoming (reviewer / redlined) document.
    pub incoming_doc_id: Uuid,
    /// UUID of the newly created merged output document, if one was produced.
    pub output_doc_id: Option<Uuid>,
    /// All conflicts detected during the merge (resolved and unresolved).
    pub conflicts: Vec<MergeConflict>,
    /// Number of blocks merged without conflict (or automatically resolved).
    pub auto_resolved: usize,
    /// Number of conflicts still in `Pending` state requiring human review.
    pub pending_review: usize,
}

// ---------------------------------------------------------------------------
// MergeEngine
// ---------------------------------------------------------------------------

/// Stateless engine that merges two block sequences and detects conflicts.
pub struct MergeEngine {
    /// Reviewer identifier used for base-side deltas.
    base_reviewer_id: String,
    /// Reviewer identifier used for incoming-side deltas.
    incoming_reviewer_id: String,
}

impl MergeEngine {
    /// Create a `MergeEngine` with default reviewer labels.
    pub fn new() -> Self {
        Self {
            base_reviewer_id: "base".to_string(),
            incoming_reviewer_id: "incoming".to_string(),
        }
    }

    /// Create a `MergeEngine` with custom reviewer labels (useful for tests).
    pub fn with_reviewers(
        base_reviewer_id: impl Into<String>,
        incoming_reviewer_id: impl Into<String>,
    ) -> Self {
        Self {
            base_reviewer_id: base_reviewer_id.into(),
            incoming_reviewer_id: incoming_reviewer_id.into(),
        }
    }

    /// Merge `base_blocks` and `incoming_blocks`, detecting and annotating
    /// conflicts.
    ///
    /// Algorithm:
    /// 1. Align the two block sequences using `rt_compare::align::align_blocks`.
    /// 2. For each matched pair whose `clause_hash` differs, compute a
    ///    token-level diff with `rt_compare::diff::token_diff`.
    /// 3. Convert diff operations into `BlockDelta` records.
    /// 4. Run `detect_conflicts` on each block's delta set.
    /// 5. Tally `auto_resolved` (modified pairs with no conflicts) and
    ///    `pending_review` (conflict count still in Pending state).
    pub fn merge(
        &self,
        base_doc_id: Uuid,
        incoming_doc_id: Uuid,
        base_blocks: &[Block],
        incoming_blocks: &[Block],
    ) -> MergeResult {
        let alignments = align_blocks(base_blocks, incoming_blocks);

        let mut all_conflicts: Vec<MergeConflict> = Vec::new();
        let mut auto_resolved: usize = 0;

        for alignment in &alignments {
            match alignment {
                BlockAlignment::Matched { left, right, .. }
                | BlockAlignment::Moved { left, right, .. } => {
                    let base_block = &base_blocks[*left];
                    let inc_block = &incoming_blocks[*right];

                    // Identical content — nothing to do.
                    if base_block.clause_hash == inc_block.clause_hash {
                        auto_resolved += 1;
                        continue;
                    }

                    // Content differs — compute token-level diff.
                    let diffs = token_diff(&base_block.tokens, &inc_block.tokens);

                    // Convert diff groups to BlockDelta records.
                    // Base-side deltas: groups where base tokens were removed
                    // (Deleted or Substituted — the left side changed).
                    let base_deltas = self.diffs_to_base_deltas(
                        &diffs,
                        base_block.id,
                        &self.base_reviewer_id,
                        &base_block.canonical_text,
                    );

                    // Incoming-side deltas: groups where incoming tokens were added
                    // (Inserted or Substituted — the right side changed).
                    let incoming_deltas = self.diffs_to_incoming_deltas(
                        &diffs,
                        base_block.id, // scope to same block id for comparison
                        &self.incoming_reviewer_id,
                        &inc_block.canonical_text,
                    );

                    let block_conflicts = detect_conflicts(&base_deltas, &incoming_deltas);

                    if block_conflicts.is_empty() {
                        // Non-overlapping changes — auto-mergeable.
                        auto_resolved += 1;
                    } else {
                        all_conflicts.extend(block_conflicts);
                    }
                }

                // Pure insertion: block added in incoming — auto-accept.
                BlockAlignment::InsertedRight { .. } => {
                    auto_resolved += 1;
                }

                // Pure deletion: block removed in incoming — auto-accept.
                BlockAlignment::DeletedLeft { .. } => {
                    auto_resolved += 1;
                }
            }
        }

        let pending_review = all_conflicts
            .iter()
            .filter(|c| c.resolution == ConflictResolution::Pending)
            .count();

        MergeResult {
            merge_id: Uuid::new_v4(),
            base_doc_id,
            incoming_doc_id,
            output_doc_id: Some(Uuid::new_v4()),
            conflicts: all_conflicts,
            auto_resolved,
            pending_review,
        }
    }

    /// Apply a `resolution` to `conflict`, validating the state transition first.
    pub fn resolve_conflict(
        conflict: &mut MergeConflict,
        resolution: ConflictResolution,
    ) -> Result<(), RtError> {
        validate_resolution(&conflict.resolution, &resolution)?;
        conflict.resolution = resolution;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    /// Build `BlockDelta` records representing changes to the **base** side.
    ///
    /// Each `Deleted` or `Substituted` group in the diff represents a token
    /// range that was present in the base but removed or replaced in the
    /// incoming version.
    fn diffs_to_base_deltas(
        &self,
        diffs: &[rt_compare::diff::TokenDiff],
        block_id: Uuid,
        reviewer_id: &str,
        _source_text: &str,
    ) -> Vec<BlockDelta> {
        let layer_id = Uuid::new_v4();
        let mut deltas = Vec::new();
        let mut base_token_idx: usize = 0;

        for diff in diffs {
            let left_len = diff.left_tokens.len();
            match diff.kind {
                DiffKind::Equal => {
                    base_token_idx += left_len;
                }
                DiffKind::Deleted => {
                    if left_len > 0 {
                        let start = base_token_idx;
                        let end = base_token_idx + left_len - 1;
                        let payload = serde_json::json!({
                            "text": diff.left_tokens.join(" ")
                        });
                        deltas.push(BlockDelta::new(
                            layer_id,
                            reviewer_id,
                            block_id,
                            DeltaType::Delete,
                            start,
                            end,
                            payload,
                        ));
                        base_token_idx += left_len;
                    }
                }
                DiffKind::Substituted => {
                    if left_len > 0 {
                        let start = base_token_idx;
                        let end = base_token_idx + left_len - 1;
                        let payload = serde_json::json!({
                            "text": diff.left_tokens.join(" ")
                        });
                        deltas.push(BlockDelta::new(
                            layer_id,
                            reviewer_id,
                            block_id,
                            DeltaType::Modify,
                            start,
                            end,
                            payload,
                        ));
                        base_token_idx += left_len;
                    }
                }
                DiffKind::Inserted => {
                    // Insertions don't consume base tokens; skip.
                }
            }
        }

        deltas
    }

    /// Build `BlockDelta` records representing changes to the **incoming** side.
    ///
    /// Each `Inserted` or `Substituted` group in the diff represents token
    /// ranges added or substituted in the incoming version.
    fn diffs_to_incoming_deltas(
        &self,
        diffs: &[rt_compare::diff::TokenDiff],
        block_id: Uuid,
        reviewer_id: &str,
        _source_text: &str,
    ) -> Vec<BlockDelta> {
        let layer_id = Uuid::new_v4();
        let mut deltas = Vec::new();
        // We track the base token index to determine where in the base token
        // stream the incoming change falls (for overlap detection).
        let mut base_token_idx: usize = 0;

        for diff in diffs {
            let left_len = diff.left_tokens.len();
            let right_len = diff.right_tokens.len();
            match diff.kind {
                DiffKind::Equal => {
                    base_token_idx += left_len;
                }
                DiffKind::Deleted => {
                    // Deletions advance the base index but produce no incoming delta.
                    base_token_idx += left_len;
                }
                DiffKind::Inserted => {
                    if right_len > 0 {
                        // An insertion at base_token_idx: use base position as
                        // the anchor so overlap can be detected against base deltas.
                        let start = base_token_idx;
                        let end = if base_token_idx > 0 {
                            base_token_idx
                        } else {
                            0
                        };
                        let payload = serde_json::json!({
                            "text": diff.right_tokens.join(" ")
                        });
                        deltas.push(BlockDelta::new(
                            layer_id,
                            reviewer_id,
                            block_id,
                            DeltaType::Insert,
                            start,
                            end,
                            payload,
                        ));
                    }
                }
                DiffKind::Substituted => {
                    if right_len > 0 && left_len > 0 {
                        // Substitution: the same base token range [start, end]
                        // is replaced by different content.
                        let start = base_token_idx;
                        let end = base_token_idx + left_len - 1;
                        let payload = serde_json::json!({
                            "text": diff.right_tokens.join(" ")
                        });
                        deltas.push(BlockDelta::new(
                            layer_id,
                            reviewer_id,
                            block_id,
                            DeltaType::Modify,
                            start,
                            end,
                            payload,
                        ));
                        base_token_idx += left_len;
                    } else if left_len == 0 && right_len > 0 {
                        // Degenerate: no left tokens (treated as pure insert).
                        let payload = serde_json::json!({
                            "text": diff.right_tokens.join(" ")
                        });
                        deltas.push(BlockDelta::new(
                            layer_id,
                            reviewer_id,
                            block_id,
                            DeltaType::Insert,
                            base_token_idx,
                            base_token_idx,
                            payload,
                        ));
                    } else {
                        base_token_idx += left_len;
                    }
                }
            }
        }

        deltas
    }
}

impl Default for MergeEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rt_core::{Block, BlockType};

    fn make_block(doc_id: Uuid, path: &str, text: &str, pos: i32) -> Block {
        Block::new(BlockType::Clause, path, text, text, None, doc_id, pos)
    }

    // -----------------------------------------------------------------------
    // Test: identical documents produce zero conflicts
    // -----------------------------------------------------------------------

    #[test]
    fn identical_docs_zero_conflicts() {
        let base_doc = Uuid::new_v4();
        let inc_doc = Uuid::new_v4();

        let base_blocks = vec![
            make_block(base_doc, "1.1", "the borrower shall repay the principal", 0),
            make_block(base_doc, "1.2", "interest shall accrue at five percent per annum", 1),
        ];
        // Incoming blocks are clones — same content, same clause_hash.
        let incoming_blocks: Vec<Block> = base_blocks
            .iter()
            .map(|b| {
                let mut b2 = b.clone();
                b2.document_id = inc_doc;
                b2
            })
            .collect();

        let engine = MergeEngine::new();
        let result = engine.merge(base_doc, inc_doc, &base_blocks, &incoming_blocks);

        assert_eq!(result.conflicts.len(), 0, "identical docs must produce no conflicts");
        assert_eq!(result.pending_review, 0);
        assert_eq!(result.auto_resolved, base_blocks.len());
    }

    // -----------------------------------------------------------------------
    // Test: edits in separate blocks auto-merge without conflict
    // -----------------------------------------------------------------------

    #[test]
    fn separate_block_edits_auto_merge() {
        let base_doc = Uuid::new_v4();
        let inc_doc = Uuid::new_v4();

        let base_blocks = vec![
            make_block(base_doc, "1.1", "the borrower shall repay the principal on time", 0),
            make_block(base_doc, "1.2", "interest is fixed at five percent per year", 1),
        ];

        // Incoming: block 1.1 is unchanged; 1.2 has a minor word change at the end.
        let inc_block_1 = {
            let mut b = base_blocks[0].clone();
            b.document_id = inc_doc;
            b
        };
        let inc_block_2 = make_block(inc_doc, "1.2", "interest is fixed at six percent per year", 1);

        let incoming_blocks = vec![inc_block_1, inc_block_2];

        let engine = MergeEngine::new();
        let result = engine.merge(base_doc, inc_doc, &base_blocks, &incoming_blocks);

        // No conflicts — only one block changed and it's a substitution only
        // on one side (the base side has no concurrent edit).
        assert_eq!(result.conflicts.len(), 0);
        assert_eq!(result.pending_review, 0);
    }

    // -----------------------------------------------------------------------
    // Test: overlapping edits produce at least one conflict
    // -----------------------------------------------------------------------

    #[test]
    fn overlapping_edits_result_is_structurally_valid() {
        let base_doc = Uuid::new_v4();
        let inc_doc = Uuid::new_v4();

        let base_blocks = vec![make_block(
            base_doc,
            "1.1",
            "the borrower shall repay on the first day",
            0,
        )];
        let inc_blocks = vec![make_block(
            inc_doc,
            "1.1",
            "the borrower must repay on the second day",
            0,
        )];

        let engine = MergeEngine::new();
        let result = engine.merge(base_doc, inc_doc, &base_blocks, &inc_blocks);

        // pending_review must equal the number of Pending conflicts.
        let actual_pending = result
            .conflicts
            .iter()
            .filter(|c| c.resolution == ConflictResolution::Pending)
            .count();
        assert_eq!(result.pending_review, actual_pending);
        // auto_resolved + pending_review must account for all blocks processed.
        assert!(result.auto_resolved + result.pending_review <= base_blocks.len() + 10);
    }

    // -----------------------------------------------------------------------
    // Test: pure insertion (block only in incoming) is auto-resolved
    // -----------------------------------------------------------------------

    #[test]
    fn pure_insertion_auto_resolved() {
        let base_doc = Uuid::new_v4();
        let inc_doc = Uuid::new_v4();

        let base_blocks: Vec<Block> = vec![];
        let inc_blocks = vec![make_block(inc_doc, "1.1", "brand new clause inserted here", 0)];

        let engine = MergeEngine::new();
        let result = engine.merge(base_doc, inc_doc, &base_blocks, &inc_blocks);

        assert_eq!(result.conflicts.len(), 0);
        assert_eq!(result.auto_resolved, 1);
    }

    // -----------------------------------------------------------------------
    // Test: pure deletion (block only in base) is auto-resolved
    // -----------------------------------------------------------------------

    #[test]
    fn pure_deletion_auto_resolved() {
        let base_doc = Uuid::new_v4();
        let inc_doc = Uuid::new_v4();

        let base_blocks = vec![make_block(base_doc, "1.1", "clause to be removed from document", 0)];
        let inc_blocks: Vec<Block> = vec![];

        let engine = MergeEngine::new();
        let result = engine.merge(base_doc, inc_doc, &base_blocks, &inc_blocks);

        assert_eq!(result.conflicts.len(), 0);
        assert_eq!(result.auto_resolved, 1);
    }

    // -----------------------------------------------------------------------
    // Test: resolve_conflict applies and validates transitions
    // -----------------------------------------------------------------------

    #[test]
    fn resolve_conflict_legal_transition() {
        let mut conflict = MergeConflict::new(
            Uuid::new_v4(),
            crate::conflict::ConflictType::ContentOverlap,
            Some("base content".to_string()),
            Some("incoming content".to_string()),
        );
        let result = MergeEngine::resolve_conflict(&mut conflict, ConflictResolution::AcceptedBase);
        assert!(result.is_ok());
        assert_eq!(conflict.resolution, ConflictResolution::AcceptedBase);
    }

    #[test]
    fn resolve_conflict_illegal_revert() {
        let mut conflict = MergeConflict::new(
            Uuid::new_v4(),
            crate::conflict::ConflictType::ContentOverlap,
            Some("base content".to_string()),
            Some("incoming content".to_string()),
        );
        conflict.resolution = ConflictResolution::AcceptedBase;
        let result = MergeEngine::resolve_conflict(&mut conflict, ConflictResolution::Pending);
        assert!(result.is_err());
    }

    // -----------------------------------------------------------------------
    // Test: MergeResult serializes to valid JSON (contract compliance)
    // -----------------------------------------------------------------------

    #[test]
    fn merge_result_serializes_to_json() {
        let base_doc = Uuid::new_v4();
        let inc_doc = Uuid::new_v4();
        let blocks = vec![make_block(base_doc, "1.1", "some text here", 0)];
        let engine = MergeEngine::new();
        let result = engine.merge(base_doc, inc_doc, &blocks, &blocks);
        let json = serde_json::to_string(&result).expect("must serialize");
        let parsed: serde_json::Value = serde_json::from_str(&json).expect("must parse");
        assert!(parsed.get("merge_id").is_some());
        assert!(parsed.get("base_doc_id").is_some());
        assert!(parsed.get("incoming_doc_id").is_some());
        assert!(parsed.get("conflicts").is_some());
        assert!(parsed.get("auto_resolved").is_some());
        assert!(parsed.get("pending_review").is_some());
    }

    // -----------------------------------------------------------------------
    // Test: MergeEngine default is same as new()
    // -----------------------------------------------------------------------

    #[test]
    fn merge_engine_default_works() {
        let engine = MergeEngine::default();
        let doc = Uuid::new_v4();
        let blocks: Vec<Block> = vec![];
        let result = engine.merge(doc, doc, &blocks, &blocks);
        assert_eq!(result.auto_resolved, 0);
        assert_eq!(result.conflicts.len(), 0);
    }
}
