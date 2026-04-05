/**
 * ArkSync WebSocket Server — generic, app-agnostic sync server.
 *
 * Equal-peer model: every device runs both a WS server and WS clients.
 * This is the server half — accepts incoming connections from other peers.
 *
 * Protocol flow per connection:
 *   1. Client sends `hello` (with addresses[]) -> server validates, replies with `hello`
 *   2. Both exchange `version_vector` messages
 *   3. Both exchange `peer_list` messages (known peers with multi-address records)
 *   4. Server computes diff -> sends `sync_changes` batches -> waits for `sync_ack`
 *   5. Client sends its `sync_changes` batches -> server replies with `sync_ack`
 *   6. Both enter live mode: mutations broadcast as `live_change` with `live_ack`
 */

import WebSocket, { WebSocketServer } from "ws";
import {
  LAN_SYNC_PORT,
  PROTOCOL_VERSION,
  PING_INTERVAL_MS,
  MAX_RETRIES,
  BATCH_ACK_TIMEOUT_MS,
  type LanSyncMessage,
  type HelloMessage,
  type VersionVectorMessage,
  type SyncChangesMessage,
  type SyncAckMessage,
  type LiveChangeMessage,
  type LiveAckMessage,
  type PeerListMessage,
  type PeerRecord,
  type SyncEntity,
  type VersionVector,
  serializeMessage,
  deserializeMessage,
  splitIntoBatches,
  generateId,
  isNewerHlc,
  mergePeerRecords,
} from "./protocol";
import { HLC } from "./hlc";
import type { StorageBackend } from "./storage";

const TAG = "[SyncServer]";

const VERSION_VECTOR_KEY = "lan_sync.version_vector";
const KNOWN_PEERS_KEY = "sync.peers";

// ---------------------------------------------------------------------------
// Peer state (per-connection)
// ---------------------------------------------------------------------------

interface PeerState {
  ws: WebSocket;
  deviceId: string;
  deviceName: string;
  addresses: string[];
  authenticated: boolean;
  syncComplete: boolean;
  pendingAcks: Map<
    string,
    {
      resolve: () => void;
      timer: ReturnType<typeof setTimeout>;
      retries: number;
    }
  >;
  /** Live changes queued while initial sync is in progress. */
  queuedLiveChanges: SyncEntity[];
}

// ---------------------------------------------------------------------------
// SyncServer
// ---------------------------------------------------------------------------

export class SyncServer {
  private wss: WebSocketServer | null = null;
  private peers: Map<WebSocket, PeerState> = new Map();
  private spaceId: string = "";
  private deviceId: string = "";
  private deviceName: string = "ArkSync";
  private ownAddresses: string[] = [];
  private pingInterval: ReturnType<typeof setInterval> | null = null;

  /** Known peers persisted in sync_kv. Full PeerRecord with multi-address. */
  private knownPeerRecords: PeerRecord[] = [];

  private onChangeHandler: ((entity: SyncEntity) => void) | null = null;
  private onPeerConnectHandler: ((deviceId: string) => void) | null = null;
  private onPeerDisconnectHandler:
    | ((deviceId: string, remaining: number) => void)
    | null = null;
  private onNewPeerDiscoveredHandler: ((peer: PeerRecord) => void) | null =
    null;

  constructor(private storage: StorageBackend) {}

  onChange(handler: (entity: SyncEntity) => void): void {
    this.onChangeHandler = handler;
  }

  onPeerConnect(handler: (deviceId: string) => void): void {
    this.onPeerConnectHandler = handler;
  }

  onPeerDisconnect(
    handler: (deviceId: string, remaining: number) => void,
  ): void {
    this.onPeerDisconnectHandler = handler;
  }

  /** Called when peer_list exchange reveals a new peer we should connect to. */
  onNewPeerDiscovered(handler: (peer: PeerRecord) => void): void {
    this.onNewPeerDiscoveredHandler = handler;
  }

