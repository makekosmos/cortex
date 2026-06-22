import { z } from "zod";

export const noteFieldKinds = [
  "text",
  "long_text",
  "number",
  "date",
  "boolean",
  "select",
  "multi_select",
  "image",
  "relation",
] as const;

export const legacyHeaderTemplateKinds = ["default", "centered_profile"] as const;
export const objectHeaderLayoutKinds = ["inline", "column"] as const;
export const objectDefaultLayoutKinds = ["page", "list", "gallery", "board"] as const;

export type NoteFieldKind = (typeof noteFieldKinds)[number];
export type LegacyHeaderTemplateKind = (typeof legacyHeaderTemplateKinds)[number];
export type ObjectHeaderLayoutKind = (typeof objectHeaderLayoutKinds)[number];
export type ObjectDefaultLayoutKind = (typeof objectDefaultLayoutKinds)[number];
export type NoteFieldDisplayMode = "featured" | "visible" | "hidden";

export interface NoteTypeField {
  id: string;
  label: string;
  kind: NoteFieldKind;
  required: boolean;
  options?: string[];
  placeholder?: string;
  visible?: boolean;
  read_only?: boolean;
  multiple?: boolean;
  system?: boolean;
  link_type?: string;
  allowed_object_types?: string[];
}

export interface NoteTypeUiSchema {
  featured_fields?: string[];
  visible_fields?: string[];
  hidden_fields?: string[];
  read_only_fields?: string[];
  field_order?: string[];
  header_layout?: ObjectHeaderLayoutKind;
  default_layout?: ObjectDefaultLayoutKind;
  default_template_id?: string | null;
  collection_name?: string;
}

export interface ResolvedNoteTypeField extends NoteTypeField {
  visible: boolean;
  read_only: boolean;
}

export interface HeaderTemplateDefinition {
  kind: LegacyHeaderTemplateKind;
  primaryFieldIds?: string[];
  secondaryFieldIds?: string[];
  imageFieldId?: string | null;
}

export interface NoteTypePresentation {
  headerLayout: ObjectHeaderLayoutKind;
  featuredFields: ResolvedNoteTypeField[];
  secondaryFields: ResolvedNoteTypeField[];
  descriptionField: ResolvedNoteTypeField | null;
  fieldOrder: string[];
  imageFieldId: string | null;
}

export interface NoteType {
  id: string;
  name: string;
  slug: string;
  icon: string | null;
  color: string | null;
  schema_json: string;
  header_template_json: string;
  ui_schema_json?: string;
  created_at: number;
  updated_at: number;
}

const noteTypeFieldSchema = z.object({
  id: z.string().min(1),
  label: z.string().min(1),
  kind: z.enum(noteFieldKinds),
  required: z.boolean(),
  options: z.array(z.string().min(1)).optional(),
  placeholder: z.string().optional(),
  visible: z.boolean().optional(),
  read_only: z.boolean().optional(),
  multiple: z.boolean().optional(),
  system: z.boolean().optional(),
  link_type: z.string().optional(),
  allowed_object_types: z.array(z.string().min(1)).optional(),
});

const headerTemplateSchema = z.object({
  kind: z.enum(legacyHeaderTemplateKinds),
  primaryFieldIds: z.array(z.string()).optional(),
  secondaryFieldIds: z.array(z.string()).optional(),
  imageFieldId: z.string().nullable().optional(),
});

export const noteTypeDefinitionSchema = z.object({
  fields: z.array(noteTypeFieldSchema),
});

const noteTypeUiSchema = z.object({
  featured_fields: z.array(z.string()).optional(),
  visible_fields: z.array(z.string()).optional(),
  hidden_fields: z.array(z.string()).optional(),
  read_only_fields: z.array(z.string()).optional(),
  field_order: z.array(z.string()).optional(),
  header_layout: z.enum(objectHeaderLayoutKinds).optional(),
  default_layout: z.enum(objectDefaultLayoutKinds).optional(),
  default_template_id: z.string().nullable().optional(),
  collection_name: z.string().min(1).optional(),
});

export const noteTypeSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  slug: z.string().min(1),
  icon: z.string().nullable(),
  color: z.string().nullable(),
  schema_json: z.string(),
  header_template_json: z.string(),
  ui_schema_json: z.string().optional(),
  created_at: z.number(),
  updated_at: z.number(),
});

function parseJsonWithContext(json: string, label: string): unknown {
  try {
    return JSON.parse(json);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`${label}: invalid JSON (${message})`);
  }
}

