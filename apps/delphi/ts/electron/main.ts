import { app, BrowserWindow, ipcMain } from 'electron';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { PeerManager } from './peer-manager';
import type { PeerChange } from '../src/services/sync/peer-protocol';
import { sidecar, dbLoadAll, dbUpsertTodo, dbDeleteTodo, dbBatchUpsertTodos, dbUpsertProject, dbDeleteProject, dbGetSyncKv, dbSetSyncKv, dbClearAll, dbSwitchSpace } from './sidecar';
import { SyncServer } from './sync-server';
import { SyncClient } from './sync-client';
import type { SyncEntity, PeerRecord } from '../src/services/sync/lan-protocol';
import { LAN_SYNC_PORT, mergePeerRecords } from '../src/services/sync/lan-protocol';
import { getOwnAddresses } from '../src/services/space/space-manager';

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

let peerManager: PeerManager | null = null;
let syncServer: SyncServer | null = null;
const syncClients: Map<string, SyncClient> = new Map(); // device_id -> SyncClient
let currentDeviceId = '';
let currentDeviceName = 'Delphi Electron';

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

  peerManager.onPeerConnect((deviceId) => {
    // Notify renderer that a new peer connected — renderer will push its local state
    for (const win of BrowserWindow.getAllWindows()) {
      win.webContents.send('peer:peerConnected', deviceId);
    }
  });

  peerManager.onPeerDisconnect((deviceId, remaining) => {
    for (const win of BrowserWindow.getAllWindows()) {
      win.webContents.send('peer:peerDisconnected', deviceId, remaining);
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

/** IPC: get the peer server address for manual connect. */
ipcMain.handle('peer:getServerAddress', async () => {
  if (!peerManager) return null;
  const port = peerManager.getPort();
  if (!port) return null;
  const nets = os.networkInterfaces();
  for (const name of Object.keys(nets)) {
    for (const net of nets[name] ?? []) {
      if (net.family === 'IPv4' && !net.internal) {
        return `${net.address}:${port}`;
      }
    }
  }
  return `localhost:${port}`;
});

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
ipcMain.handle('db:switchSpace', (_e, spaceId: string) => dbSwitchSpace(spaceId))

ipcMain.handle('peer:stop', async () => {
  if (peerManager) {
    peerManager.stop();
    peerManager = null;
  }
  return true;
})

// --- Sync IPC ---

/** Collect own addresses from network interfaces. */
function collectOwnAddresses(): string[] {
  return getOwnAddresses(LAN_SYNC_PORT);
}

/** Forward a sync entity change to all renderer windows. */
function notifyRendererSyncChange(entity: SyncEntity): void {
  for (const win of BrowserWindow.getAllWindows()) {
    win.webContents.send('lan-sync:change', entity);
  }
}

/** Notify renderers about peer connect/disconnect. */
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

/** Get total connected peer count (server + client connections). */
function getTotalConnectedPeerCount(): number {
  let count = syncServer?.connectedPeerCount ?? 0;
  for (const client of syncClients.values()) {
    if (client.isConnected) count++;
  }
  return count;
}

/** Start a SyncClient for a peer record. */
function startSyncClientForPeer(peer: PeerRecord, deviceId: string, deviceName: string, spaceId: string, ownAddresses: string[]): void {
  if (peer.device_id === deviceId) return; // don't connect to self
  if (syncClients.has(peer.device_id)) return; // already have a client
  if (syncServer?.isConnectedTo(peer.device_id)) return; // already connected inbound

  const client = new SyncClient({
    peer,
    deviceId,
    deviceName,
    spaceId,
    ownAddresses,
    onChange: (entity) => {
      notifyRendererSyncChange(entity);
      // Re-broadcast to other peers via sync server
      syncServer?.broadcastLiveChange(entity, peer.device_id);
    },
    onConnected: (peerDeviceId, peerDeviceName) => {
      console.log(`[Main] SyncClient connected to ${peerDeviceName} (${peerDeviceId})`);
      syncServer?.registerExternalPeer(peerDeviceId, peerDeviceName, peer.addresses);
      notifyRendererPeerConnect(peerDeviceId);
    },
    onDisconnected: (peerDeviceId) => {
      console.log(`[Main] SyncClient disconnected from ${peerDeviceId}`);
      notifyRendererPeerDisconnect(peerDeviceId, getTotalConnectedPeerCount());
    },
    onPeerList: (peers) => {
      if (!syncServer) return;
      // Merge into our known peers
      const knownPeers = syncServer.getKnownPeers();
      const merged = mergePeerRecords(knownPeers, peers.filter(p => p.device_id !== deviceId));
      // Start clients for any newly discovered peers
      for (const newPeer of merged) {
        if (!syncClients.has(newPeer.device_id) && !syncServer.isConnectedTo(newPeer.device_id)) {
          startSyncClientForPeer(newPeer, deviceId, deviceName, spaceId, ownAddresses);
        }
      }
    },
  });

  syncClients.set(peer.device_id, client);
  client.start();
}

/** Connect to seed addresses from QR payload — used on first join when no known peers exist. */
function connectToSeedAddresses(addresses: string[], deviceId: string, deviceName: string, ownAddresses: string[]): void {
  if (!syncServer || addresses.length === 0) return;

  // Skip if these addresses belong to an already-known peer
  const knownPeers = syncServer.getKnownPeers();
  const alreadyKnown = knownPeers.some(p => p.addresses.some(a => addresses.includes(a)));
  if (alreadyKnown) return;

  const tempId = `seed-${Date.now()}`;
  let client: SyncClient;

  client = new SyncClient({
    peer: { device_id: tempId, device_name: 'Bootstrap', addresses, last_seen: new Date().toISOString() },
    deviceId,
    deviceName,
    spaceId: '',
    ownAddresses,
    onChange: (entity) => {
      notifyRendererSyncChange(entity);
      syncServer?.broadcastLiveChange(entity, tempId);
    },
    onConnected: (peerDeviceId, peerDeviceName) => {
      // Re-key from temp ID to real device ID
      syncClients.delete(tempId);
      if (!syncClients.has(peerDeviceId)) {
        syncClients.set(peerDeviceId, client);
      }
      syncServer?.registerExternalPeer(peerDeviceId, peerDeviceName, addresses);
      notifyRendererPeerConnect(peerDeviceId);
    },
    onDisconnected: (peerDeviceId) => {
      syncClients.delete(peerDeviceId);
      syncClients.delete(tempId);
      notifyRendererPeerDisconnect(peerDeviceId, getTotalConnectedPeerCount());
    },
    onPeerList: (peers) => {
      if (!syncServer) return;
      const knownPeers = syncServer.getKnownPeers();
      const merged = mergePeerRecords(knownPeers, peers.filter(p => p.device_id !== deviceId));
      for (const newPeer of merged) {
        if (!syncClients.has(newPeer.device_id) && !syncServer.isConnectedTo(newPeer.device_id)) {
          startSyncClientForPeer(newPeer, deviceId, deviceName, '', ownAddresses);
        }
      }
    },
  });

  syncClients.set(tempId, client);
  client.start();
  console.log(`[Main] Connecting to seed addresses: ${addresses.join(', ')}`);
}

/** Start the sync server and connect to known peers. */
async function startSync(deviceId: string, deviceName?: string, seedAddresses?: string[]): Promise<void> {
  if (syncServer) {
    syncServer.stop();
    syncServer = null;
  }

  // Stop existing clients
  for (const client of syncClients.values()) {
    client.stop();
  }
  syncClients.clear();

  currentDeviceId = deviceId;
  currentDeviceName = deviceName ?? 'Delphi Electron';

  const ownAddresses = collectOwnAddresses();
  const name = currentDeviceName;

  syncServer = new SyncServer();

  // When a peer sends us a change, forward to all renderer windows
  syncServer.onChange((entity: SyncEntity) => {
    notifyRendererSyncChange(entity);
  });

  syncServer.onPeerConnect((peerDeviceId) => {
    notifyRendererPeerConnect(peerDeviceId);
  });

  syncServer.onPeerDisconnect((peerDeviceId, _remaining) => {
    notifyRendererPeerDisconnect(peerDeviceId, getTotalConnectedPeerCount());
  });

  // When we learn about new peers via peer_list, start clients for them
  syncServer.onNewPeerDiscovered((peer) => {
    startSyncClientForPeer(peer, deviceId, name, '', ownAddresses);
  });

  try {
    await syncServer.start(undefined, deviceId, name, ownAddresses);
    console.log('[Main] Sync server started');

    // Connect to known peers from previous sessions
    const knownPeers = syncServer.getKnownPeers();
    for (const peer of knownPeers) {
      startSyncClientForPeer(peer, deviceId, name, '', ownAddresses);
    }

    // Connect to seed addresses from QR payload (first join, no known peers)
    if (seedAddresses && seedAddresses.length > 0) {
      connectToSeedAddresses(seedAddresses, deviceId, name, ownAddresses);
    }
  } catch (err) {
    console.error('[Main] Failed to start sync server:', err);
    syncServer = null;
  }
}

/** IPC: start sync (spaceId kept for backward compat but ignored). */
ipcMain.handle('lan-sync:start', async (_e, _spaceId: string | undefined, deviceId: string, deviceName?: string, seedAddresses?: string[]) => {
  await startSync(deviceId, deviceName, seedAddresses);
  return syncServer !== null;
});

/** IPC: stop sync. */
ipcMain.handle('lan-sync:stop', async () => {
  for (const client of syncClients.values()) {
    client.stop();
  }
  syncClients.clear();
  if (syncServer) {
    syncServer.stop();
    syncServer = null;
  }
  return true;
});

/** IPC: get sync status. */
ipcMain.handle('lan-sync:getStatus', async () => {
  if (!syncServer) return { active: false, peers: 0, peerNames: [] };
  const peerNames = syncServer.getConnectedPeerNames();
  // Also include peers connected via clients
  for (const client of syncClients.values()) {
    if (client.isConnected) {
      peerNames.push(client.peerName);
    }
  }
  return { active: true, peers: getTotalConnectedPeerCount(), peerNames };
});

/** IPC: broadcast a local change to sync peers. */
ipcMain.handle('lan-sync:broadcastChange', async (_e, entity: SyncEntity) => {
  if (!syncServer) return false;
  // Update HLC for this entity
  const hlc = await syncServer.updateEntityHlc(entity.id);
  entity.hlc = hlc;
  // Broadcast via server (to inbound peers)
  syncServer.broadcastLiveChange(entity);
  // Broadcast via clients (to outbound peers)
  for (const client of syncClients.values()) {
    client.broadcastLiveChange(entity);
  }
  return true;
});

/** IPC: get own addresses for QR generation. */
ipcMain.handle('sync:getOwnAddresses', async () => {
  return collectOwnAddresses();
});

/** IPC: get QR payload (code + addresses). */
ipcMain.handle('sync:getQrPayload', async (_e, code: string) => {
  const { generateQrPayload } = await import('../src/services/space/space-manager');
  const addresses = collectOwnAddresses();
  return generateQrPayload(code, addresses);
});

/** IPC: get known peers list. */
ipcMain.handle('sync:getPeers', async () => {
  if (!syncServer) return [];
  return syncServer.getKnownPeers();
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

app.on('before-quit', () => {
  // Stop sync clients
  for (const client of syncClients.values()) {
    client.stop();
  }
  syncClients.clear();
  if (syncServer) {
    syncServer.stop();
    syncServer = null;
  }
  sidecar.shutdown();
});

app.on('activate', () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});
