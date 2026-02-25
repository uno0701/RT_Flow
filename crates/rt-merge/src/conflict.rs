use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::layer::{BlockDelta, DeltaType};

// ---------------------------------------------------------------------------
// ConflictType
// ---------------------------------------------------------------------------

/// Category describing how a merge conflict arose.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictType {
    /// Two reviewers edited overlapping token ranges within the same block.
    ContentOverlap,
    /// Two reviewers moved the same block to different structural positions.
    MoveCollision,
    /// One reviewer deleted a block that another reviewer modified.
    DeleteModify,
}

// ---------------------------------------------------------------------------
// ConflictResolution
// ---------------------------------------------------------------------------

/// Resolution state of a [`MergeConflict`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConflictResolution {
    /// The conflict has not yet been reviewed.
    Pending,
    /// The base (original) version was accepted.
    AcceptedBase,
    /// The incoming (reviewer) version was accepted.
    AcceptedIncoming,
    /// A manual resolution was applied (neither base nor incoming verbatim).
    Manual,
}

// ---------------------------------------------------------------------------
// MergeConflict
// ---------------------------------------------------------------------------

/// A single merge conflict requiring human or automated resolution.
///
/// Matches the `MergeConflict` definition in `contracts/merge-result.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeConflict {
    /// Stable unique identifier for this conflict record (UUIDv4).
    pub id: Uuid,
    /// UUID of the block where the conflict originated.
    pub block_id: Uuid,
    /// Category describing how the conflict arose.
    pub conflict_type: ConflictType,
    /// Canonical text of the block as it appears in the base document.
    /// `None` when the block does not exist in the base (e.g. delete_modify
    /// where the base deleted the block).
    pub base_content: Option<String>,
    /// Canonical text of the block as it appears in the incoming document.
    /// `None` when the block does not exist in the incoming document.
    pub incoming_content: Option<String>,
    /// Current resolution state of this conflict.
    pub resolution: ConflictResolution,
}

impl MergeConflict {
    /// Construct a new `MergeConflict` in the `Pending` state with a
    /// freshly generated `id`.
    pub fn new(
        block_id: Uuid,
        conflict_type: ConflictType,
        base_content: Option<String>,
        incoming_content: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            block_id,
            conflict_type,
            base_content,
            incoming_content,
            resolution: ConflictResolution::Pending,
        }
    }

    /// Return `true` when this conflict has been resolved (any state other
    /// than `Pending`).
    pub fn is_resolved(&self) -> bool {
        self.resolution != ConflictResolution::Pending
    }
}

// ---------------------------------------------------------------------------
// Public API: detect_conflicts
// ---------------------------------------------------------------------------

