/**
 * ArkSync WebSocket Client -- connects to a peer's WS server.
 *
 * Equal-peer model: every device runs both a WS server and WS clients.
 * This is the client half -- connects outbound to known peers.
 *
 * Features:
 * - Takes a PeerRecord with multiple addresses
 * - Races connections to ALL addresses in parallel
 * - First successful connection = active, others cancelled
 * - Same protocol as sync-server (hello, version_vector, peer_list, batch, live)
 * - On disconnect: schedule reconnect trying all addresses again
 *
 * Generic: uses StorageBackend interface instead of app-specific persistence.
 */

import WebSocket from "ws";
import {
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
} from "./protocol";
import type { StorageBackend } from "./storage";

const TAG = "[SyncClient]";

const VERSION_VECTOR_KEY = "lan_sync.version_vector";
const CONNECT_TIMEOUT_MS = 5_000;
const RECONNECT_BASE_MS = 2_000;
const RECONNECT_MAX_MS = 30_000;

export interface SyncClientOptions {
  peer: PeerRecord;
  deviceId: string;
  deviceName: string;
  spaceId: string;
  ownAddresses: string[];
  storage: StorageBackend;
  onChange?: (entity: SyncEntity) => void;
  onConnected?: (peerDeviceId: string, peerDeviceName: string) => void;
  onDisconnected?: (peerDeviceId: string) => void;
  onPeerList?: (peers: PeerRecord[]) => void;
}

export class SyncClient {
  private ws: WebSocket | null = null;
  private options: SyncClientOptions;
  private storage: StorageBackend;
  private authenticated = false;
  private syncComplete = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectDelay = RECONNECT_BASE_MS;
  private stopped = false;
  private pendingAcks: Map<
    string,
    {
      resolve: () => void;
      timer: ReturnType<typeof setTimeout>;
      retries: number;
    }
  > = new Map();
  private queuedLiveChanges: SyncEntity[] = [];
  private pingInterval: ReturnType<typeof setInterval> | null = null;
  private peerDeviceName: string = "";

  constructor(options: SyncClientOptions) {
    this.options = options;
    this.storage = options.storage;
  }

  get isConnected(): boolean {
    return (
      this.ws !== null &&
      this.ws.readyState === WebSocket.OPEN &&
      this.authenticated
    );
  }

  get peerDeviceId(): string {
    return this.options.peer.device_id;
  }

  get peerName(): string {
    return this.peerDeviceName || this.options.peer.device_name;
  }

  /** Update the peer record (e.g. with new addresses). */
  updatePeer(peer: PeerRecord): void {
    this.options.peer = peer;
  }

  /** Start connecting to the peer. */
  start(): void {
    this.stopped = false;
    this.connect();
  }

