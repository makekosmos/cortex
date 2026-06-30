import {
  getNoteTypeCollectionName,
  normalizeSlug,
  noteTypeSchema,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  type NoteType,
} from "@/lib/typedNotes";
import { writeEntryMarkdown } from "@/editor-content/content";
import {
  SYSTEM_TYPE_COLLECTION,
  SYSTEM_TYPE_COLLECTION_ID,
  isSystemType,
  normalizeSystemNoteType,
  shouldShowAsEdenCollection,
} from "@/lib/systemTypes";
import {
  EDEN_TASK_OBJECT_TYPE_ID,
  normalizeTaskObjectTypeSchemaJson,
  normalizeTaskObjectTypeUiSchemaJson,
} from "./kepler-task-sync";

export const DEFAULT_ARK_TYPE_ID = "note_obj";

export interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

export interface ArkObjectSummaryRecord {
  id: string;
  typeId: string;
  title: string;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
  deletedAt?: string | null;
}

export interface ArkObjectTypeRecord {
  id: string;
  name: string;
  schemaJson: string;
  uiSchemaJson: string;
  createdAt: string;
  updatedAt: string;
  systemLocked: boolean;
}

export interface ArkObjectLinkRecord {
  id: string;
  sourceObjectId: string;
  targetObjectId: string;
  linkType: string;
  createdAt: string;
}

const EMPTY_STRING_ARRAY = Object.freeze([]) as unknown as string[];

