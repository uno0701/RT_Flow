/**
 * Local DOCX reader using mammoth.js.
 * Reads a real DOCX file and converts it to Block[] for the renderer.
 * This is the JS-side bridge until the native Rust ingestion is wired in.
 */

import * as fs from 'fs';
import * as path from 'path';
import * as crypto from 'crypto';

// mammoth is a CommonJS module
const mammoth = require('mammoth');

interface Block {
  id: string;
  document_id: string;
  parent_id: string | null;
  block_type: string;
  level: number;
  structural_path: string;
  anchor_signature: string;
  clause_hash: string;
  canonical_text: string;
  display_text: string;
  formatting_meta: {
    style_name: string | null;
    numbering_id: number | null;
    numbering_level: number | null;
    is_redline: boolean;
    tracked_change: null;
  };
  position_index: number;
  tokens: any[];
  runs: Array<{
    text: string;
    formatting: {
      bold: boolean;
      italic: boolean;
      underline: boolean;
      strikethrough: boolean;
      font_size: number | null;
      color: string | null;
    };
  }>;
  children: Block[];
}

interface DocResult {
  document: {
    id: string;
    name: string;
    source_path: string;
    doc_type: string;
    schema_version: string;
    normalization_version: string;
    hash_contract_version: string;
    ingested_at: string;
    metadata: null;
  };
  blocks: Block[];
}

function sha256(text: string): string {
  return crypto.createHash('sha256').update(text, 'utf8').digest('hex');
}

function uuid(): string {
  return crypto.randomUUID();
}

function makeBlock(
  docId: string,
  blockType: string,
  text: string,
  structuralPath: string,
  level: number,
  positionIndex: number,
  parentId: string | null = null,
  styleName: string | null = null,
  bold: boolean = false,
  italic: boolean = false,
  underline: boolean = false,
): Block {
  const canonical = text.replace(/\s+/g, ' ').trim();
  const id = uuid();
  const first128 = canonical.substring(0, 128);
  const anchorInput = `${blockType}|${structuralPath}|${first128}`;

  return {
    id,
    document_id: docId,
    parent_id: parentId,
    block_type: blockType,
    level,
    structural_path: structuralPath,
    anchor_signature: sha256(anchorInput),
    clause_hash: sha256(canonical),
    canonical_text: canonical,
    display_text: text,
    formatting_meta: {
      style_name: styleName,
      numbering_id: null,
      numbering_level: null,
      is_redline: false,
      tracked_change: null,
    },
    position_index: positionIndex,
    tokens: [],
    runs: [
      {
        text,
        formatting: {
          bold,
          italic,
          underline,
          strikethrough: false,
          font_size: null,
          color: null,
        },
      },
    ],
    children: [],
  };
}

/**
 * Parse a DOCX file into Block[] using mammoth for HTML extraction,
 * then convert the HTML structure into hierarchical blocks.
 */
export async function readDocx(filePath: string): Promise<DocResult> {
  const docId = uuid();
  const fileName = path.basename(filePath);
  const buffer = fs.readFileSync(filePath);

  // Use mammoth to extract structured HTML
  const result = await mammoth.convertToHtml(
    { buffer },
    {
      styleMap: [
        "p[style-name='Heading 1'] => h1:fresh",
        "p[style-name='Heading 2'] => h2:fresh",
        "p[style-name='Heading 3'] => h3:fresh",
        "p[style-name='Heading 4'] => h4:fresh",
        "p[style-name='Title'] => h1:fresh",
      ],
    }
  );

  const html: string = result.value;

  // Also extract raw text for better block construction
  const textResult = await mammoth.extractRawText({ buffer });
  const rawText: string = textResult.value;

  // Parse HTML into blocks
  const blocks = htmlToBlocks(html, docId, rawText);

  return {
    document: {
      id: docId,
      name: fileName,
      source_path: filePath,
      doc_type: 'original',
      schema_version: '1.0.0',
      normalization_version: '1.0.0',
      hash_contract_version: '1.0.0',
      ingested_at: new Date().toISOString(),
      metadata: null,
    },
    blocks,
  };
}

/**
 * Convert mammoth HTML output into hierarchical Block[].
 * Mammoth outputs: <h1>, <h2>, <h3>, <p>, <table>, <ol>, <ul>, etc.
 */
