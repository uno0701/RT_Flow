import React, { useEffect, useRef } from 'react';
import type { Block, BlockType, Run } from '../../shared/types/block';

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

export type BlockHighlight = 'inserted' | 'deleted' | 'modified';

interface BlockRendererProps {
  blocks: Block[];
  highlights?: Map<string, BlockHighlight>;
  onBlockClick?: (blockId: string) => void;
  scrollToBlockId?: string;
}

/**
 * Recursively renders a Block[] tree as semantic HTML.
 *
 * highlight colours:
 *   inserted → green left-border
 *   deleted  → red  left-border + reduced opacity
 *   modified → yellow left-border
 */
export const BlockRenderer: React.FC<BlockRendererProps> = ({
  blocks,
  highlights,
  onBlockClick,
  scrollToBlockId,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!scrollToBlockId || !containerRef.current) return;
    const el = containerRef.current.querySelector<HTMLElement>(
      `[data-block-id="${scrollToBlockId}"]`
    );
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    }
  }, [scrollToBlockId]);

  return (
    <div className="block-renderer" ref={containerRef}>
      {blocks.map((block) => (
        <RenderBlock
          key={block.id}
          block={block}
          highlights={highlights}
          onBlockClick={onBlockClick}
        />
      ))}
    </div>
  );
};

// ---------------------------------------------------------------------------
// Internal: single block
// ---------------------------------------------------------------------------

interface RenderBlockProps {
  block: Block;
  highlights?: Map<string, BlockHighlight>;
  onBlockClick?: (blockId: string) => void;
}

const RenderBlock: React.FC<RenderBlockProps> = ({ block, highlights, onBlockClick }) => {
  const highlight = highlights?.get(block.id);

  const highlightClass = highlight ? `block-highlight-${highlight}` : '';
  const clickable = !!onBlockClick;

  const sharedProps: React.HTMLAttributes<HTMLElement> & { 'data-block-id': string } = {
    'data-block-id': block.id,
    className: [
      'block',
      blockTypeClass(block.block_type),
      highlightClass,
      clickable ? 'clickable' : '',
    ]
      .filter(Boolean)
      .join(' '),
    onClick: clickable ? () => onBlockClick!(block.id) : undefined,
  };

  // Render each block type
  switch (block.block_type) {
    case 'section':
      return (
        <section {...sharedProps}>
          {block.structural_path && (
            <h2 className="block-heading">
              <span className="block-structural-path">{block.structural_path}</span>
              <RenderRuns runs={block.runs} />
            </h2>
          )}
          {block.children.map((child) => (
            <RenderBlock
              key={child.id}
              block={child}
              highlights={highlights}
              onBlockClick={onBlockClick}
            />
          ))}
        </section>
      );

    case 'clause':
      return (
        <div {...sharedProps}>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'baseline' }}>
            {block.structural_path && (
              <span className="block-structural-path">{block.structural_path}</span>
            )}
            <span>
              <RenderRuns runs={block.runs} />
            </span>
          </div>
          {block.children.map((child) => (
            <RenderBlock
              key={child.id}
              block={child}
              highlights={highlights}
              onBlockClick={onBlockClick}
            />
          ))}
        </div>
      );

    case 'subclause':
      return (
        <div {...sharedProps}>
          <div style={{ display: 'flex', gap: '8px', alignItems: 'baseline' }}>
            {block.structural_path && (
              <span className="block-structural-path">{block.structural_path}</span>
            )}
            <span>
              <RenderRuns runs={block.runs} />
            </span>
          </div>
          {block.children.map((child) => (
            <RenderBlock
              key={child.id}
              block={child}
              highlights={highlights}
              onBlockClick={onBlockClick}
            />
          ))}
        </div>
      );

    case 'paragraph':
      return (
        <div {...sharedProps}>
          <p>
            <RenderRuns runs={block.runs} />
          </p>
          {block.children.map((child) => (
            <RenderBlock
              key={child.id}
              block={child}
              highlights={highlights}
              onBlockClick={onBlockClick}
            />
          ))}
        </div>
      );

    case 'table':
      return (
        <div {...sharedProps}>
          <table>
            <tbody>
              {block.children.map((row) => (
                <RenderBlock
                  key={row.id}
                  block={row}
                  highlights={highlights}
                  onBlockClick={onBlockClick}
                />
              ))}
            </tbody>
          </table>
        </div>
      );

    case 'table_row':
      return (
        <tr data-block-id={block.id}>
          {block.children.map((cell) => (
            <RenderBlock
              key={cell.id}
              block={cell}
              highlights={highlights}
              onBlockClick={onBlockClick}
            />
          ))}
        </tr>
      );

    case 'table_cell':
      return (
        <td data-block-id={block.id} className={highlightClass}>
          <RenderRuns runs={block.runs} />
          {block.children.map((child) => (
            <RenderBlock
              key={child.id}
              block={child}
              highlights={highlights}
              onBlockClick={onBlockClick}
            />
          ))}
        </td>
      );

    default:
      return (
        <div {...sharedProps}>
          <RenderRuns runs={block.runs} />
          {block.children.map((child) => (
            <RenderBlock
              key={child.id}
              block={child}
              highlights={highlights}
              onBlockClick={onBlockClick}
            />
          ))}
        </div>
      );
  }
};

// ---------------------------------------------------------------------------
// Internal: run renderer
// ---------------------------------------------------------------------------

const RenderRuns: React.FC<{ runs: Run[] }> = ({ runs }) => {
  if (!runs || runs.length === 0) return null;

  return (
    <>
      {runs.map((run, i) => {
        const { bold, italic, underline, strikethrough, font_size, color } = run.formatting;
        const classes: string[] = [];
        if (bold) classes.push('run-bold');
        if (italic) classes.push('run-italic');
        if (underline) classes.push('run-underline');
        if (strikethrough) classes.push('run-strikethrough');

        const style: React.CSSProperties = {};
        if (font_size) style.fontSize = `${font_size}pt`;
        if (color) style.color = color;

        return (
          <span
            key={i}
            className={classes.length > 0 ? classes.join(' ') : undefined}
            style={Object.keys(style).length > 0 ? style : undefined}
          >
            {run.text}
          </span>
        );
      })}
    </>
  );
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function blockTypeClass(type: BlockType): string {
  switch (type) {
    case 'section':    return 'block-section';
    case 'clause':     return 'block-clause';
    case 'subclause':  return 'block-subclause';
    case 'paragraph':  return 'block-paragraph';
    case 'table':      return 'block-table';
    case 'table_row':  return '';
    case 'table_cell': return '';
    default:           return '';
  }
}

export default BlockRenderer;
