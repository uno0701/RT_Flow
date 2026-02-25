/**
 * RustBridge — bridge to the native Rust FFI layer via Electron IPC.
 *
 * In production this class will call the native N-API module exposed on
 * `window.rtflow` (injected by the preload script).  For now it returns
 * realistic mock data so the UI can be developed and tested without the
 * native binary being present.
 */

import type { Block, Document } from '../../shared/types/block';
import type {
  CompareResult,
  MergeResult,
  WorkflowState,
  WorkflowEvent,
  RtflowResult,
} from '../../shared/types/results';

// ---------------------------------------------------------------------------
// Electron preload contract
// ---------------------------------------------------------------------------

/** Shape of the object that the preload script injects as `window.rtflow`. */
interface RtflowNative {
  ingestDocument: (filePath: string) => Promise<string>; // returns JSON RtflowResult<Block[]>
  compareDocuments: (leftId: string, rightId: string) => Promise<string>;
  mergeDocuments: (baseId: string, incomingId: string) => Promise<string>;
  submitWorkflowEvent: (workflowId: string, kind: string, payload: string) => Promise<string>;
  getWorkflowState: (workflowId: string) => Promise<string>;
}

declare global {
  interface Window {
    rtflow?: RtflowNative;
  }
}

// ---------------------------------------------------------------------------
// RustBridge class
// ---------------------------------------------------------------------------

export class RustBridge {
  private nativeAvailable: boolean | null = null; // null = unknown, try once

  constructor() {
    if (typeof window === 'undefined' || !window.rtflow) {
      this.nativeAvailable = false;
      console.info('[RustBridge] No preload bridge — using mock data.');
    }
  }

  /**
   * Try the native call; on failure fall back to mock.
   * Once a native call fails, all subsequent calls use mock directly.
   */
  private async tryNative<T>(
    nativeFn: () => Promise<string>,
    mockFn: () => T | Promise<T>
  ): Promise<T> {
    if (this.nativeAvailable === false) {
      return mockFn();
    }

    try {
      const raw = await nativeFn();
      const parsed = JSON.parse(raw);
      if (parsed.ok && parsed.data !== undefined) {
        this.nativeAvailable = true;
        return parsed.data as T;
      }
      // Native returned an error envelope — fall back to mock
      console.info('[RustBridge] Native call returned error, falling back to mock:', parsed.error);
      this.nativeAvailable = false;
      return mockFn();
    } catch (err) {
      console.info('[RustBridge] Native call failed, falling back to mock:', err);
      this.nativeAvailable = false;
      return mockFn();
    }
  }

  // -------------------------------------------------------------------------
  // Ingest
  // -------------------------------------------------------------------------

  async ingestDocument(filePath: string): Promise<Block[]> {
    return this.tryNative(
      () => window.rtflow!.ingestDocument(filePath),
      () => mockBlocks(filePath)
    );
  }

  // -------------------------------------------------------------------------
  // Compare
  // -------------------------------------------------------------------------

  async compareDocuments(
    leftDocId: string,
    rightDocId: string
  ): Promise<CompareResult> {
    return this.tryNative(
      () => window.rtflow!.compareDocuments(leftDocId, rightDocId),
      () => mockCompareResult(leftDocId, rightDocId)
    );
  }

  // -------------------------------------------------------------------------
  // Merge
  // -------------------------------------------------------------------------

  async mergeDocuments(
    baseDocId: string,
    incomingDocId: string
  ): Promise<MergeResult> {
    return this.tryNative(
      () => window.rtflow!.mergeDocuments(baseDocId, incomingDocId),
      () => mockMergeResult(baseDocId, incomingDocId)
    );
  }

  // -------------------------------------------------------------------------
  // Workflow
  // -------------------------------------------------------------------------

  async submitWorkflowEvent(
    workflowId: string,
    eventType: string,
    payload: Record<string, unknown>
  ): Promise<WorkflowState> {
    return this.tryNative(
      () => window.rtflow!.submitWorkflowEvent(workflowId, eventType, JSON.stringify(payload)),
      () => mockWorkflowTransition(workflowId, eventType)
    );
  }

  async getWorkflowState(workflowId: string): Promise<WorkflowState> {
    return this.tryNative(
      () => window.rtflow!.getWorkflowState(workflowId),
      () => mockWorkflowState(workflowId)
    );
  }
}

