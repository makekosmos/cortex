/* eslint-disable no-console */

/**
 * Sync entity types and data-conversion utilities for Delphi TS.
 *
 * The active Electron sync runtime lives in the Rust sidecar and is accessed
 * through `lan-sync:*` IPC. This file only keeps pure mapping helpers shared by
 * the renderer. Browser builds also reuse the lightweight Ark HTTP helpers
 * below for initial bootstrap.
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
// TodoItem <-> ArkChange mapping (GTD model)
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

  const id = rawId.toLowerCase();

  return {
    id,

    title,

    description:
      (data.description as string | null) ??
      (data.notes as string | null) ??
      null,

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
 * Convert an ArkChange to a TodoItem (GTD model).
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

export async function fetchTasksFromArk(): Promise<TodoItem[]> {
  const url = getArkUrl();
  const key = getArkApiKey();

  if (!url || !key) return [];

  try {
    const resp = await fetch(`${url}/events?limit=1000`, {
      headers: { "X-API-Key": key },
    });

    if (!resp.ok) {
      throw new Error(`Ark HTTP ${resp.status} while loading tasks`);
    }

    const events: Record<string, unknown>[] = await resp.json();
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

      const dedupeKey = id.toLowerCase();
      const existing = todoMap.get(dedupeKey);

      if (!existing || todo.isCompleted || todo.isCancelled) {
        todoMap.set(dedupeKey, todo);
      }
    }

    return Array.from(todoMap.values());
  } catch (e) {
    console.warn("[ArkSync] Failed to fetch tasks from Ark:", e);
    return [];
  }
}

export async function fetchProjectsFromArk(): Promise<Project[]> {
  const url = getArkUrl();
  const key = getArkApiKey();

  if (!url || !key) return [];

  try {
    const resp = await fetch(`${url}/events?limit=1000`, {
      headers: { "X-API-Key": key },
    });

    if (!resp.ok) {
      throw new Error(`Ark HTTP ${resp.status} while loading projects`);
    }

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

    return projects;
  } catch (e) {
    console.warn("[ArkSync] Failed to fetch projects from Ark:", e);
    return [];
  }
}
