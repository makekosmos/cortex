/* eslint-disable no-console */

/**
 * Event-mapping utilities for Delphi browser mode.
 *
 * Desktop Delphi now reads/writes tasks through ARK `task_obj` in the
 * extension shim. Browser builds still reuse the lightweight Ark HTTP helpers
 * below for initial bootstrap.
 */

import { normalizeApiUrl } from "@/helpers/normalize";

import type { Project, TodoItem } from "@/types/task";

import { Priority, ProjectStatus } from "@/types/task";

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
        notes: (dataRaw.notes as string | null) ?? (dataRaw.description as string | null) ?? null,
        priority: Number(dataRaw.priority ?? Priority.None) as Priority,
        scheduledDate: (dataRaw.scheduledDate as string | null) ?? null,
        deadline:
          (dataRaw.deadline as string | null) ?? (dataRaw.due_date as string | null) ?? null,
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
          (dataRaw.projectId as string | null) ?? (dataRaw.list_id as string | null) ?? null,
        areaId: (dataRaw.areaId as string | null) ?? null,
        tagIds: Array.isArray(dataRaw.tagIds) ? (dataRaw.tagIds as string[]) : [],
        checklistItems: Array.isArray(dataRaw.checklistItems)
          ? (dataRaw.checklistItems as TodoItem["checklistItems"])
          : [],
        recurrenceRule: (dataRaw.recurrenceRule as TodoItem["recurrenceRule"]) ?? null,
        billable: Boolean(dataRaw.billable),
        price:
          typeof dataRaw.price === "number" && Number.isFinite(dataRaw.price)
            ? (dataRaw.price as number)
            : null,
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
        billable: Boolean(dataRaw.billable),
        price:
          typeof dataRaw.price === "number" && Number.isFinite(dataRaw.price)
            ? (dataRaw.price as number)
            : null,
      });
    }

    return projects;
  } catch (e) {
    console.warn("[ArkSync] Failed to fetch projects from Ark:", e);
    return [];
  }
}
