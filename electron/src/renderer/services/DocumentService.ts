/**
 * DocumentService — high-level document operations for the renderer process.
 *
 * Wraps RustBridge calls and manages an in-memory document registry so that
 * opened documents can be referenced by ID without re-ingesting them.
 */

import type { Block, Document } from '../../shared/types/block';
import { RustBridge } from './RustBridge';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface OpenDocumentResult {
  document: Document;
  blocks: Block[];
}

// ---------------------------------------------------------------------------
// DocumentService
// ---------------------------------------------------------------------------

export class DocumentService {
  private readonly bridge: RustBridge;
  /** In-memory cache: docId → { document, blocks } */
  private readonly registry = new Map<string, OpenDocumentResult>();

  constructor(bridge?: RustBridge) {
    this.bridge = bridge ?? new RustBridge();
  }

  // -------------------------------------------------------------------------
  // openDocument
  // -------------------------------------------------------------------------

  /**
   * Open a document from a filesystem path.
   *
   * If the document has already been opened (same path mapped to the same
   * derived ID) the cached result is returned immediately.
   */
  async openDocument(filePath: string): Promise<OpenDocumentResult> {
    // Derive a deterministic doc ID from the path (stable across re-opens)
    const derivedId = pathToDocId(filePath);
    const cached = this.registry.get(derivedId);
    if (cached) return cached;

    const blocks = await this.bridge.ingestDocument(filePath);
    const docId = blocks[0]?.document_id ?? derivedId;

    const document: Document = {
      id: docId,
      name: fileNameFromPath(filePath),
      source_path: filePath,
      doc_type: 'original',
      schema_version: '1.0.0',
      normalization_version: '1.0.0',
      hash_contract_version: '1.0.0',
      ingested_at: new Date().toISOString(),
      metadata: null,
    };

    const result: OpenDocumentResult = { document, blocks };
    this.registry.set(docId, result);
    return result;
  }

  // -------------------------------------------------------------------------
  // exportDocument
  // -------------------------------------------------------------------------

  /**
   * Export a block tree to the given output path.
   *
   * Stub implementation — in production this would serialise the blocks to
   * DOCX via the Rust FFI layer.
   */
  async exportDocument(blocks: Block[], outputPath: string): Promise<void> {
    // TODO: call bridge.exportDocument once the native method is implemented
    console.info(`[DocumentService] Export to ${outputPath} (${blocks.length} root blocks) — stub`);
    return Promise.resolve();
  }

  // -------------------------------------------------------------------------
  // getCached
  // -------------------------------------------------------------------------

  getCached(docId: string): OpenDocumentResult | undefined {
    return this.registry.get(docId);
  }

  getAll(): OpenDocumentResult[] {
    return Array.from(this.registry.values());
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function fileNameFromPath(filePath: string): string {
  const parts = filePath.replace(/\\/g, '/').split('/');
  const name = parts[parts.length - 1] ?? filePath;
  return name.replace(/\.[^.]+$/, ''); // strip extension
}

/**
 * Derive a short stable ID from a file path.
 * Uses a simple djb2-style hash — not cryptographically strong, but
 * deterministic and collision-resistant enough for an in-process registry.
 */
function pathToDocId(filePath: string): string {
  let hash = 5381;
  for (let i = 0; i < filePath.length; i++) {
    hash = ((hash << 5) + hash) ^ filePath.charCodeAt(i);
    hash = hash >>> 0; // keep unsigned 32-bit
  }
  return `doc-${hash.toString(16).padStart(8, '0')}`;
}
