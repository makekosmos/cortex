import {
  arkTimestampToMillis,
  mapArkObjectToEntry,
  millisToArkTimestamp,
  type ArkObjectLinkRecord,
  type ArkObjectRecord,
  type ArkObjectTypeRecord,
} from "./kepler-entry-mappers";

type ArkRequest = <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;

function ensureList<T>(value: unknown): T[] {
  if (Array.isArray(value)) return value as T[];
  if (value && typeof value === "object") {
    const r = value as Record<string, unknown>;
    if (Array.isArray(r.items)) return r.items as T[];
    if (Array.isArray(r.objects)) return r.objects as T[];
    if (Array.isArray(r.links)) return r.links as T[];
    if (Array.isArray(r.types)) return r.types as T[];
  }
  return [];
}

export function createTrashStorageApi(ark: ArkRequest): {
  listTrashEntries: () => Promise<Entry[]>;
  restoreEntry: (entryId: string) => Promise<{ ok: boolean; entryId?: string }>;
  permanentDeleteEntry: (entryId: string) => Promise<{ ok: boolean; entryId?: string }>;
  purgeExpiredTrash: () => Promise<{ ok: boolean; purgedCount: number }>;
  getVaultStorageInfo: () => Promise<VaultStorageInfo>;
  getDiskFreeSpace: () => Promise<number>;
} {
  const listAllObjects = (): Promise<ArkObjectRecord[]> =>
    ark<unknown>("list_objects").then(ensureList<ArkObjectRecord>);

  return {
    async listTrashEntries(): Promise<Entry[]> {
      const [objects, links, objectTypes] = await Promise.all([
        listAllObjects(),
        ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
        ark<unknown>("list_object_types").then(ensureList<ArkObjectTypeRecord>),
      ]);
      const typesById = new Map(objectTypes.map((t) => [t.id, t]));
      return objects
        .filter((o) => !!o.deletedAt)
        .map((o) => mapArkObjectToEntry(o, links, typesById.get(o.typeId)))
        .sort((a, b) => (b.deleted_at ?? 0) - (a.deleted_at ?? 0));
    },

    async restoreEntry(entryId: string): Promise<{ ok: boolean; entryId?: string }> {
      const existing = await ark<ArkObjectRecord | null>("get_object", { id: entryId });
      if (!existing) return { ok: false, entryId };
      await ark<boolean>("upsert_object", {
        object: {
          id: existing.id,
          typeId: existing.typeId,
          title: existing.title,
          contentJson: existing.contentJson,
          propsJson: existing.propsJson,
          createdAt: existing.createdAt,
          updatedAt: millisToArkTimestamp(Date.now()),
          deletedAt: null,
        },
      });
      return { ok: true, entryId };
    },

    async permanentDeleteEntry(entryId: string): Promise<{ ok: boolean; entryId?: string }> {
      await ark<boolean>("delete_object", { id: entryId });
      return { ok: true, entryId };
    },

    async purgeExpiredTrash(): Promise<{ ok: boolean; purgedCount: number }> {
      return { ok: true, purgedCount: 0 };
    },

    async getVaultStorageInfo(): Promise<VaultStorageInfo> {
      const objects = await listAllObjects();
      const active = objects.filter((o) => !o.deletedAt);
      const trashed = objects.filter((o) => !!o.deletedAt);
      const sizeOf = (o: ArkObjectRecord) =>
        JSON.stringify(o.contentJson ?? "").length + (o.title?.length ?? 0);
      const textBytes = active.reduce((sum, o) => sum + sizeOf(o), 0);
      const trashBytes = trashed.reduce((sum, o) => sum + sizeOf(o), 0);
      return {
        textBytes,
        trashBytes,
        dbBytes: textBytes + trashBytes,
        vaultBytes: textBytes + trashBytes,
        entryCount: active.length,
        trashCount: trashed.length,
      };
    },

    async getDiskFreeSpace(): Promise<number> {
      return 0;
    },
  };
}
