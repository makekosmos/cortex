// Compositional store для Dashboard. Минимальный — Vue ref'ы + загрузка
// через `window.kepler` IPC API (см. preload.ts).
//
// Намерено НЕ использует Pinia: dashboard живёт изолированно в своём окне,
// state per-window, hot-reload и debugging проще через нативные ref'ы.

import { ref } from "vue";
import type { DashboardObjectRow, DashboardObjectType } from "./types";

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

function arkRequest<T>(operation: string, params?: Record<string, unknown>): Promise<T> {
  return window.kepler.ark.request<T>(operation, params);
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

function toRow(rec: RawObjectRecord): DashboardObjectRow {
  return {
    id: rec.id,
    typeId: rec.typeId,
    primary: pickPrimary(rec),
    createdAt: rec.createdAt,
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
