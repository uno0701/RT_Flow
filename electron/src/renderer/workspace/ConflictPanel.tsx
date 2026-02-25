import React, { useState } from 'react';
import type { MergeConflict, ConflictResolution } from '../../shared/types/workflow';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface ConflictPanelProps {
  conflicts: MergeConflict[];
  onResolve: (conflictId: string, resolution: ConflictResolution) => void;
  selectedConflictId?: string;
  onSelectConflict?: (conflictId: string) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Slide-out panel listing merge conflicts with resolution controls.
 *
 * - Lists all conflicts with status indicators.
 * - Clicking a conflict selects it and shows a diff detail view.
 * - Resolution buttons: Accept Base | Accept Incoming | Manual Edit.
 * - Progress bar shows how many conflicts have been resolved.
 */
export const ConflictPanel: React.FC<ConflictPanelProps> = ({
  conflicts,
  onResolve,
  selectedConflictId,
  onSelectConflict,
}) => {
  const [internalSelected, setInternalSelected] = useState<string | null>(
    selectedConflictId ?? null
  );

  const selectedId = selectedConflictId ?? internalSelected;
  const selectedConflict = conflicts.find((c) => c.id === selectedId) ?? null;

  const resolvedCount = conflicts.filter((c) => c.resolution !== null).length;
  const total = conflicts.length;

  const handleSelect = (id: string) => {
    setInternalSelected(id);
    onSelectConflict?.(id);
  };

  const handleResolve = (resolution: ConflictResolution) => {
    if (!selectedId) return;
    onResolve(selectedId, resolution);
  };

  return (
    <div className="conflict-panel">
      {/* Header */}
      <div className="conflict-panel-header">
        <span className="conflict-panel-title">Conflicts</span>
        {total > 0 && (
          <span className="conflict-progress">
            {resolvedCount} / {total}
          </span>
        )}
      </div>

      {/* Progress bar */}
      {total > 0 && (
        <div
          style={{
            height: 3,
            background: 'var(--color-border)',
            flexShrink: 0,
          }}
        >
          <div
            style={{
              height: '100%',
              width: `${(resolvedCount / total) * 100}%`,
              background: resolvedCount === total
                ? 'var(--color-inserted-border)'
                : 'var(--color-accent)',
              transition: 'width 300ms ease',
            }}
          />
        </div>
      )}

      {/* List */}
      <div className="conflict-list">
        {conflicts.length === 0 ? (
          <div className="empty-state" style={{ padding: '24px 16px' }}>
            <div className="empty-state-title">No conflicts</div>
            <div className="empty-state-desc">All blocks merged cleanly.</div>
          </div>
        ) : (
          conflicts.map((conflict) => (
            <ConflictListItem
              key={conflict.id}
              conflict={conflict}
              isSelected={conflict.id === selectedId}
              onClick={() => handleSelect(conflict.id)}
            />
          ))
        )}
      </div>

      {/* Detail panel for selected conflict */}
      {selectedConflict && (
        <ConflictDetail
          conflict={selectedConflict}
          onResolve={handleResolve}
        />
      )}
    </div>
  );
};

// ---------------------------------------------------------------------------
// Sub-components
// ---------------------------------------------------------------------------

interface ConflictListItemProps {
  conflict: MergeConflict;
  isSelected: boolean;
  onClick: () => void;
}

const ConflictListItem: React.FC<ConflictListItemProps> = ({
  conflict,
  isSelected,
  onClick,
}) => {
  const resolved = conflict.resolution !== null;
  return (
    <div
      className={[
        'conflict-item',
        isSelected ? 'selected' : '',
        resolved ? 'resolved' : '',
      ]
        .filter(Boolean)
        .join(' ')}
      onClick={onClick}
      role="button"
      tabIndex={0}
      onKeyDown={(e) => e.key === 'Enter' && onClick()}
      aria-selected={isSelected}
    >
      <div className="conflict-item-path">{conflict.structural_path}</div>
      <div className="conflict-item-desc">{conflict.description}</div>
      <div className={`conflict-item-status ${resolved ? 'resolved' : 'unresolved'}`}>
        {resolved ? resolutionLabel(conflict.resolution!) : 'Unresolved'}
      </div>
    </div>
  );
};

interface ConflictDetailProps {
  conflict: MergeConflict;
  onResolve: (resolution: ConflictResolution) => void;
}

const ConflictDetail: React.FC<ConflictDetailProps> = ({ conflict, onResolve }) => {
  return (
    <div className="conflict-detail">
      <div className="conflict-detail-section">
        <div className="conflict-detail-label">Base</div>
        <div className="conflict-detail-text base">
          {conflict.base_text || '(empty)'}
        </div>
      </div>
      <div className="conflict-detail-section">
        <div className="conflict-detail-label">Incoming</div>
        <div className="conflict-detail-text incoming">
          {conflict.incoming_text || '(empty)'}
        </div>
      </div>
      <div className="conflict-actions">
        <button
          className="conflict-action-btn accept-base"
          onClick={() => onResolve('accepted_base')}
          disabled={conflict.resolution === 'accepted_base'}
        >
          Accept Base
        </button>
        <button
          className="conflict-action-btn accept-incoming"
          onClick={() => onResolve('accepted_incoming')}
          disabled={conflict.resolution === 'accepted_incoming'}
        >
          Accept Incoming
        </button>
        <button
          className="conflict-action-btn"
          onClick={() => onResolve('manual')}
          disabled={conflict.resolution === 'manual'}
        >
          Manual Edit
        </button>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function resolutionLabel(resolution: ConflictResolution): string {
  switch (resolution) {
    case 'accepted_base':     return 'Accepted Base';
    case 'accepted_incoming': return 'Accepted Incoming';
    case 'manual':            return 'Manual Edit';
    default:                  return resolution;
  }
}

export default ConflictPanel;
