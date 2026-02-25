use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// ReviewLayer
// ---------------------------------------------------------------------------

/// A named review layer representing one reviewer's set of edits to a document.
///
/// Multiple `ReviewLayer`s on the same document form the input to the merge
/// engine.  Each reviewer's changes are captured as a set of [`BlockDelta`]s
/// attached to this layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewLayer {
    /// Stable unique identifier for this review layer (UUIDv4).
    pub id: Uuid,
    /// The workflow this review layer belongs to.
    pub workflow_id: Uuid,
    /// Identifier of the reviewer who owns this layer.
    pub reviewer_id: String,
    /// The document being reviewed.
    pub document_id: Uuid,
    /// UTC timestamp when this layer was created.
    pub created_at: DateTime<Utc>,
}

impl ReviewLayer {
    /// Construct a new `ReviewLayer` with a freshly generated `id` and
    /// `created_at` set to now.
    pub fn new(workflow_id: Uuid, reviewer_id: impl Into<String>, document_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            workflow_id,
            reviewer_id: reviewer_id.into(),
            document_id,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// DeltaType
// ---------------------------------------------------------------------------

/// The kind of change represented by a [`BlockDelta`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeltaType {
    /// Tokens were inserted at the given position.
    Insert,
    /// Tokens were removed from the given range.
    Delete,
    /// Tokens in the given range were replaced with new content.
    Modify,
}

// ---------------------------------------------------------------------------
// BlockDelta
// ---------------------------------------------------------------------------

/// A single atomic change to a block made within a review layer.
///
/// Token range semantics:
/// - `token_start` is the index of the first affected token (inclusive).
/// - `token_end` is the index of the last affected token (inclusive).
/// - For `Insert` deltas `token_start == token_end` represents an insertion
///   point before that token index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockDelta {
    /// Stable unique identifier for this delta (UUIDv4).
    pub id: Uuid,
    /// The review layer this delta belongs to.
    pub review_layer_id: Uuid,
    /// The reviewer who authored this delta.
    pub reviewer_id: String,
    /// The block being modified.
    pub block_id: Uuid,
    /// The kind of change.
    pub delta_type: DeltaType,
    /// First token index affected (inclusive).
    pub token_start: usize,
    /// Last token index affected (inclusive).
    pub token_end: usize,
    /// Arbitrary JSON payload carrying the change content (e.g., inserted
    /// text, replacement tokens, formatting attributes).
    pub delta_payload: serde_json::Value,
    /// UTC timestamp when this delta was recorded.
    pub created_at: DateTime<Utc>,
}

impl BlockDelta {
    /// Construct a new `BlockDelta` with a freshly generated `id` and
    /// `created_at` set to now.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        review_layer_id: Uuid,
        reviewer_id: impl Into<String>,
        block_id: Uuid,
        delta_type: DeltaType,
        token_start: usize,
        token_end: usize,
        delta_payload: serde_json::Value,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            review_layer_id,
            reviewer_id: reviewer_id.into(),
            block_id,
            delta_type,
            token_start,
            token_end,
            delta_payload,
            created_at: Utc::now(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn layer_id() -> Uuid {
        Uuid::new_v4()
    }

    fn block_id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn review_layer_has_unique_id() {
        let wf = Uuid::new_v4();
        let doc = Uuid::new_v4();
        let l1 = ReviewLayer::new(wf, "alice", doc);
        let l2 = ReviewLayer::new(wf, "alice", doc);
        assert_ne!(l1.id, l2.id);
    }

    #[test]
    fn block_delta_has_unique_id() {
        let lid = layer_id();
        let bid = block_id();
        let d1 = BlockDelta::new(lid, "alice", bid, DeltaType::Modify, 0, 5, serde_json::json!({}));
        let d2 = BlockDelta::new(lid, "alice", bid, DeltaType::Modify, 0, 5, serde_json::json!({}));
        assert_ne!(d1.id, d2.id);
    }

    #[test]
    fn delta_type_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&DeltaType::Insert).unwrap(),
            "\"insert\""
        );
        assert_eq!(
            serde_json::to_string(&DeltaType::Delete).unwrap(),
            "\"delete\""
        );
        assert_eq!(
            serde_json::to_string(&DeltaType::Modify).unwrap(),
            "\"modify\""
        );
    }

    #[test]
    fn block_delta_roundtrips_json() {
        let lid = layer_id();
        let bid = block_id();
        let delta = BlockDelta::new(
            lid,
            "bob",
            bid,
            DeltaType::Insert,
            3,
            3,
            serde_json::json!({"text": "new text"}),
        );
        let json = serde_json::to_string(&delta).unwrap();
        let delta2: BlockDelta = serde_json::from_str(&json).unwrap();
        assert_eq!(delta.id, delta2.id);
        assert_eq!(delta.token_start, delta2.token_start);
        assert_eq!(delta.delta_type, delta2.delta_type);
    }
}
