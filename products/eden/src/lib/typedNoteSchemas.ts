import { z } from "zod";

const noteFieldKinds = [
  "text",
  "url",
  "long_text",
  "number",
  "date",
  "boolean",
  "select",
  "multi_select",
  "image",
  "relation",
] as const;

const legacyHeaderTemplateKinds = ["default", "centered_profile"] as const;
const objectHeaderLayoutKinds = ["inline", "column"] as const;
const objectDefaultLayoutKinds = ["page", "list", "gallery", "board"] as const;

type NoteFieldKind = (typeof noteFieldKinds)[number];
export type LegacyHeaderTemplateKind = (typeof legacyHeaderTemplateKinds)[number];
export type ObjectHeaderLayoutKind = (typeof objectHeaderLayoutKinds)[number];
type ObjectDefaultLayoutKind = (typeof objectDefaultLayoutKinds)[number];

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

export const headerTemplateSchema = z.object({
  kind: z.enum(legacyHeaderTemplateKinds),
  primaryFieldIds: z.array(z.string()).optional(),
  secondaryFieldIds: z.array(z.string()).optional(),
  imageFieldId: z.string().nullable().optional(),
});

export const noteTypeDefinitionSchema = z.object({
  fields: z.array(noteTypeFieldSchema),
});

export const noteTypeUiSchema = z.object({
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
