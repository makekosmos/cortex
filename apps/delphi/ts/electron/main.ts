import { app, BrowserWindow, ipcMain, Menu } from 'electron';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { ArkClient } from '@arksync/node';

/** Minimal sync entity type (matches Rust SyncEntity wire format). */
interface SyncEntity {
  type: string;
  id: string;
  data: Record<string, unknown>;
  hlc: string;
  deleted?: boolean;
}
import {
  sidecar,
  dbLoadAll,
  dbUpsertTodo,
  dbDeleteTodo,
  dbBatchUpsertTodos,
  dbUpsertProject,
  dbDeleteProject,
  dbGetSyncKv,
  dbSetSyncKv,
  dbClearAll,
  dbDeleteTrashed,
  dbSwitchSpace,
  syncLeaveSpace,
  syncBroadcastChange,
  syncGetConnectedPeers,
  syncAddSeedPeer,
  syncGetOwnAddresses,
  type SidecarEvent,
  type SyncEntityPayload,
} from './sidecar';

/** WebSocket port used by the ark-core sync server. */
const LAN_SYNC_PORT = 21531;

// Suppress mDNS multicast errors — these happen on networks that don't support
// multicast (VPN, some Wi-Fi). They're non-fatal; P2P degrades to relay-only.
process.on('uncaughtException', (err: NodeJS.ErrnoException) => {
  if (err.code === 'EHOSTUNREACH' || err.code === 'ENETUNREACH' || err.code === 'EADDRNOTAVAIL') {
    console.warn('[Main] mDNS unavailable on this network:', err.message);
    return;
  }
  // Re-throw anything else
  console.error('[Main] Uncaught exception:', err);
  throw err;
});

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const isDev = !app.isPackaged;

/**
 * ArkClient instance from @arksync/node. Created lazily when sync starts.
 * Wraps the sync lifecycle (start/stop/broadcastChange) while DB operations
 * remain in sidecar.ts.
 */
let arkClient: ArkClient | null = null;

/** True once start_sync has been issued to the sidecar — used as a cheap
 *  `active` probe for `lan-sync:getStatus` without a round-trip. */
let syncActive = false;
/** Track peer names per device_id as reported by sidecar events. */
const syncPeerNames: Map<string, string> = new Map();
let currentDeviceId = '';

/**
 * Human-readable host name for this device. Prefers the OS host name
 * ("Kirill's MacBook Pro") over any renderer-supplied label so peers see
 * meaningful names instead of the Electron app name.
 *
 * `.local` suffix (Bonjour) is stripped for display cleanliness.
 */
function getHostDeviceName(): string {
  try {
    const raw = os.hostname() || '';
    const cleaned = raw.replace(/\.local$/i, '').trim();
    return cleaned || 'Delphi Electron';
  } catch {
    return 'Delphi Electron';
  }
}

let currentDeviceName = getHostDeviceName();

function getDataDir(): string {
  const dir = path.join(app.getPath('appData'), 'Kepler');
  fs.mkdirSync(dir, { recursive: true });
  return dir;
}

const WINDOW_STATE_FILE = path.join(app.getPath('userData'), 'window-state.json');

function loadWindowState(): { width: number; height: number; x?: number; y?: number; isMaximized?: boolean } {
  try {
    if (fs.existsSync(WINDOW_STATE_FILE)) {
      return JSON.parse(fs.readFileSync(WINDOW_STATE_FILE, 'utf-8'));
    }
  } catch { /* ignore */ }
  return { width: 1440, height: 1080 };
}

function saveWindowState(win: BrowserWindow) {
  const isMaximized = win.isMaximized();
  const bounds = isMaximized ? (win as any)._lastBounds ?? win.getBounds() : win.getBounds();
  fs.writeFileSync(WINDOW_STATE_FILE, JSON.stringify({ ...bounds, isMaximized }), 'utf-8');
}

