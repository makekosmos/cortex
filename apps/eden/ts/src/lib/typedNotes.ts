import { z } from "zod";

export const noteFieldKinds = [
  "text",
  "long_text",
  "number",
  "date",
  "boolean",
  "select",
  "image",
] as const;
export const headerLayoutKinds = ["default", "centered_profile"] as const;

export type NoteFieldKind = (typeof noteFieldKinds)[number];
export type HeaderLayoutKind = (typeof headerLayoutKinds)[number];

export interface NoteTypeField {
  id: string;
  label: string;
  kind: NoteFieldKind;
  required: boolean;
  options?: string[];
  placeholder?: string;
}

export interface HeaderTemplateDefinition {
  kind: HeaderLayoutKind;
  primaryFieldIds?: string[];
  secondaryFieldIds?: string[];
  imageFieldId?: string | null;
}

export interface NoteType {
  id: string;
  name: string;
  slug: string;
  icon: string | null;
  color: string | null;
  schema_json: string;
  header_template_json: string;
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
});

const headerTemplateSchema = z.object({
  kind: z.enum(headerLayoutKinds),
  primaryFieldIds: z.array(z.string()).optional(),
  secondaryFieldIds: z.array(z.string()).optional(),
  imageFieldId: z.string().nullable().optional(),
});

export const noteTypeDefinitionSchema = z.object({
  fields: z.array(noteTypeFieldSchema),
});

export const noteTypeSchema = z.object({
  id: z.string().min(1),
  name: z.string().min(1),
  slug: z.string().min(1),
  icon: z.string().nullable(),
  color: z.string().nullable(),
  schema_json: z.string(),
  header_template_json: z.string(),
  created_at: z.number(),
  updated_at: z.number(),
});

export function parseNoteTypeDefinition(schemaJson: string) {
  const parsed = JSON.parse(schemaJson);
  return noteTypeDefinitionSchema.parse(parsed);
}

export function parseHeaderTemplate(templateJson: string) {
  const parsed = JSON.parse(templateJson);
  return headerTemplateSchema.parse(parsed);
}

export function createDefaultHeaderTemplate(
  kind: HeaderLayoutKind = "default",
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
        default:
          return [field.id, ""];
      }
    }),
  );
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
      case "select":
        schema = z.string();
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
  return buildHeaderPropsSchema(noteType).safeParse(rawProps);
}

export function safeParseHeaderProps(
  noteType: NoteType | null,
  rawJson: string | null | undefined,
) {
  const defaultProps = createDefaultHeaderProps(noteType);

  try {
    const parsed = rawJson ? JSON.parse(rawJson) : {};
    const result = validateHeaderProps(noteType, parsed);
    if (result.success) {
      return result.data as Record<string, unknown>;
    }

    if (parsed && typeof parsed === "object" && !Array.isArray(parsed)) {
      return {
        ...defaultProps,
        ...(parsed as Record<string, unknown>),
      };
    }
  } catch {}

  return defaultProps;
}

export function normalizeSlug(input: string) {
  const normalized = input
    .trim()
    .toLowerCase()
    .replace(/[\s/\\]+/g, "-")
    .replace(/^-+|-+$/g, "");

  return normalized || "note-type";
}
