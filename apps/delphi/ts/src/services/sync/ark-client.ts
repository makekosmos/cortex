/**
 * Ark WebSocket sync client for Delphi web.
 *
 * Connects to the Ark relay server, exchanges version vectors,
 * sends/receives realtime changes, handles reconnection and heartbeat.
 *
 * Adapted from the Elysium ark-client.ts but uses localStorage
 * instead of SQLite for vector persistence.
 */

import type { Task } from '@/types/task';

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface ArkChange {
  event_id: string;
  change_type: 'create' | 'update' | 'delete';
  data: Record<string, unknown>;
  device_id?: string;
  device_seq?: number;
}

type MessageHandler = (change: ArkChange) => void;
type StatusHandler = (connected: boolean) => void;

// ---------------------------------------------------------------------------
// Version vector persistence (localStorage)
// ---------------------------------------------------------------------------

const VECTOR_KEY = 'delphi.sync_vector';
const DEVICE_ID_KEY = 'delphi.sync_device_id';

function loadVector(): Record<string, number> {
  try {
    const raw = localStorage.getItem(VECTOR_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveVector(vector: Record<string, number>) {
  localStorage.setItem(VECTOR_KEY, JSON.stringify(vector));
}

function getOrCreateDeviceId(): string {
  let id = localStorage.getItem(DEVICE_ID_KEY);
  if (!id) {
    id = `delphi-web-${crypto.randomUUID()}`;
    localStorage.setItem(DEVICE_ID_KEY, id);
  }
  return id;
}

// ---------------------------------------------------------------------------
// Ark settings persistence
// ---------------------------------------------------------------------------

const ARK_URL_KEY = 'delphi.ark_url';
const ARK_KEY_KEY = 'delphi.ark_api_key';

export function getArkUrl(): string {
  return localStorage.getItem(ARK_URL_KEY) ?? '';
}

export function setArkUrl(url: string) {
  localStorage.setItem(ARK_URL_KEY, url.trim().replace(/\/+$/, ''));
}

export function getArkApiKey(): string {
  return localStorage.getItem(ARK_KEY_KEY) ?? '';
}

export function setArkApiKey(key: string) {
  localStorage.setItem(ARK_KEY_KEY, key.trim());
}

// ---------------------------------------------------------------------------
// Task <-> ArkChange mapping
// ---------------------------------------------------------------------------

export function taskToArkChange(
  task: Task,
  changeType: 'create' | 'update' | 'delete',
): ArkChange {
  return {
    event_id: task.id,
    change_type: changeType,
    data: {
      event_type: 'task',
      category: 'productivity',
      source: 'delphi-web',
      source_id: task.id,
      summary: task.title,
      occurred_at: task.created_at.toISOString(),
      data: {
        id: task.id,
        title: task.title,
        description: task.description ?? null,
        completed: task.completed,
        priority: task.priority ?? 0,
        due_date: task.due_date ?? null,
        list_id: task.list_id ?? null,
        created_at: task.created_at.toISOString(),
        updated_at: task.updated_at?.toISOString() ?? null,
      },
    },
  };
}

export function arkChangeToTask(change: ArkChange): Task | null {
  const data = change.data?.data as Record<string, unknown> | undefined;
  if (!data || typeof data !== 'object') return null;

  const id = (data.id as string) || change.event_id;
  const title = data.title as string;
  if (!id || typeof title !== 'string') return null;

  return {
    id,
    title,
    description: (data.description as string | null) ?? null,
    completed: Boolean(data.completed),
    priority: Number(data.priority ?? 0),
    due_date: (data.due_date as string | null) ?? null,
    list_id: (data.list_id as string | null) ?? null,
    created_at: new Date(String(data.created_at ?? Date.now())),
    updated_at: data.updated_at
      ? new Date(String(data.updated_at))
      : undefined,
  };
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
  private vector: Record<string, number>;
  private deviceSeq: number;

  private serverUrl = '';
  private apiKey = '';
  private deviceId: string;

  private onChangeHandlers: MessageHandler[] = [];
  private onStatusHandlers: StatusHandler[] = [];

  constructor() {
    this.deviceId = getOrCreateDeviceId();
    this.vector = loadVector();
    this.deviceSeq = this.vector[this.deviceId] ?? 0;
  }

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

  connect(serverUrl: string, apiKey: string) {
    this.serverUrl = serverUrl.replace(/\/+$/, '');
    this.apiKey = apiKey;
    this.vector = loadVector();
    this.deviceSeq = this.vector[this.deviceId] ?? 0;

    this.doConnect();
  }

  disconnect() {
    this.cancelReconnect();
    if (this.ws) {
      try {
        this.ws.close(1000, 'user disconnect');
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
          type: 'change',
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

    const wsBase = this.serverUrl.replace(/^http/, 'ws');
    const url = `${wsBase}/ws/sync?key=${encodeURIComponent(this.apiKey)}`;

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
          type: 'sync_start',
          device_id: this.deviceId,
          device_name: 'Delphi Web',
          platform: 'web',
          vector: this.vector,
        }),
      );
    };

    this.ws.onmessage = (event) => {
      let msg: Record<string, unknown>;
      try {
        msg = JSON.parse(
          typeof event.data === 'string' ? event.data : '',
        );
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
      case 'sync_changes': {
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

      case 'change': {
        // Realtime change from another device
        const ch = msg as unknown as ArkChange;
        this.updateVector(ch);
        this.emitChange(ch);
        break;
      }

      case 'change_ack': {
        // Server acknowledged our change
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

      case 'sync_ack': {
        // Batch acknowledged
        break;
      }

      case 'ping': {
        // Respond with pong
        if (this.ws) {
          try {
            this.ws.send(JSON.stringify({ type: 'pong' }));
          } catch {
            // ignore
          }
        }
        break;
      }

      case 'error': {
        console.warn('[ArkSync] server error:', msg.message);
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
        console.warn('[ArkSync] handler error:', e);
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
