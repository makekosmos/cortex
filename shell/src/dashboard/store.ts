// Compositional store для Dashboard. Минимальный — Vue ref'ы + загрузка
// через `window.kepler` IPC API (см. preload.ts).
//
// Намерено НЕ использует Pinia: dashboard живёт изолированно в своём окне,
// state per-window, hot-reload и debugging проще через нативные ref'ы.

import { ref } from "vue";
import type { DashboardObjectRow, DashboardObjectType, DashboardUsageRow } from "./types";
import { HIDDEN_DASHBOARD_TYPE_IDS } from "./typeVisuals";

export const objectTypes = ref<DashboardObjectType[]>([]);
export const objectTypesLoading = ref<boolean>(false);

// null → «Всё» (all objects), id → object type, "__usage__" → usage tracker.
export const currentTypeId = ref<string | null>(null);

export const objects = ref<DashboardObjectRow[]>([]);
export const objectsLoading = ref<boolean>(false);

export const usageRows = ref<DashboardUsageRow[]>([]);
export const usageLoading = ref<boolean>(false);

const objectRowsCache = new Map<string, DashboardObjectRow[]>();
let objectTypesLoaded = false;
let usageRowsLoaded = false;

function objectCacheKey(typeId: string | null): string {
  return typeId ?? "__all__";
}

interface RawObjectRecord {
  id: string;
  typeId: string;
  title?: string;
  contentJson?: unknown;
  propsJson?: unknown;
  createdAt: string;
  updatedAt?: string;
  deletedAt?: string | null;
}

interface RawObjectTypeRecord {
  id: string;
  name?: string;
  schemaJson?: string;
}

interface RawTopAppEntry {
  id: string;
  displayName: string;
  processName: string;
  normalizedPath: string;
  iconRef?: string | null;
  runtimeMs: number;
  foregroundMs: number;
  idleMs: number;
  sessions: number;
  lastSeenAt?: string | null;
}

interface RawUsageAnalyticsSnapshot {
  topApps: RawTopAppEntry[];
}

interface RawAppIndexEntry {
  exec_path: string;
  icon_path: string | null;
}

function arkRequest<T>(operation: string, params?: Record<string, unknown>): Promise<T> {
  return window.kepler.ark.request<T>(operation, params);
}

export async function loadObjectTypes(): Promise<void> {
  if (objectTypesLoaded) return;
  objectTypesLoading.value = true;
  try {
    const raw = await arkRequest<RawObjectTypeRecord[]>("list_object_types");
    objectTypes.value = raw
      .filter((t) => !HIDDEN_DASHBOARD_TYPE_IDS.has(t.id))
      .map((t) => ({
        id: t.id,
        name: t.name && t.name.length > 0 ? t.name : t.id,
      }));
    objectTypesLoaded = true;
  } catch (e) {
    console.warn("[dashboard] loadObjectTypes failed", e);
    objectTypes.value = [];
  } finally {
    objectTypesLoading.value = false;
  }
}

function pickPrimary(rec: RawObjectRecord): string {
  if (rec.title && rec.title.length > 0) return rec.title;
  const c = rec.contentJson;
  if (c && typeof c === "object" && !Array.isArray(c)) {
    for (const v of Object.values(c as Record<string, unknown>)) {
      if (typeof v === "string" && v.length > 0) return v;
    }
  }
  return rec.id;
}

function typeNameFor(typeId: string): string {
  return objectTypes.value.find((type) => type.id === typeId)?.name ?? typeId;
}

function toRow(rec: RawObjectRecord): DashboardObjectRow {
  return {
    id: rec.id,
    typeId: rec.typeId,
    typeName: typeNameFor(rec.typeId),
    primary: pickPrimary(rec),
    createdAt: rec.createdAt,
    updatedAt: rec.updatedAt ?? rec.createdAt,
  };
}

export async function loadObjects(typeId: string | null): Promise<void> {
  currentTypeId.value = typeId;
  const cacheKey = objectCacheKey(typeId);
  const cached = objectRowsCache.get(cacheKey);
  if (cached) {
    objects.value = cached;
    objectsLoading.value = false;
    return;
  }

  objectsLoading.value = true;
  try {
    let raw: RawObjectRecord[];
    if (typeId === null) {
      raw = await arkRequest<RawObjectRecord[]>("list_objects");
    } else {
      raw = await arkRequest<RawObjectRecord[]>("list_objects_by_type", {
        type_id: typeId,
      });
    }
    const rows = raw
      .filter((r) => !r.deletedAt && !HIDDEN_DASHBOARD_TYPE_IDS.has(r.typeId))
      .map(toRow);
    objectRowsCache.set(cacheKey, rows);
    if (currentTypeId.value === typeId) {
      objects.value = rows;
    }
  } catch (e) {
    console.warn("[dashboard] loadObjects failed", e);
    if (currentTypeId.value === typeId) {
      objects.value = [];
    }
  } finally {
    if (currentTypeId.value === typeId) {
      objectsLoading.value = false;
    }
  }
}

function normalizePath(path: string): string {
  return path.trim().replace(/\//g, "\\").toLowerCase();
}

export function chooseUsageIconRef(
  usageIconRef: string | null | undefined,
  appIndexIconRef: string | null | undefined,
): string | null {
  if (appIndexIconRef) return appIndexIconRef;
  if (usageIconRef && usageIconRef.startsWith("data:image/")) return usageIconRef;
  return null;
}

function toUsageRow(rec: RawTopAppEntry, appIcons: Map<string, string>): DashboardUsageRow {
  return {
    id: rec.id,
    processName: rec.processName,
    displayName: rec.displayName,
    // См. postmortems.md § 2026-06-01 — app_index отдаёт renderer-safe data URL,
    // а tracked_apps.icon_ref может быть stale path'ом, который ломает <img>.
    iconRef: chooseUsageIconRef(rec.iconRef, appIcons.get(normalizePath(rec.normalizedPath))),
    runtimeMs: rec.runtimeMs,
    foregroundMs: rec.foregroundMs,
    idleMs: rec.idleMs,
    sessions: rec.sessions,
    lastSeenAt: rec.lastSeenAt,
    normalizedPath: rec.normalizedPath,
  };
}

export async function loadUsageRows(): Promise<void> {
  currentTypeId.value = "__usage__";
  if (usageRowsLoaded) {
    usageLoading.value = false;
    return;
  }

  usageLoading.value = true;
  try {
    // См. postmortems.md § 2026-06-01: usage tracker lives outside object_types.
    const [raw, apps] = await Promise.all([
      arkRequest<RawUsageAnalyticsSnapshot>("get_usage_analytics", {
        range_days: 3650,
        top_apps_limit: 500,
        recent_sessions_limit: 1,
      }),
      arkRequest<{ apps: RawAppIndexEntry[] }>("app_index.list_all", { limit: 1000 }).catch(() => ({
        apps: [],
      })),
    ]);
    const appIcons = new Map(
      apps.apps
        .filter((app) => app.icon_path)
        .map((app) => [normalizePath(app.exec_path), app.icon_path as string]),
    );
    const rows = raw.topApps.map((row) => toUsageRow(row, appIcons));
    usageRowsLoaded = true;
    if (currentTypeId.value === "__usage__") {
      usageRows.value = rows;
    }
  } catch (e) {
    console.warn("[dashboard] loadUsageRows failed", e);
    if (currentTypeId.value === "__usage__") {
      usageRows.value = [];
    }
  } finally {
    if (currentTypeId.value === "__usage__") {
      usageLoading.value = false;
    }
  }
}