function createWindow() {
  Menu.setApplicationMenu(null);

  const saved = loadWindowState();

  const win = new BrowserWindow({
    width: saved.width,
    height: saved.height,
    x: saved.x,
    y: saved.y,
    minWidth: 1280,
    minHeight: 720,
    title: 'Delphi',
    titleBarStyle: 'hiddenInset',
    trafficLightPosition: { x: 18, y: 18 },
    webPreferences: {
      preload: path.join(__dirname, 'preload.mjs'),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  if (saved.isMaximized) win.maximize();

  // Track bounds before maximize so we can save the windowed size
  const onResize = () => { if (!win.isMaximized()) (win as any)._lastBounds = win.getBounds(); };
  const onMove = () => { if (!win.isMaximized()) (win as any)._lastBounds = win.getBounds(); };
  win.on('resize', onResize);
  win.on('move', onMove);
  win.on('close', () => {
    win.removeListener('resize', onResize);
    win.removeListener('move', onMove);
    saveWindowState(win);
  });

  if (isDev && process.env.VITE_DEV_SERVER_URL) {
    win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    win.loadFile(path.join(__dirname, '../dist/index.html'));
  }

  win.webContents.on('before-input-event', (_event, input) => {
    if (input.key === 'F12') {
      win.webContents.toggleDevTools();
    }
  });
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

// --- P2P Peer Sync IPC (legacy mesh-credentials helpers removed) ---
// Sync lifecycle is now handled by ArkClient from @arksync/node.
// The IPC channels (lan-sync:start, lan-sync:stop, etc.) are retained below.

/** IPC: stub for legacy renderer calls that asked for peer:isActive. */
ipcMain.handle('peer:isActive', async () => syncActive);

/** IPC: stub — server address is now reported via sidecar get_own_addresses. */
ipcMain.handle('peer:getServerAddress', async () => null);

// --- IPC handlers for space persistence (file-based, survives localStorage wipe) ---

const spacesFile = path.join(getDataDir(), 'spaces.json');

function readSpacesFile(): { active: string | null; spaces: Array<{ code: string; name: string; createdAt: string }> } {
  try {
    if (fs.existsSync(spacesFile)) {
      return JSON.parse(fs.readFileSync(spacesFile, 'utf-8'));
    }
  } catch {}
  return { active: null, spaces: [] };
}

function writeSpacesFile(data: { active: string | null; spaces: Array<{ code: string; name: string; createdAt: string }> }): void {
  fs.writeFileSync(spacesFile, JSON.stringify(data, null, 2), 'utf-8');
}

ipcMain.handle('space:getActive', () => readSpacesFile().active);
ipcMain.handle('space:setActive', (_e, code: string | null) => {
  const data = readSpacesFile();
  data.active = code;
  writeSpacesFile(data);
});
ipcMain.handle('space:getAll', () => readSpacesFile().spaces);
ipcMain.handle('space:save', (_e, space: { code: string; name: string; createdAt: string }) => {
  const data = readSpacesFile();
  data.spaces = data.spaces.filter(s => s.code !== space.code);
  data.spaces.unshift(space);
  writeSpacesFile(data);
});
ipcMain.handle('space:remove', (_e, code: string) => {
  const data = readSpacesFile();
  data.spaces = data.spaces.filter(s => s.code !== code);
  if (data.active === code) data.active = null;
  writeSpacesFile(data);
});
ipcMain.handle('space:rename', (_e, code: string, newName: string) => {
  const data = readSpacesFile();
  const space = data.spaces.find(s => s.code === code);
  if (space) {
    space.name = newName;
    writeSpacesFile(data);
    return true;
  }
  return false;
});
ipcMain.handle('space:getDbPath', () => path.join(getDataDir(), 'spaces'));

// Scan spaces/ directory for orphaned DBs not in spaces.json
ipcMain.handle('space:scanOrphaned', () => {
  const spacesDir = path.join(getDataDir(), 'spaces');
  if (!fs.existsSync(spacesDir)) return [];
  const known = readSpacesFile().spaces.map(s => {
    // Derive spaceId from code — SHA-256(normalized)[:16]
    const crypto = require('node:crypto');
    const normalized = s.code.replace(/-/g, '').replace(/ /g, '').toUpperCase();
    return crypto.createHash('sha256').update(normalized, 'utf-8').digest('hex').slice(0, 16);
  });
  const knownSet = new Set(known);
  const orphaned: string[] = [];
  for (const entry of fs.readdirSync(spacesDir, { withFileTypes: true })) {
    if (entry.isDirectory() && !knownSet.has(entry.name)) {
      const dbFile = path.join(spacesDir, entry.name, 'ark.db');
      const legacyDbFile = path.join(spacesDir, entry.name, 'delphi.db');
      if (fs.existsSync(dbFile) || fs.existsSync(legacyDbFile)) {
        orphaned.push(entry.name);
      }
    }
  }
  return orphaned;
});

// --- Migrate old spaces from legacy locations to Kepler ---
{
  const newSpacesDir = path.join(getDataDir(), 'spaces');
  const legacyDirs = [
    path.join(app.getPath('userData'), 'spaces'),           // old userData/spaces
    path.join(app.getPath('appData'), 'delphi', 'data', 'spaces'), // old delphi/data/spaces
  ];
  for (const oldDir of legacyDirs) {
    if (fs.existsSync(oldDir) && oldDir !== newSpacesDir) {
      try {
        fs.mkdirSync(newSpacesDir, { recursive: true });
        for (const entry of fs.readdirSync(oldDir, { withFileTypes: true })) {
          if (entry.isDirectory()) {
            const src = path.join(oldDir, entry.name);
            const dst = path.join(newSpacesDir, entry.name);
            if (!fs.existsSync(dst)) {
              fs.cpSync(src, dst, { recursive: true });
              console.log(`[Main] Migrated space DB from ${oldDir}: ${entry.name}`);
            }
          }
        }
      } catch (err) {
        console.error('[Main] Failed to migrate old spaces:', err);
      }
    }
  }
  // Also migrate mesh_credentials.json and spaces.json from old delphi/data
  const oldDataDir = path.join(app.getPath('appData'), 'delphi', 'data');
  if (fs.existsSync(oldDataDir) && oldDataDir !== getDataDir()) {
    for (const file of ['mesh_credentials.json', 'spaces.json']) {
      const src = path.join(oldDataDir, file);
      const dst = path.join(getDataDir(), file);
      if (fs.existsSync(src) && !fs.existsSync(dst)) {
        fs.cpSync(src, dst);
        console.log(`[Main] Migrated ${file} to Kepler`);
      }
    }
  }
}

// --- IPC handlers for local DB ---

ipcMain.handle('db:loadAll', () => dbLoadAll())
ipcMain.handle('db:upsertTodo', (_e, todo) => dbUpsertTodo(todo))
ipcMain.handle('db:deleteTodo', (_e, id) => dbDeleteTodo(id))
ipcMain.handle('db:batchUpsertTodos', (_e, todos) => dbBatchUpsertTodos(todos))
ipcMain.handle('db:upsertProject', (_e, project) => dbUpsertProject(project))
ipcMain.handle('db:deleteProject', (_e, id) => dbDeleteProject(id))
ipcMain.handle('db:getSyncKv', (_e, key) => dbGetSyncKv(key))
ipcMain.handle('db:setSyncKv', (_e, key, value) => dbSetSyncKv(key, value))
ipcMain.handle('db:clearAll', () => dbClearAll())
ipcMain.handle('db:deleteTrashed', () => dbDeleteTrashed())
ipcMain.handle('db:switchSpace', (_e, spaceId: string) => dbSwitchSpace(spaceId))

ipcMain.handle('db:deleteSpace', async (_e, spaceId: string) => {
  const spaceDir = path.join(getDataDir(), 'spaces', spaceId);

  // If the sidecar is currently using this space's DB, release it first
  const currentPath = sidecar.currentDbPath;
  if (currentPath && currentPath.startsWith(spaceDir)) {
    sidecar.releaseAndReset();
    // Give the OS time to release file locks
    await new Promise(r => setTimeout(r, 200));
  }

  // Retry deletion in case file locks are slow to release (Windows)
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      fs.rmSync(spaceDir, { recursive: true, force: true });
      console.log(`[Main] Deleted space DB: ${spaceDir}`);
      return true;
    } catch (err: unknown) {
      const code = (err as NodeJS.ErrnoException).code;
      if (code === 'EPERM' && attempt < 2) {
        await new Promise(r => setTimeout(r, 500));
        continue;
      }
      console.error(`[Main] Failed to delete space DB:`, err);
      return false;
    }
  }
  return false;
})

ipcMain.handle('peer:stop', async () => {
  // Legacy handler — no-op now that PeerManager is removed.
  return true;
})

// --- Sync IPC (backed by ArkClient from @arksync/node → ark-core-rpc sidecar) ---
//
// Sync lifecycle (start/stop/broadcast) is delegated to ArkClient (@arksync/node).
// ArkClient is wired to the existing sidecar via requestFn + onEventFn injection
// so that DB ops and sync ops share the same ark-core-rpc process.
// DB operations (dbLoadAll, dbUpsertTodo, …) remain in sidecar.ts.

/** Forward a sync entity change to all renderer windows. */
function notifyRendererSyncChange(entity: SyncEntity): void {
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.send('lan-sync:change', entity);
  }
}

function notifyRendererPeerConnect(deviceId: string): void {
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.send('lan-sync:peerConnected', deviceId);
  }
}