export function parseNoteTypeDefinition(schemaJson: string) {
  const parsed = parseJsonWithContext(schemaJson, "note type schema");
  return noteTypeDefinitionSchema.parse(parsed);
}

export function parseHeaderTemplate(templateJson: string) {
  const parsed = parseJsonWithContext(templateJson, "note type header template");
  return headerTemplateSchema.parse(parsed);
}

export function createDefaultHeaderTemplate(
  kind: LegacyHeaderTemplateKind = "default",
): HeaderTemplateDefinition {
  return {
    kind,
    primaryFieldIds: [],
    secondaryFieldIds: [],
    imageFieldId: null,
  };
}

export function createDefaultNoteTypeDefinition() {
  return {
    fields: [],
  };
}

export function createDefaultNoteTypeUiSchema(): NoteTypeUiSchema {
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
    featured_fields: parsed.featured_fields ?? defaults.featured_fields,
    visible_fields: parsed.visible_fields ?? defaults.visible_fields,
    hidden_fields: parsed.hidden_fields ?? defaults.hidden_fields,
    read_only_fields: parsed.read_only_fields ?? defaults.read_only_fields,
    field_order: parsed.field_order ?? defaults.field_order,
    collection_name: parsed.collection_name ?? defaults.collection_name,
  };
}

