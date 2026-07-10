import {
  headerTemplateSchema,
  noteTypeDefinitionSchema,
  noteTypeUiSchema,
} from "./typedNoteSchemas";
import type {
  HeaderTemplateDefinition,
  LegacyHeaderTemplateKind,
  NoteType,
  NoteTypeField,
  NoteTypePresentation,
  NoteTypeUiSchema,
  ObjectHeaderLayoutKind,
  ResolvedNoteTypeField,
} from "./typedNoteSchemas";

export { noteTypeSchema } from "./typedNoteSchemas";
export type {
  NoteTypeField,
  NoteTypeUiSchema,
  ResolvedNoteTypeField,
  NoteTypePresentation,
  NoteType,
} from "./typedNoteSchemas";

const EMPTY_STRING_ARRAY = Object.freeze([]) as unknown as string[];

function parseJsonWithContext(json: string, label: string): unknown {
  try {
    return JSON.parse(json);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`${label}: invalid JSON (${message})`, { cause: error });
  }
}

function arrayOrDefault<T>(value: T[] | undefined, fallback: T[] | undefined): T[] {
  return value ?? fallback ?? [];
}

export function parseNoteTypeDefinition(schemaJson: string) {
  const parsed = parseJsonWithContext(schemaJson, "note type schema");
  return noteTypeDefinitionSchema.parse(parsed);
}

export function parseHeaderTemplate(templateJson: string) {
  const parsed = parseJsonWithContext(templateJson, "note type header template");
  return headerTemplateSchema.parse(parsed);
}

function createDefaultHeaderTemplate(
  kind: LegacyHeaderTemplateKind = "default",
): HeaderTemplateDefinition {
  return {
    kind,
    primaryFieldIds: [],
    secondaryFieldIds: [],
    imageFieldId: null,
  };
}

function createDefaultNoteTypeDefinition() {
  return {
    fields: [],
  };
}

function createDefaultNoteTypeUiSchema(): NoteTypeUiSchema {
  return {
    featured_fields: [],
    visible_fields: [],
    hidden_fields: ["created_at", "updated_at", "deleted_at"],
    read_only_fields: [],
    field_order: [],
    header_layout: "inline",
    default_layout: "page",
    default_template_id: null,
    collection_name: undefined,
  };
}

export function parseNoteTypeUiSchema(uiSchemaJson?: string | null): NoteTypeUiSchema {
  const defaults = createDefaultNoteTypeUiSchema();
  if (!uiSchemaJson?.trim()) {
    return defaults;
  }

  const parsed = noteTypeUiSchema.parse(parseJsonWithContext(uiSchemaJson, "note type UI schema"));
  return {
    ...defaults,
    ...parsed,
    featured_fields: arrayOrDefault(parsed.featured_fields, defaults.featured_fields),
    visible_fields: arrayOrDefault(parsed.visible_fields, defaults.visible_fields),
    hidden_fields: arrayOrDefault(parsed.hidden_fields, defaults.hidden_fields),
    read_only_fields: arrayOrDefault(parsed.read_only_fields, defaults.read_only_fields),
    field_order: arrayOrDefault(parsed.field_order, defaults.field_order),
    collection_name: parsed.collection_name ?? defaults.collection_name,
  };
}

function pluralizeNoteTypeName(name: string): string {
  const trimmed = name.trim();
  if (!trimmed) {
    return "Объекты";
  }

  const normalized = trimmed.toLowerCase();

  if (normalized.endsWith("ка")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("га")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("ха")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("жа")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("ша")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("ща")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("ча")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("а")) {
    return `${trimmed.slice(0, -1)}ы`;
  }

  if (normalized.endsWith("я")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("й")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (normalized.endsWith("ь")) {
    return `${trimmed.slice(0, -1)}и`;
  }

  if (/[bcdfghjklmnpqrstvwxyz]$/i.test(trimmed)) {
    return `${trimmed}s`;
  }

  return `${trimmed}ы`;
}

export function getNoteTypeCollectionName(noteType: NoteType | null): string {
  if (!noteType) {
    return "Объекты";
  }

  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);
  return uiSchema.collection_name?.trim() || pluralizeNoteTypeName(noteType.name);
}

export function resolveNoteTypeHeaderLayout(noteType: NoteType | null): ObjectHeaderLayoutKind {
  if (!noteType) {
    return "inline";
  }

  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);
  if (uiSchema.header_layout) {
    return uiSchema.header_layout;
  }

  try {
    const template = parseHeaderTemplate(noteType.header_template_json);
    return template.kind === "centered_profile" ? "column" : "inline";
  } catch {
    return "inline";
  }
}

