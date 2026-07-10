import { shouldIncludeTypeInEdenListForLiveUpdate } from "../store/liveListFilter";
import type { NoteType } from "@/lib/typedNotes";
import { validateHeaderProps } from "@/lib/typedNoteHeaderProps";
import { SYSTEM_TYPE_COLLECTION_ID } from "@/lib/systemTypes";
import { SYSTEM_TYPE_JOURNAL_ID, SYSTEM_TYPE_NOTE_ID } from "@/lib/systemTypeDefinitions";
import {
  DEFAULT_ARK_TYPE_ID,
  arkTimestampToMillis,
  mapArkObjectSummaryToEntry,
  mapArkObjectToEntry,
  mapEntryToArkObject,
  millisToArkTimestamp,
  normalizeEntry,
  parseHeaderPropsJson,
  shouldIncludeObjectInEdenList,
  type ArkObjectLinkRecord,
  type ArkObjectRecord,
  type ArkObjectSummaryRecord,
  type ArkObjectTypeRecord,
} from "./kepler-entry-mappers";
import { createNoteTypeApi } from "./kepler-note-type-api";

type ArkRequest = <T = unknown>(operation: string, params?: Record<string, unknown>) => Promise<T>;

const LEGACY_JOURNAL_TITLE_PATTERN = /^\d{4}-\d{2}-\d{2}$/;

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

