import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'node:path';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { PeerManager } from './peer-manager';
import type { PeerChange } from '../src/services/sync/peer-protocol';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const isDev = !app.isPackaged;

let peerManager: PeerManager | null = null;

function getDataDir(): string {
  const dir = path.join(app.getPath('appData'), 'delphi', 'data');
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

function createWindow() {
  const win = new BrowserWindow({
    width: 800,
    height: 600,
    minWidth: 400,
    minHeight: 200,
    title: 'Delphi',
    titleBarStyle: 'hiddenInset',
    webPreferences: {
      preload: path.join(__dirname, 'preload.mjs'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (isDev && process.env.VITE_DEV_SERVER_URL) {
    win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    win.loadFile(path.join(__dirname, '../dist/index.html'));
  }
}

// --- IPC handlers for filesystem storage ---

/** Resolve a renderer-supplied path within the data directory, rejecting path traversal. */
function safePath(relativePath: string): string {
  const dataDir = getDataDir();
  const fullPath = path.resolve(dataDir, relativePath);
  if (!fullPath.startsWith(dataDir + path.sep) && fullPath !== dataDir) {
    throw new Error(`Path traversal blocked: ${relativePath}`);
  }
  return fullPath;
}

ipcMain.handle('fs:exists', async (_event, filePath: string) => {
  return fs.existsSync(safePath(filePath));
});

ipcMain.handle('fs:readTextFile', async (_event, filePath: string) => {
  return fs.readFileSync(safePath(filePath), 'utf-8');
});

ipcMain.handle(
  'fs:writeTextFile',
  async (_event, filePath: string, contents: string) => {
    const fullPath = safePath(filePath);
    const dir = path.dirname(fullPath);
    fs.mkdirSync(dir, { recursive: true });
    fs.writeFileSync(fullPath, contents, 'utf-8');
  },
);

ipcMain.handle('fs:mkdir', async (_event, dirPath: string) => {
  fs.mkdirSync(safePath(dirPath), { recursive: true });
});

// --- P2P Peer Sync IPC ---

/** Read mesh credentials from the data directory. */
function loadMeshCredentials(): { meshSecret: string; deviceId: string; deviceName: string } | null {
  try {
    const credsPath = path.join(getDataDir(), 'mesh_credentials.json');
    if (!fs.existsSync(credsPath)) return null;
    const raw = JSON.parse(fs.readFileSync(credsPath, 'utf-8'));
    if (!raw.meshSecret || !raw.deviceId) return null;
    return {
      meshSecret: raw.meshSecret,
      deviceId: raw.deviceId,
      deviceName: raw.deviceName || 'Delphi Electron',
    };
  } catch {
    return null;
  }
}

/** Start the P2P peer manager if mesh credentials are available. */
async function startPeerManager(): Promise<void> {
  if (peerManager) return;

  const creds = loadMeshCredentials();
  if (!creds) {
    console.log('[Main] No mesh credentials found, P2P sync disabled');
    return;
  }

  peerManager = new PeerManager({
    meshSecret: creds.meshSecret,
    deviceId: creds.deviceId,
    deviceName: creds.deviceName,
    platform: 'electron',
  });

  peerManager.onChange((change, _fromDevice) => {
    // Forward incoming peer changes to all renderer windows
    for (const win of BrowserWindow.getAllWindows()) {
      win.webContents.send('peer:change', change);
    }
  });

  try {
    await peerManager.start();
    console.log('[Main] P2P peer manager started');
  } catch (err) {
    console.error('[Main] Failed to start peer manager:', err);
    peerManager = null;
  }
}

/** IPC: send a change to all connected peers. */
ipcMain.handle('peer:broadcastChange', async (_event, change: PeerChange) => {
  if (peerManager) {
    peerManager.broadcastChange(change);
    return true;
  }
  return false;
});

/** IPC: save mesh credentials and (re)start peer manager. */
ipcMain.handle(
  'peer:setMeshCredentials',
  async (_event, meshSecret: string, deviceId: string, deviceName: string) => {
    const credsPath = path.join(getDataDir(), 'mesh_credentials.json');
    fs.writeFileSync(credsPath, JSON.stringify({ meshSecret, deviceId, deviceName }), 'utf-8');

    // Restart peer manager with new credentials
    if (peerManager) {
      peerManager.stop();
      peerManager = null;
    }
    await startPeerManager();
    return true;
  },
);

/** IPC: check if P2P is active. */
ipcMain.handle('peer:isActive', async () => {
  return peerManager !== null;
});

// --- App lifecycle ---

app.whenReady().then(async () => {
  createWindow();
  await startPeerManager();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