export function arkTimestampToMillis(value: string | null | undefined, fallback: number): number {
  if (!value) return fallback;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

export function millisToArkTimestamp(value: number | null | undefined): string {
  const ts = typeof value === "number" ? value : Date.now();
  return new Date(ts).toISOString();
}

export function parseHeaderPropsJson(raw: string | null | undefined): Record<string, unknown> {
  if (!raw?.trim()) return {};
  try {
    const parsed = JSON.parse(raw);
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function stringifyHeaderProps(props: Record<string, unknown>): string {
  return JSON.stringify(props);
}

export function normalizeEntry(entry: Entry): Entry {
  return {
    ...entry,
    content_loaded: entry.content_loaded ?? true,
    type_id: entry.type_id ?? null,
    header_layout: entry.header_layout ?? null,
    header_props_json: entry.header_props_json ?? "{}",
    schema_version: entry.schema_version ?? 1,
    deleted_at: entry.deleted_at ?? null,
  };
}

export function normalizeNoteType(noteType: NoteType): NoteType {
  return noteTypeSchema.parse({
    ...noteType,
    ui_schema_json: noteType.ui_schema_json ?? JSON.stringify({}),
  });
}

export function mapArkObjectToEntry(
  object: ArkObjectRecord,
  links: ArkObjectLinkRecord[],
  objectType?: ArkObjectTypeRecord,
): Entry {
  const createdAt = arkTimestampToMillis(object.createdAt, 0);
  const updatedAt = arkTimestampToMillis(object.updatedAt, createdAt);
  const headerProps = {
    ...object.propsJson,
    related_notes: links
      .filter((l) => l.sourceObjectId === object.id && l.linkType === "related")
      .map((l) => l.targetObjectId),
  };

  return normalizeEntry({
    id: object.id,
    title: object.title,
    content_json: JSON.stringify(object.contentJson ?? writeEntryMarkdown("")),
    content_loaded: true,
    created_at: createdAt,
    updated_at: updatedAt,
    folder_id: null,
    type_id: object.typeId,
    header_layout: objectType
      ? (parseNoteTypeUiSchema(objectType.uiSchemaJson).header_layout ?? "default")
      : "default",
    header_props_json: stringifyHeaderProps(headerProps),
    schema_version: 1,
    deleted_at: object.deletedAt ? arkTimestampToMillis(object.deletedAt, updatedAt) : null,
  });
}

export function mapArkObjectSummaryToEntry(
  object: ArkObjectSummaryRecord,
  links: ArkObjectLinkRecord[],
  objectType?: ArkObjectTypeRecord,
): Entry {
  const createdAt = arkTimestampToMillis(object.createdAt, 0);
  const updatedAt = arkTimestampToMillis(object.updatedAt, createdAt);
  const headerProps = {
    ...object.propsJson,
    related_notes: links
      .filter((l) => l.sourceObjectId === object.id && l.linkType === "related")
      .map((l) => l.targetObjectId),
  };

  return normalizeEntry({
    id: object.id,
    title: object.title,
    content_json: JSON.stringify(writeEntryMarkdown("")),
    content_loaded: false,
    created_at: createdAt,
    updated_at: updatedAt,
    folder_id: null,
    type_id: object.typeId,
    header_layout: objectType
      ? (parseNoteTypeUiSchema(objectType.uiSchemaJson).header_layout ?? "default")
      : "default",
    header_props_json: stringifyHeaderProps(headerProps),
    schema_version: 1,
    deleted_at: object.deletedAt ? arkTimestampToMillis(object.deletedAt, updatedAt) : null,
  });
}

export function mapEntryToArkObject(entry: Entry): ArkObjectRecord {
  const headerProps = parseHeaderPropsJson(entry.header_props_json);
  const { related_notes: _ignored, ...propsJson } = headerProps;
  let contentJson: unknown = writeEntryMarkdown("");
  try {
    contentJson = JSON.parse(entry.content_json || JSON.stringify(contentJson));
  } catch {
    // keep default
  }

  return {
    id: entry.id,
    typeId: entry.type_id ?? DEFAULT_ARK_TYPE_ID,
    title: entry.title,
    contentJson,
    propsJson,
    createdAt: millisToArkTimestamp(entry.created_at),
    updatedAt: millisToArkTimestamp(entry.updated_at),
    deletedAt: entry.deleted_at ? millisToArkTimestamp(entry.deleted_at) : null,
  };
}

export function mapArkObjectTypeToNoteType(objectType: ArkObjectTypeRecord): NoteType {
  const schemaJson =
    objectType.id === EDEN_TASK_OBJECT_TYPE_ID
      ? normalizeTaskObjectTypeSchemaJson(objectType.schemaJson)
      : objectType.schemaJson;
  const uiSchemaJson =
    objectType.id === EDEN_TASK_OBJECT_TYPE_ID
      ? normalizeTaskObjectTypeUiSchemaJson(objectType.uiSchemaJson)
      : objectType.uiSchemaJson;
  const createdAt = arkTimestampToMillis(objectType.createdAt, 0);
  const updatedAt = arkTimestampToMillis(objectType.updatedAt, createdAt);
  const uiSchema = parseNoteTypeUiSchema(uiSchemaJson);
  const headerTemplate = {
    kind: uiSchema.header_layout === "column" ? "centered_profile" : "default",
    primaryFieldIds: uiSchema.featured_fields ?? [],
    secondaryFieldIds: uiSchema.visible_fields ?? [],
    imageFieldId: null,
  };

  return normalizeSystemNoteType(
    normalizeNoteType({
      id: objectType.id,
      name: objectType.name,
      slug: normalizeSlug(objectType.id),
      icon: iconForObjectTypeId(objectType.id),
      color: colorForObjectTypeId(objectType.id),
      schema_json: schemaJson,
      header_template_json: JSON.stringify(headerTemplate),
      ui_schema_json: uiSchemaJson,
      created_at: createdAt,
      updated_at: updatedAt,
    }),
  );
}

function iconForObjectTypeId(objectTypeId: string): string {
  if (objectTypeId === "game_obj") return "game-controller";
  if (objectTypeId === "task_obj") return "checkmark-circle";
  return "document-text";
}

function colorForObjectTypeId(objectTypeId: string): string {
  if (objectTypeId === "game_obj") return "#ef4444";
  if (objectTypeId === "task_obj") return "#f59e0b";
  return "#2aa7ee";
}

export function mapNoteTypeToArkObjectType(noteType: NoteType): ArkObjectTypeRecord {
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);
  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const featuredFromUi = uiSchema.featured_fields ?? EMPTY_STRING_ARRAY;
  const visibleFromFields = definition.fields.filter((f) => f.visible !== false).map((f) => f.id);
  const readOnlyFromFields = definition.fields.filter((f) => f.read_only === true).map((f) => f.id);

  return {
    id: noteType.id,
    name: noteType.name,
    schemaJson: noteType.schema_json,
    uiSchemaJson: JSON.stringify({
      featured_fields: featuredFromUi,
      visible_fields: uiSchema.visible_fields?.length ? uiSchema.visible_fields : visibleFromFields,
      hidden_fields: uiSchema.hidden_fields ?? ["created_at", "updated_at", "deleted_at"],
      read_only_fields: uiSchema.read_only_fields?.length
        ? uiSchema.read_only_fields
        : readOnlyFromFields,
      field_order: uiSchema.field_order?.length
        ? uiSchema.field_order
        : definition.fields.map((f) => f.id),
      header_layout: uiSchema.header_layout ?? "inline",
      default_layout: uiSchema.default_layout ?? "page",
      default_template_id: uiSchema.default_template_id ?? null,
      collection_name: uiSchema.collection_name,
    }),
    createdAt: millisToArkTimestamp(noteType.created_at),
    updatedAt: millisToArkTimestamp(noteType.updated_at),
    systemLocked: isSystemType(noteType.id),
  };
}

export function collectionObjectIdForType(noteTypeId: string): string {
  return `collection:${noteTypeId}`;
}

export function mapNoteTypeToCollectionEntry(
  noteType: NoteType,
  existing?: ArkObjectRecord | null,
): Entry {
  const now = Date.now();
  const createdAt = existing ? arkTimestampToMillis(existing.createdAt, now) : now;
  const updatedAt = existing ? arkTimestampToMillis(existing.updatedAt, createdAt) : now;

  return normalizeEntry({
    id: collectionObjectIdForType(noteType.id),
    title: getNoteTypeCollectionName(noteType),
    content_json: JSON.stringify(writeEntryMarkdown("")),
    created_at: createdAt,
    updated_at: updatedAt,
    folder_id: null,
    type_id: SYSTEM_TYPE_COLLECTION_ID,
    header_layout:
      parseNoteTypeUiSchema(SYSTEM_TYPE_COLLECTION.ui_schema_json).header_layout ?? "inline",
    header_props_json: stringifyHeaderProps({
      ...existing?.propsJson,
      object_type_id: noteType.id,
    }),
    schema_version: 1,
    deleted_at: existing?.deletedAt ? arkTimestampToMillis(existing.deletedAt, updatedAt) : null,
  });
}

export function shouldIncludeObjectInEdenList(object: {
  typeId: string;
  propsJson?: Record<string, unknown>;
}): boolean {
  if (object.typeId !== SYSTEM_TYPE_COLLECTION_ID) return true;
  const objectTypeId = object.propsJson?.object_type_id;
  return typeof objectTypeId === "string" && shouldShowAsEdenCollection(objectTypeId);
}
