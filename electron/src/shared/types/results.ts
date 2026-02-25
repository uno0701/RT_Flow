/**
 * TypeScript interfaces for the result and state types exchanged between
 * the native rt-ffi layer and the Electron/renderer process.
 *
 * Field names use snake_case to match serde's JSON output so that
 * JSON.parse / JSON.stringify round-trips cleanly without a mapping layer.
 */

import type { Block } from './block';

// ---------------------------------------------------------------------------
// Generic FFI result envelope
// ---------------------------------------------------------------------------

/**
 * Generic result envelope returned (after JSON deserialisation) for every
 * `rtflow_*` call.
 *
 * The Rust side allocates `RtflowResult { ok, data, error }` where `data`
 * and `error` are raw C strings.  The managed layers (C# / Node N-API)
 * deserialise the JSON inside `data` into `T` before surfacing this type.
 */
export interface RtflowResult<T> {
  /** `true` when the native call succeeded. */
  ok: boolean;
  /** Typed payload on success; absent or `undefined` on failure. */
  data?: T;
  /** Human-readable error message on failure; absent or `undefined` on success. */
  error?: string;
}

// ---------------------------------------------------------------------------
// Compare
// ---------------------------------------------------------------------------

/** Disposition of a single block after comparison. */
export type DiffKind = 'equal' | 'inserted' | 'deleted' | 'modified';

/**
 * Comparison result for one aligned pair (or singleton) of blocks across two
 * documents.
 */
export interface BlockDiff {
  /** Disposition of this block. */
  kind: DiffKind;
  /**
   * Block from the left (base) document.
   * `null` for pure insertions.
   */
  left_block: Block | null;
  /**
   * Block from the right (incoming) document.
   * `null` for pure deletions.
   */
  right_block: Block | null;
  /**
   * Anchor signature used to align the two sides; present when `kind` is
   * `"equal"` or `"modified"`.
   */
  anchor_signature: string | null;
}

/**
 * Top-level result returned by `rtflow_compare`.
 * Placeholder — full field set will be fleshed out when rt-compare is implemented.
 */
export interface CompareResult {
  /** UUID of the left (base) document. */
  left_doc_id: string;
  /** UUID of the right (incoming) document. */
  right_doc_id: string;
  /** Ordered list of per-block diffs. */
  diffs: BlockDiff[];
  /** Number of blocks that differ between the two documents. */
  changed_count: number;
  /** Number of blocks that are identical in both documents. */
  equal_count: number;
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/** Outcome of merging a single block. */
export type MergeBlockOutcome =
  | 'accepted_base'
  | 'accepted_incoming'
  | 'auto_merged'
  | 'conflict';

/**
 * Per-block merge decision.
 */
export interface MergeBlockResult {
  /** Outcome for this block. */
  outcome: MergeBlockOutcome;
  /**
   * The resulting merged block.
   * `null` when the block was deleted on both sides or suppressed.
   */
  block: Block | null;
  /**
   * Human-readable conflict description; present only when
   * `outcome === "conflict"`.
   */
  conflict_detail: string | null;
}

/**
 * Top-level result returned by `rtflow_merge`.
 * Placeholder — full field set will be fleshed out when rt-merge is implemented.
 */
export interface MergeResult {
  /** UUID of the base document. */
  base_doc_id: string;
  /** UUID of the incoming document. */
  incoming_doc_id: string;
  /** UUID of the newly created merged document. */
  merged_doc_id: string;
  /** Per-block merge decisions, in document order. */
  block_results: MergeBlockResult[];
  /** Number of blocks that were merged without conflict. */
  auto_merged_count: number;
  /** Number of blocks that require manual resolution. */
  conflict_count: number;
}

// ---------------------------------------------------------------------------
// Workflow
// ---------------------------------------------------------------------------

/** High-level lifecycle stage of a workflow. */
export type WorkflowStatus =
  | 'pending'
  | 'in_review'
  | 'changes_requested'
  | 'approved'
  | 'rejected'
  | 'merged'
  | 'archived';

/**
 * A single event that may be submitted to the workflow state machine via
 * `rtflow_workflow_event`.
 * Placeholder — event variants will be enumerated when rt-workflow is implemented.
 */
export interface WorkflowEvent {
  /**
   * Discriminant identifying the event variant (e.g. `"submit_for_review"`,
   * `"approve"`, `"request_changes"`).
   */
  kind: string;
  /** UUID of the actor who triggered the event. */
  actor_id: string;
  /** ISO 8601 UTC timestamp when the event was created. */
  timestamp: string;
  /** Arbitrary event-specific payload. */
  payload: Record<string, unknown>;
}

/**
 * Snapshot of a workflow's current state, returned by both
 * `rtflow_workflow_state` and `rtflow_workflow_event`.
 * Placeholder — full field set will be fleshed out when rt-workflow is implemented.
 */
export interface WorkflowState {
  /** Stable UUID for this workflow instance. */
  workflow_id: string;
  /** UUID of the document under review. */
  doc_id: string;
  /** Current lifecycle stage. */
  status: WorkflowStatus;
  /** UUID of the user who initiated the workflow. */
  initiator_id: string;
  /** UUIDs of users assigned as reviewers. */
  reviewer_ids: string[];
  /** UUIDs of users who have approved the document so far. */
  approver_ids: string[];
  /** Ordered history of events applied to this workflow. */
  event_history: WorkflowEvent[];
  /** ISO 8601 UTC timestamp when the workflow was created. */
  created_at: string;
  /** ISO 8601 UTC timestamp of the most recent state transition. */
  updated_at: string;
}
