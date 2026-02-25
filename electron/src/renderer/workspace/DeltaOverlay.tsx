import React, { useState, useRef, useEffect, useCallback } from 'react';
import type { CompareResult, BlockDiff, DiffKind } from '../../shared/types/results';
import type { Block } from '../../shared/types/block';
import { BlockRenderer } from './BlockRenderer';
import type { BlockHighlight } from './BlockRenderer';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface DeltaOverlayProps {
  compareResult: CompareResult;
  leftBlocks: Block[];
  rightBlocks: Block[];
  onDeltaClick?: (anchorSignature: string) => void;
  currentDeltaIndex?: number;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Side-by-side diff view with synchronised scrolling.
 *
 * Left pane  → original (base), deletions highlighted in red.
 * Right pane → modified (incoming), insertions highlighted in green.
 * Modifications highlighted in yellow on both sides.
 *
 * The toolbar provides Previous / Next navigation through non-equal blocks.
 */
export const DeltaOverlay: React.FC<DeltaOverlayProps> = ({
  compareResult,
  leftBlocks,
  rightBlocks,
  onDeltaClick,
  currentDeltaIndex: externalDeltaIndex,
}) => {
  const [internalDeltaIndex, setInternalDeltaIndex] = useState(0);
  const deltaIndex = externalDeltaIndex ?? internalDeltaIndex;

  const leftScrollRef = useRef<HTMLDivElement>(null);
  const rightScrollRef = useRef<HTMLDivElement>(null);
  const isSyncing = useRef(false);

  // Only non-equal diffs are navigable
  const navigableDiffs = compareResult.diffs.filter((d) => d.kind !== 'equal');
  const total = navigableDiffs.length;
  const currentDiff = navigableDiffs[deltaIndex] ?? null;

  // Build highlight maps from the full diff list
  const leftHighlights = buildHighlightMap(compareResult.diffs, 'left');
  const rightHighlights = buildHighlightMap(compareResult.diffs, 'right');

  // Scroll-sync: left drives right and vice-versa
  const syncScroll = useCallback((source: 'left' | 'right') => {
    if (isSyncing.current) return;
    isSyncing.current = true;
    const srcEl = source === 'left' ? leftScrollRef.current : rightScrollRef.current;
    const dstEl = source === 'left' ? rightScrollRef.current : leftScrollRef.current;
    if (srcEl && dstEl) {
      const ratio =
        srcEl.scrollTop / Math.max(1, srcEl.scrollHeight - srcEl.clientHeight);
      dstEl.scrollTop = ratio * (dstEl.scrollHeight - dstEl.clientHeight);
    }
    // Use rAF to clear the flag after the scroll event propagates
    requestAnimationFrame(() => {
      isSyncing.current = false;
    });
  }, []);

  // Navigate to a diff: scroll the relevant block into view on both sides
  const scrollToDiff = useCallback(
    (diff: BlockDiff | null) => {
      if (!diff) return;
      const scrollToId = (ref: React.RefObject<HTMLDivElement>, blockId: string | undefined) => {
        if (!blockId || !ref.current) return;
        const el = ref.current.querySelector<HTMLElement>(`[data-block-id="${blockId}"]`);
        if (el) {
          el.scrollIntoView({ behavior: 'smooth', block: 'center' });
        }
      };
      scrollToId(leftScrollRef, diff.left_block?.id);
      scrollToId(rightScrollRef, diff.right_block?.id);
      if (onDeltaClick && diff.anchor_signature) {
        onDeltaClick(diff.anchor_signature);
      }
    },
    [onDeltaClick]
  );

  // Scroll when delta index changes
  useEffect(() => {
    scrollToDiff(currentDiff);
  }, [deltaIndex, scrollToDiff, currentDiff]);

  const goNext = () => {
    if (total === 0) return;
    const next = (deltaIndex + 1) % total;
    setInternalDeltaIndex(next);
    scrollToDiff(navigableDiffs[next]);
  };

  const goPrev = () => {
    if (total === 0) return;
    const prev = (deltaIndex - 1 + total) % total;
    setInternalDeltaIndex(prev);
    scrollToDiff(navigableDiffs[prev]);
  };

  const currentAnchor = currentDiff?.anchor_signature;
  const leftScrollBlockId = currentDiff?.left_block?.id;
  const rightScrollBlockId = currentDiff?.right_block?.id;

  return (
    <div className="delta-overlay">
      {/* Navigation toolbar */}
      <div className="delta-overlay-toolbar">
        <button
          className="delta-nav-btn"
          onClick={goPrev}
          disabled={total === 0}
          title="Previous change"
          aria-label="Previous change"
        >
          &#8593;
        </button>
        <span className="delta-counter">
          {total === 0
            ? 'No changes'
            : `${deltaIndex + 1} / ${total}`}
        </span>
        <button
          className="delta-nav-btn"
          onClick={goNext}
          disabled={total === 0}
          title="Next change"
          aria-label="Next change"
        >
          &#8595;
        </button>
        <div style={{ marginLeft: 'auto', display: 'flex', gap: '8px', fontSize: '11px', color: 'var(--color-text-muted)' }}>
          <DiffLegend />
        </div>
      </div>

      {/* Split view */}
      <div className="delta-split">
        {/* Left (base) */}
        <div className="delta-split-pane">
          <div className="delta-split-label">Base ({compareResult.left_doc_id.slice(0, 8)}…)</div>
          <div
            className="delta-split-scroll"
            ref={leftScrollRef}
            onScroll={() => syncScroll('left')}
          >
            <BlockRenderer
              blocks={leftBlocks}
              highlights={leftHighlights}
              onBlockClick={(id) => {
                // find which diff contains this block
                const diff = compareResult.diffs.find((d) => d.left_block?.id === id);
                if (diff && onDeltaClick && diff.anchor_signature) {
                  onDeltaClick(diff.anchor_signature);
                }
              }}
              scrollToBlockId={leftScrollBlockId}
            />
          </div>
        </div>

        <div className="delta-split-divider" />

        {/* Right (incoming) */}
        <div className="delta-split-pane">
          <div className="delta-split-label">Incoming ({compareResult.right_doc_id.slice(0, 8)}…)</div>
          <div
            className="delta-split-scroll"
            ref={rightScrollRef}
            onScroll={() => syncScroll('right')}
          >
            <BlockRenderer
              blocks={rightBlocks}
              highlights={rightHighlights}
              onBlockClick={(id) => {
                const diff = compareResult.diffs.find((d) => d.right_block?.id === id);
                if (diff && onDeltaClick && diff.anchor_signature) {
                  onDeltaClick(diff.anchor_signature);
                }
              }}
              scrollToBlockId={rightScrollBlockId}
            />
          </div>
        </div>
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function buildHighlightMap(
  diffs: BlockDiff[],
  side: 'left' | 'right'
): Map<string, BlockHighlight> {
  const map = new Map<string, BlockHighlight>();
  for (const diff of diffs) {
    if (diff.kind === 'equal') continue;
    const block = side === 'left' ? diff.left_block : diff.right_block;
    if (!block) continue;

    let highlight: BlockHighlight;
    if (diff.kind === 'inserted') {
      highlight = 'inserted';
    } else if (diff.kind === 'deleted') {
      highlight = 'deleted';
    } else {
      // modified
      highlight = 'modified';
    }
    map.set(block.id, highlight);
  }
  return map;
}

// ---------------------------------------------------------------------------
// Legend
// ---------------------------------------------------------------------------

const DiffLegend: React.FC = () => (
  <>
    <span style={{ color: 'var(--color-inserted-text)' }}>&#9632; Inserted</span>
    <span style={{ color: 'var(--color-deleted-text)' }}>&#9632; Deleted</span>
    <span style={{ color: 'var(--color-modified-text)' }}>&#9632; Modified</span>
  </>
);

export default DeltaOverlay;
