/**
 * Ark WebSocket sync client for Olympia.
 *
 * Connects to the Ark relay server, exchanges version vectors,
 * sends/receives realtime changes, handles reconnection and heartbeat.
 */

import AsyncStorage from "@react-native-async-storage/async-storage";

// ---------------------------------------------------------------------------

// Types

// ---------------------------------------------------------------------------

export interface ArkChange {
  event_id: string;

  change_type: "create" | "update" | "delete";

  data: Record<string, unknown>;

  device_id?: string;

  device_seq?: number;
}

type MessageHandler = (change: ArkChange) => void;

type StatusHandler = (connected: boolean) => void;

// ---------------------------------------------------------------------------

// Version vector persistence (AsyncStorage)

// ---------------------------------------------------------------------------

const VECTOR_KEY = "ark_sync_vector";

const DEVICE_ID_KEY = "ark_sync_device_id";

let _cachedVector: Record<string, number> = {};

async function loadVector(): Promise<Record<string, number>> {
  try {
    const raw = await AsyncStorage.getItem(VECTOR_KEY);

    _cachedVector = raw ? JSON.parse(raw) : {};
  } catch {
    _cachedVector = {};
  }

  return _cachedVector;
}

function getVectorSync(): Record<string, number> {
  return _cachedVector;
}

function saveVector(vector: Record<string, number>) {
  _cachedVector = vector;

  AsyncStorage.setItem(VECTOR_KEY, JSON.stringify(vector)).catch(() => {});
}

// ---------------------------------------------------------------------------

// Client

// ---------------------------------------------------------------------------

const MAX_BACKOFF = 30_000;

export class ArkSyncClient {
  private ws: WebSocket | null = null;

  private _connected = false;

  private _synced = false;

  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  private backoff = 1000;

  private vector: Record<string, number> = {};

  private deviceSeq = 0;

  private serverUrl = "";

  private apiKey = "";

  private deviceId = "";

  private deviceName = "";

  private onChangeHandlers: MessageHandler[] = [];

  private onStatusHandlers: StatusHandler[] = [];

  // -- Public API ----------------------------------------------------------

  get isConnected(): boolean {
    return this._connected;
  }

  get isSynced(): boolean {
    return this._synced;
  }

  onChange(handler: MessageHandler) {
    this.onChangeHandlers.push(handler);

    return () => {
      this.onChangeHandlers = this.onChangeHandlers.filter(
        (h) => h !== handler,
      );
    };
  }

  onStatus(handler: StatusHandler) {
    this.onStatusHandlers.push(handler);

    return () => {
      this.onStatusHandlers = this.onStatusHandlers.filter(
        (h) => h !== handler,
      );
    };
  }

  async connect(
    serverUrl: string,
    apiKey: string,
    deviceId: string,
    deviceName: string,
  ) {
    this.serverUrl = serverUrl;

    this.apiKey = apiKey;

    this.deviceId = deviceId;

    this.deviceName = deviceName;

    this.vector = await loadVector();

    this.deviceSeq = this.vector[deviceId] ?? 0;

    this.doConnect();
  }

  disconnect() {
    this.cancelReconnect();

    if (this.ws) {
      try {
        this.ws.close(1000, "user disconnect");
      } catch {
        // ignore
      }

      this.ws = null;
    }

    this.setConnected(false);

    this._synced = false;
  }

  /**
   * Send a single realtime change to the server.
   * Returns false if not connected (caller should queue for later).
   */

  sendChange(change: ArkChange): boolean {
    if (!this.ws || !this._connected) return false;

    this.deviceSeq += 1;

    this.vector[this.deviceId] = this.deviceSeq;

    saveVector(this.vector);

    try {
      this.ws.send(
        JSON.stringify({
          type: "change",

          event_id: change.event_id,

          change_type: change.change_type,

          data: change.data,
        }),
      );

      return true;
    } catch {
      return false;
    }
  }

  // -- Internal ------------------------------------------------------------

  private doConnect() {
    this.cancelReconnect();

    const base = this.serverUrl.replace(/\/$/, "");

    const url = `${base}/ws/sync?key=${encodeURIComponent(this.apiKey)}`;

    try {
      this.ws = new WebSocket(url);
    } catch {
      this.scheduleReconnect();

      return;
    }

    this.ws.onopen = () => {
      this.backoff = 1000;

      // Send sync_start

      this.ws!.send(
        JSON.stringify({
          type: "sync_start",

          device_id: this.deviceId,

          device_name: this.deviceName,

          platform: "react-native",

          vector: this.vector,
        }),
      );
    };

    this.ws.onmessage = (event) => {
      let msg: Record<string, unknown>;

      try {
        msg = JSON.parse(typeof event.data === "string" ? event.data : "");
      } catch {
        return;
      }

      this.handleMessage(msg);
    };

    this.ws.onerror = () => {
      // onclose will fire after this
    };

    this.ws.onclose = () => {
      this.ws = null;

      this.setConnected(false);

      this._synced = false;

      this.scheduleReconnect();
    };
  }

  private handleMessage(msg: Record<string, unknown>) {
    const type = msg.type as string;

    switch (type) {
      case "sync_changes": {
        // Initial batch of missed changes

        const changes = (msg.changes as ArkChange[]) ?? [];

        for (const ch of changes) {
          this.updateVector(ch);

          this.emitChange(ch);
        }

        this.setConnected(true);

        this._synced = true;

        break;
      }

      case "change": {
        // Realtime change from another device

        const ch = msg as unknown as ArkChange;

        this.updateVector(ch);

        this.emitChange(ch);

        break;
      }

      case "change_ack": {
        // Server acknowledged our change — seq confirmed

        const seq = msg.device_seq as number | undefined;

        if (seq != null) {
          this.vector[this.deviceId] = Math.max(
            this.vector[this.deviceId] ?? 0,
            seq,
          );

          saveVector(this.vector);
        }

        break;
      }

      case "sync_ack": {
        // Batch acknowledged

        break;
      }

      case "ping": {
        // Respond with pong

        if (this.ws) {
          try {
            this.ws.send(JSON.stringify({ type: "pong" }));
          } catch {
            // ignore
          }
        }

        break;
      }

      case "error": {
        console.warn("[ArkSync] server error:", msg.message);

        break;
      }
    }
  }

  private updateVector(change: ArkChange) {
    const did = change.device_id;

    const seq = change.device_seq;

    if (did && seq != null) {
      this.vector[did] = Math.max(this.vector[did] ?? 0, seq);

      saveVector(this.vector);
    }
  }

  private emitChange(change: ArkChange) {
    for (const h of this.onChangeHandlers) {
      try {
        h(change);
      } catch (e) {
        console.warn("[ArkSync] handler error:", e);
      }
    }
  }

  private setConnected(value: boolean) {
    if (this._connected !== value) {
      this._connected = value;

      for (const h of this.onStatusHandlers) h(value);
    }
  }

  private scheduleReconnect() {
    if (this.reconnectTimer) return;

    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;

      this.doConnect();
    }, this.backoff);

    this.backoff = Math.min(this.backoff * 2, MAX_BACKOFF);
  }

  private cancelReconnect() {
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);

      this.reconnectTimer = null;
    }
  }
}

/** Singleton instance used across the app. */

export const arkSync = new ArkSyncClient();