function notifyRendererPeerDisconnect(deviceId: string, remaining: number): void {
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.send('lan-sync:peerDisconnected', deviceId, remaining);
  }
}

/**
 * Subscribe once to the sidecar event stream and fan events out to the
 * renderer via the same IPC channels the old in-process `SyncServer` used.
 * The subscription persists for the lifetime of the Electron process so
 * re-calls to `start_sync` don't need to re-subscribe.
 */
sidecar.onEvent((event: SidecarEvent) => {
  switch (event.event) {
    case 'entity_changed': {
      const entity = event.entity as unknown as SyncEntity;
      if (entity) notifyRendererSyncChange(entity);
      break;
    }
    case 'peer_connected': {
      const deviceId = String(event.device_id ?? '');
      const deviceName = String(event.device_name ?? '');
      if (deviceId) {
        if (deviceName) syncPeerNames.set(deviceId, deviceName);
        notifyRendererPeerConnect(deviceId);
      }
      break;
    }
    case 'peer_disconnected': {
      const deviceId = String(event.device_id ?? '');
      const remaining = typeof event.remaining === 'number' ? event.remaining : 0;
      if (deviceId) {
        syncPeerNames.delete(deviceId);
        notifyRendererPeerDisconnect(deviceId, remaining);
      }
      break;
    }
    case 'peer_list_updated': {
      const peers = Array.isArray(event.peers) ? (event.peers as Array<{ device_id?: string; device_name?: string }>) : [];
      const next = new Map<string, string>();
      for (const p of peers) {
        if (p?.device_id) next.set(p.device_id, p.device_name ?? '');
      }
      // Keep the name cache in sync with the latest snapshot; don't
      // emit renderer events on this path — it's advisory metadata.
      syncPeerNames.clear();
      for (const [id, name] of next) syncPeerNames.set(id, name);
      break;
    }
    default:
      // Unknown event — log but don't crash.
      console.debug('[Main] Unknown sidecar event:', event);
  }
});