function htmlToBlocks(html: string, docId: string, _rawText: string): Block[] {
  const blocks: Block[] = [];

  // Simple regex-based HTML parser for mammoth output
  // Mammoth produces clean, predictable HTML
  const tagRegex = /<(h[1-6]|p|tr|td|th|li|table|ol|ul|blockquote)([^>]*)>([\s\S]*?)<\/\1>/gi;

  // First pass: split into top-level elements
  const elements: Array<{ tag: string; content: string; attrs: string }> = [];

  // More robust: split by top-level tags
  let remaining = html;
  const topLevelRegex = /<(h[1-6]|p|table|ol|ul|blockquote)([^>]*)>([\s\S]*?)<\/\1>/gi;
  let match;

  while ((match = topLevelRegex.exec(html)) !== null) {
    elements.push({
      tag: match[1].toLowerCase(),
      attrs: match[2],
      content: match[3],
    });
  }

  // If regex found nothing, fall back to splitting raw text by lines
  if (elements.length === 0) {
    const lines = stripHtml(html).split('\n').filter((l) => l.trim().length > 0);
    let sectionCounter = 0;
    let paraCounter = 0;

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) continue;

      // Heuristic: ALL CAPS or short lines might be headings
      const isHeading = trimmed === trimmed.toUpperCase() && trimmed.length < 100 && trimmed.length > 2;

      if (isHeading) {
        sectionCounter++;
        paraCounter = 0;
        blocks.push(
          makeBlock(docId, 'section', trimmed, `${sectionCounter}`, 0, sectionCounter - 1, null, 'Heading')
        );
      } else {
        paraCounter++;
        const parentId = blocks.length > 0 && blocks[blocks.length - 1].block_type === 'section'
          ? blocks[blocks.length - 1].id
          : null;
        const sp = parentId
          ? `${blocks.find((b) => b.id === parentId)?.structural_path}.${paraCounter}`
          : `${paraCounter}`;
        blocks.push(
          makeBlock(docId, 'paragraph', trimmed, sp, parentId ? 1 : 0, paraCounter - 1, parentId)
        );
      }
    }

    return buildHierarchy(blocks);
  }

  // Convert HTML elements to blocks
  let sectionCounter = 0;
  let clauseCounter = 0;
  let paraCounter = 0;
  let currentSectionId: string | null = null;
  let currentClauseId: string | null = null;

  for (const el of elements) {
    const text = stripHtml(el.content).trim();
    if (!text) continue;

    switch (el.tag) {
      case 'h1':
      case 'h2': {
        sectionCounter++;
        clauseCounter = 0;
        paraCounter = 0;
        const block = makeBlock(
          docId, 'section', text, `${sectionCounter}`, 0, sectionCounter - 1, null, `Heading ${el.tag[1]}`
        );
        currentSectionId = block.id;
        currentClauseId = null;
        blocks.push(block);
        break;
      }
      case 'h3':
      case 'h4':
      case 'h5':
      case 'h6': {
        clauseCounter++;
        paraCounter = 0;
        const sp = currentSectionId
          ? `${blocks.find((b) => b.id === currentSectionId)?.structural_path}.${clauseCounter}`
          : `${clauseCounter}`;
        const block = makeBlock(
          docId, 'clause', text, sp, currentSectionId ? 1 : 0, clauseCounter - 1, currentSectionId, `Heading ${el.tag[1]}`
        );
        currentClauseId = block.id;
        blocks.push(block);
        break;
      }
      case 'table': {
        paraCounter++;
        const tableBlock = parseTable(el.content, docId, currentSectionId, paraCounter);
        blocks.push(...tableBlock);
        break;
      }
      case 'ol':
      case 'ul': {
        const listItems = parseList(el.content, docId, currentSectionId ?? currentClauseId, clauseCounter, sectionCounter);
        blocks.push(...listItems);
        clauseCounter += listItems.length;
        break;
      }
      default: {
        // <p> and other elements
        paraCounter++;
        const parentId = currentClauseId ?? currentSectionId;
        const level = parentId ? (currentClauseId ? 2 : 1) : 0;
        let sp: string;
        if (currentClauseId) {
          const clauseBlock = blocks.find((b) => b.id === currentClauseId);
          sp = `${clauseBlock?.structural_path}.${paraCounter}`;
        } else if (currentSectionId) {
          const secBlock = blocks.find((b) => b.id === currentSectionId);
          sp = `${secBlock?.structural_path}.${paraCounter}`;
        } else {
          sp = `${paraCounter}`;
        }

        // Detect formatting from HTML
        const hasBold = el.content.includes('<strong>') || el.content.includes('<b>');
        const hasItalic = el.content.includes('<em>') || el.content.includes('<i>');
        const hasUnderline = el.content.includes('text-decoration: underline');

        const block = makeBlock(
          docId, 'paragraph', text, sp, level, paraCounter - 1, parentId, null, hasBold, hasItalic, hasUnderline
        );

        // Build runs from inline HTML formatting
        block.runs = parseRuns(el.content);

        blocks.push(block);
        break;
      }
    }
  }

  return buildHierarchy(blocks);
}

