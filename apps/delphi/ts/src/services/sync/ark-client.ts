/* eslint-disable no-console */

/**
 * Ark WebSocket sync client for Delphi web.
 *
 * Connects to the Ark relay server, exchanges version vectors,
 * sends/receives realtime changes, handles reconnection and heartbeat.
 *
 * Adapted from the Elysium ark-client.ts but uses localStorage
 * instead of SQLite for vector persistence.
 */

import { normalizeApiUrl } from "@/helpers/normalize";

import type { Project, Task, TodoItem } from "@/types/task";

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

type TodoResolver = (id: string) => TodoItem | undefined;

type AllTodosResolver = () => TodoItem[];

type FullSyncHandler = (
  serverTaskIds: Set<string>,

  outboxTaskIds: Set<string>,
) => void;

// ---------------------------------------------------------------------------

// Version vector persistence (localStorage)

// ---------------------------------------------------------------------------

const VECTOR_KEY = "delphi.sync_vector";

const DEVICE_ID_KEY = "delphi.sync_device_id";

const SERVER_EPOCH_KEY = "delphi.server_epoch";

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

const ARK_URL_KEY = "delphi.ark_url";

const ARK_KEY_KEY = "delphi.ark_api_key";

export function getArkUrl(): string {
  return localStorage.getItem(ARK_URL_KEY) ?? "";
}

export function setArkUrl(url: string) {
  localStorage.setItem(ARK_URL_KEY, normalizeApiUrl(url));
}

export function getArkApiKey(): string {
  return localStorage.getItem(ARK_KEY_KEY) ?? "";
}

export function setArkApiKey(key: string) {
  localStorage.setItem(ARK_KEY_KEY, key.trim());
}

// ---------------------------------------------------------------------------

// TodoItem <-> ArkChange mapping (GTD model — used by todos store)

// ---------------------------------------------------------------------------

export function todoItemToArkChange(
  todo: TodoItem,

  changeType: "create" | "update" | "delete",
): ArkChange {
  return {
    event_id: todo.id,

    change_type: changeType,

    data: {
      event_type: "task",

      category: "productivity",

      source: "delphi-web",

      source_id: todo.id,

      summary: todo.title,

      occurred_at: todo.createdAt,

      data: {
        id: todo.id,

        title: todo.title,

        notes: todo.notes ?? null,

        priority: todo.priority,

        scheduledDate: todo.scheduledDate ?? null,

        deadline: todo.deadline ?? null,

        reminderDate: todo.reminderDate ?? null,

        isToday: todo.isToday,

        isEvening: todo.isEvening,

        isSomeday: todo.isSomeday,

        isCompleted: todo.isCompleted,

        completedAt: todo.completedAt ?? null,

        isCancelled: todo.isCancelled,

        cancelledAt: todo.cancelledAt ?? null,

        isTrashed: todo.isTrashed,

        sortOrder: todo.sortOrder,

        headingId: todo.headingId ?? null,

        projectId: todo.projectId ?? null,

        areaId: todo.areaId ?? null,

        tagIds: todo.tagIds,

        checklistItems: todo.checklistItems,

        recurrenceRule: todo.recurrenceRule ?? null,

        createdAt: todo.createdAt,
      },
    },
  };
}

// ---------------------------------------------------------------------------

// Task <-> ArkChange mapping

// ---------------------------------------------------------------------------