/** IPC: start sync via ArkClient (@arksync/node). */
ipcMain.handle('lan-sync:start', async (_e, spaceId: string | undefined, deviceId: string, deviceName?: string, seedAddresses?: string[]) => {
  if (!spaceId) {
    console.warn('[Main] lan-sync:start called without spaceId, ignoring');
    return false;
  }
  currentDeviceId = deviceId;
  const name = getHostDeviceName();
  currentDeviceName = name;
  void deviceName;

  // Stop any existing client before creating a new one.
  if (arkClient) {
    try { await arkClient.stop(); } catch { /* ignore */ }
    arkClient = null;
  }

  // Create ArkClient wired to the existing sidecar (DB + sync share one process).
  arkClient = new ArkClient({
    spaceId,
    deviceId,
    deviceName: name,
    port: LAN_SYNC_PORT,
    // Inject the sidecar's request function so no second process is spawned.
    requestFn: <T>(req: Record<string, unknown>) => sidecar.request<T>(req as Parameters<typeof sidecar.request>[0]),
    onEventFn: (listener) => sidecar.onEvent(listener as Parameters<typeof sidecar.onEvent>[0]),
  });

  // Seed peers are added after start via addSeedPeer.
  try {
    await arkClient.start();
    if (seedAddresses?.length) {
      await syncAddSeedPeer(seedAddresses);
    }
    syncActive = true;
    syncPeerNames.clear();
    console.log(`[Main] Sync started via ArkClient (device=${name})`);
    return true;
  } catch (err) {
    console.error('[Main] start_sync via ArkClient failed:', err);
    syncActive = false;
    arkClient = null;
    return false;
  }
});