/** Strip HTML tags, decode entities */
function stripHtml(html: string): string {
  return html
    .replace(/<[^>]+>/g, '')
    .replace(/&amp;/g, '&')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#39;/g, "'")
    .replace(/&nbsp;/g, ' ');
}

/** Parse inline HTML into Run[] with formatting */
function parseRuns(html: string): Block['runs'] {
  const runs: Block['runs'] = [];

  // Simple approach: split on formatting tags
  const segments = html.split(/(<\/?(?:strong|b|em|i|u|s|del)>)/gi);
  let bold = false, italic = false, underline = false, strike = false;

  for (const seg of segments) {
    const lower = seg.toLowerCase();
    if (lower === '<strong>' || lower === '<b>') { bold = true; continue; }
    if (lower === '</strong>' || lower === '</b>') { bold = false; continue; }
    if (lower === '<em>' || lower === '<i>') { italic = true; continue; }
    if (lower === '</em>' || lower === '</i>') { italic = false; continue; }
    if (lower === '<u>') { underline = true; continue; }
    if (lower === '</u>') { underline = false; continue; }
    if (lower === '<s>' || lower === '<del>') { strike = true; continue; }
    if (lower === '</s>' || lower === '</del>') { strike = false; continue; }

    const text = stripHtml(seg).replace(/\s+/g, ' ');
    if (!text.trim()) continue;

    runs.push({
      text,
      formatting: {
        bold,
        italic,
        underline,
        strikethrough: strike,
        font_size: null,
        color: null,
      },
    });
  }

  // If no runs parsed, return a single plain run
  if (runs.length === 0) {
    const text = stripHtml(html).trim();
    if (text) {
      runs.push({
        text,
        formatting: { bold: false, italic: false, underline: false, strikethrough: false, font_size: null, color: null },
      });
    }
  }

  return runs;
}

/** Parse <table> HTML into table/row/cell blocks */
function parseTable(tableHtml: string, docId: string, parentId: string | null, posIndex: number): Block[] {
  const blocks: Block[] = [];
  const tableId = uuid();
  const tableBlock = makeBlock(docId, 'table', '[Table]', `table-${posIndex}`, parentId ? 1 : 0, posIndex, parentId);
  tableBlock.id = tableId;
  blocks.push(tableBlock);

  const rowRegex = /<tr[^>]*>([\s\S]*?)<\/tr>/gi;
  let rowMatch;
  let rowIdx = 0;

  while ((rowMatch = rowRegex.exec(tableHtml)) !== null) {
    const rowId = uuid();
    const rowBlock = makeBlock(docId, 'table_row', '', `table-${posIndex}.r${rowIdx}`, (parentId ? 1 : 0) + 1, rowIdx, tableId);
    rowBlock.id = rowId;
    blocks.push(rowBlock);

    const cellRegex = /<t[dh][^>]*>([\s\S]*?)<\/t[dh]>/gi;
    let cellMatch;
    let cellIdx = 0;

    while ((cellMatch = cellRegex.exec(rowMatch[1])) !== null) {
      const cellText = stripHtml(cellMatch[1]).trim();
      const cellBlock = makeBlock(
        docId, 'table_cell', cellText, `table-${posIndex}.r${rowIdx}.c${cellIdx}`,
        (parentId ? 1 : 0) + 2, cellIdx, rowId
      );
      blocks.push(cellBlock);
      cellIdx++;
    }
    rowIdx++;
  }

  return blocks;
}

/** Parse <ol>/<ul> into clause/subclause blocks */
function parseList(listHtml: string, docId: string, parentId: string | null, clauseStart: number, sectionNum: number): Block[] {
  const blocks: Block[] = [];
  const liRegex = /<li[^>]*>([\s\S]*?)<\/li>/gi;
  let match;
  let idx = 0;

  while ((match = liRegex.exec(listHtml)) !== null) {
    const text = stripHtml(match[1]).trim();
    if (!text) continue;

    const sp = parentId
      ? `${sectionNum}.${clauseStart + idx + 1}`
      : `${clauseStart + idx + 1}`;

    blocks.push(
      makeBlock(docId, 'clause', text, sp, parentId ? 1 : 0, clauseStart + idx, parentId)
    );
    idx++;
  }

  return blocks;
}

/** Build parent-child hierarchy from flat blocks based on parent_id */
function buildHierarchy(flatBlocks: Block[]): Block[] {
  const map = new Map<string, Block>();
  for (const b of flatBlocks) {
    b.children = [];
    map.set(b.id, b);
  }

  const roots: Block[] = [];
  for (const b of flatBlocks) {
    if (b.parent_id && map.has(b.parent_id)) {
      map.get(b.parent_id)!.children.push(b);
    } else {
      roots.push(b);
    }
  }

  return roots;
}
