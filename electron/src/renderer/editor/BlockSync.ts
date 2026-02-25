/**
 * Utilities to synchronise DOM edits back to the Block[] model and compute
 * block-level diffs between two versions of a block tree.
 */

import type { Block } from '../../shared/types/block';

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

export interface BlockDelta {
  /** ID of the block that changed. */
  blockId: string;
  /** New canonical text for the block. */
  newText: string;
  /** Previous canonical text before the edit. */
  originalText: string;
}

// ---------------------------------------------------------------------------
// syncDomToBlocks
// ---------------------------------------------------------------------------

/**
 * Walk a contenteditable container, find elements carrying `data-block-id`
 * attributes, and return an updated copy of `originalBlocks` with
 * `canonical_text` and `display_text` reflecting the current DOM content.
 *
 * Only blocks whose text has actually changed are cloned; unchanged blocks
 * are returned by reference (preserving identity / React reconciliation keys).
 */
export function syncDomToBlocks(
  container: HTMLElement,
  originalBlocks: Block[]
): Block[] {
  // Build a flat map: id → domText
  const domTextMap = new Map<string, string>();
  const els = container.querySelectorAll<HTMLElement>('[data-block-id]');
  els.forEach((el) => {
    const id = el.getAttribute('data-block-id');
    if (id) {
      domTextMap.set(id, normaliseWhitespace(el.innerText ?? el.textContent ?? ''));
    }
  });

  return patchBlocks(originalBlocks, domTextMap);
}

// ---------------------------------------------------------------------------
// diffBlocks
// ---------------------------------------------------------------------------

/**
 * Compute a flat list of deltas between two block trees.
 * Compares blocks by ID; blocks present in `original` but not `updated` (or
 * vice-versa) are ignored — only textual differences are reported.
 */
export function diffBlocks(original: Block[], updated: Block[]): BlockDelta[] {
  const originalMap = flattenById(original);
  const updatedMap = flattenById(updated);
  const deltas: BlockDelta[] = [];

  for (const [id, updatedBlock] of updatedMap) {
    const originalBlock = originalMap.get(id);
    if (!originalBlock) continue; // new block — not tracked here
    if (originalBlock.canonical_text !== updatedBlock.canonical_text) {
      deltas.push({
        blockId: id,
        newText: updatedBlock.canonical_text,
        originalText: originalBlock.canonical_text,
      });
    }
  }

  return deltas;
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/** Recursively patch block text from domTextMap; returns new array. */
function patchBlocks(blocks: Block[], domTextMap: Map<string, string>): Block[] {
  return blocks.map((block) => {
    const newText = domTextMap.get(block.id);
    const patchedChildren = patchBlocks(block.children, domTextMap);
    const childrenChanged = patchedChildren !== block.children &&
      patchedChildren.some((c, i) => c !== block.children[i]);

    if (newText !== undefined && newText !== block.canonical_text) {
      return {
        ...block,
        canonical_text: newText,
        display_text: newText,
        children: childrenChanged ? patchedChildren : block.children,
      };
    }

    if (childrenChanged) {
      return { ...block, children: patchedChildren };
    }

    return block;
  });
}

/** Flatten a block tree into a map keyed by block ID. */
function flattenById(blocks: Block[]): Map<string, Block> {
  const map = new Map<string, Block>();
  const visit = (b: Block) => {
    map.set(b.id, b);
    b.children.forEach(visit);
  };
  blocks.forEach(visit);
  return map;
}

/** Collapse consecutive whitespace / newlines into single spaces. */
function normaliseWhitespace(text: string): string {
  return text.replace(/\s+/g, ' ').trim();
}
