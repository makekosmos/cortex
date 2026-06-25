import {
  normalizeSlug,
  parseHeaderTemplate,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  type NoteType,
} from "@/lib/typedNotes";
import { SYSTEM_TYPE_COLLECTION, shouldShowAsEdenCollection } from "@/lib/systemTypes";
import {
  collectionObjectIdForType,
  mapArkObjectTypeToNoteType,
  mapEntryToArkObject,
  mapNoteTypeToArkObjectType,
  mapNoteTypeToCollectionEntry,
  normalizeNoteType,
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
    if (Array.isArray(r.types)) return r.types as T[];
  }
  return [];
}

export function createNoteTypeApi(ark: ArkRequest): {
  getNoteTypeById: (noteTypeId: string) => Promise<NoteType | null>;
  listNoteTypes: () => Promise<NoteType[]>;
  ensureCollectionObjects: (noteTypes: NoteType[]) => Promise<Entry[]>;
  saveNoteType: (noteType: NoteType) => Promise<SaveNoteTypeResult>;
  deleteNoteType: (noteTypeId: string) => Promise<boolean>;
} {
  return {
    async getNoteTypeById(noteTypeId: string): Promise<NoteType | null> {
      const objectType = await ark<ArkObjectTypeRecord | null>("get_object_type", {
        id: noteTypeId,
      });
      return objectType ? mapArkObjectTypeToNoteType(objectType) : null;
    },

    async listNoteTypes(): Promise<NoteType[]> {
      const types = await ark<unknown>("list_object_types").then(ensureList<ArkObjectTypeRecord>);
      return types.map(mapArkObjectTypeToNoteType);
    },

    async ensureCollectionObjects(noteTypes: NoteType[]): Promise<Entry[]> {
      const collectionType = mapNoteTypeToArkObjectType(SYSTEM_TYPE_COLLECTION);
      await ark<boolean>("upsert_object_type", {
        object_type: {
          id: collectionType.id,
          name: collectionType.name,
          schemaJson: collectionType.schemaJson,
          uiSchemaJson: collectionType.uiSchemaJson,
          createdAt: collectionType.createdAt,
          updatedAt: collectionType.updatedAt,
          systemLocked: collectionType.systemLocked,
        },
      });

      const entries: Entry[] = [];
      for (const noteType of noteTypes) {
        if (!shouldShowAsEdenCollection(noteType.id)) continue;

        const existing = await ark<ArkObjectRecord | null>("get_object", {
          id: collectionObjectIdForType(noteType.id),
        }).catch(() => null);
        const entry = mapNoteTypeToCollectionEntry(noteType, existing);
        const arkObject = mapEntryToArkObject({
          ...entry,
          deleted_at: null,
        });

        await ark<boolean>("upsert_object", {
          object: {
            id: arkObject.id,
            typeId: arkObject.typeId,
            title: arkObject.title,
            contentJson: arkObject.contentJson,
            propsJson: arkObject.propsJson,
            createdAt: arkObject.createdAt,
            updatedAt: arkObject.updatedAt,
            deletedAt: null,
          },
        });
        entries.push({ ...entry, deleted_at: null });
      }

      return entries;
    },

    async saveNoteType(noteType: NoteType): Promise<SaveNoteTypeResult> {
      try {
        parseNoteTypeDefinition(noteType.schema_json);
        parseHeaderTemplate(noteType.header_template_json);
        parseNoteTypeUiSchema(noteType.ui_schema_json);
      } catch {
        return {
          ok: false,
          reason: "invalid_definition",
          message: "Схема типа заметки заполнена некорректно",
        };
      }

      const normalized = normalizeNoteType({
        ...noteType,
        slug: normalizeSlug(noteType.slug || noteType.name),
      });

      const objectType = mapNoteTypeToArkObjectType(normalized);
      await ark<boolean>("upsert_object_type", {
        object_type: {
          id: objectType.id,
          name: objectType.name,
          schemaJson: objectType.schemaJson,
          uiSchemaJson: objectType.uiSchemaJson,
          createdAt: objectType.createdAt,
          updatedAt: objectType.updatedAt,
          systemLocked: objectType.systemLocked,
        },
      });

      return { ok: true, noteType: normalized };
    },

    async deleteNoteType(noteTypeId: string): Promise<boolean> {
      await ark<boolean>("delete_object_type", { id: noteTypeId });
      return true;
    },
  };
}