function getLegacyFeaturedFieldIds(noteType: NoteType | null): string[] {
  if (!noteType) {
    return [];
  }

  try {
    const template = parseHeaderTemplate(noteType.header_template_json);
    return [
      ...arrayOrDefault(template.primaryFieldIds, EMPTY_STRING_ARRAY),
      ...arrayOrDefault(template.secondaryFieldIds, EMPTY_STRING_ARRAY),
    ];
  } catch {
    return [];
  }
}

function getLegacyImageFieldId(noteType: NoteType | null): string | null {
  if (!noteType) {
    return null;
  }

  try {
    return parseHeaderTemplate(noteType.header_template_json).imageFieldId ?? null;
  } catch {
    return null;
  }
}

function getResolvedFieldOrder(definitionFields: NoteTypeField[], fieldOrder: string[]): string[] {
  const orderedIds = fieldOrder.filter((fieldId) =>
    definitionFields.some((field) => field.id === fieldId),
  );
  const fallbackIds = definitionFields
    .map((field) => field.id)
    .filter((fieldId) => !orderedIds.includes(fieldId));

  return [...orderedIds, ...fallbackIds];
}

export function resolveNoteTypeFields(noteType: NoteType | null): ResolvedNoteTypeField[] {
  if (!noteType) {
    return [];
  }

  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);
  const featuredFields = new Set(arrayOrDefault(uiSchema.featured_fields, EMPTY_STRING_ARRAY));
  const visibleFields = new Set(arrayOrDefault(uiSchema.visible_fields, EMPTY_STRING_ARRAY));
  const hiddenFields = new Set(arrayOrDefault(uiSchema.hidden_fields, EMPTY_STRING_ARRAY));
  const readOnlyFields = new Set(arrayOrDefault(uiSchema.read_only_fields, EMPTY_STRING_ARRAY));
  const explicitVisibilityConfig =
    featuredFields.size > 0 || visibleFields.size > 0 || hiddenFields.size > 0;
  const fieldOrder = getResolvedFieldOrder(
    definition.fields,
    arrayOrDefault(uiSchema.field_order, EMPTY_STRING_ARRAY),
  );
  const orderIndex = new Map(fieldOrder.map((fieldId, index) => [fieldId, index]));

  return [...definition.fields]
    .sort((left, right) => (orderIndex.get(left.id) ?? 0) - (orderIndex.get(right.id) ?? 0))
    .map((field) => {
      const visibleByField = field.visible !== false;
      let visible = visibleByField;

      if (hiddenFields.has(field.id)) {
        visible = false;
      } else if (explicitVisibilityConfig) {
        visible = featuredFields.has(field.id) || visibleFields.has(field.id);
      }

      return {
        ...field,
        visible,
        read_only: field.read_only === true || readOnlyFields.has(field.id),
      };
    });
}

export function getResolvedNoteTypeField(
  noteType: NoteType | null,
  fieldId: string,
): ResolvedNoteTypeField | null {
  return resolveNoteTypeFields(noteType).find((field) => field.id === fieldId) ?? null;
}

export function getNoteTypePresentation(noteType: NoteType | null): NoteTypePresentation {
  const resolvedFields = resolveNoteTypeFields(noteType);
  const uiSchema = parseNoteTypeUiSchema(noteType?.ui_schema_json);
  const featuredFields = new Set(
    (uiSchema.featured_fields?.length
      ? uiSchema.featured_fields
      : getLegacyFeaturedFieldIds(noteType)
    ).filter(Boolean),
  );
  const fieldOrder = getResolvedFieldOrder(
    noteType ? parseNoteTypeDefinition(noteType.schema_json).fields : [],
    uiSchema.field_order ?? [],
  );
  const descriptionField =
    resolvedFields.find((field) => field.id === "description" && field.visible) ?? null;
  const nonDescriptionFields = resolvedFields.filter(
    (field) => field.visible && field.id !== "description",
  );
  const orderedFeatured = nonDescriptionFields.filter((field) => featuredFields.has(field.id));
  const orderedVisible = nonDescriptionFields.filter((field) => !featuredFields.has(field.id));
  const imageFieldId =
    getLegacyImageFieldId(noteType) ??
    resolvedFields.find((field) => field.kind === "image" && field.visible)?.id ??
    null;

  return {
    headerLayout: resolveNoteTypeHeaderLayout(noteType),
    descriptionField,
    featuredFields: orderedFeatured,
    secondaryFields: orderedVisible,
    fieldOrder,
    imageFieldId,
  };
}

export function normalizeSlug(input: string) {
  const normalized = input
    .trim()
    .toLowerCase()
    .replace(/[\s/\\]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized || "note-type";
}
