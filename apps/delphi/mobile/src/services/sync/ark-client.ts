/**
 * Ark WebSocket sync client for Delphi Mobile.
 *
 * Adapted from the Electron version. Uses in-memory state + SQLite
 * persistence via the store layer (no localStorage).
 */

import { randomUUID } from "expo-crypto";
import type { TodoItem, Project } from "@/types/task";
import { Priority, ProjectStatus } from "@/types/task";

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
// TodoItem <-> ArkChange mapping
// ---------------------------------------------------------------------------

export function todoToArkChange(
  todo: TodoItem,
  changeType: "create" | "update" | "delete",
): ArkChange {
  return {
    event_id: todo.id,
    change_type: changeType,
    data: {
      event_type: "task",
      category: "productivity",
      source: "delphi-mobile",
      source_id: todo.id,
      summary: todo.title,
      occurred_at: todo.createdAt,
      data: { ...todo },
    },
  };
}

export function arkChangeToTodoItem(change: ArkChange): TodoItem | null {
  const outerData = change.data as Record<string, unknown> | undefined;
  const data = outerData?.data as Record<string, unknown> | undefined;
  if (!data || typeof data !== "object") return null;

  const eventType = outerData?.event_type as string | undefined;
  if (eventType && eventType !== "task" && eventType !== "task_created")
    return null;

  const id =
    (data.id as string) || (outerData?.source_id as string) || change.event_id;
  const title = (data.title as string) ?? (outerData?.summary as string);
  if (!id || typeof title !== "string") return null;

  return {
    id,
    title,
    notes:
      (data.notes as string | null) ??
      (data.description as string | null) ??
      null,
    priority: Number(data.priority ?? Priority.None) as Priority,
    scheduledDate: (data.scheduledDate as string | null) ?? null,
    deadline:
      (data.deadline as string | null) ??
      (data.due_date as string | null) ??
      null,
    reminderDate: (data.reminderDate as string | null) ?? null,
    isToday: Boolean(data.isToday ?? false),
    isEvening: Boolean(data.isEvening ?? false),
    isSomeday: Boolean(data.isSomeday ?? false),
    isCompleted: Boolean(data.isCompleted ?? data.completed ?? false),
    completedAt: (data.completedAt as string | null) ?? null,
    isCancelled: Boolean(data.isCancelled ?? false),
    cancelledAt: (data.cancelledAt as string | null) ?? null,
    isTrashed: Boolean(data.isTrashed ?? false),
    sortOrder: Number(data.sortOrder ?? 0),
    createdAt:
      (data.createdAt as string) ??
      (data.created_at as string) ??
      new Date().toISOString(),
    headingId: (data.headingId as string | null) ?? null,
    projectId:
      (data.projectId as string | null) ??
      (data.list_id as string | null) ??
      null,
    areaId: (data.areaId as string | null) ?? null,
    tagIds: Array.isArray(data.tagIds) ? (data.tagIds as string[]) : [],
    checklistItems: Array.isArray(data.checklistItems)
      ? (data.checklistItems as TodoItem["checklistItems"])
      : [],
    recurrenceRule: (data.recurrenceRule as TodoItem["recurrenceRule"]) ?? null,
  };
}

// ---------------------------------------------------------------------------
// Project <-> ArkChange mapping
// ---------------------------------------------------------------------------

export function projectToArkChange(
  project: Project,
  changeType: "create" | "update" | "delete",
): ArkChange {
  return {
    event_id: project.id,
    change_type: changeType,
    data: {
      event_type: "project",
      category: "productivity",
      source: "delphi-mobile",
      source_id: project.id,
      summary: project.title,
      occurred_at: project.createdAt,
      data: { ...project },
    },
  };
}

export function arkChangeToProject(change: ArkChange): Project | null {
  const outerData = change.data as Record<string, unknown> | undefined;
  const data = outerData?.data as Record<string, unknown> | undefined;
  if (!data || typeof data !== "object") return null;

  const eventType = outerData?.event_type as string | undefined;
  if (eventType !== "project") return null;

  const id =
    (data.id as string) || (outerData?.source_id as string) || change.event_id;
  const title = (data.title as string) ?? (outerData?.summary as string);
  if (!id || typeof title !== "string") return null;

  return {
    id,
    title,
    notes: (data.notes as string | null) ?? null,
    status: (data.status as ProjectStatus) ?? ProjectStatus.Active,
    scheduledDate: (data.scheduledDate as string | null) ?? null,
    deadline: (data.deadline as string | null) ?? null,
    sortOrder: Number(data.sortOrder ?? 0),
    colorTag: (data.colorTag as string | null) ?? null,
    createdAt: (data.createdAt as string) ?? new Date().toISOString(),
    areaId: (data.areaId as string | null) ?? null,
  };
}

export function arkChangeEventType(change: ArkChange): string | undefined {
  const outerData = change.data as Record<string, unknown> | undefined;
  return outerData?.event_type as string | undefined;
}

// ---------------------------------------------------------------------------
// Fetch tasks from Ark via HTTP
// ---------------------------------------------------------------------------

