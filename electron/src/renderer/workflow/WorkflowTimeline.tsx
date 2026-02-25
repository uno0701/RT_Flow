import React from 'react';
import type { WorkflowEvent } from '../../shared/types/results';
import { WORKFLOW_EVENT_LABELS } from '../../shared/types/workflow';

interface WorkflowTimelineProps {
  events: WorkflowEvent[];
  currentActorId?: string;
}

/**
 * Vertical timeline of workflow events, newest-first.
 *
 * Each event shows:
 *   - A coloured dot (accent for the most recent event)
 *   - The event kind label
 *   - Actor ID and formatted timestamp
 */
export const WorkflowTimeline: React.FC<WorkflowTimelineProps> = ({
  events,
  currentActorId,
}) => {
  if (events.length === 0) {
    return (
      <div className="empty-state" style={{ padding: '24px 16px' }}>
        <div className="empty-state-title">No events yet</div>
        <div className="empty-state-desc">Workflow history will appear here.</div>
      </div>
    );
  }

  // Show newest first
  const ordered = [...events].reverse();

  return (
    <div className="workflow-timeline">
      {ordered.map((event, i) => {
        const label = WORKFLOW_EVENT_LABELS[event.kind] ?? event.kind;
        const isMostRecent = i === 0;
        const isCurrentActor = currentActorId && event.actor_id === currentActorId;

        return (
          <div key={`${event.kind}-${event.timestamp}-${i}`} className="timeline-event">
            <div className={`timeline-dot${isMostRecent ? ' current' : ''}`} />
            <div className="timeline-body">
              <div className="timeline-kind">{label}</div>
              <div className="timeline-meta">
                <span title={event.actor_id}>
                  {isCurrentActor ? 'You' : truncateId(event.actor_id)}
                </span>
                {' · '}
                <span title={event.timestamp}>
                  {formatTimestamp(event.timestamp)}
                </span>
              </div>
              {Object.keys(event.payload).length > 0 && (
                <PayloadSummary payload={event.payload} />
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
};

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

const PayloadSummary: React.FC<{ payload: Record<string, unknown> }> = ({ payload }) => {
  const entries = Object.entries(payload).slice(0, 3);
  return (
    <div
      style={{
        marginTop: 4,
        fontSize: 11,
        color: 'var(--color-text-faint)',
        fontFamily: 'var(--font-mono)',
      }}
    >
      {entries.map(([k, v]) => (
        <span key={k} style={{ marginRight: 8 }}>
          {k}: {String(v)}
        </span>
      ))}
      {Object.keys(payload).length > 3 && (
        <span>+{Object.keys(payload).length - 3} more</span>
      )}
    </div>
  );
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function truncateId(id: string): string {
  if (id.length <= 12) return id;
  return id.slice(0, 8) + '…';
}

function formatTimestamp(iso: string): string {
  try {
    const d = new Date(iso);
    return d.toLocaleString(undefined, {
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    });
  } catch {
    return iso;
  }
}

export default WorkflowTimeline;
