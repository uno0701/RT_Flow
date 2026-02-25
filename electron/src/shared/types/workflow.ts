/**
 * Additional workflow types used by the UI layer.
 * These supplement the WorkflowState / WorkflowEvent interfaces in results.ts.
 */

import type { WorkflowStatus } from './results';

/** A merge conflict requiring manual resolution. */
export interface MergeConflict {
  /** Stable unique identifier for this conflict. */
  id: string;
  /** UUID of the block involved in the conflict. */
  block_id: string;
  /** Human-readable structural address of the conflicting block. */
  structural_path: string;
  /** Plain-text description of why the conflict occurred. */
  description: string;
  /** Text content from the base document side. */
  base_text: string;
  /** Text content from the incoming document side. */
  incoming_text: string;
  /** Resolution state of this conflict. */
  resolution: ConflictResolution | null;
}

/** Possible resolutions for a merge conflict. */
export type ConflictResolution =
  | 'accepted_base'
  | 'accepted_incoming'
  | 'manual';

/** Valid workflow transitions from a given status. */
export const WORKFLOW_TRANSITIONS: Record<WorkflowStatus, string[]> = {
  pending: ['submit_for_review'],
  in_review: ['approve', 'request_changes', 'reject'],
  changes_requested: ['submit_for_review', 'reject'],
  approved: ['merge', 'reject'],
  rejected: ['submit_for_review'],
  merged: ['archive'],
  archived: [],
};

/** Human-readable labels for workflow status values. */
export const WORKFLOW_STATUS_LABELS: Record<WorkflowStatus, string> = {
  pending: 'Pending',
  in_review: 'In Review',
  changes_requested: 'Changes Requested',
  approved: 'Approved',
  rejected: 'Rejected',
  merged: 'Merged',
  archived: 'Archived',
};

/** Human-readable labels for workflow event kind values. */
export const WORKFLOW_EVENT_LABELS: Record<string, string> = {
  submit_for_review: 'Submit for Review',
  approve: 'Approve',
  request_changes: 'Request Changes',
  reject: 'Reject',
  merge: 'Merge',
  archive: 'Archive',
};