export async function fetchTasksFromArk(
  url: string,
  key: string,
): Promise<TodoItem[]> {
  if (!url || !key) return [];

  try {
    const resp = await fetch(`${url}/events?limit=1000`, {
      headers: { "X-API-Key": key },
    });
    if (!resp.ok) return [];
    const events: Record<string, unknown>[] = await resp.json();

    const todos: TodoItem[] = [];
    for (const event of events) {
      const evtType = event.event_type as string | undefined;
      if (evtType && evtType !== "task" && evtType !== "task_created") continue;

      const dataRaw = event.data as Record<string, unknown> | undefined;
      if (!dataRaw || typeof dataRaw !== "object") continue;

      const sourceId = event.source_id as string | undefined;
      const id = (dataRaw.id as string) || sourceId || (event.id as string);
      const title = (dataRaw.title as string) ?? (event.summary as string);
      if (!id || typeof title !== "string") continue;

      todos.push({
        id,
        title,
        notes:
          (dataRaw.notes as string | null) ??
          (dataRaw.description as string | null) ??
          null,
        priority: Number(dataRaw.priority ?? Priority.None) as Priority,
        scheduledDate: (dataRaw.scheduledDate as string | null) ?? null,
        deadline:
          (dataRaw.deadline as string | null) ??
          (dataRaw.due_date as string | null) ??
          null,
        reminderDate: (dataRaw.reminderDate as string | null) ?? null,
        isToday: Boolean(dataRaw.isToday ?? false),
        isEvening: Boolean(dataRaw.isEvening ?? false),
        isSomeday: Boolean(dataRaw.isSomeday ?? false),
        isCompleted: Boolean(
          dataRaw.isCompleted ?? dataRaw.completed ?? false,
        ),
        completedAt: (dataRaw.completedAt as string | null) ?? null,
        isCancelled: Boolean(dataRaw.isCancelled ?? false),
        cancelledAt: (dataRaw.cancelledAt as string | null) ?? null,
        isTrashed: Boolean(dataRaw.isTrashed ?? false),
        sortOrder: Number(dataRaw.sortOrder ?? 0),
        createdAt:
          (dataRaw.createdAt as string) ??
          (dataRaw.created_at as string) ??
          (event.occurred_at as string) ??
          new Date().toISOString(),
        headingId: (dataRaw.headingId as string | null) ?? null,
        projectId:
          (dataRaw.projectId as string | null) ??
          (dataRaw.list_id as string | null) ??
          null,
        areaId: (dataRaw.areaId as string | null) ?? null,
        tagIds: Array.isArray(dataRaw.tagIds)
          ? (dataRaw.tagIds as string[])
          : [],
        checklistItems: Array.isArray(dataRaw.checklistItems)
          ? (dataRaw.checklistItems as TodoItem["checklistItems"])
          : [],
        recurrenceRule:
          (dataRaw.recurrenceRule as TodoItem["recurrenceRule"]) ?? null,
      });
    }
    console.log(`[ArkSync] Fetched ${todos.length} tasks from Ark HTTP API`);
    return todos;
  } catch (e) {
    console.warn("[ArkSync] Failed to fetch tasks from Ark:", e);
    return [];
  }
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
  private deviceId: string;

  private onChangeHandlers: MessageHandler[] = [];
  private onStatusHandlers: StatusHandler[] = [];
  private outbox: ArkChange[] = [];

  constructor() {
    this.deviceId = `delphi-mobile-${randomUUID()}`;
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
    if (
      this.ws &&
      (this.ws.readyState === WebSocket.CONNECTING ||
        this.ws.readyState === WebSocket.OPEN)
    ) {
      return;
    }

    this.serverUrl = serverUrl.replace(/\/+$/, "");
    this.apiKey = apiKey;
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

  sendChange(change: ArkChange): boolean {
    if (!this.ws || !this._connected) {
      this.outbox.push(change);
      return false;
    }

    this.deviceSeq += 1;
    this.vector[this.deviceId] = this.deviceSeq;

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
      this.outbox.push(change);
      return false;
    }
  }

  // -- Internal ------------------------------------------------------------

  private flushOutbox() {
    if (this.outbox.length === 0) return;
    const pending = [...this.outbox];
    this.outbox = [];
    for (const change of pending) {
      this.sendChange(change);
    }
  }

  private doConnect() {
    this.cancelReconnect();

    const wsBase = this.serverUrl.replace(/^http/, "ws");
    const url = `${wsBase}/ws/sync?key=${encodeURIComponent(this.apiKey)}`;

    console.log("[ArkSync] Connecting to", url);

    try {
      this.ws = new WebSocket(url);
    } catch (e) {
      console.error("[ArkSync] WebSocket constructor failed:", e);
      this.scheduleReconnect();
      return;
    }

    this.ws.onopen = () => {
      this.backoff = 1000;
      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) return;
      console.log("[ArkSync] WebSocket opened, sending sync_start");
      this.ws.send(
        JSON.stringify({
          type: "sync_start",
          device_id: this.deviceId,
          device_name: "Delphi Mobile",
          platform: "react-native",
          vector: this.vector,
        }),
      );
    };

    this.ws.onmessage = (event) => {
      let msg: Record<string, unknown>;
      try {
        msg = JSON.parse(
          typeof event.data === "string" ? event.data : "",
        );
      } catch {
        return;
      }
      this.handleMessage(msg);
    };

    this.ws.onerror = () => {
      // Silently handle — onclose will fire next and trigger reconnect
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
        const changes = (msg.changes as ArkChange[]) ?? [];
        console.log(
          `[ArkSync] Received sync_changes: ${changes.length} changes`,
        );
        for (const ch of changes) {
          this.updateVector(ch);
          this.emitChange(ch);
        }
        this.setConnected(true);
        this._synced = true;
        this.flushOutbox();
        break;
      }

      case "change": {
        const ch = msg as unknown as ArkChange;
        this.updateVector(ch);
        this.emitChange(ch);
        break;
      }

      case "change_ack": {
        const seq = msg.device_seq as number | undefined;
        if (seq != null) {
          this.vector[this.deviceId] = Math.max(
            this.vector[this.deviceId] ?? 0,
            seq,
          );
        }
        break;
      }

      case "ping": {
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

/** Singleton instance. */
export const arkSync = new ArkSyncClient();
