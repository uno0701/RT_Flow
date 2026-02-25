/**
 * Electron preload script.
 *
 * Runs in a privileged context with access to Node.js APIs, but is sandboxed
 * from the renderer.  Uses contextBridge to expose a minimal, typed API on
 * `window.electronAPI` and `window.rtflow`.
 *
 * The renderer accesses these objects directly; no require() calls are
 * permitted in the renderer process.
 */

import { contextBridge, ipcRenderer } from 'electron';

// ---------------------------------------------------------------------------
// electronAPI — dialog helpers
// ---------------------------------------------------------------------------

contextBridge.exposeInMainWorld('electronAPI', {
  /**
   * Show the native open-file dialog.
   * Returns the selected file path, or null if the dialog was cancelled.
   */
  showOpenDialog: (): Promise<string | null> =>
    ipcRenderer.invoke('dialog:open-file'),

  /**
   * Show the native save-file dialog.
   * Returns the chosen file path, or null if the dialog was cancelled.
   */
  showSaveDialog: (defaultName: string): Promise<string | null> =>
    ipcRenderer.invoke('dialog:save-file', defaultName),
});

// ---------------------------------------------------------------------------
// rtflow — Rust FFI bridge
// ---------------------------------------------------------------------------

contextBridge.exposeInMainWorld('rtflow', {
  /**
   * Ingest a document and return the block tree as a JSON string.
   * The JSON payload is `RtflowResult<Block[]>`.
   */
  ingestDocument: (filePath: string): Promise<string> =>
    ipcRenderer.invoke('rtflow:ingest', filePath),

  /**
   * Compare two documents identified by their doc IDs.
   * Returns `RtflowResult<CompareResult>` as JSON.
   */
  compareDocuments: (leftDocId: string, rightDocId: string): Promise<string> =>
    ipcRenderer.invoke('rtflow:compare', leftDocId, rightDocId),

  /**
   * Merge two documents.
   * Returns `RtflowResult<MergeResult>` as JSON.
   */
  mergeDocuments: (baseDocId: string, incomingDocId: string): Promise<string> =>
    ipcRenderer.invoke('rtflow:merge', baseDocId, incomingDocId),

  /**
   * Submit a workflow event.
   * Returns `RtflowResult<WorkflowState>` as JSON.
   */
  submitWorkflowEvent: (
    workflowId: string,
    kind: string,
    payloadJson: string
  ): Promise<string> =>
    ipcRenderer.invoke('rtflow:workflow-event', workflowId, kind, payloadJson),

  /**
   * Fetch the current workflow state.
   * Returns `RtflowResult<WorkflowState>` as JSON.
   */
  getWorkflowState: (workflowId: string): Promise<string> =>
    ipcRenderer.invoke('rtflow:workflow-state', workflowId),
});