// ---------------------------------------------------------------------------
// Helper: unwrap RtflowResult<T>
// ---------------------------------------------------------------------------

function unwrap<T>(result: RtflowResult<T>): T {
  if (!result.ok || result.data === undefined) {
    throw new Error(result.error ?? 'Unknown error from native module');
  }
  return result.data;
}

// ---------------------------------------------------------------------------
// Mock data generators
// ---------------------------------------------------------------------------

function uuid(): string {
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (c) => {
    const r = (Math.random() * 16) | 0;
    return (c === 'x' ? r : (r & 0x3) | 0x8).toString(16);
  });
}

const DOC_ID_A = 'doc-aaaa-1111-2222-333344445555';
const DOC_ID_B = 'doc-bbbb-6666-7777-888899990000';

function makeRun(text: string): import('../../shared/types/block').Run {
  return {
    text,
    formatting: {
      bold: false,
      italic: false,
      underline: false,
      strikethrough: false,
      font_size: null,
      color: null,
    },
  };
}

function makeBlock(
  opts: Partial<Block> & { block_type: Block['block_type']; canonical_text: string }
): Block {
  const id = opts.id ?? uuid();
  return {
    id,
    document_id: opts.document_id ?? DOC_ID_A,
    parent_id: opts.parent_id ?? null,
    block_type: opts.block_type,
    level: opts.level ?? 0,
    structural_path: opts.structural_path ?? '',
    anchor_signature: opts.anchor_signature ?? id,
    clause_hash: opts.clause_hash ?? id,
    canonical_text: opts.canonical_text,
    display_text: opts.display_text ?? opts.canonical_text,
    formatting_meta: opts.formatting_meta ?? {
      style_name: null,
      numbering_id: null,
      numbering_level: null,
      is_redline: false,
      tracked_change: null,
    },
    position_index: opts.position_index ?? 0,
    tokens: opts.tokens ?? [],
    runs: opts.runs ?? [makeRun(opts.canonical_text)],
    children: opts.children ?? [],
  };
}

function mockBlocks(filePath: string): Block[] {
  const docName = filePath.split('/').pop() ?? 'document.docx';
  const sectionId = uuid();
  const clause1Id = uuid();
  const clause2Id = uuid();
  const sub1Id = uuid();

  const section = makeBlock({
    id: sectionId,
    block_type: 'section',
    structural_path: '1',
    canonical_text: `AGREEMENT — ${docName}`,
    level: 0,
    children: [
      makeBlock({
        id: clause1Id,
        document_id: DOC_ID_A,
        parent_id: sectionId,
        block_type: 'clause',
        structural_path: '1.1',
        canonical_text: 'Definitions. In this Agreement, the following terms shall have the meanings set out below.',
        level: 1,
        position_index: 0,
        children: [
          makeBlock({
            id: sub1Id,
            document_id: DOC_ID_A,
            parent_id: clause1Id,
            block_type: 'subclause',
            structural_path: '1.1(a)',
            canonical_text: '"Affiliate" means any entity that directly or indirectly controls, is controlled by, or is under common control with, such party.',
            level: 2,
            position_index: 0,
          }),
        ],
      }),
      makeBlock({
        id: clause2Id,
        document_id: DOC_ID_A,
        parent_id: sectionId,
        block_type: 'clause',
        structural_path: '1.2',
        canonical_text: 'Term. This Agreement shall commence on the Effective Date and shall continue for a period of twelve (12) months thereafter, unless earlier terminated in accordance with the provisions hereof.',
        level: 1,
        position_index: 1,
      }),
    ],
  });

  const intro = makeBlock({
    block_type: 'paragraph',
    structural_path: '',
    canonical_text: 'THIS AGREEMENT is entered into as of the date last signed below between the parties identified herein.',
    level: 0,
  });

  return [intro, section];
}

