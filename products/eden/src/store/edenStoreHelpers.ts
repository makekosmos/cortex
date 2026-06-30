import {
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPES,
  normalizeSystemNoteType,
} from "@/lib/systemTypes";

export type ActiveScreen = "notes" | "settings" | "type-collection";

export const SYSTEM_TYPES_BY_ID = new Map(SYSTEM_TYPES.map((noteType) => [noteType.id, noteType]));

const LAST_ENTRY_STORAGE_KEY = "eden:nav:lastEntryId";

interface QueuedSaveRequest {
  entry: Entry;
  waiters: Array<{
    resolve: (result: SaveEntryResult | null) => void;
    reject: (error: unknown) => void;
  }>;
}

export interface EntrySaveCoordinator {
  inFlight: boolean;
  queued: QueuedSaveRequest | null;
}

export function writeLastVisitedEntryId(id: string): void {
  try {
    window.localStorage.setItem(LAST_ENTRY_STORAGE_KEY, id);
  } catch {
    // localStorage can be unavailable in tests or restricted renderer contexts.
  }
}

export function readLastVisitedEntryId(): string | null {
  try {
    return window.localStorage.getItem(LAST_ENTRY_STORAGE_KEY);
  } catch {
    return null;
  }
}

export function mergeNoteTypesWithSystem(noteTypesData: NoteType[]) {
  const byId = new Map<string, NoteType>();
  for (const systemType of SYSTEM_TYPES) {
    byId.set(systemType.id, normalizeSystemNoteType(systemType));
  }
  for (const noteType of noteTypesData) {
    byId.set(noteType.id, normalizeSystemNoteType(noteType));
  }
  return [...byId.values()];
}

function parseEntryHeaderProps(entry: Entry): Record<string, unknown> {
  try {
    const parsed = JSON.parse(entry.header_props_json || "{}");
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

export function getCollectionTargetTypeId(entry: Entry | null | undefined): string | null {
  if (!entry || entry.type_id !== SYSTEM_TYPE_COLLECTION_ID) return null;
  const objectTypeId = parseEntryHeaderProps(entry).object_type_id;
  return typeof objectTypeId === "string" && objectTypeId.trim() ? objectTypeId : null;
}

export function mergeEntriesById(entries: Entry[], additions: Entry[]): Entry[] {
  if (additions.length === 0) return entries;
  const byId = new Map(entries.map((entry) => [entry.id, entry] as const));
  for (const entry of additions) {
    byId.set(entry.id, entry);
  }
  return [...byId.values()].sort((left, right) => right.updated_at - left.updated_at);
}

export function waitForLoadingFrame(): Promise<void> {
  return new Promise((resolve) => {
    if (typeof window.requestAnimationFrame === "function") {
      window.requestAnimationFrame(() => {
        window.requestAnimationFrame(() => {
          window.setTimeout(resolve, 0);
        });
      });
      return;
    }

    window.setTimeout(resolve, 0);
  });
}

export function pruneTransientSaveState(
  latestSaveTimestamps: Map<string, number>,
  saveCoordinators: Record<string, EntrySaveCoordinator>,
  existingEntries: Entry[],
) {
  const validIds = new Set(existingEntries.map((entry) => entry.id));

  for (const entryId of latestSaveTimestamps.keys()) {
    if (!validIds.has(entryId)) {
      latestSaveTimestamps.delete(entryId);
    }
  }

  for (const entryId of Object.keys(saveCoordinators)) {
    if (!validIds.has(entryId) && !saveCoordinators[entryId]?.inFlight) {
      delete saveCoordinators[entryId];
    }
  }
}