/// Detect conflicts between two sets of deltas that apply to the **same block**.
///
/// Conflict rules (per spec):
/// - `ContentOverlap`: two deltas from different sources whose token ranges
///   overlap (i.e., they both touch at least one common token index).
/// - `DeleteModify`: one delta has `DeltaType::Delete` and the other has
///   `DeltaType::Modify` or `DeltaType::Insert`.
///
/// Non-conflicting:
/// - Deltas whose token ranges are entirely disjoint.
///
/// `base_deltas` are the deltas from the base reviewer (or "base" side),
/// `incoming_deltas` are from the incoming reviewer.  Both sets must already
/// be scoped to the same `block_id`.
///
/// Returns a `Vec<MergeConflict>` — one entry per conflicting pair detected.
/// If no conflicts are found the returned vector is empty.
pub fn detect_conflicts(
    base_deltas: &[BlockDelta],
    incoming_deltas: &[BlockDelta],
) -> Vec<MergeConflict> {
    let mut conflicts = Vec::new();

    for base_delta in base_deltas {
        for inc_delta in incoming_deltas {
            // Deltas must be for the same block; if block_ids differ, skip
            // (caller is responsible for grouping correctly, but be defensive).
            if base_delta.block_id != inc_delta.block_id {
                continue;
            }

            // --- DeleteModify conflict ---
            // One side deletes the block/range, the other modifies it.
            let base_is_delete = base_delta.delta_type == DeltaType::Delete;
            let inc_is_delete = inc_delta.delta_type == DeltaType::Delete;

            if base_is_delete && inc_delta.delta_type != DeltaType::Delete {
                // Base deleted, incoming modified → DeleteModify conflict.
                conflicts.push(MergeConflict::new(
                    base_delta.block_id,
                    ConflictType::DeleteModify,
                    None, // block deleted in base
                    payload_text(&inc_delta.delta_payload),
                ));
                continue;
            }

            if inc_is_delete && base_delta.delta_type != DeltaType::Delete {
                // Incoming deleted, base modified → DeleteModify conflict.
                conflicts.push(MergeConflict::new(
                    base_delta.block_id,
                    ConflictType::DeleteModify,
                    payload_text(&base_delta.delta_payload),
                    None, // block deleted in incoming
                ));
                continue;
            }

            // --- ContentOverlap conflict ---
            // Both sides are non-delete operations whose token ranges overlap.
            if !base_is_delete
                && !inc_is_delete
                && ranges_overlap(
                    base_delta.token_start,
                    base_delta.token_end,
                    inc_delta.token_start,
                    inc_delta.token_end,
                )
            {
                conflicts.push(MergeConflict::new(
                    base_delta.block_id,
                    ConflictType::ContentOverlap,
                    payload_text(&base_delta.delta_payload),
                    payload_text(&inc_delta.delta_payload),
                ));
            }
        }
    }

    conflicts
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Return `true` when the two inclusive token ranges [a_start, a_end] and
/// [b_start, b_end] share at least one common index.
pub(crate) fn ranges_overlap(
    a_start: usize,
    a_end: usize,
    b_start: usize,
    b_end: usize,
) -> bool {
    // Two ranges overlap iff neither is entirely before the other.
    a_start <= b_end && b_start <= a_end
}

/// Extract a human-readable string from the delta payload, if present.
fn payload_text(payload: &serde_json::Value) -> Option<String> {
    payload
        .get("text")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layer::{BlockDelta, DeltaType};
    use serde_json::json;

    fn make_delta(
        block_id: Uuid,
        delta_type: DeltaType,
        token_start: usize,
        token_end: usize,
    ) -> BlockDelta {
        BlockDelta::new(
            Uuid::new_v4(),
            "reviewer",
            block_id,
            delta_type,
            token_start,
            token_end,
            json!({"text": format!("tokens {}-{}", token_start, token_end)}),
        )
    }

    // -----------------------------------------------------------------------
    // ranges_overlap tests
    // -----------------------------------------------------------------------

    #[test]
    fn ranges_overlap_identical() {
        assert!(ranges_overlap(2, 5, 2, 5));
    }

    #[test]
    fn ranges_overlap_partial_left() {
        assert!(ranges_overlap(0, 4, 3, 7));
    }

    #[test]
    fn ranges_overlap_partial_right() {
        assert!(ranges_overlap(3, 7, 0, 4));
    }

    #[test]
    fn ranges_overlap_contained() {
        assert!(ranges_overlap(1, 10, 3, 6));
    }

    #[test]
    fn ranges_overlap_touching_at_endpoint() {
        // [0, 3] and [3, 7] share index 3 → overlap.
        assert!(ranges_overlap(0, 3, 3, 7));
    }

    #[test]
    fn ranges_no_overlap_disjoint() {
        // [0, 2] and [4, 7] → no overlap.
        assert!(!ranges_overlap(0, 2, 4, 7));
    }

    #[test]
    fn ranges_no_overlap_adjacent() {
        // [0, 2] and [3, 7] → adjacent but not overlapping (gap at 3).
        assert!(!ranges_overlap(0, 2, 3, 7));
    }

    // -----------------------------------------------------------------------
    // detect_conflicts tests
    // -----------------------------------------------------------------------

    #[test]
    fn overlapping_ranges_detected_as_conflict() {
        let bid = Uuid::new_v4();
        let base = vec![make_delta(bid, DeltaType::Modify, 0, 5)];
        let incoming = vec![make_delta(bid, DeltaType::Modify, 3, 8)];
        let conflicts = detect_conflicts(&base, &incoming);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::ContentOverlap);
        assert_eq!(conflicts[0].resolution, ConflictResolution::Pending);
    }

    #[test]
    fn non_overlapping_ranges_no_conflict() {
        let bid = Uuid::new_v4();
        let base = vec![make_delta(bid, DeltaType::Modify, 0, 3)];
        let incoming = vec![make_delta(bid, DeltaType::Modify, 5, 9)];
        let conflicts = detect_conflicts(&base, &incoming);
        assert!(conflicts.is_empty(), "non-overlapping ranges must not conflict");
    }

    #[test]
    fn delete_vs_modify_detected() {
        let bid = Uuid::new_v4();
        let base = vec![make_delta(bid, DeltaType::Delete, 0, 10)];
        let incoming = vec![make_delta(bid, DeltaType::Modify, 2, 7)];
        let conflicts = detect_conflicts(&base, &incoming);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::DeleteModify);
        // Base deleted → base_content is None.
        assert!(conflicts[0].base_content.is_none());
    }

    #[test]
    fn modify_vs_delete_detected() {
        let bid = Uuid::new_v4();
        let base = vec![make_delta(bid, DeltaType::Modify, 0, 5)];
        let incoming = vec![make_delta(bid, DeltaType::Delete, 0, 10)];
        let conflicts = detect_conflicts(&base, &incoming);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::DeleteModify);
        // Incoming deleted → incoming_content is None.
        assert!(conflicts[0].incoming_content.is_none());
    }

    #[test]
    fn no_conflict_when_both_delete_same_range() {
        // Two identical deletes are not a conflict — both sides agree.
        let bid = Uuid::new_v4();
        let base = vec![make_delta(bid, DeltaType::Delete, 0, 5)];
        let incoming = vec![make_delta(bid, DeltaType::Delete, 0, 5)];
        // Two deletes → no DeleteModify rule fires; ranges overlap but both
        // are deletes so ContentOverlap does not fire either.
        let conflicts = detect_conflicts(&base, &incoming);
        // Both sides deleted → not a conflict (they agree).
        assert!(conflicts.is_empty());
    }

    #[test]
    fn insert_vs_modify_overlap_conflict() {
        let bid = Uuid::new_v4();
        let base = vec![make_delta(bid, DeltaType::Insert, 4, 4)];
        let incoming = vec![make_delta(bid, DeltaType::Modify, 3, 6)];
        let conflicts = detect_conflicts(&base, &incoming);
        // Insert at position 4 overlaps with Modify [3,6].
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].conflict_type, ConflictType::ContentOverlap);
    }

    #[test]
    fn multiple_conflicting_pairs() {
        let bid = Uuid::new_v4();
        let base = vec![
            make_delta(bid, DeltaType::Modify, 0, 4),
            make_delta(bid, DeltaType::Modify, 10, 14),
        ];
        let incoming = vec![
            make_delta(bid, DeltaType::Modify, 2, 6),
            make_delta(bid, DeltaType::Modify, 12, 16),
        ];
        let conflicts = detect_conflicts(&base, &incoming);
        // base[0] conflicts with incoming[0]; base[1] conflicts with incoming[1].
        assert_eq!(conflicts.len(), 2);
    }
}
