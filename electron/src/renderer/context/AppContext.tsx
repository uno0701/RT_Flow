/**
 * AppContext — application-wide state management via React context.
 *
 * Manages the list of open documents, active compare/merge results,
 * workflow state, and conflict resolution state.
 */

import React, { createContext, useContext, useReducer, useCallback, useMemo } from 'react';
import type { Block, Document } from '../../shared/types/block';
import type {
  CompareResult,
  MergeResult,
  WorkflowState,
} from '../../shared/types/results';
import type { MergeConflict, ConflictResolution } from '../../shared/types/workflow';
import { RustBridge } from '../services/RustBridge';
import { DocumentService } from '../services/DocumentService';

// ---------------------------------------------------------------------------
// State shape
// ---------------------------------------------------------------------------

export interface OpenDocument {
  document: Document;
  blocks: Block[];
}

export interface AppState {
  openDocuments: OpenDocument[];
  activeDocumentId: string | null;
  compareResult: CompareResult | null;
  mergeResult: MergeResult | null;
  conflicts: MergeConflict[];
  workflowState: WorkflowState | null;
  /** IDs of documents currently shown in the three panes. */
  paneDocumentIds: (string | null)[];
  isLoading: boolean;
  error: string | null;
}

const initialState: AppState = {
  openDocuments: [],
  activeDocumentId: null,
  compareResult: null,
  mergeResult: null,
  conflicts: [],
  workflowState: null,
  paneDocumentIds: [null, null, null],
  isLoading: false,
  error: null,
};

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

type AppAction =
  | { type: 'SET_LOADING'; payload: boolean }
  | { type: 'SET_ERROR'; payload: string | null }
  | { type: 'OPEN_DOCUMENT'; payload: OpenDocument }
  | { type: 'UPDATE_BLOCKS'; payload: { docId: string; blocks: Block[] } }
  | { type: 'SET_ACTIVE_DOCUMENT'; payload: string }
  | { type: 'SET_COMPARE_RESULT'; payload: CompareResult }
  | { type: 'SET_MERGE_RESULT'; payload: MergeResult }
  | { type: 'SET_CONFLICTS'; payload: MergeConflict[] }
  | { type: 'RESOLVE_CONFLICT'; payload: { id: string; resolution: ConflictResolution } }
  | { type: 'SET_WORKFLOW'; payload: WorkflowState }
  | { type: 'SET_PANE_DOCUMENT'; payload: { paneIndex: number; docId: string | null } }
  | { type: 'CLEAR_COMPARE' };

// ---------------------------------------------------------------------------
// Reducer
// ---------------------------------------------------------------------------

function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case 'SET_LOADING':
      return { ...state, isLoading: action.payload };

    case 'SET_ERROR':
      return { ...state, error: action.payload };

    case 'OPEN_DOCUMENT': {
      const exists = state.openDocuments.some(
        (d) => d.document.id === action.payload.document.id
      );
      const openDocuments = exists
        ? state.openDocuments.map((d) =>
            d.document.id === action.payload.document.id ? action.payload : d
          )
        : [...state.openDocuments, action.payload];

      // Assign to the first null pane, or shift all left if all occupied
      const paneDocumentIds = [...state.paneDocumentIds];
      const firstNull = paneDocumentIds.indexOf(null);
      if (firstNull !== -1) {
        paneDocumentIds[firstNull] = action.payload.document.id;
      } else {
        paneDocumentIds[0] = paneDocumentIds[1];
        paneDocumentIds[1] = paneDocumentIds[2];
        paneDocumentIds[2] = action.payload.document.id;
      }

      return {
        ...state,
        openDocuments,
        activeDocumentId: action.payload.document.id,
        paneDocumentIds,
      };
    }

    case 'UPDATE_BLOCKS':
      return {
        ...state,
        openDocuments: state.openDocuments.map((d) =>
          d.document.id === action.payload.docId
            ? { ...d, blocks: action.payload.blocks }
            : d
        ),
      };

    case 'SET_ACTIVE_DOCUMENT':
      return { ...state, activeDocumentId: action.payload };

    case 'SET_COMPARE_RESULT':
      return { ...state, compareResult: action.payload };

    case 'SET_MERGE_RESULT':
      return { ...state, mergeResult: action.payload };

    case 'SET_CONFLICTS':
      return { ...state, conflicts: action.payload };

    case 'RESOLVE_CONFLICT':
      return {
        ...state,
        conflicts: state.conflicts.map((c) =>
          c.id === action.payload.id
            ? { ...c, resolution: action.payload.resolution }
            : c
        ),
      };

    case 'SET_WORKFLOW':
      return { ...state, workflowState: action.payload };

    case 'SET_PANE_DOCUMENT': {
      const paneDocumentIds = [...state.paneDocumentIds];
      paneDocumentIds[action.payload.paneIndex] = action.payload.docId;
      return { ...state, paneDocumentIds };
    }

    case 'CLEAR_COMPARE':
      return { ...state, compareResult: null, mergeResult: null, conflicts: [] };

    default:
      return state;
  }
}

// ---------------------------------------------------------------------------
// Context value
// ---------------------------------------------------------------------------

interface AppContextValue {
  state: AppState;
  dispatch: React.Dispatch<AppAction>;
  bridge: RustBridge;
  docService: DocumentService;
  /** Open a file dialog (via IPC) and ingest the selected document. */
  openDocumentDialog: () => Promise<void>;
  /** Compare the first two open documents. */
  compareOpenDocuments: () => Promise<void>;
  /** Merge the first two open documents. */
  mergeOpenDocuments: () => Promise<void>;
  /** Resolve a merge conflict. */
  resolveConflict: (id: string, resolution: ConflictResolution) => void;
  /** Submit a workflow event. */
  submitWorkflowEvent: (eventType: string) => Promise<void>;
}