export function taskToArkChange(
  task: Task,

  changeType: "create" | "update" | "delete",
): ArkChange {
  return {
    event_id: task.id,

    change_type: changeType,

    data: {
      event_type: "task",

      category: "productivity",

      source: "delphi-web",

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
  // The inner data dict (task fields) is at change.data.data for both Swift and TS formats

  const outerData = change.data as Record<string, unknown> | undefined;

  const data = outerData?.data as Record<string, unknown> | undefined;

  if (!data || typeof data !== "object") return null;

  // Only process task events (accept both "task" and "task_created" for compat)

  const eventType = outerData?.event_type as string | undefined;

  if (eventType && eventType !== "task" && eventType !== "task_created")
    return null;

  const rawId =
    (data.id as string) || (outerData?.source_id as string) || change.event_id;

  const title = (data.title as string) ?? (outerData?.summary as string);

  if (!rawId || typeof title !== "string") return null;

  const id = rawId.toLowerCase();

  return {
    id,

    title,

    // Swift sends "notes", TS sends "description"

    description:
      (data.description as string | null) ??
      (data.notes as string | null) ??
      null,

    // Swift sends "isCompleted", TS sends "completed"

    completed: Boolean(data.completed ?? data.isCompleted ?? false),

    priority: Number(data.priority ?? 0),

    due_date:
      (data.due_date as string | null) ??
      (data.deadline as string | null) ??
      null,

    list_id: (data.list_id as string | null) ?? null,

    created_at: new Date(
      String(data.created_at ?? data.createdAt ?? Date.now()),
    ),

    updated_at:
      (data.updated_at ?? data.updatedAt)
        ? new Date(String(data.updated_at ?? data.updatedAt))
        : undefined,
  };
}

/**
 * Convert an ArkChange to a TodoItem (new GTD model).
 * Swift clients send all GTD fields (isToday, isSomeday, etc.) in data.data.
 */

export function arkChangeToTodoItem(change: ArkChange): TodoItem | null {
  const outerData = change.data as Record<string, unknown> | undefined;

  const data = outerData?.data as Record<string, unknown> | undefined;

  if (!data || typeof data !== "object") return null;

  const eventType = outerData?.event_type as string | undefined;

  if (eventType && eventType !== "task" && eventType !== "task_created")
    return null;

  const rawId =
    (data.id as string) || (outerData?.source_id as string) || change.event_id;

  const title = (data.title as string) ?? (outerData?.summary as string);

  if (!rawId || typeof title !== "string") return null;

  // Normalize UUID to lowercase — Swift sends uppercase, Android/Web lowercase

  const id = rawId.toLowerCase();

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

      source: "delphi-web",

      source_id: project.id,

      summary: project.title,

      occurred_at: project.createdAt,

      data: {
        id: project.id,

        title: project.title,

        notes: project.notes ?? null,

        status: project.status,

        scheduledDate: project.scheduledDate ?? null,

        deadline: project.deadline ?? null,

        sortOrder: project.sortOrder,

        colorTag: project.colorTag ?? null,

        createdAt: project.createdAt,

        areaId: project.areaId ?? null,
      },
    },
  };
}

