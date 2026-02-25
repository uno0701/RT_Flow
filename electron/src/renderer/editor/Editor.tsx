import React, { useRef, useEffect, useCallback, useState } from 'react';
import type { Block } from '../../shared/types/block';
import { BlockRenderer } from '../workspace/BlockRenderer';
import type { BlockHighlight } from '../workspace/BlockRenderer';
import { syncDomToBlocks, diffBlocks } from './BlockSync';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface EditorProps {
  blocks: Block[];
  onChange: (updatedBlocks: Block[]) => void;
  readOnly?: boolean;
  /** When true, insertions/deletions are visually marked. */
  trackChanges?: boolean;
  /** Optional map of block IDs to track-change highlight states. */
  trackHighlights?: Map<string, BlockHighlight>;
}

// Debounce delay in ms for syncing DOM → Block[]
const DEBOUNCE_MS = 300;

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

/**
 * Block-backed document editor.
 *
 * In read-only mode the document is rendered as plain semantic HTML.
 * In edit mode the content is made contenteditable and changes are
 * debounced before being synced back to the Block[] model via BlockSync.
 *
 * Track-changes mode accepts an optional `trackHighlights` map so that the
 * caller can overlay insertion/deletion colours while editing.
 */
export const Editor: React.FC<EditorProps> = ({
  blocks,
  onChange,
  readOnly = false,
  trackChanges = false,
  trackHighlights,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const debounceTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const latestBlocks = useRef<Block[]>(blocks);
  const [isDirty, setIsDirty] = useState(false);

  // Keep ref in sync so the debounced handler always sees the latest blocks
  useEffect(() => {
    latestBlocks.current = blocks;
  }, [blocks]);

  // Sync DOM → Block[] on input (debounced)
  const handleInput = useCallback(() => {
    if (!containerRef.current) return;
    setIsDirty(true);

    if (debounceTimer.current) {
      clearTimeout(debounceTimer.current);
    }

    debounceTimer.current = setTimeout(() => {
      if (!containerRef.current) return;
      const updated = syncDomToBlocks(containerRef.current, latestBlocks.current);
      const deltas = diffBlocks(latestBlocks.current, updated);
      if (deltas.length > 0) {
        onChange(updated);
        setIsDirty(false);
      }
    }, DEBOUNCE_MS);
  }, [onChange]);

  // Cleanup timer on unmount
  useEffect(() => {
    return () => {
      if (debounceTimer.current) {
        clearTimeout(debounceTimer.current);
      }
    };
  }, []);

  const editorClass = [
    'editor',
    readOnly ? 'editor-read-only' : '',
  ]
    .filter(Boolean)
    .join(' ');

  return (
    <div className={editorClass}>
      {!readOnly && (
        <EditorToolbar isDirty={isDirty} trackChanges={trackChanges} />
      )}
      <div
        ref={containerRef}
        className="editor-content"
        contentEditable={!readOnly}
        suppressContentEditableWarning
        onInput={handleInput}
        aria-label="Document editor"
        aria-readonly={readOnly}
        spellCheck={false}
      >
        {/* BlockRenderer renders the read-only semantic view inside the
            contenteditable container. React does NOT reconcile this after
            the user starts editing (suppressContentEditableWarning), which
            means user edits are preserved until a programmatic block update
            re-renders the tree. */}
        <BlockRenderer
          blocks={blocks}
          highlights={trackChanges ? trackHighlights : undefined}
          onBlockClick={undefined}
        />
      </div>
    </div>
  );
};

// ---------------------------------------------------------------------------
// EditorToolbar
// ---------------------------------------------------------------------------

interface EditorToolbarProps {
  isDirty: boolean;
  trackChanges: boolean;
}

const EditorToolbar: React.FC<EditorToolbarProps> = ({ isDirty, trackChanges }) => (
  <div className="editor-toolbar">
    <span
      style={{
        fontSize: 11,
        color: 'var(--color-text-faint)',
        fontStyle: isDirty ? 'italic' : 'normal',
      }}
    >
      {isDirty ? 'Unsaved changes…' : 'All changes saved'}
    </span>
    {trackChanges && (
      <span
        style={{
          marginLeft: 'auto',
          fontSize: 11,
          color: 'var(--color-modified-text)',
          fontWeight: 600,
        }}
      >
        Track Changes ON
      </span>
    )}
  </div>
);

export default Editor;