function mockCompareResult(leftDocId: string, rightDocId: string): CompareResult {
  const anchorA = uuid();
  const anchorB = uuid();
  const anchorC = uuid();

  const baseBlock = makeBlock({
    id: uuid(),
    document_id: leftDocId,
    block_type: 'clause',
    structural_path: '1.2',
    canonical_text: 'Term. This Agreement shall continue for twelve (12) months.',
    anchor_signature: anchorA,
  });

  const modifiedBlock = makeBlock({
    id: uuid(),
    document_id: rightDocId,
    block_type: 'clause',
    structural_path: '1.2',
    canonical_text: 'Term. This Agreement shall continue for twenty-four (24) months.',
    anchor_signature: anchorA,
  });

  const insertedBlock = makeBlock({
    id: uuid(),
    document_id: rightDocId,
    block_type: 'clause',
    structural_path: '1.3',
    canonical_text: 'Renewal. This Agreement shall automatically renew unless terminated.',
    anchor_signature: anchorB,
  });

  const deletedBlock = makeBlock({
    id: uuid(),
    document_id: leftDocId,
    block_type: 'clause',
    structural_path: '1.4',
    canonical_text: 'No-Shop. During the term, neither party shall negotiate with third parties.',
    anchor_signature: anchorC,
  });

  return {
    left_doc_id: leftDocId,
    right_doc_id: rightDocId,
    diffs: [
      {
        kind: 'modified',
        left_block: baseBlock,
        right_block: modifiedBlock,
        anchor_signature: anchorA,
      },
      {
        kind: 'inserted',
        left_block: null,
        right_block: insertedBlock,
        anchor_signature: anchorB,
      },
      {
        kind: 'deleted',
        left_block: deletedBlock,
        right_block: null,
        anchor_signature: anchorC,
      },
    ],
    changed_count: 3,
    equal_count: 5,
  };
}

function mockMergeResult(baseDocId: string, incomingDocId: string): MergeResult {
  const mergedId = uuid();
  const conflictBlock = makeBlock({
    id: uuid(),
    document_id: mergedId,
    block_type: 'clause',
    structural_path: '1.2',
    canonical_text: 'Term. This Agreement shall continue for twelve (12) months.',
  });

  return {
    base_doc_id: baseDocId,
    incoming_doc_id: incomingDocId,
    merged_doc_id: mergedId,
    block_results: [
      {
        outcome: 'conflict',
        block: conflictBlock,
        conflict_detail: 'Both versions modified clause 1.2 with incompatible changes.',
      },
      {
        outcome: 'auto_merged',
        block: makeBlock({
          id: uuid(),
          document_id: mergedId,
          block_type: 'paragraph',
          structural_path: '',
          canonical_text: 'Merged paragraph text.',
        }),
        conflict_detail: null,
      },
    ],
    auto_merged_count: 1,
    conflict_count: 1,
  };
}

const _workflowStates = new Map<string, WorkflowState>();

function mockWorkflowState(workflowId: string): WorkflowState {
  const existing = _workflowStates.get(workflowId);
  if (existing) return existing;

  const now = new Date().toISOString();
  const state: WorkflowState = {
    workflow_id: workflowId,
    doc_id: DOC_ID_A,
    status: 'pending',
    initiator_id: 'user-0000-1111',
    reviewer_ids: ['user-aaaa-bbbb', 'user-cccc-dddd'],
    approver_ids: [],
    event_history: [
      {
        kind: 'created',
        actor_id: 'user-0000-1111',
        timestamp: now,
        payload: {},
      },
    ],
    created_at: now,
    updated_at: now,
  };
  _workflowStates.set(workflowId, state);
  return state;
}

function mockWorkflowTransition(workflowId: string, eventType: string): WorkflowState {
  const state = { ...mockWorkflowState(workflowId) };
  const now = new Date().toISOString();

  const newEvent: WorkflowEvent = {
    kind: eventType,
    actor_id: 'user-0000-1111',
    timestamp: now,
    payload: {},
  };

  // Advance status based on the event
  const nextStatus = nextStatusForEvent(state.status, eventType);
  const updated: WorkflowState = {
    ...state,
    status: nextStatus,
    updated_at: now,
    event_history: [...state.event_history, newEvent],
    approver_ids:
      eventType === 'approve'
        ? [...state.approver_ids, 'user-0000-1111']
        : state.approver_ids,
  };

  _workflowStates.set(workflowId, updated);
  return updated;
}

function nextStatusForEvent(
  current: WorkflowState['status'],
  event: string
): WorkflowState['status'] {
  const map: Record<string, WorkflowState['status']> = {
    submit_for_review: 'in_review',
    approve: 'approved',
    request_changes: 'changes_requested',
    reject: 'rejected',
    merge: 'merged',
    archive: 'archived',
  };
  return map[event] ?? current;
}
