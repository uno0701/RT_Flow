import React, { useState, useRef, useCallback } from 'react';
import { Pane } from './Pane';

export interface PaneConfig {
  id: string;
  type: 'editor' | 'redline' | 'snapshot' | 'browser';
  title: string;
  content: React.ReactNode;
  badge?: string | number;
}

interface ThreePaneLayoutProps {
  panes: PaneConfig[];
  onPaneClose?: (id: string) => void;
  onPaneSwap?: (fromId: string, toId: string) => void;
}

/**
 * Renders up to three resizable panes side by side.
 *
 * - 1 pane  → full width
 * - 2 panes → 50 / 50 (resizable)
 * - 3 panes → 33 / 33 / 33 (resizable)
 *
 * The user can drag the divider handles to redistribute widths.
 * Pane widths are stored as percentage-based flex-basis values so the
 * layout remains proportional when the window is resized.
 */
export const ThreePaneLayout: React.FC<ThreePaneLayoutProps> = ({
  panes,
  onPaneClose,
}) => {
  const visiblePanes = panes.slice(0, 3);
  const count = visiblePanes.length;

  // initialise equal widths in percent
  const initialWidths = (): number[] => {
    if (count === 0) return [];
    const w = 100 / count;
    return visiblePanes.map(() => w);
  };

  const [widths, setWidths] = useState<number[]>(initialWidths);
  const containerRef = useRef<HTMLDivElement>(null);
  const draggingDivider = useRef<number | null>(null);
  const dragStartX = useRef<number>(0);
  const dragStartWidths = useRef<number[]>([]);

  // Re-sync widths when the pane list changes length
  React.useEffect(() => {
    if (count === 0) {
      setWidths([]);
      return;
    }
    const w = 100 / count;
    setWidths(visiblePanes.map(() => w));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [count]);

  const onDividerMouseDown = useCallback(
    (e: React.MouseEvent, dividerIndex: number) => {
      e.preventDefault();
      draggingDivider.current = dividerIndex;
      dragStartX.current = e.clientX;
      dragStartWidths.current = [...widths];

      const onMouseMove = (ev: MouseEvent) => {
        if (draggingDivider.current === null || !containerRef.current) return;
        const containerWidth = containerRef.current.offsetWidth;
        const deltaPercent = ((ev.clientX - dragStartX.current) / containerWidth) * 100;
        const divIdx = draggingDivider.current;

        const newWidths = [...dragStartWidths.current];
        const MIN_WIDTH = 10; // percent

        newWidths[divIdx] = Math.max(MIN_WIDTH, newWidths[divIdx] + deltaPercent);
        newWidths[divIdx + 1] = Math.max(
          MIN_WIDTH,
          dragStartWidths.current[divIdx] + dragStartWidths.current[divIdx + 1] - newWidths[divIdx]
        );

        // Clamp: ensure total remains 100
        const total = newWidths.reduce((s, w) => s + w, 0);
        const factor = 100 / total;
        setWidths(newWidths.map((w) => w * factor));
      };

      const onMouseUp = () => {
        draggingDivider.current = null;
        window.removeEventListener('mousemove', onMouseMove);
        window.removeEventListener('mouseup', onMouseUp);
      };

      window.addEventListener('mousemove', onMouseMove);
      window.addEventListener('mouseup', onMouseUp);
    },
    [widths]
  );

  if (count === 0) {
    return (
      <div className="three-pane-layout">
        <div className="empty-state">
          <div className="empty-state-icon">&#9633;</div>
          <div className="empty-state-title">No panes open</div>
          <div className="empty-state-desc">Open a document to get started.</div>
        </div>
      </div>
    );
  }

  return (
    <div className="three-pane-layout" ref={containerRef}>
      {visiblePanes.map((pane, i) => (
        <React.Fragment key={pane.id}>
          <Pane
            id={pane.id}
            title={pane.title}
            badge={pane.badge}
            onClose={onPaneClose}
            style={{ flexBasis: `${widths[i] ?? 100 / count}%`, flexShrink: 0 }}
          >
            {pane.content}
          </Pane>
          {i < count - 1 && (
            <div
              className="pane-divider"
              onMouseDown={(e) => onDividerMouseDown(e, i)}
              title="Drag to resize"
            />
          )}
        </React.Fragment>
      ))}
    </div>
  );
};

export default ThreePaneLayout;