export function pluralizeNoteTypeName(name: string): string {
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
    return [...(template.primaryFieldIds ?? []), ...(template.secondaryFieldIds ?? [])];
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
  const featuredFields = new Set(uiSchema.featured_fields ?? []);
  const visibleFields = new Set(uiSchema.visible_fields ?? []);
  const hiddenFields = new Set(uiSchema.hidden_fields ?? []);
  const readOnlyFields = new Set(uiSchema.read_only_fields ?? []);
  const explicitVisibilityConfig =
    featuredFields.size > 0 || visibleFields.size > 0 || hiddenFields.size > 0;
  const fieldOrder = getResolvedFieldOrder(definition.fields, uiSchema.field_order ?? []);
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

export function getNoteTypeFieldDisplayMode(
  noteType: NoteType | null,
  fieldId: string,
): NoteFieldDisplayMode {
  const uiSchema = parseNoteTypeUiSchema(noteType?.ui_schema_json);
  if ((uiSchema.hidden_fields ?? []).includes(fieldId)) {
    return "hidden";
  }
  if ((uiSchema.featured_fields ?? []).includes(fieldId)) {
    return "featured";
  }
  if ((uiSchema.visible_fields ?? []).includes(fieldId)) {
    return "visible";
  }

  const field = getResolvedNoteTypeField(noteType, fieldId);
  return field?.visible === false ? "hidden" : "visible";
}

export function createDefaultHeaderProps(noteType: NoteType | null) {
  if (!noteType) {
    return {};
  }

  const definition = parseNoteTypeDefinition(noteType.schema_json);

  return Object.fromEntries(
    definition.fields.map((field) => {
      switch (field.kind) {
        case "boolean":
          return [field.id, false];
        case "number":
          return [field.id, ""];
        case "multi_select":
          return [field.id, []];
        case "relation":
          return [field.id, field.multiple === false ? "" : []];
        default:
          return [field.id, ""];
      }
    }),
  );
}

function isPersonLikeNoteType(noteType: NoteType | null): boolean {
  if (!noteType) {
    return false;
  }

  try {
    const definition = parseNoteTypeDefinition(noteType.schema_json);
    const fieldIds = new Set(definition.fields.map((field) => field.id));
    return (
      noteType.slug === "person" ||
      (fieldIds.has("first_name") && fieldIds.has("last_name") && fieldIds.has("patronymic"))
    );
  } catch {
    return false;
  }
}

function splitPersonTitle(title: string): {
  firstName: string;
  lastName: string;
  patronymic: string;
} {
  const parts = title.trim().split(/\s+/).filter(Boolean);

  return {
    firstName: parts[0] ?? "",
    lastName: parts[1] ?? "",
    patronymic: parts.slice(2).join(" "),
  };
}

export function createHeaderPropsForTypeChange(noteType: NoteType | null, sourceTitle: string) {
  const props = createDefaultHeaderProps(noteType);
  if (!isPersonLikeNoteType(noteType)) {
    return props;
  }

  const { firstName, lastName, patronymic } = splitPersonTitle(sourceTitle);

  return {
    ...props,
    first_name: firstName,
    last_name: lastName,
    patronymic,
  };
}

function coerceHeaderFieldValue(field: NoteTypeField, value: unknown) {
  switch (field.kind) {
    case "text":
    case "long_text":
    case "date":
    case "image":
      if (typeof value === "string") {
        return value;
      }

      if (typeof value === "number" && Number.isFinite(value)) {
        return String(value);
      }

      return "";
    case "number":
      if (typeof value === "number" && Number.isFinite(value)) {
        return value;
      }

      if (typeof value === "string") {
        const trimmed = value.trim();
        return /^$|^-?\d+(\.\d+)?$/.test(trimmed) ? trimmed : "";
      }

      return "";
    case "boolean":
      if (value === true || value === false) {
        return value;
      }

      if (value === 1 || value === "1" || value === "true") {
        return true;
      }

      if (value === 0 || value === "0" || value === "false") {
        return false;
      }

      return false;
    case "multi_select":
      if (Array.isArray(value)) {
        return value
          .filter((item): item is string => typeof item === "string")
          .map((item) => item.trim())
          .filter(Boolean);
      }

      if (typeof value === "string") {
        return value
          .split(",")
          .map((item) => item.trim())
          .filter(Boolean);
      }

      return [];
    case "relation":
      if (field.multiple === false) {
        if (typeof value === "string") {
          return value.trim();
        }

        if (Array.isArray(value)) {
          const firstValue = value.find((item): item is string => typeof item === "string");
          return firstValue?.trim() ?? "";
        }

        return "";
      }

      if (Array.isArray(value)) {
        return value
          .filter((item): item is string => typeof item === "string")
          .map((item) => item.trim())
          .filter(Boolean);
      }

      if (typeof value === "string") {
        const trimmed = value.trim();
        return trimmed ? [trimmed] : [];
      }

      return [];
    case "select":
      if (typeof value === "string") {
        return value;
      }

      if (typeof value === "number" && Number.isFinite(value)) {
        return String(value);
      }

      if (Array.isArray(value)) {
        const firstValue = value.find((item): item is string => typeof item === "string");
        return firstValue ?? "";
      }

      return "";
    default:
      return value;
  }
}

function normalizeHeaderPropsRecord(noteType: NoteType | null, rawProps: Record<string, unknown>) {
  if (!noteType) {
    return rawProps;
  }

  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const normalized: Record<string, unknown> = { ...rawProps };

  for (const field of definition.fields) {
    normalized[field.id] = coerceHeaderFieldValue(field, rawProps[field.id]);
  }

  return normalized;
}

export function normalizeHeaderProps(noteType: NoteType | null, rawProps: unknown) {
  if (!rawProps || typeof rawProps !== "object" || Array.isArray(rawProps)) {
    return createDefaultHeaderProps(noteType);
  }

  return {
    ...createDefaultHeaderProps(noteType),
    ...normalizeHeaderPropsRecord(noteType, rawProps as Record<string, unknown>),
  };
}

export function buildHeaderPropsSchema(noteType: NoteType | null) {
  if (!noteType) {
    return z.record(z.string(), z.unknown());
  }

  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const shape: Record<string, z.ZodTypeAny> = {};

  for (const field of definition.fields) {
    let schema: z.ZodTypeAny;

    switch (field.kind) {
      case "number":
        schema = z.union([z.number(), z.string().regex(/^$|^-?\d+(\.\d+)?$/)]);
        break;
      case "date":
        schema = z.string();
        break;
      case "boolean":
        schema = z.boolean();
        break;
      case "multi_select":
        schema = z.array(z.string());
        break;
      case "relation":
        schema = field.multiple === false ? z.string() : z.array(z.string());
        break;
      default:
        schema = z.string();
        break;
    }

    shape[field.id] = field.required ? schema : schema.optional();
  }

  return z.object(shape).passthrough();
}

export function validateHeaderProps(noteType: NoteType | null, rawProps: unknown) {
  return buildHeaderPropsSchema(noteType).safeParse(normalizeHeaderProps(noteType, rawProps));
}

export function safeParseHeaderProps(
  noteType: NoteType | null,
  rawJson: string | null | undefined,
) {
  try {
    const parsed = rawJson ? JSON.parse(rawJson) : {};
    const result = validateHeaderProps(noteType, parsed);

    if (result.success) {
      return result.data as Record<string, unknown>;
    }
  } catch {
    return createDefaultHeaderProps(noteType);
  }

  return createDefaultHeaderProps(noteType);
}

export function normalizeSlug(input: string) {
  const normalized = input
    .trim()
    .toLowerCase()
    .replace(/[\s/\\]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized || "note-type";
}