  get connectedPeerCount(): number {
    return [...this.peers.values()].filter((p) => p.authenticated).length;
  }

  get knownPeerCount(): number {
    return this.knownPeerRecords.length;
  }

  getKnownPeers(): PeerRecord[] {
    return [...this.knownPeerRecords];
  }

  getConnectedPeerNames(): string[] {
    const seen = new Map<string, string>();
    for (const p of this.peers.values()) {
      if (p.authenticated && p.deviceId) {
        seen.set(p.deviceId, p.deviceName);
      }
    }
    return [...seen.values()];
  }

  /** Return deduplicated list of connected peer entries (id + name). */
  getConnectedPeerEntries(): Array<{ deviceId: string; deviceName: string }> {
    const seen = new Map<string, string>();
    for (const p of this.peers.values()) {
      if (p.authenticated && p.deviceId) {
        seen.set(p.deviceId, p.deviceName);
      }
    }
    return [...seen.entries()].map(([deviceId, deviceName]) => ({
      deviceId,
      deviceName,
    }));
  }

  /** Start the WS server. */
  async start(
    spaceId: string | undefined,
    deviceId: string,
    deviceName?: string,
    ownAddresses?: string[],
  ): Promise<void> {
    this.spaceId = spaceId ?? "";
    this.deviceId = deviceId;
    if (deviceName) this.deviceName = deviceName;
    if (ownAddresses) this.ownAddresses = ownAddresses;

    if (this.wss) {
      this.stop();
    }

    // Load known peers from previous sessions
    await this.loadKnownPeers();

    return new Promise((resolve, reject) => {
      this.wss = new WebSocketServer({ port: LAN_SYNC_PORT }, () => {
        console.log(
          `${TAG} Server listening on port ${LAN_SYNC_PORT}, ${this.knownPeerCount} known peers`,
        );
        resolve();
      });

      this.wss.on("error", (err) => {
        console.error(`${TAG} Server error:`, err.message);
        reject(err);
      });

      this.wss.on("connection", (ws) => {
        this.handleConnection(ws);
      });

      this.pingInterval = setInterval(() => {
        for (const [ws, peer] of this.peers) {
          if (peer.authenticated && ws.readyState === WebSocket.OPEN) {
            ws.ping();
          }
        }
      }, PING_INTERVAL_MS);
    });
  }

  /** Stop the WS server and close all connections. */
  stop(): void {
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }

    for (const [ws, peer] of this.peers) {
      for (const ack of peer.pendingAcks.values()) {
        clearTimeout(ack.timer);
      }
      try {
        ws.close(1000, "server stopping");
      } catch {
        /* ignore */
      }
    }
    this.peers.clear();

