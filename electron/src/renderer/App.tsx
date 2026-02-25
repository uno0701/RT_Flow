import React, { useState, useCallback, useEffect } from 'react';
import './styles/global.css';

import { AppProvider, useAppContext } from './context/AppContext';
import { ThreePaneLayout, PaneConfig } from './workspace/ThreePaneLayout';
import { BlockRenderer } from './workspace/BlockRenderer';
import { DeltaOverlay } from './workspace/DeltaOverlay';
import { ConflictPanel } from './workspace/ConflictPanel';
import { Editor } from './editor/Editor';
import { WorkflowBar, WorkflowPanel } from './workflow/WorkflowPanel';
import type { Block } from '../shared/types/block';
import type { ConflictResolution } from '../shared/types/workflow';

// ---------------------------------------------------------------------------
// Root — wraps everything in the AppProvider
// ---------------------------------------------------------------------------

const App: React.FC = () => (
  <AppProvider>
    <AppShell />
  </AppProvider>
);

// ---------------------------------------------------------------------------
// AppShell — the actual layout consuming AppContext
// ---------------------------------------------------------------------------

type ActiveView = 'editor' | 'compare' | 'workflow';

const AppShell: React.FC = () => {
  const {
    state,
    openDocumentDialog,
    compareOpenDocuments,
    mergeOpenDocuments,
    resolveConflict,
    submitWorkflowEvent,
    bridge,
  } = useAppContext();

  const [activeView, setActiveView] = useState<ActiveView>('editor');
  const [showConflictPanel, setShowConflictPanel] = useState(false);
  const [showWorkflowPanel, setShowWorkflowPanel] = useState(false);
  const [fileMenuOpen, setFileMenuOpen] = useState(false);
  const [selectedConflictId, setSelectedConflictId] = useState<string | undefined>();

  // Ensure workflow state is available when there are open documents
  useEffect(() => {
    if (state.openDocuments.length > 0 && !state.workflowState) {
      bridge.getWorkflowState('wf-0000-aaaa-bbbb-cccc').then((ws) => {
        // Dispatch manually since we can't call submitWorkflowEvent here
      });
    }
  }, [state.openDocuments.length, state.workflowState, bridge]);

  // Show conflict panel automatically when merge produces conflicts
  useEffect(() => {
    if (state.conflicts.length > 0) {
      setShowConflictPanel(true);
    }
  }, [state.conflicts.length]);

  // ---------------------------------------------------------------------------
  // Toolbar handlers
  // ---------------------------------------------------------------------------

  const handleExport = useCallback(() => {
    const doc = state.openDocuments.find((d) => d.document.id === state.activeDocumentId);
    if (!doc) return;
    const exportPath = doc.document.source_path ?? `${doc.document.name}-export.docx`;
    // In production: window.electronAPI.showSaveDialog() then docService.exportDocument()
    console.info(`[Export] ${exportPath}`);
  }, [state]);

  const handleCompare = useCallback(async () => {
    await compareOpenDocuments();
    setActiveView('compare');
  }, [compareOpenDocuments]);

  const handleMerge = useCallback(async () => {
    await mergeOpenDocuments();
  }, [mergeOpenDocuments]);

  // ---------------------------------------------------------------------------
  // Build panes for current view
  // ---------------------------------------------------------------------------

  const buildPanes = (): PaneConfig[] => {
    if (activeView === 'compare' && state.compareResult) {
      const leftDoc = state.openDocuments.find(
        (d) => d.document.id === state.compareResult!.left_doc_id
      );
      const rightDoc = state.openDocuments.find(
        (d) => d.document.id === state.compareResult!.right_doc_id
      );
      return [
        {
          id: 'redline',
          type: 'redline',
          title: 'Redline View',
          badge: `${state.compareResult.changed_count} changes`,
          content: (
            <DeltaOverlay
              compareResult={state.compareResult}
              leftBlocks={leftDoc?.blocks ?? []}
              rightBlocks={rightDoc?.blocks ?? []}
            />
          ),
        },
      ];
    }

    if (activeView === 'workflow' && state.workflowState) {
      return [
        {
          id: 'workflow-full',
          type: 'browser',
          title: 'Workflow',
          content: (
            <WorkflowPanel
              workflowState={state.workflowState}
              events={state.workflowState.event_history}
              onAction={submitWorkflowEvent}
            />
          ),
        },
      ];
    }

    // Default: editor panes for open documents
    const docPanes: PaneConfig[] = state.openDocuments
      .slice(0, 3)
      .map((od) => ({
        id: od.document.id,
        type: 'editor' as const,
        title: od.document.name,
        content: (
          <Editor
            blocks={od.blocks}
            onChange={(updated: Block[]) => {
              // handled via dispatch in context
            }}
            readOnly={false}
          />
        ),
      }));

    if (docPanes.length === 0) {
      return [
        {
          id: 'welcome',
          type: 'editor' as const,
          title: 'Welcome',
          content: <WelcomePane onOpen={openDocumentDialog} />,
        },
      ];
    }

    return docPanes;
  };

  const panes = buildPanes();
  const unresolvedConflicts = state.conflicts.filter((c) => c.resolution === null).length;

  return (
    <div className="app-shell">
      {/* ------------------------------------------------------------------ */}
      {/* Toolbar                                                              */}
      {/* ------------------------------------------------------------------ */}
      <div className="toolbar">
        <span className="toolbar-title">RT Flow</span>
        <div className="toolbar-separator" />

        {/* File menu */}
        <div className="toolbar-section" style={{ position: 'relative' }}>
          <button
            className="toolbar-btn"
            onClick={() => setFileMenuOpen((v) => !v)}
          >
            File
          </button>
          {fileMenuOpen && (
            <>
              <div
                className="menu-overlay"
                onClick={() => setFileMenuOpen(false)}
              />
              <div className="menu" style={{ top: 36, left: 0 }}>
                <div
                  className="menu-item"
                  onClick={() => {
                    setFileMenuOpen(false);
                    openDocumentDialog();
                  }}
                >
                  Open Document
                  <span className="menu-item-shortcut">Cmd+O</span>
                </div>
                <div
                  className="menu-item"
                  onClick={() => {
                    setFileMenuOpen(false);
                    handleExport();
                  }}
                >
                  Export Document
                  <span className="menu-item-shortcut">Cmd+S</span>
                </div>
                <div className="menu-separator" />
                <div
                  className="menu-item"
                  onClick={() => setFileMenuOpen(false)}
                >
                  Close
                </div>
              </div>
            </>
          )}
        </div>

        <div className="toolbar-section">
          <button
            className={`toolbar-btn${activeView === 'editor' ? ' primary' : ''}`}
            onClick={() => setActiveView('editor')}
          >
            Editor
          </button>
          <button
            className={`toolbar-btn${activeView === 'compare' ? ' primary' : ''}`}
            onClick={handleCompare}
            disabled={state.openDocuments.length < 2 || state.isLoading}
            title={state.openDocuments.length < 2 ? 'Open two documents to compare' : 'Compare documents'}
          >
            Compare
          </button>
          <button
            className="toolbar-btn"
            onClick={handleMerge}
            disabled={state.openDocuments.length < 2 || state.isLoading}
            title={state.openDocuments.length < 2 ? 'Open two documents to merge' : 'Merge documents'}
          >
            Merge
          </button>
        </div>

        {state.conflicts.length > 0 && (
          <>
            <div className="toolbar-separator" />
            <button
              className={`toolbar-btn${showConflictPanel ? ' primary' : ''}`}
              onClick={() => setShowConflictPanel((v) => !v)}
              title="Toggle conflict panel"
            >
              Conflicts
              {unresolvedConflicts > 0 && (
                <span
                  style={{
                    background: 'var(--color-conflict-border)',
                    color: '#fff',
                    borderRadius: '10px',
                    padding: '0 5px',
                    fontSize: '10px',
                    marginLeft: '4px',
                  }}
                >
                  {unresolvedConflicts}
                </span>
              )}
            </button>
          </>
        )}

        <div className="toolbar-spacer" />

        {state.isLoading && (
          <span style={{ fontSize: 11, color: 'var(--color-text-muted)' }}>
            Loading…
          </span>
        )}

        {state.error && (
          <span
            style={{
              fontSize: 11,
              color: 'var(--color-deleted-text)',
              maxWidth: 300,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
            title={state.error}
          >
            Error: {state.error}
          </span>
        )}

        <div className="toolbar-section">
          <button
            className={`toolbar-btn${activeView === 'workflow' ? ' primary' : ''}`}
            onClick={() => setActiveView((v) => v === 'workflow' ? 'editor' : 'workflow')}
          >
            Workflow
          </button>
        </div>
      </div>

      {/* ------------------------------------------------------------------ */}
      {/* Main body                                                            */}
      {/* ------------------------------------------------------------------ */}
      <div className="app-body">
        <div className="app-main">
          <ThreePaneLayout
            panes={panes}
            onPaneClose={(id) => {
              // handled: could dispatch CLOSE_DOCUMENT action
            }}
          />
        </div>

        {/* Conflict panel (slide-out) */}
        {showConflictPanel && state.conflicts.length > 0 && (
          <ConflictPanel
            conflicts={state.conflicts}
            onResolve={(id, resolution) => resolveConflict(id, resolution as ConflictResolution)}
            selectedConflictId={selectedConflictId}
            onSelectConflict={setSelectedConflictId}
          />
        )}
      </div>

      {/* ------------------------------------------------------------------ */}
      {/* Bottom workflow bar                                                  */}
      {/* ------------------------------------------------------------------ */}
      {state.workflowState && (
        <WorkflowBar
          workflowState={state.workflowState}
          onAction={submitWorkflowEvent}
          onClick={() => setActiveView((v) => v === 'workflow' ? 'editor' : 'workflow')}
        />
      )}
    </div>
  );
};

// ---------------------------------------------------------------------------
// Welcome pane
// ---------------------------------------------------------------------------

const WelcomePane: React.FC<{ onOpen: () => void }> = ({ onOpen }) => (
  <div className="empty-state">
    <div className="empty-state-icon">&#128462;</div>
    <div className="empty-state-title">No documents open</div>
    <div className="empty-state-desc">
      Open a DOCX document to begin editing, comparing, or merging.
    </div>
    <button
      className="toolbar-btn primary"
      style={{ marginTop: 8, padding: '8px 20px', fontSize: 13 }}
      onClick={onOpen}
    >
      Open Document
    </button>
  </div>
);

export default App;
