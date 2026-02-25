import React from 'react';
import type { WorkflowState, WorkflowEvent } from '../../shared/types/results';
import {
  WORKFLOW_TRANSITIONS,
  WORKFLOW_STATUS_LABELS,
  WORKFLOW_EVENT_LABELS,
} from '../../shared/types/workflow';
import { WorkflowTimeline } from './WorkflowTimeline';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface WorkflowPanelProps {
  workflowState: WorkflowState;
  events: WorkflowEvent[];
  onAction: (eventType: string) => void;
  currentActorId?: string;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Full workflow panel (used when expanded from the bottom bar).
 *
 * Shows:
 *   - Current status badge
 *   - Metadata (doc ID, initiator, reviewer count)
 *   - Action buttons for legal transitions from the current state
 *   - Reviewer list
 *   - Event timeline
 */
export const WorkflowPanel: React.FC<WorkflowPanelProps> = ({
  workflowState,
  events,
  onAction,
  currentActorId,
}) => {
  const transitions = WORKFLOW_TRANSITIONS[workflowState.status] ?? [];
  const statusLabel = WORKFLOW_STATUS_LABELS[workflowState.status] ?? workflowState.status;

  return (
    <div className="workflow-panel">
      {/* Header */}
      <div className="workflow-panel-header">
        <span className="workflow-panel-title">Workflow</span>
        <WorkflowStatusBadge status={workflowState.status} label={statusLabel} />
      </div>

      {/* Meta info */}
      <div
        style={{
          padding: '8px 16px',
          borderBottom: '1px solid var(--color-border)',
          fontSize: 12,
          color: 'var(--color-text-muted)',
          display: 'flex',
          flexDirection: 'column',
          gap: 4,
          flexShrink: 0,
        }}
      >
        <div>
          <span style={{ fontWeight: 600 }}>Document: </span>
          <span style={{ fontFamily: 'var(--font-mono)' }}>
            {workflowState.doc_id.slice(0, 16)}…
          </span>
        </div>
        <div>
          <span style={{ fontWeight: 600 }}>Workflow: </span>
          <span style={{ fontFamily: 'var(--font-mono)' }}>
            {workflowState.workflow_id.slice(0, 16)}…
          </span>
        </div>
        <div>
          <span style={{ fontWeight: 600 }}>Reviewers: </span>
          {workflowState.reviewer_ids.length === 0
            ? 'None assigned'
            : workflowState.reviewer_ids.map((id) => truncateId(id)).join(', ')}
        </div>
        <div>
          <span style={{ fontWeight: 600 }}>Approvals: </span>
          {workflowState.approver_ids.length} / {workflowState.reviewer_ids.length || '—'}
        </div>
      </div>

      {/* Action buttons */}
      {transitions.length > 0 && (
        <div
          style={{
            padding: '8px 16px',
            borderBottom: '1px solid var(--color-border)',
            display: 'flex',
            gap: 8,
            flexWrap: 'wrap',
            flexShrink: 0,
          }}
        >
          {transitions.map((t) => (
            <button
              key={t}
              className={`toolbar-btn${t === 'approve' || t === 'merge' ? ' primary' : ''}`}
              onClick={() => onAction(t)}
              style={{ fontSize: 12 }}
            >
              {WORKFLOW_EVENT_LABELS[t] ?? t}
            </button>
          ))}
        </div>
      )}

      {/* Timeline (scrollable) */}
      <WorkflowTimeline events={events} currentActorId={currentActorId} />
    </div>
  );
};

// ---------------------------------------------------------------------------
// WorkflowStatusBadge
// ---------------------------------------------------------------------------

interface WorkflowStatusBadgeProps {
  status: string;
  label: string;
}

export const WorkflowStatusBadge: React.FC<WorkflowStatusBadgeProps> = ({ status, label }) => (
  <span className={`workflow-status-badge ${status}`}>{label}</span>
);

// ---------------------------------------------------------------------------
// WorkflowBar (compact bottom-bar summary)
// ---------------------------------------------------------------------------

interface WorkflowBarProps {
  workflowState: WorkflowState;
  onAction: (eventType: string) => void;
  onClick?: () => void;
}

export const WorkflowBar: React.FC<WorkflowBarProps> = ({
  workflowState,
  onAction,
  onClick,
}) => {
  const transitions = WORKFLOW_TRANSITIONS[workflowState.status] ?? [];
  const statusLabel = WORKFLOW_STATUS_LABELS[workflowState.status] ?? workflowState.status;

  return (
    <div className="workflow-bar" onClick={onClick}>
      <span className="workflow-bar-label">Workflow</span>
      <WorkflowStatusBadge status={workflowState.status} label={statusLabel} />
      {workflowState.reviewer_ids.length > 0 && (
        <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>
          {workflowState.approver_ids.length}/{workflowState.reviewer_ids.length} approved
        </span>
      )}
      <div className="workflow-actions" onClick={(e) => e.stopPropagation()}>
        {transitions.slice(0, 2).map((t) => (
          <button
            key={t}
            className={`toolbar-btn${t === 'approve' || t === 'merge' ? ' primary' : ''}`}
            style={{ fontSize: 11, padding: '2px 8px' }}
            onClick={() => onAction(t)}
          >
            {WORKFLOW_EVENT_LABELS[t] ?? t}
          </button>
        ))}
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function truncateId(id: string): string {
  return id.length <= 8 ? id : id.slice(0, 8) + '…';
}

export default WorkflowPanel;