export function createEntryApi(
  ark: ArkRequest,
  readVisibleObjectTypeIds: () => string[],
): {
  loadListableEntry: (id: string, typeIdHint?: string) => Promise<Entry | undefined>;
  listEntries: () => Promise<Entry[]>;
  listAllEntries: () => Promise<Entry[]>;
  loadEntry: (id: string) => Promise<Entry | undefined>;
  saveEntry: (entry: Entry) => Promise<SaveEntryResult>;
  deleteEntry: (entryId: string) => Promise<DeleteEntryResult>;
  listNoteTypes: () => Promise<NoteType[]>;
  ensureCollectionObjects: (noteTypes: NoteType[]) => Promise<Entry[]>;
  saveNoteType: (noteType: NoteType) => Promise<SaveNoteTypeResult>;
  deleteNoteType: (noteTypeId: string) => Promise<boolean>;
  searchEntries: (query: string) => Promise<SearchResult[]>;
} {
  const { getNoteTypeById, listNoteTypes, ensureCollectionObjects, saveNoteType, deleteNoteType } =
    createNoteTypeApi(ark);

  async function listObjectsForVisibleTypes(typeIds: string[]): Promise<ArkObjectRecord[]> {
    if (typeIds.length === 0) {
      return ark<unknown>("list_objects").then(ensureList<ArkObjectRecord>);
    }

    const chunks = await Promise.all(
      typeIds.map((typeId) =>
        ark<unknown>("list_objects_by_type", { type_id: typeId }).then(ensureList<ArkObjectRecord>),
      ),
    );
    return chunks.flat();
  }

  async function listObjectSummariesForVisibleTypes(
    typeIds: string[],
  ): Promise<ArkObjectSummaryRecord[]> {
    try {
      if (typeIds.length === 0) {
        return await ark<unknown>("list_object_summaries").then(ensureList<ArkObjectSummaryRecord>);
      }

      const chunks = await Promise.all(
        typeIds.map((typeId) =>
          ark<unknown>("list_object_summaries_by_type", {
            type_id: typeId,
          }).then(ensureList<ArkObjectSummaryRecord>),
        ),
      );
      return chunks.flat();
    } catch (err) {
      console.warn("[eden-extension] list_object_summaries unavailable, using full objects:", err);
      const fullObjects = await listObjectsForVisibleTypes(typeIds);
      return fullObjects.map((object) => ({
        id: object.id,
        typeId: object.typeId,
        title: object.title,
        propsJson: object.propsJson,
        createdAt: object.createdAt,
        updatedAt: object.updatedAt,
        deletedAt: object.deletedAt ?? null,
      }));
    }
  }

  async function listAllObjects(): Promise<ArkObjectRecord[]> {
    return ark<unknown>("list_objects").then(ensureList<ArkObjectRecord>);
  }

  async function loadEntry(id: string): Promise<Entry | undefined> {
    const [object, links] = await Promise.all([
      ark<ArkObjectRecord | null>("get_object", { id }),
      ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
    ]);

    if (!object || object.deletedAt) return undefined;
    const objectType = await ark<ArkObjectTypeRecord | null>("get_object_type", {
      id: object.typeId,
    }).catch(() => null);
    return mapArkObjectToEntry(object, links, objectType ?? undefined);
  }

  async function validateEntryTypeMetadata(entry: Entry): Promise<SaveEntryResult | null> {
    if (!entry.type_id) return null;

    const noteType = await getNoteTypeById(entry.type_id);
    if (!noteType) {
      return {
        ok: false,
        reason: "invalid_type_metadata",
        message: "Тип заметки больше не существует",
      };
    }

    let parsedProps: unknown = {};
    try {
      parsedProps = JSON.parse(entry.header_props_json || "{}");
    } catch {
      return {
        ok: false,
        reason: "invalid_type_metadata",
        message: "Верхушка заметки сохранена в неверном формате",
      };
    }

    const validation = validateHeaderProps(noteType, parsedProps);
    if (!validation.success) {
      return {
        ok: false,
        reason: "invalid_type_metadata",
        message: "Структура верхушки заметки больше не соответствует типу",
      };
    }
    return null;
  }

  async function ensureEntryTypeAvailable(noteTypeId: string): Promise<boolean> {
    const t = await ark<ArkObjectTypeRecord | null>("get_object_type", {
      id: noteTypeId,
    });
    return Boolean(t);
  }

  function parseEntryContentJson(
    contentJson: string,
  ): { ok: true; value: unknown } | { ok: false } {
    try {
      return { ok: true, value: JSON.parse(contentJson || "{}") };
    } catch {
      return { ok: false };
    }
  }

  function isStaleEntryWrite(
    entry: Entry,
    existing: ArkObjectRecord,
    nextContentJson: unknown,
  ): boolean {
    const existingUpdatedAt = arkTimestampToMillis(existing.updatedAt, 0);
    if (existingUpdatedAt <= entry.updated_at) return false;

    const nextObject = mapEntryToArkObject(entry);
    return (
      JSON.stringify(existing.contentJson ?? null) !== JSON.stringify(nextContentJson ?? null) ||
      existing.title !== nextObject.title ||
      JSON.stringify(existing.propsJson ?? {}) !== JSON.stringify(nextObject.propsJson ?? {})
    );
  }

  async function syncRelatedLinks(entry: Entry): Promise<void> {
    const headerProps = parseHeaderPropsJson(entry.header_props_json);
    const relatedNotes = Array.isArray(headerProps.related_notes)
      ? (headerProps.related_notes as unknown[]).filter((v): v is string => typeof v === "string")
      : [];

    const existingLinks = await ark<unknown>("list_object_links").then(
      ensureList<ArkObjectLinkRecord>,
    );

    const ownedLinks = existingLinks.filter(
      (l) => l.sourceObjectId === entry.id && l.linkType === "related",
    );

    for (const link of ownedLinks) {
      if (!relatedNotes.includes(link.targetObjectId)) {
        await ark<boolean>("delete_object_link", { id: link.id });
      }
    }

    for (const targetObjectId of relatedNotes) {
      if (ownedLinks.some((l) => l.targetObjectId === targetObjectId)) continue;
      await ark<boolean>("upsert_object_link", {
        object_link: {
          id: `${entry.id}:related:${targetObjectId}`,
          sourceObjectId: entry.id,
          targetObjectId,
          linkType: "related",
          createdAt: millisToArkTimestamp(entry.updated_at),
        },
      });
    }
  }

  return {
    async loadListableEntry(id: string, typeIdHint?: string): Promise<Entry | undefined> {
      const visibleTypeIds = readVisibleObjectTypeIds();

      if (typeIdHint !== undefined && typeIdHint !== SYSTEM_TYPE_COLLECTION_ID) {
        const quickCheck = shouldIncludeTypeInEdenListForLiveUpdate({
          typeId: typeIdHint,
          propsJson: {},
          visibleTypeIds,
        });
        if (!quickCheck) return undefined;
      }

      const entry = await loadEntry(id);
      if (!entry || entry.deleted_at) return undefined;

      let propsJson: Record<string, unknown> = {};
      try {
        const parsed = JSON.parse(entry.header_props_json || "{}");
        if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
          propsJson = parsed as Record<string, unknown>;
        }
      } catch {
        // ignore
      }

      const listable = shouldIncludeTypeInEdenListForLiveUpdate({
        typeId: entry.type_id ?? "",
        propsJson,
        visibleTypeIds,
      });

      return listable ? entry : undefined;
    },

    async listEntries(): Promise<Entry[]> {
      const visibleTypeIds = readVisibleObjectTypeIds();
      const objects = await listObjectSummariesForVisibleTypes(visibleTypeIds);

      return objects
        .filter((o) => !o.deletedAt)
        .filter((o) => !isLegacyDatedJournalObject(o))
        .filter(shouldIncludeObjectInEdenList)
        .map((o) => mapArkObjectSummaryToEntry(o, [], undefined))
        .sort((a, b) => b.updated_at - a.updated_at);
    },

    async listAllEntries(): Promise<Entry[]> {
      const [objects, links] = await Promise.all([
        listAllObjects(),
        ark<unknown>("list_object_links").then(ensureList<ArkObjectLinkRecord>),
      ]);

      return objects
        .filter((object) => !object.deletedAt)
        .filter(shouldIncludeObjectInEdenList)
        .map((object) => mapArkObjectToEntry(object, links, undefined))
        .sort((a, b) => b.updated_at - a.updated_at);
    },

    loadEntry,

    async saveEntry(entry: Entry): Promise<SaveEntryResult> {
      const normalized = normalizeEntry({
        ...entry,
        type_id: entry.type_id ?? DEFAULT_ARK_TYPE_ID,
        header_layout: entry.header_layout ?? "default",
      });

      const validation = await validateEntryTypeMetadata(normalized);
      if (validation) return validation;

      const typeId = normalized.type_id ?? DEFAULT_ARK_TYPE_ID;
      if (!(await ensureEntryTypeAvailable(typeId))) {
        return {
          ok: false,
          reason: "invalid_type_metadata",
          message: "Тип заметки больше не существует",
        };
      }

      if (normalized.content_loaded === false) {
        return {
          ok: false,
          reason: "content_not_loaded",
          message: "Тело заметки ещё не загружено",
        };
      }

      const parsedContent = parseEntryContentJson(normalized.content_json);
      if (!parsedContent.ok) {
        return {
          ok: false,
          reason: "invalid_content_json",
          message: "Тело заметки сохранено в неверном формате",
        };
      }

      const normalizedTitle = normalized.title.trim().toLocaleLowerCase("ru");
      const existingObjects = await listAllObjects();
      const existingCurrentObject = existingObjects.find(
        (candidate) => candidate.id === normalized.id,
      );
      if (
        existingCurrentObject &&
        isStaleEntryWrite(normalized, existingCurrentObject, parsedContent.value)
      ) {
        return {
          ok: false,
          reason: "stale_entry",
          message: "Заметка уже была обновлена более новой версией",
        };
      }

      const conflicting = existingObjects.find((candidate) => {
        if (candidate.id === normalized.id || candidate.deletedAt) return false;
        return candidate.title.trim().toLocaleLowerCase("ru") === normalizedTitle;
      });

      if (conflicting && normalizedTitle.length > 0) {
        return {
          ok: false,
          reason: "duplicate_title",
          conflictingEntryId: conflicting.id,
          title: normalized.title,
          folder_id: normalized.folder_id,
        };
      }

      const arkObject = mapEntryToArkObject(normalized);
      await ark<boolean>("upsert_object", {
        object: {
          id: arkObject.id,
          typeId: arkObject.typeId,
          title: arkObject.title,
          contentJson: arkObject.contentJson,
          propsJson: arkObject.propsJson,
          createdAt: arkObject.createdAt,
          updatedAt: arkObject.updatedAt,
          deletedAt: arkObject.deletedAt ?? null,
        },
      });
      await syncRelatedLinks(normalized);

      return { ok: true, entryId: normalized.id };
    },

    async deleteEntry(entryId: string): Promise<DeleteEntryResult> {
      const existing = await ark<ArkObjectRecord | null>("get_object", {
        id: entryId,
      });
      if (!existing || existing.deletedAt) {
        return {
          ok: false,
          reason: "entry_not_found",
          message: "Заметка не найдена в ARK",
        };
      }
      const deletedAt = millisToArkTimestamp(Date.now());
      await ark<boolean>("upsert_object", {
        object: {
          ...existing,
          updatedAt: deletedAt,
          deletedAt,
        },
      });
      return { ok: true, entryId };
    },

    listNoteTypes,
    ensureCollectionObjects,
    saveNoteType,
    deleteNoteType,

    async searchEntries(query: string): Promise<SearchResult[]> {
      if (!query.trim()) return [];
      try {
        const results = await ark<SearchResult[]>("search_objects", { query });
        if (!Array.isArray(results)) return [];
        const seen = new Set<string>();
        return results.filter((r) => {
          const key = `${r.entryId}:${r.line}:${r.text}`;
          if (seen.has(key)) return false;
          seen.add(key);
          return true;
        });
      } catch (e) {
        console.warn("[eden-extension] search_objects failed:", e);
        return [];
      }
    },
  };
}

function isLegacyDatedJournalObject(object: Pick<ArkObjectSummaryRecord, "typeId" | "title">) {
  return (
    (object.typeId === SYSTEM_TYPE_JOURNAL_ID || object.typeId === SYSTEM_TYPE_NOTE_ID) &&
    LEGACY_JOURNAL_TITLE_PATTERN.test(object.title.trim())
  );
}