  /** Stop the client and don't reconnect. */
  stop(): void {
    this.stopped = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.pingInterval) {
      clearInterval(this.pingInterval);
      this.pingInterval = null;
    }
    for (const ack of this.pendingAcks.values()) {
      clearTimeout(ack.timer);
    }
    this.pendingAcks.clear();
    if (this.ws) {
      try {
        this.ws.close(1000, "client stopping");
      } catch {
        /* ignore */
      }
      this.ws = null;
    }
  }

  /** Broadcast a live change through this client connection. */
  broadcastLiveChange(entity: SyncEntity): void {
    if (
      !this.ws ||
      this.ws.readyState !== WebSocket.OPEN ||
      !this.authenticated
    )
      return;

    if (!this.syncComplete) {
      this.queuedLiveChanges.push(entity);
      return;
    }

    const changeId = generateId();
    const msg: LiveChangeMessage = {
      type: "live_change",
      change_id: changeId,
      entity,
    };
    this.sendWithRetry(msg, changeId);
  }

  // ---------------------------------------------------------------------------
  // Connection: race all addresses
  // ---------------------------------------------------------------------------

  private connect(): void {
    if (this.stopped) return;

    const addresses = this.options.peer.addresses;
    if (addresses.length === 0) {
      console.warn(
        `${TAG} No addresses for peer ${this.options.peer.device_name}, scheduling reconnect`,
      );
      this.scheduleReconnect();
      return;
    }

    // Sort: last_address first (if available), then others
    const lastAddr = this.options.peer.last_address;
    const sorted = [...addresses].sort((a, b) => {
      if (a === lastAddr) return -1;
      if (b === lastAddr) return 1;
      return 0;
    });

    let connected = false;
    const candidates: WebSocket[] = [];
    let pendingCount = sorted.length;

    for (const addr of sorted) {
      // Build ws:// URL. addr may be "ip:port" or "[ipv6]:port"
      const url = `ws://${addr}`;
      try {
        const candidate = new WebSocket(url, {
          handshakeTimeout: CONNECT_TIMEOUT_MS,
        });
        candidates.push(candidate);

        candidate.on("open", () => {
          if (connected) {
            // Another candidate won the race
            try {
              candidate.close();
            } catch {
              /* ignore */
            }
            return;
          }
          connected = true;
          console.log(
            `${TAG} Connected to ${this.options.peer.device_name} via ${addr}`,
          );

          // Update last_address
          this.options.peer.last_address = addr;

          // Cancel other candidates
          for (const other of candidates) {
            if (other !== candidate) {
              try {
                other.close();
              } catch {
                /* ignore */
              }
            }
          }

          this.ws = candidate;
          this.reconnectDelay = RECONNECT_BASE_MS;
          this.setupConnection(candidate);
          this.sendHello();
        });

        candidate.on("error", () => {
          pendingCount--;
          if (!connected && pendingCount === 0) {
            console.warn(
              `${TAG} All addresses failed for ${this.options.peer.device_name}`,
            );
            this.scheduleReconnect();
          }
        });

        candidate.on("close", () => {
          if (!connected) {
            pendingCount--;
            if (pendingCount === 0) {
              this.scheduleReconnect();
            }
          }
        });
      } catch {
        pendingCount--;
      }
    }
  }

  private setupConnection(ws: WebSocket): void {
    ws.on("message", (raw) => {
      const msg = deserializeMessage(raw.toString());
      if (!msg) return;
      this.handleMessage(msg);
    });

    ws.on("close", () => {
      const wasAuthenticated = this.authenticated;
      this.authenticated = false;
      this.syncComplete = false;
      this.ws = null;

      if (this.pingInterval) {
        clearInterval(this.pingInterval);
        this.pingInterval = null;
      }

      for (const ack of this.pendingAcks.values()) {
        clearTimeout(ack.timer);
      }
      this.pendingAcks.clear();

      if (wasAuthenticated) {
        console.log(
          `${TAG} Disconnected from ${this.peerName} (${this.peerDeviceId})`,
        );
        this.options.onDisconnected?.(this.peerDeviceId);
      }

      if (!this.stopped) {
        this.scheduleReconnect();
      }
    });

    ws.on("error", (err) => {
      console.warn(
        `${TAG} Connection error with ${this.peerName}:`,
        err.message,
      );
    });

    // Send WS pings
    this.pingInterval = setInterval(() => {
      if (ws.readyState === WebSocket.OPEN) {
        ws.ping();
      }
    }, PING_INTERVAL_MS);
  }

  private scheduleReconnect(): void {
    if (this.stopped || this.reconnectTimer) return;

    const jitter = Math.random() * 1000;
    const delay = Math.min(this.reconnectDelay + jitter, RECONNECT_MAX_MS);

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.reconnectDelay = Math.min(
        this.reconnectDelay * 1.5,
        RECONNECT_MAX_MS,
      );
      this.connect();
    }, delay);
  }

  // ---------------------------------------------------------------------------
  // Protocol
  // ---------------------------------------------------------------------------

  private sendHello(): void {
    const msg: HelloMessage = {
      type: "hello",
      protocol_version: PROTOCOL_VERSION,
      device_id: this.options.deviceId,
      device_name: this.options.deviceName,
      space_id: this.options.spaceId,
      addresses: this.options.ownAddresses,
    };
    this.send(msg);
  }

  private handleMessage(msg: LanSyncMessage): void {
    switch (msg.type) {
      case "hello":
        this.handleHello(msg);
        break;
      case "version_vector":
        this.handleVersionVector(msg);
        break;
      case "sync_changes":
        this.handleSyncChanges(msg);
        break;
      case "sync_ack":
        this.handleSyncAck(msg);
        break;
      case "live_change":
        this.handleLiveChange(msg);
        break;
      case "live_ack":
        this.handleLiveAck(msg);
        break;
      case "peer_list":
        this.handlePeerList(msg);
        break;
      case "ping":
        this.send({ type: "pong", ts: msg.ts });
        break;
      case "pong":
        break;
    }
  }

  private handleHello(msg: HelloMessage): void {
    this.authenticated = true;
    this.peerDeviceName = msg.device_name;
    console.log(
      `${TAG} Authenticated with ${msg.device_name} (${msg.device_id})`,
    );
    this.options.onConnected?.(msg.device_id, msg.device_name);

    // Send our version vector
    this.sendVersionVector();
  }

  private async sendVersionVector(): Promise<void> {
    let vector = await this.loadVersionVector();

    if (Object.keys(vector).length === 0) {
      await this.loadAllEntities(vector);
      vector = await this.loadVersionVector();
    }

    this.send({ type: "version_vector", vector });
  }

  private async handleVersionVector(msg: VersionVectorMessage): Promise<void> {
    if (!this.authenticated) return;

    let localVector = await this.loadVersionVector();
    const remoteVector = msg.vector;

    if (Object.keys(localVector).length === 0) {
      const allEntities = await this.loadAllEntities(localVector);
      localVector = await this.loadVersionVector();

      if (allEntities.length > 0) {
        await this.sendBatches(allEntities);
      } else {
        this.send({
          type: "sync_changes",
          batch_id: generateId(),
          entities: [],
          is_last: true,
        });
      }

      this.syncComplete = true;
      this.flushQueuedLiveChanges();
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
      await this.sendBatches(toSend);
    } else {
      this.send({
        type: "sync_changes",
        batch_id: generateId(),
        entities: [],
        is_last: true,
      });
    }

    this.syncComplete = true;
    this.flushQueuedLiveChanges();
  }

  private async handleSyncChanges(msg: SyncChangesMessage): Promise<void> {
    if (!this.authenticated) return;

    const localVector = await this.loadVersionVector();
    let accepted = 0;

    for (const entity of msg.entities) {
      const localHlc = localVector[entity.id];

      if (!localHlc || isNewerHlc(entity.hlc, localHlc)) {
        await this.applyEntity(entity);
        if (entity.deleted) {
          delete localVector[entity.id];
        } else {
          localVector[entity.id] = entity.hlc;
        }
        accepted++;

        this.options.onChange?.(entity);
      }
    }

    await this.saveVersionVector(localVector);

    this.send({
      type: "sync_ack",
      batch_id: msg.batch_id,
      accepted,
    });

    if (msg.is_last) {
      console.log(`${TAG} Received all sync batches from ${this.peerName}`);
    }
  }

  private handleSyncAck(msg: SyncAckMessage): void {
    const pending = this.pendingAcks.get(msg.batch_id);
    if (pending) {
      clearTimeout(pending.timer);
      this.pendingAcks.delete(msg.batch_id);
      pending.resolve();
    }
  }

  private async handleLiveChange(msg: LiveChangeMessage): Promise<void> {
    if (!this.authenticated) return;

    const localVector = await this.loadVersionVector();
    const localHlc = localVector[msg.entity.id];

    if (!localHlc || isNewerHlc(msg.entity.hlc, localHlc)) {
      await this.applyEntity(msg.entity);
      if (msg.entity.deleted) {
        delete localVector[msg.entity.id];
      } else {
        localVector[msg.entity.id] = msg.entity.hlc;
      }
      await this.saveVersionVector(localVector);

      this.options.onChange?.(msg.entity);
    }

    this.send({ type: "live_ack", change_id: msg.change_id });
  }

  private handleLiveAck(msg: LiveAckMessage): void {
    const pending = this.pendingAcks.get(msg.change_id);
    if (pending) {
      clearTimeout(pending.timer);
      this.pendingAcks.delete(msg.change_id);
      pending.resolve();
    }
  }

  private handlePeerList(msg: PeerListMessage): void {
    if (!this.authenticated) return;
    this.options.onPeerList?.(msg.peers);
  }

  // ---------------------------------------------------------------------------
  // Batches
  // ---------------------------------------------------------------------------

  private async sendBatches(entities: SyncEntity[]): Promise<void> {
    const batches = splitIntoBatches(entities);
    if (batches.length === 0) {
      this.send({
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
      this.sendWithRetry(msg, batchId);
    }
  }

  private flushQueuedLiveChanges(): void {
    if (this.queuedLiveChanges.length === 0) return;
    for (const entity of this.queuedLiveChanges) {
      const changeId = generateId();
      const msg: LiveChangeMessage = {
        type: "live_change",
        change_id: changeId,
        entity,
      };
      this.sendWithRetry(msg, changeId);
    }
    this.queuedLiveChanges = [];
  }

  // ---------------------------------------------------------------------------
  // Entity persistence — delegated to StorageBackend
  // ---------------------------------------------------------------------------

  private async applyEntity(entity: SyncEntity): Promise<void> {
    await this.storage.applyEntity(entity);
  }

  // ---------------------------------------------------------------------------
  // Version vector
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
  // Load all entities — delegated to StorageBackend
  // ---------------------------------------------------------------------------

  private async loadAllEntities(vector: VersionVector): Promise<SyncEntity[]> {
    return this.storage.loadEntities(vector);
  }

  // ---------------------------------------------------------------------------
  // Send helpers
  // ---------------------------------------------------------------------------

  private send(msg: LanSyncMessage): void {
    if (this.ws && this.ws.readyState === WebSocket.OPEN) {
      this.ws.send(serializeMessage(msg));
    }
  }

  private sendWithRetry(msg: LanSyncMessage, ackId: string): void {
    let retries = 0;

    const trySend = () => {
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
      this.send(msg);

      const timer = setTimeout(
        () => {
          retries++;
          if (retries < MAX_RETRIES) {
            trySend();
          } else {
            this.pendingAcks.delete(ackId);
          }
        },
        msg.type === "live_change" ? 5_000 : BATCH_ACK_TIMEOUT_MS,
      );

      this.pendingAcks.set(ackId, { resolve: () => {}, timer, retries });
    };

    trySend();
  }
}