/** IPC: stop sync via ArkClient. */
ipcMain.handle('lan-sync:stop', async () => {
  if (arkClient) {
    try { await arkClient.stop(); } catch (err) { console.warn('[Main] stop_sync failed:', err); }
    arkClient = null;
  }
  syncActive = false;
  syncPeerNames.clear();
  return true;
});

/** IPC: get sync status from sidecar. */
ipcMain.handle('lan-sync:getStatus', async () => {
  if (!syncActive) return { active: false, peers: 0, peerNames: [] };
  try {
    const peers = await syncGetConnectedPeers();
    // Dedup on the Electron side as well — the Rust side already dedups but
    // we want to be defensive in case a future race leaks duplicates.
    const seen = new Map<string, string>();
    for (const p of peers) {
      seen.set(p.device_id, p.device_name);
    }
    return {
      active: true,
      peers: seen.size,
      peerNames: [...seen.values()],
    };
  } catch (err) {
    console.warn('[Main] get_connected_peers failed:', err);
    return { active: true, peers: 0, peerNames: [] };
  }
});

/** IPC: broadcast a local change to sync peers via sidecar. */
ipcMain.handle('lan-sync:broadcastChange', async (_e, entity: SyncEntity) => {
  if (!syncActive) return false;
  try {
    // Deep clone to strip Vue proxies before IPC -> sidecar.
    const cloned = JSON.parse(JSON.stringify(entity)) as SyncEntityPayload;
    await syncBroadcastChange(cloned);
    return true;
  } catch (err) {
    console.warn('[Main] broadcast_change via sidecar failed:', err);
    return false;
  }
});

/** IPC: get own addresses for QR generation. */
ipcMain.handle('sync:getOwnAddresses', async () => {
  try {
    return await syncGetOwnAddresses(LAN_SYNC_PORT);
  } catch (err) {
    console.warn('[Main] get_own_addresses via sidecar failed, falling back to os:', err);
    // Cheap fallback: enumerate via os.networkInterfaces() without importing
    // @arksync/node at runtime.
    const addrs: string[] = [];
    const nets = os.networkInterfaces();
    for (const name of Object.keys(nets)) {
      for (const net of nets[name] ?? []) {
        if (net.internal) continue;
        if (net.family === 'IPv4') addrs.push(`${net.address}:${LAN_SYNC_PORT}`);
      }
    }
    return addrs;
  }
});

/** IPC: get QR payload (code + addresses). */
ipcMain.handle('sync:getQrPayload', async (_e, code: string) => {
  const { generateQrPayload } = await import('../src/services/space/space-manager');
  const addresses = await syncGetOwnAddresses(LAN_SYNC_PORT).catch(() => [] as string[]);
  return generateQrPayload(code, addresses);
});

/** IPC: get known peers list (currently just the connected peers). */
ipcMain.handle('sync:getPeers', async () => {
  if (!syncActive) return [];
  try {
    const peers = await syncGetConnectedPeers();
    return peers.map((p) => ({
      device_id: p.device_id,
      device_name: p.device_name,
      addresses: [] as string[],
      last_seen: new Date().toISOString(),
    }));
  } catch {
    return [];
  }
});

/** IPC: leave the current space (tears down sync runtime). */
ipcMain.handle('lan-sync:leaveSpace', async () => {
  try {
    await syncLeaveSpace();
  } catch (err) {
    console.warn('[Main] leave_space failed:', err);
  }
  syncActive = false;
  syncPeerNames.clear();
  return true;
});

/** IPC: add a seed peer (used on QR-code bootstrap). */
ipcMain.handle('lan-sync:addSeedPeer', async (_e, addresses: string[]) => {
  if (!syncActive) return false;
  try {
    await syncAddSeedPeer(addresses);
    return true;
  } catch (err) {
    console.warn('[Main] add_seed_peer failed:', err);
    return false;
  }
});

// --- App lifecycle ---

app.whenReady().then(async () => {
  createWindow();
});

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

app.on('before-quit', () => {
  // Best-effort: stop ArkClient and shut down the sidecar.
  if (arkClient) {
    arkClient.stop().catch(() => {});
    arkClient = null;
  }
  syncActive = false;
  sidecar.shutdown();
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