export function arkChangeToProject(change: ArkChange): Project | null {
  const outerData = change.data as Record<string, unknown> | undefined;

  const data = outerData?.data as Record<string, unknown> | undefined;

  if (!data || typeof data !== "object") return null;

  const eventType = outerData?.event_type as string | undefined;

  if (eventType !== "project") return null;

  const rawId =
    (data.id as string) || (outerData?.source_id as string) || change.event_id;

  const title = (data.title as string) ?? (outerData?.summary as string);

  if (!rawId || typeof title !== "string") return null;

  const id = rawId.toLowerCase();

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

/**
 * Returns the event_type from an ArkChange's outer data envelope.
 */

export function arkChangeEventType(change: ArkChange): string | undefined {
  const outerData = change.data as Record<string, unknown> | undefined;

  return outerData?.event_type as string | undefined;
}

// ---------------------------------------------------------------------------

// Fetch tasks from Ark via HTTP (for initial load in Electron)

// ---------------------------------------------------------------------------

export async function fetchTasksFromArk(): Promise<TodoItem[]> {
  const url = getArkUrl();

  const key = getArkApiKey();

  if (!url || !key) return [];

  try {
    const resp = await fetch(`${url}/events?limit=1000`, {
      headers: { "X-API-Key": key },
    });

    if (!resp.ok) return [];

    const events: Record<string, unknown>[] = await resp.json();

    // Deduplicate by lowercase id — events table may have the same logical task

    // stored under multiple sources (delphi, delphi-android, null, …).

    const todoMap = new Map<string, TodoItem>();

    for (const event of events) {
      const evtType = event.event_type as string | undefined;

      if (evtType && evtType !== "task" && evtType !== "task_created") continue;

      const dataRaw = event.data as Record<string, unknown> | undefined;

      if (!dataRaw || typeof dataRaw !== "object") continue;

      const sourceId = event.source_id as string | undefined;

      const id = (dataRaw.id as string) || sourceId || (event.id as string);

      const title = (dataRaw.title as string) ?? (event.summary as string);

      if (!id || typeof title !== "string") continue;

      const todo: TodoItem = {
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

        isCompleted: Boolean(dataRaw.isCompleted ?? dataRaw.completed ?? false),

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
      };

      // Deduplicate: prefer completed/cancelled over not-completed for same id

      const dedupeKey = id.toLowerCase();

      const existing = todoMap.get(dedupeKey);

      if (!existing || todo.isCompleted || todo.isCancelled) {
        todoMap.set(dedupeKey, todo);
      }
    }

    const todos = Array.from(todoMap.values());

    console.log(
      `[ArkSync] Fetched ${todos.length} unique tasks from Ark HTTP API`,
    );

    return todos;
  } catch (e) {
    console.warn("[ArkSync] Failed to fetch tasks from Ark:", e);

    return [];
  }
}

// ---------------------------------------------------------------------------

// Fetch projects from Ark via HTTP (for initial load in Electron)

// ---------------------------------------------------------------------------

export async function fetchProjectsFromArk(): Promise<Project[]> {
  const url = getArkUrl();

  const key = getArkApiKey();

  if (!url || !key) return [];

  try {
    const resp = await fetch(`${url}/events?limit=1000`, {
      headers: { "X-API-Key": key },
    });

    if (!resp.ok) return [];

    const events: Record<string, unknown>[] = await resp.json();

    const projects: Project[] = [];

    for (const event of events) {
      const evtType = event.event_type as string | undefined;

      if (evtType !== "project") continue;

      const dataRaw = event.data as Record<string, unknown> | undefined;

      if (!dataRaw || typeof dataRaw !== "object") continue;

      const sourceId = event.source_id as string | undefined;

      const id = (dataRaw.id as string) || sourceId || (event.id as string);

      const title = (dataRaw.title as string) ?? (event.summary as string);

      if (!id || typeof title !== "string") continue;

      projects.push({
        id,

        title,

        notes: (dataRaw.notes as string | null) ?? null,

        status: (dataRaw.status as ProjectStatus) ?? ProjectStatus.Active,

        scheduledDate: (dataRaw.scheduledDate as string | null) ?? null,

        deadline: (dataRaw.deadline as string | null) ?? null,

        sortOrder: Number(dataRaw.sortOrder ?? 0),

        colorTag: (dataRaw.colorTag as string | null) ?? null,

        createdAt:
          (dataRaw.createdAt as string) ??
          (event.occurred_at as string) ??
          new Date().toISOString(),

        areaId: (dataRaw.areaId as string | null) ?? null,
      });
    }

    console.log(
      `[ArkSync] Fetched ${projects.length} projects from Ark HTTP API`,
    );

    return projects;
  } catch (e) {
    console.warn("[ArkSync] Failed to fetch projects from Ark:", e);

    return [];
  }
}

// ---------------------------------------------------------------------------

// Client

// ---------------------------------------------------------------------------

const MAX_BACKOFF = 30_000;

const OUTBOX_KEY = "delphi.sync_outbox";

function loadOutbox(): ArkChange[] {
  try {
    const raw = localStorage.getItem(OUTBOX_KEY);

    return raw ? JSON.parse(raw) : [];
  } catch {
    return [];
  }
}

function saveOutbox(outbox: ArkChange[]) {
  localStorage.setItem(OUTBOX_KEY, JSON.stringify(outbox));
}

export class ArkSyncClient {
  private ws: WebSocket | null = null;

  private _connected = false;

  private _synced = false;

  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  private backoff = 1000;

  private vector: Record<string, number>;

  private deviceSeq: number;

  private serverUrl = "";

  private apiKey = "";

  private deviceId: string;

  private onChangeHandlers: MessageHandler[] = [];

  private onStatusHandlers: StatusHandler[] = [];

  private todoResolver: TodoResolver | null = null;

  private allTodosResolver: AllTodosResolver | null = null;

  private fullSyncHandlers: FullSyncHandler[] = [];

  /** Outbox for changes made while disconnected. Persisted in localStorage. */

  private outbox: ArkChange[];

  constructor() {
    this.deviceId = getOrCreateDeviceId();

    this.vector = loadVector();

    this.deviceSeq = this.vector[this.deviceId] ?? 0;

    this.outbox = loadOutbox();
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

  /**
   * Register a resolver that returns the current in-memory TodoItem for a
   * given id.  Used by flushOutbox to avoid sending stale cached payloads
   * after receiving sync_changes from another device.
   */

  setTodoResolver(resolver: TodoResolver) {
    this.todoResolver = resolver;
  }

  setAllTodosResolver(resolver: AllTodosResolver) {
    this.allTodosResolver = resolver;
  }

  onFullSync(handler: FullSyncHandler) {
    this.fullSyncHandlers.push(handler);

    return () => {
      this.fullSyncHandlers = this.fullSyncHandlers.filter(
        (h) => h !== handler,
      );
    };
  }

  private emitFullSync(serverTaskIds: Set<string>, outboxTaskIds: Set<string>) {
    for (const h of this.fullSyncHandlers) {
      try {
        h(serverTaskIds, outboxTaskIds);
      } catch {
        /* ignore */
      }
    }
  }

  connect(serverUrl: string, apiKey: string) {
    // Prevent duplicate connections

    if (
      this.ws &&
      (this.ws.readyState === WebSocket.CONNECTING ||
        this.ws.readyState === WebSocket.OPEN)
    ) {
      console.log("[ArkSync] Already connected/connecting, skipping");

      return;
    }

    this.serverUrl = normalizeApiUrl(serverUrl);

    this.apiKey = apiKey;

    this.vector = loadVector();

    this.deviceSeq = this.vector[this.deviceId] ?? 0;

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
   * If not connected, queues the change in the outbox for later delivery.
   */

  sendChange(change: ArkChange): boolean {
    if (!this.ws || !this._connected) {
      console.log(
        "[ArkSync] Not connected, queuing change:",

        change.event_id,

        change.change_type,
      );

      this.outbox.push(change);

      saveOutbox(this.outbox);

      return false;
    }

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

      console.log(
        "[ArkSync] Sent change:",

        change.event_id,

        change.change_type,
      );

      return true;
    } catch (e) {
      console.warn("[ArkSync] Send failed, queuing:", e);

      this.outbox.push(change);

      saveOutbox(this.outbox);

      return false;
    }
  }

  /** Flush queued outbox changes after reconnection.
   *
   * Before sending each pending change we attempt to refresh the payload from
   * the current Pinia store state (via todoResolver).  This prevents stale
   * outbox entries — e.g. a task that was completed on another device and
   * already applied locally via sync_changes — from overwriting the server
   * with the old pre-sync data.
   */

  private flushOutbox() {
    if (this.outbox.length === 0) return;

    console.log(`[ArkSync] Flushing ${this.outbox.length} outbox changes`);

    const pending = [...this.outbox];

    this.outbox = [];

    saveOutbox(this.outbox);

    for (let change of pending) {
      // Refresh non-delete task changes from current store state so we always

      // push the current truth rather than the pre-sync cached payload.

      if (
        change.change_type !== "delete" &&
        (change.data as Record<string, unknown>)?.event_type === "task" &&
        this.todoResolver
      ) {
        const sourceId =
          ((change.data as Record<string, unknown>)?.source_id as
            | string
            | undefined) ?? change.event_id;

        const current = this.todoResolver(sourceId);

        if (current) {
          change = todoItemToArkChange(current, change.change_type);

          console.log(
            "[ArkSync] Outbox: refreshed stale payload for",

            sourceId,
          );
        } else {
          // Task was deleted locally — skip sending the update

          console.log(
            "[ArkSync] Outbox: skipping change for locally-deleted task",

            sourceId,
          );

          continue;
        }
      }

      this.sendChange(change);
    }
  }

  /** Send local tasks that the server doesn't have (bidirectional initial sync). */

  private sendMissingToServer(serverChanges: ArkChange[]) {
    if (!this.allTodosResolver || !this.ws || !this._connected) return;

    const serverKnownIds = new Set(
      serverChanges

        .map((c) =>
          ((c.data as Record<string, unknown>)?.source_id as string | undefined)

            ?.toLowerCase(),
        )

        .filter((id): id is string => Boolean(id)),
    );

    const allTodos = this.allTodosResolver();

    const missing = allTodos.filter(
      (t) => !serverKnownIds.has(t.id.toLowerCase()),
    );

    if (missing.length === 0) return;

    console.log(`[ArkSync] Sending ${missing.length} missing tasks to server`);

    const batchChanges = missing.map((todo) => {
      this.deviceSeq += 1;

      const change = todoItemToArkChange(todo, "create");

      return {
        ...change,

        event_id: todo.id,

        device_id: this.deviceId,

        device_seq: this.deviceSeq,
      };
    });

    this.vector[this.deviceId] = this.deviceSeq;

    saveVector(this.vector);

    try {
      this.ws.send(
        JSON.stringify({ type: "sync_changes", changes: batchChanges }),
      );

      console.log(`[ArkSync] Sent batch of ${missing.length} missing tasks`);
    } catch (e) {
      console.warn("[ArkSync] Failed to send missing tasks:", e);
    }
  }

  // -- Internal ------------------------------------------------------------

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

    this.ws.addEventListener("open", () => {
      this.backoff = 1000;

      if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
        console.warn("[ArkSync] onopen fired but socket not OPEN, skipping");

        return;
      }

      console.log("[ArkSync] WebSocket opened, sending sync_start");

      this.ws.send(
        JSON.stringify({
          type: "sync_start",

          device_id: this.deviceId,

          device_name: "Delphi Web",

          platform: "web",

          vector: this.vector,
        }),
      );
    });

    this.ws.addEventListener("message", (event) => {
      let msg: Record<string, unknown>;

      try {
        msg = JSON.parse(typeof event.data === "string" ? event.data : "");
      } catch {
        return;
      }

      this.handleMessage(msg);
    });

    this.ws.addEventListener("error", (e) => {
      console.error("[ArkSync] WebSocket error:", e);
    });

    this.ws.addEventListener("close", (e) => {
      console.log("[ArkSync] WebSocket closed:", e.code, e.reason);

      this.ws = null;

      this.setConnected(false);

      this._synced = false;

      this.scheduleReconnect();
    });
  }

  private handleMessage(msg: Record<string, unknown>) {
    const type = msg.type as string;

    switch (type) {
      case "sync_changes": {
        const changes = (msg.changes as ArkChange[]) ?? [];

        const isFullSync = Boolean(msg.is_full_sync);

        const incomingEpoch = msg.server_epoch as string | undefined;

        const storedEpoch = localStorage.getItem(SERVER_EPOCH_KEY);

        const epochChanged =
          incomingEpoch && storedEpoch && incomingEpoch !== storedEpoch;

        console.log(
          `[ArkSync] sync_changes: ${changes.length} items, full=${isFullSync}, epoch=${incomingEpoch?.slice(0, 8)}${epochChanged ? " (CHANGED — server was reset)" : ""}`,
        );

        // Server epoch changed = server DB was wiped. Clients push all their local data

        // without deleting anything locally — the client is the authority here.

        if (epochChanged) {
          console.warn(
            "[ArkSync] Server epoch changed — pushing full local state",
          );

          localStorage.removeItem(VECTOR_KEY);

          this.vector = {};

          this.deviceSeq = 0;
        }

        // Persist the new epoch

        if (incomingEpoch) {
          localStorage.setItem(SERVER_EPOCH_KEY, incomingEpoch);
        }

        for (const ch of changes) {
          this.updateVector(ch);

          this.emitChange(ch);
        }

        this.setConnected(true);

        this._synced = true;

        this.flushOutbox();

        if (isFullSync || epochChanged) {
          // Collect server's known task IDs for zombie-safe reconciliation

          const serverTaskIds = new Set<string>();

          for (const ch of changes) {
            const data = ch.data as Record<string, unknown> | undefined;

            if (data?.event_type === "task") {
              const srcId = (data?.source_id as string | undefined)

                ?.toLowerCase();

              if (srcId) serverTaskIds.add(srcId);
            }
          }

          const outboxTaskIds = new Set(
            this.outbox

              .map((c) => c.event_id?.toLowerCase())

              .filter((id): id is string => Boolean(id)),
          );

          // Zombie cleanup only when epoch matches (no server reset) —

          // on epoch change we keep all local data and push it to server instead.

          if (isFullSync && !epochChanged) {
            this.emitFullSync(serverTaskIds, outboxTaskIds);
          }

          this.sendMissingToServer(changes);
        }

        break;
      }

      case "change": {
        // Realtime change from another device

        const ch = msg as unknown as ArkChange;

        console.log(
          "[ArkSync] Received realtime change:",

          ch.event_id,

          ch.change_type,

          (ch.data as Record<string, unknown>)?.summary,
        );

        this.updateVector(ch);

        this.emitChange(ch);

        break;
      }

      case "change_ack": {
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
