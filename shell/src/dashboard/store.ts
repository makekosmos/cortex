// Compositional store для Dashboard. Минимальный — Vue ref'ы + загрузка
// через `window.kepler` IPC API (см. preload.ts).
//
// Намерено НЕ использует Pinia: dashboard живёт изолированно в своём окне,
// state per-window, hot-reload и debugging проще через нативные ref'ы.

import { ref } from "vue";
import type {
  DashboardObjectRow,
  DashboardObjectType,
  SpaceMeta,
} from "./types";

export const spaces = ref<SpaceMeta[]>([]);
export const spacesLoading = ref<boolean>(false);
export const spacesError = ref<string | null>(null);

export const selectedSpaceId = ref<string | null>(null);

export const objectTypes = ref<DashboardObjectType[]>([]);
export const objectTypesLoading = ref<boolean>(false);

// null → «Всё» (все типы микшем), id → конкретный type, "__settings__" → settings stub.
export const currentTypeId = ref<string | null>(null);

export const objects = ref<DashboardObjectRow[]>([]);
export const objectsLoading = ref<boolean>(false);

interface RawObjectRecord {
  id: string;
  typeId: string;
  title?: string;
  contentJson?: unknown;
  propsJson?: unknown;
  createdAt: string;
  deletedAt?: string | null;
}

interface RawObjectTypeRecord {
  id: string;
  name?: string;
  schemaJson?: string;
}

function arkRequest<T>(
  operation: string,
  params?: Record<string, unknown>,
): Promise<T> {
  return window.kepler.ark.request<T>(operation, params);
}

export async function loadSpaces(): Promise<void> {
  spacesLoading.value = true;
  spacesError.value = null;
  try {
    const list = await window.kepler.spaces.list();
    spaces.value = list;
  } catch (e) {
    spacesError.value = e instanceof Error ? e.message : String(e);
    spaces.value = [];
  } finally {
    spacesLoading.value = false;
  }
}

export async function loadObjectTypes(): Promise<void> {
  objectTypesLoading.value = true;
  try {
    const raw = await arkRequest<RawObjectTypeRecord[]>("list_object_types");
    objectTypes.value = raw.map((t) => ({
      id: t.id,
      name: t.name && t.name.length > 0 ? t.name : t.id,
    }));
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

function pickDataField(rec: RawObjectRecord, index: number): string {
  const candidates: unknown[] = [];
  const c = rec.contentJson;
  if (c && typeof c === "object" && !Array.isArray(c)) {
    candidates.push(...Object.values(c as Record<string, unknown>));
  }
  const p = rec.propsJson;
  if (p && typeof p === "object" && !Array.isArray(p)) {
    candidates.push(...Object.values(p as Record<string, unknown>));
  }
  // Первое (index=0) уже использовано как primary если title пустой; даём
  // обоим колонкам смещение от 1.
  const value = candidates[index + 1];
  if (value === undefined || value === null) return "—";
  if (typeof value === "string") return value.length > 0 ? value : "—";
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }
  try {
    const s = JSON.stringify(value);
    return s.length > 48 ? `${s.slice(0, 48)}…` : s;
  } catch {
    return "—";
  }
}

function toRow(rec: RawObjectRecord): DashboardObjectRow {
  return {
    id: rec.id,
    typeId: rec.typeId,
    primary: pickPrimary(rec),
    createdAt: rec.createdAt,
    dataX: pickDataField(rec, 0),
    dataY: pickDataField(rec, 1),
  };
}

export async function loadObjects(typeId: string | null): Promise<void> {
  objectsLoading.value = true;
  currentTypeId.value = typeId;
  try {
    let raw: RawObjectRecord[];
    if (typeId === null) {
      raw = await arkRequest<RawObjectRecord[]>("list_objects");
    } else {
      raw = await arkRequest<RawObjectRecord[]>("list_objects_by_type", {
        type_id: typeId,
      });
    }
    objects.value = raw.filter((r) => !r.deletedAt).map(toRow);
  } catch (e) {
    console.warn("[dashboard] loadObjects failed", e);
    objects.value = [];
  } finally {
    objectsLoading.value = false;
  }
}