    if (this.wss) {
      this.wss.close();
      this.wss = null;
    }
    console.log(`${TAG} Server stopped`);
  }

  /** Update own addresses (e.g. when network changes). */
  setOwnAddresses(addresses: string[]): void {
    this.ownAddresses = addresses;
  }

  /** Broadcast a live change to all authenticated peers. */
  broadcastLiveChange(entity: SyncEntity, excludeDeviceId?: string): void {
    const changeId = generateId();
    const msg: LiveChangeMessage = {
      type: "live_change",
      change_id: changeId,
      entity,
    };

    for (const [ws, peer] of this.peers) {
      if (!peer.authenticated) continue;
      if (peer.deviceId === excludeDeviceId) continue;
      if (ws.readyState !== WebSocket.OPEN) continue;

      if (!peer.syncComplete) {
        peer.queuedLiveChanges.push(entity);
        continue;
      }

      this.sendWithRetry(ws, peer, msg, changeId);
    }
  }

  /** Register an externally-managed peer connection (from SyncClient). */
  registerExternalPeer(
    deviceId: string,
    deviceName: string,
    addresses: string[],
  ): void {
    this.updatePeerRecord(deviceId, deviceName, addresses);
  }

  /** Check if we have a connected peer with the given device_id. */
  isConnectedTo(deviceId: string): boolean {
    for (const peer of this.peers.values()) {
      if (peer.deviceId === deviceId && peer.authenticated) return true;
    }
    return false;
  }

  // ---------------------------------------------------------------------------
  // Connection handling
  // ---------------------------------------------------------------------------

  private handleConnection(ws: WebSocket): void {
    const peer: PeerState = {
      ws,
      deviceId: "",
      deviceName: "",
      addresses: [],
      authenticated: false,
      syncComplete: false,
      pendingAcks: new Map(),
      queuedLiveChanges: [],
    };
    this.peers.set(ws, peer);

    ws.on("message", (raw) => {
      const msg = deserializeMessage(raw.toString());
      if (!msg) return;
      this.handleMessage(ws, peer, msg);
    });

    ws.on("close", () => {
      for (const ack of peer.pendingAcks.values()) {
        clearTimeout(ack.timer);
      }
      this.peers.delete(ws);
      if (peer.authenticated) {
        console.log(
          `${TAG} Peer disconnected: ${peer.deviceName} (${peer.deviceId})`,
        );
        this.onPeerDisconnectHandler?.(peer.deviceId, this.connectedPeerCount);
      }
    });

    ws.on("error", (err) => {
      console.warn(`${TAG} Peer error:`, err.message);
    });
  }

  private handleMessage(
    ws: WebSocket,
    peer: PeerState,
    msg: LanSyncMessage,
  ): void {
    switch (msg.type) {
      case "hello":
        this.handleHello(ws, peer, msg);
        break;
      case "version_vector":
        this.handleVersionVector(ws, peer, msg);
        break;
      case "sync_changes":
        this.handleSyncChanges(ws, peer, msg);
        break;
      case "sync_ack":
        this.handleSyncAck(peer, msg);
        break;
      case "live_change":
        this.handleLiveChange(ws, peer, msg);
        break;
      case "live_ack":
        this.handleLiveAck(peer, msg);
        break;
      case "peer_list":
        this.handlePeerList(ws, peer, msg);
        break;
      case "pong":
        break;
      case "ping":
        this.send(ws, { type: "pong", ts: msg.ts });
        break;
    }
  }

  // ---------------------------------------------------------------------------
  // Handshake
  // ---------------------------------------------------------------------------

  private handleHello(ws: WebSocket, peer: PeerState, msg: HelloMessage): void {
    if (msg.protocol_version !== PROTOCOL_VERSION) {
      console.warn(
        `${TAG} Protocol version mismatch: ${msg.protocol_version} vs ${PROTOCOL_VERSION}`,
      );
      ws.close(1002, "protocol version mismatch");
      return;
    }

    peer.deviceId = msg.device_id;
    peer.deviceName = msg.device_name;
    peer.addresses = msg.addresses ?? [];
    peer.authenticated = true;

    console.log(`${TAG} Peer authenticated: ${peer.deviceName}`);

    // Merge announced addresses into our known peer records
    this.updatePeerRecord(peer.deviceId, peer.deviceName, peer.addresses);

    // Send our hello back (with our own addresses)
    this.send(ws, {
      type: "hello",
      protocol_version: PROTOCOL_VERSION,
      device_id: this.deviceId,
      device_name: this.deviceName,
      space_id: this.spaceId,
      addresses: this.ownAddresses,
    });

    this.onPeerConnectHandler?.(peer.deviceId);

    // Next step: send our version vector
    this.sendVersionVector(ws);

    // Send peer list after a short delay to let version_vector flow first
    setTimeout(() => {
      this.sendPeerList(ws);
    }, 100);
  }

  // ---------------------------------------------------------------------------
  // Peer list exchange
  // ---------------------------------------------------------------------------

  private sendPeerList(ws: WebSocket): void {
    const msg: PeerListMessage = {
      type: "peer_list",
      peers: this.knownPeerRecords,
    };
    this.send(ws, msg);
  }

  private handlePeerList(
    _ws: WebSocket,
    peer: PeerState,
    msg: PeerListMessage,
  ): void {
    if (!peer.authenticated) return;

    const incomingPeers = msg.peers.filter(
      (p) => p.device_id !== this.deviceId,
    );
    const beforeIds = new Set(this.knownPeerRecords.map((p) => p.device_id));
    this.knownPeerRecords = mergePeerRecords(
      this.knownPeerRecords,
      incomingPeers,
    );
    this.saveKnownPeers().catch(() => {});

    // Notify about newly discovered peers we should connect to
    for (const incoming of incomingPeers) {
      if (incoming.device_id === peer.deviceId) continue; // already connected to sender
      if (this.isConnectedTo(incoming.device_id)) continue;
      const record = this.knownPeerRecords.find(
        (p) => p.device_id === incoming.device_id,
      );
      if (record) {
        this.onNewPeerDiscoveredHandler?.(record);
      }
    }

    if (this.knownPeerRecords.length > beforeIds.size) {
      console.log(
        `${TAG} Peer list updated: ${beforeIds.size} -> ${this.knownPeerRecords.length} known peers`,
      );
    }
  }

  // ---------------------------------------------------------------------------
  // Version vector exchange
  // ---------------------------------------------------------------------------

  private async sendVersionVector(ws: WebSocket): Promise<void> {
    let vector = await this.loadVersionVector();

    if (Object.keys(vector).length === 0) {
      await this.loadAllEntities(vector);
      vector = await this.loadVersionVector();
    }

    this.send(ws, { type: "version_vector", vector });
  }

  private async handleVersionVector(
    ws: WebSocket,
    peer: PeerState,
    msg: VersionVectorMessage,
  ): Promise<void> {
    if (!peer.authenticated) return;

    let localVector = await this.loadVersionVector();
    const remoteVector = msg.vector;

    if (Object.keys(localVector).length === 0) {
      const allEntities = await this.loadAllEntities(localVector);
      localVector = await this.loadVersionVector();

      if (allEntities.length > 0) {
        await this.sendBatches(ws, peer, allEntities);
      } else {
        this.send(ws, {
          type: "sync_changes",
          batch_id: generateId(),
          entities: [],
          is_last: true,
        });
      }

      peer.syncComplete = true;
      console.log(
        `${TAG} Sync complete with ${peer.deviceName} (sent our batches)`,
      );
      this.flushQueuedLiveChanges(ws, peer);
      return;
    }

    const allEntities = await this.loadAllEntities(localVector);
    const remoteNeeds = new Set<string>();

    for (const entity of allEntities) {
      const remoteHlc = remoteVector[entity.id];
      if (!remoteHlc) {
        remoteNeeds.add(entity.id);
      } else if (isNewerHlc(entity.hlc, remoteHlc)) {
        remoteNeeds.add(entity.id);
      }
    }

    if (remoteNeeds.size > 0) {
      const toSend = allEntities.filter((e) => remoteNeeds.has(e.id));
      await this.sendBatches(ws, peer, toSend);
    } else {
      this.send(ws, {
        type: "sync_changes",
        batch_id: generateId(),
        entities: [],
        is_last: true,
      });
    }

    peer.syncComplete = true;
    console.log(
      `${TAG} Sync complete with ${peer.deviceName} (sent our batches)`,
    );
    this.flushQueuedLiveChanges(ws, peer);
  }

  // ---------------------------------------------------------------------------
  // Sync batches
  // ---------------------------------------------------------------------------

  private async sendBatches(
    ws: WebSocket,
    peer: PeerState,
    entities: SyncEntity[],
  ): Promise<void> {
    const batches = splitIntoBatches(entities);
    if (batches.length === 0) {
      this.send(ws, {
        type: "sync_changes",
        batch_id: generateId(),
        entities: [],
        is_last: true,
      });
      return;
    }

    for (let i = 0; i < batches.length; i++) {
      const batchId = generateId();
      const isLast = i === batches.length - 1;
      const msg: SyncChangesMessage = {
        type: "sync_changes",
        batch_id: batchId,
        entities: batches[i],
        is_last: isLast,
      };
      this.sendWithRetry(ws, peer, msg, batchId);
    }
  }

  private async handleSyncChanges(
    ws: WebSocket,
    peer: PeerState,
    msg: SyncChangesMessage,
  ): Promise<void> {
    if (!peer.authenticated) return;

    const localVector = await this.loadVersionVector();
    let accepted = 0;

    for (const entity of msg.entities) {
      const localHlc = localVector[entity.id];

      if (!localHlc || isNewerHlc(entity.hlc, localHlc)) {
        await this.applyEntity(entity);
        localVector[entity.id] = entity.hlc;
        accepted++;

        this.onChangeHandler?.(entity);
        this.broadcastLiveChange(entity, peer.deviceId);
      }
    }

    await this.saveVersionVector(localVector);

    this.send(ws, {
      type: "sync_ack",
      batch_id: msg.batch_id,
      accepted,
    });

    if (msg.is_last) {
      console.log(`${TAG} Received all sync batches from ${peer.deviceName}`);
    }
  }

  private handleSyncAck(peer: PeerState, msg: SyncAckMessage): void {
    const pending = peer.pendingAcks.get(msg.batch_id);
    if (pending) {
      clearTimeout(pending.timer);
      peer.pendingAcks.delete(msg.batch_id);
      pending.resolve();
    }
  }

  // ---------------------------------------------------------------------------
  // Live mode
  // ---------------------------------------------------------------------------

  private flushQueuedLiveChanges(ws: WebSocket, peer: PeerState): void {
    if (peer.queuedLiveChanges.length === 0) return;

    for (const entity of peer.queuedLiveChanges) {
      const changeId = generateId();
      const msg: LiveChangeMessage = {
        type: "live_change",
        change_id: changeId,
        entity,
      };
      this.sendWithRetry(ws, peer, msg, changeId);
    }
    peer.queuedLiveChanges = [];
  }

  private async handleLiveChange(
    ws: WebSocket,
    peer: PeerState,
    msg: LiveChangeMessage,
  ): Promise<void> {
    if (!peer.authenticated) return;

    const localVector = await this.loadVersionVector();
    const localHlc = localVector[msg.entity.id];

    if (!localHlc || isNewerHlc(msg.entity.hlc, localHlc)) {
      await this.applyEntity(msg.entity);
      localVector[msg.entity.id] = msg.entity.hlc;
      await this.saveVersionVector(localVector);

      this.onChangeHandler?.(msg.entity);
      this.broadcastLiveChange(msg.entity, peer.deviceId);
    }

    this.send(ws, { type: "live_ack", change_id: msg.change_id });
  }

  private handleLiveAck(peer: PeerState, msg: LiveAckMessage): void {
    const pending = peer.pendingAcks.get(msg.change_id);
    if (pending) {
      clearTimeout(pending.timer);
      peer.pendingAcks.delete(msg.change_id);
      pending.resolve();
    }
  }

  // ---------------------------------------------------------------------------
  // Entity persistence (delegated to StorageBackend)
  // ---------------------------------------------------------------------------

  private async applyEntity(entity: SyncEntity): Promise<void> {
    await this.storage.applyEntity(entity);
  }

  // ---------------------------------------------------------------------------
  // Peer record management
  // ---------------------------------------------------------------------------

  private updatePeerRecord(
    deviceId: string,
    deviceName: string,
    addresses: string[],
  ): void {
    const newRecord: PeerRecord = {
      device_id: deviceId,
      device_name: deviceName,
      addresses,
      last_seen: new Date().toISOString(),
    };
    this.knownPeerRecords = mergePeerRecords(this.knownPeerRecords, [
      newRecord,
    ]);
    this.saveKnownPeers().catch(() => {});
  }

  // ---------------------------------------------------------------------------
  // Version vector persistence (via StorageBackend)
  // ---------------------------------------------------------------------------

  private async loadVersionVector(): Promise<VersionVector> {
    try {
      const raw = await this.storage.getKv(VERSION_VECTOR_KEY);
      return raw ? JSON.parse(raw) : {};
    } catch {
      return {};
    }
  }

  private async saveVersionVector(vector: VersionVector): Promise<void> {
    await this.storage.setKv(VERSION_VECTOR_KEY, JSON.stringify(vector));
  }

  // ---------------------------------------------------------------------------
  // Known peers persistence (via StorageBackend)
  // ---------------------------------------------------------------------------

  private async loadKnownPeers(): Promise<void> {
    try {
      const raw = await this.storage.getKv(KNOWN_PEERS_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        // Handle both old format (Record<string, {device_name, last_seen}>) and new format (PeerRecord[])
        if (Array.isArray(parsed)) {
          this.knownPeerRecords = parsed;
        } else {
          // Migrate old format
          this.knownPeerRecords = Object.entries(parsed).map(
            ([deviceId, info]) => ({
              device_id: deviceId,
              device_name: (info as { device_name: string }).device_name,
              addresses: [],
              last_seen: (info as { last_seen: string }).last_seen,
            }),
          );
        }
      } else {
        this.knownPeerRecords = [];
      }
    } catch {
      this.knownPeerRecords = [];
    }
  }

  private async saveKnownPeers(): Promise<void> {
    await this.storage.setKv(
      KNOWN_PEERS_KEY,
      JSON.stringify(this.knownPeerRecords),
    );
  }

  // ---------------------------------------------------------------------------
  // Load all entities (delegated to StorageBackend)
  // ---------------------------------------------------------------------------

  private async loadAllEntities(vector: VersionVector): Promise<SyncEntity[]> {
    const entities = await this.storage.loadEntities(vector);

    let vectorUpdated = false;
    for (const entity of entities) {
      if (!vector[entity.id]) {
        vector[entity.id] = entity.hlc;
        vectorUpdated = true;
      }
    }
    if (vectorUpdated) {
      await this.saveVersionVector(vector);
    }

    return entities;
  }

  // ---------------------------------------------------------------------------
  // Send helpers
  // ---------------------------------------------------------------------------

  private send(ws: WebSocket, msg: LanSyncMessage): void {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(serializeMessage(msg));
    }
  }

  private sendWithRetry(
    ws: WebSocket,
    peer: PeerState,
    msg: LanSyncMessage,
    ackId: string,
  ): void {
    let retries = 0;

    const trySend = () => {
      if (ws.readyState !== WebSocket.OPEN) return;
      this.send(ws, msg);

      const timer = setTimeout(
        () => {
          retries++;
          if (retries < MAX_RETRIES) {
            console.warn(
              `${TAG} Retry ${retries}/${MAX_RETRIES} for ack ${ackId}`,
            );
            trySend();
          } else {
            peer.pendingAcks.delete(ackId);
            console.error(`${TAG} Max retries reached for ack ${ackId}`);
          }
        },
        msg.type === "live_change" ? 5_000 : BATCH_ACK_TIMEOUT_MS,
      );

      peer.pendingAcks.set(ackId, { resolve: () => {}, timer, retries });
    };

    trySend();
  }

  /**
   * Update the version vector for a locally-mutated entity.
   * Called by the host application when a local change occurs.
   */
  async updateEntityHlc(entityId: string): Promise<string> {
    const vector = await this.loadVersionVector();
    const hlcStr = HLC.now(this.deviceId).toString();
    vector[entityId] = hlcStr;
    await this.saveVersionVector(vector);
    return hlcStr;
  }
}