// ---------------------------------------------------------------------------
// Context + Provider
// ---------------------------------------------------------------------------

const AppContext = createContext<AppContextValue | null>(null);

export const AppProvider: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  const [state, dispatch] = useReducer(appReducer, initialState);

  const bridge = useMemo(() => new RustBridge(), []);
  const docService = useMemo(() => new DocumentService(bridge), [bridge]);

  // -- Open document --------------------------------------------------------

  const openDocumentDialog = useCallback(async () => {
    dispatch({ type: 'SET_LOADING', payload: true });
    dispatch({ type: 'SET_ERROR', payload: null });
    try {
      // In production, call window.electronAPI.showOpenDialog from preload.
      // Fall back to a synthetic file path when running without Electron.
      let filePath: string | null = null;

      if (typeof window !== 'undefined' && (window as any).electronAPI?.showOpenDialog) {
        filePath = await (window as any).electronAPI.showOpenDialog();
      } else {
        // Development fallback
        filePath = '/mock/sample-agreement.docx';
      }

      if (!filePath) {
        dispatch({ type: 'SET_LOADING', payload: false });
        return;
      }

      const result = await docService.openDocument(filePath);
      dispatch({
        type: 'OPEN_DOCUMENT',
        payload: { document: result.document, blocks: result.blocks },
      });
    } catch (err) {
      dispatch({ type: 'SET_ERROR', payload: String(err) });
    } finally {
      dispatch({ type: 'SET_LOADING', payload: false });
    }
  }, [docService]);

  // -- Compare --------------------------------------------------------------

  const compareOpenDocuments = useCallback(async () => {
    const docs = state.openDocuments;
    if (docs.length < 2) {
      dispatch({ type: 'SET_ERROR', payload: 'Open at least two documents to compare.' });
      return;
    }
    dispatch({ type: 'SET_LOADING', payload: true });
    dispatch({ type: 'SET_ERROR', payload: null });
    try {
      const result = await bridge.compareDocuments(
        docs[0].document.id,
        docs[1].document.id
      );
      dispatch({ type: 'SET_COMPARE_RESULT', payload: result });
    } catch (err) {
      dispatch({ type: 'SET_ERROR', payload: String(err) });
    } finally {
      dispatch({ type: 'SET_LOADING', payload: false });
    }
  }, [bridge, state.openDocuments]);

  // -- Merge ----------------------------------------------------------------

  const mergeOpenDocuments = useCallback(async () => {
    const docs = state.openDocuments;
    if (docs.length < 2) {
      dispatch({ type: 'SET_ERROR', payload: 'Open at least two documents to merge.' });
      return;
    }
    dispatch({ type: 'SET_LOADING', payload: true });
    dispatch({ type: 'SET_ERROR', payload: null });
    try {
      const result = await bridge.mergeDocuments(
        docs[0].document.id,
        docs[1].document.id
      );
      dispatch({ type: 'SET_MERGE_RESULT', payload: result });

      // Build conflict list from merge results
      const conflicts: MergeConflict[] = result.block_results
        .filter((r) => r.outcome === 'conflict' && r.block !== null)
        .map((r, i): MergeConflict => ({
          id: `conflict-${i}`,
          block_id: r.block!.id,
          structural_path: r.block!.structural_path,
          description: r.conflict_detail ?? 'Conflicting changes.',
          base_text: r.block!.canonical_text,
          incoming_text: r.conflict_detail ?? '',
          resolution: null,
        }));

      dispatch({ type: 'SET_CONFLICTS', payload: conflicts });
    } catch (err) {
      dispatch({ type: 'SET_ERROR', payload: String(err) });
    } finally {
      dispatch({ type: 'SET_LOADING', payload: false });
    }
  }, [bridge, state.openDocuments]);

  // -- Conflict resolution --------------------------------------------------

  const resolveConflict = useCallback(
    (id: string, resolution: ConflictResolution) => {
      dispatch({ type: 'RESOLVE_CONFLICT', payload: { id, resolution } });
    },
    []
  );

  // -- Workflow -------------------------------------------------------------

  const MOCK_WORKFLOW_ID = 'wf-0000-aaaa-bbbb-cccc';

  const submitWorkflowEvent = useCallback(
    async (eventType: string) => {
      const workflowId = state.workflowState?.workflow_id ?? MOCK_WORKFLOW_ID;
      dispatch({ type: 'SET_LOADING', payload: true });
      try {
        const updated = await bridge.submitWorkflowEvent(workflowId, eventType, {});
        dispatch({ type: 'SET_WORKFLOW', payload: updated });
      } catch (err) {
        dispatch({ type: 'SET_ERROR', payload: String(err) });
      } finally {
        dispatch({ type: 'SET_LOADING', payload: false });
      }
    },
    [bridge, state.workflowState]
  );

  const value: AppContextValue = {
    state,
    dispatch,
    bridge,
    docService,
    openDocumentDialog,
    compareOpenDocuments,
    mergeOpenDocuments,
    resolveConflict,
    submitWorkflowEvent,
  };

  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
};

// ---------------------------------------------------------------------------
// Hook
// ---------------------------------------------------------------------------

export function useAppContext(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) {
    throw new Error('useAppContext must be used inside <AppProvider>');
  }
  return ctx;
}

export default AppContext;
