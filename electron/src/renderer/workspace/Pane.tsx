import React from 'react';

export interface PaneProps {
  id: string;
  title: string;
  badge?: string | number;
  children: React.ReactNode;
  onClose?: (id: string) => void;
  style?: React.CSSProperties;
  className?: string;
}

/**
 * Individual pane wrapper with a header bar and scrollable content area.
 * Used as a building block inside ThreePaneLayout.
 */
export const Pane: React.FC<PaneProps> = ({
  id,
  title,
  badge,
  children,
  onClose,
  style,
  className,
}) => {
  return (
    <div
      className={`pane${className ? ` ${className}` : ''}`}
      style={style}
    >
      <div className="pane-header">
        <span className="pane-header-title">{title}</span>
        {badge !== undefined && (
          <span className="pane-header-badge">{badge}</span>
        )}
        {onClose && (
          <button
            className="pane-close-btn"
            onClick={() => onClose(id)}
            title="Close pane"
            aria-label={`Close ${title} pane`}
          >
            ×
          </button>
        )}
      </div>
      <div className="pane-content">
        {children}
      </div>
    </div>
  );
};

export default Pane;
