import { app, BrowserWindow, ipcMain, dialog } from 'electron';
import * as path from 'path';
import { readDocx } from './docx-reader';

// ---------------------------------------------------------------------------
// Window creation
// ---------------------------------------------------------------------------

function createWindow(): void {
  const mainWindow = new BrowserWindow({
    width: 1440,
    height: 900,
    minWidth: 900,
    minHeight: 600,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js'),
      sandbox: false,
    },
    title: 'RT Flow',
    backgroundColor: '#1a1b1e',
    // On macOS use a native titlebar with transparent traffic lights
    titleBarStyle: process.platform === 'darwin' ? 'hiddenInset' : 'default',
  });

  // Load index.html from the source renderer directory.
  // __dirname resolves to dist/main/ after build, so we go up two levels to reach src/.
  const rendererPath = path.join(__dirname, '..', '..', 'src', 'renderer', 'index.html');
  mainWindow.loadFile(rendererPath);

  // Open DevTools in development mode.
  if (process.env.NODE_ENV === 'development') {
    mainWindow.webContents.openDevTools();
  }
}

// ---------------------------------------------------------------------------
// IPC handlers
// ---------------------------------------------------------------------------

/** Show a native open-file dialog and return the selected path (or null). */
ipcMain.handle('dialog:open-file', async (): Promise<string | null> => {
  const result = await dialog.showOpenDialog({
    title: 'Open Document',
    filters: [
      { name: 'Word Documents', extensions: ['docx', 'doc'] },
      { name: 'All Files', extensions: ['*'] },
    ],
    properties: ['openFile'],
  });

  if (result.canceled || result.filePaths.length === 0) {
    return null;
  }
  return result.filePaths[0];
});

/** Show a native save-file dialog and return the chosen path (or null). */
ipcMain.handle('dialog:save-file', async (
  _event,
  defaultName: string
): Promise<string | null> => {
  const result = await dialog.showSaveDialog({
    title: 'Export Document',
    defaultPath: defaultName,
    filters: [
      { name: 'Word Document', extensions: ['docx'] },
      { name: 'All Files', extensions: ['*'] },
    ],
  });

  if (result.canceled || !result.filePath) {
    return null;
  }
  return result.filePath;
});

// ---------------------------------------------------------------------------
// Native Rust module bridge (stubbed for Phase 6)
//
// In production these handlers will load the compiled N-API addon from
// `./native/rt_ffi.node` and forward calls to it.  For now they return
// an error envelope so the renderer falls back to its mock implementations.
// ---------------------------------------------------------------------------

/** Ingest a document — uses mammoth.js DOCX reader (JS bridge). */
ipcMain.handle('rtflow:ingest', async (
  _event,
  filePath: string
): Promise<string> => {
  try {
    const result = await readDocx(filePath);
    return JSON.stringify({ ok: true, data: result.blocks });
  } catch (err) {
    console.error('[rtflow:ingest] Error reading DOCX:', err);
    return JSON.stringify({ ok: false, error: String(err) });
  }
});

/** Stub: compare two documents via the native rt-ffi module. */
ipcMain.handle('rtflow:compare', async (
  _event,
  leftDocId: string,
  rightDocId: string
): Promise<string> => {
  // TODO: load native addon and call rtflow_compare(leftDocId, rightDocId)
  return JSON.stringify({ ok: false, error: 'Native module not loaded (stub)' });
});

/** Stub: merge two documents via the native rt-ffi module. */
ipcMain.handle('rtflow:merge', async (
  _event,
  baseDocId: string,
  incomingDocId: string
): Promise<string> => {
  // TODO: load native addon and call rtflow_merge(baseDocId, incomingDocId)
  return JSON.stringify({ ok: false, error: 'Native module not loaded (stub)' });
});

/** Stub: submit a workflow event via the native rt-ffi module. */
ipcMain.handle('rtflow:workflow-event', async (
  _event,
  workflowId: string,
  eventKind: string,
  payloadJson: string
): Promise<string> => {
  // TODO: load native addon and call rtflow_workflow_event(...)
  return JSON.stringify({ ok: false, error: 'Native module not loaded (stub)' });
});

/** Stub: get workflow state via the native rt-ffi module. */
ipcMain.handle('rtflow:workflow-state', async (
  _event,
  workflowId: string
): Promise<string> => {
  // TODO: load native addon and call rtflow_workflow_state(workflowId)
  return JSON.stringify({ ok: false, error: 'Native module not loaded (stub)' });
});

// ---------------------------------------------------------------------------
// App lifecycle
// ---------------------------------------------------------------------------

app.whenReady().then(() => {
  createWindow();

  app.on('activate', () => {
    // On macOS re-create a window when the dock icon is clicked and no other
    // windows are open.
    if (BrowserWindow.getAllWindows().length === 0) {
      createWindow();
    }
  });
});

app.on('window-all-closed', () => {
  // On macOS applications stay active until the user quits explicitly.
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
