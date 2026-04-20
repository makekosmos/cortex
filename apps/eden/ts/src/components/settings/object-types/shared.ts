import {
  createDefaultHeaderTemplate,
  createDefaultNoteTypeUiSchema,
  getNoteTypeCollectionName,
  getNoteTypeFieldDisplayMode,
  parseNoteTypeDefinition,
  parseNoteTypeUiSchema,
  type NoteFieldDisplayMode,
  type NoteFieldKind,
  type NoteTypeField,
  type ObjectHeaderLayoutKind,
} from "@/lib/typedNotes";
import { isSystemType } from "@/lib/systemTypes";

export interface TypeEditorFieldDraft extends NoteTypeField {
  displayMode: NoteFieldDisplayMode;
  structuralLocked: boolean;
}

export interface TypeDraft {
  id?: string;
  name: string;
  collectionName: string;
  slug: string;
  icon: string;
  color: string;
  headerLayout: ObjectHeaderLayoutKind;
  fields: TypeEditorFieldDraft[];
}

export const ICON_OPTIONS = [
  "document",
  "book",
  "calendar",
  "game-controller",
  "barbell",
  "fitness",
  "planet",
  "library",
  "folder",
  "sparkles",
] as const;

export const FIELD_KIND_OPTIONS: Array<{ value: NoteFieldKind; label: string }> = [
  { value: "text", label: "Текст" },
  { value: "long_text", label: "Длинный текст" },
  { value: "number", label: "Число" },
  { value: "date", label: "Дата" },
  { value: "boolean", label: "Да / Нет" },
  { value: "select", label: "Выбор" },
  { value: "multi_select", label: "Множественный выбор" },
  { value: "image", label: "Изображение" },
  { value: "relation", label: "Связь" },
];

export function sampleValueForField(field: TypeEditorFieldDraft) {
  if (field.id === "description") {
    return "Краткое описание объекта, которое живет отдельно от основного текста заметки.";
  }

  if (field.id === "genres") {
    return "Adventure, Strategy";
  }

  if (field.id === "exe_path") {
    return "D:\\Games\\Example\\game.exe";
  }

  if (field.id === "save_path") {
    return "D:\\Users\\Player\\Saved Games\\Example";
  }

  return field.label || "Значение";
}

export function toSchemaFields(fields: TypeEditorFieldDraft[]): NoteTypeField[] {
  return fields.map(({ displayMode, structuralLocked, ...field }) => ({
    ...field,
    visible: displayMode !== "hidden",
  }));
}

export function buildUiSchema(typeDraft: TypeDraft) {
  const baseHiddenFields = new Set(["created_at", "updated_at", "deleted_at"]);
  const featuredFields = typeDraft.fields
    .filter((field) => field.displayMode === "featured")
    .map((field) => field.id);
  const visibleFields = typeDraft.fields
    .filter((field) => field.displayMode === "featured" || field.displayMode === "visible")
    .map((field) => field.id);
  const hiddenFields = [
    ...baseHiddenFields,
    ...typeDraft.fields
      .filter((field) => field.displayMode === "hidden")
      .map((field) => field.id),
  ];

  return {
    ...createDefaultNoteTypeUiSchema(),
    featured_fields: featuredFields,
    visible_fields: visibleFields,
    hidden_fields: hiddenFields,
    read_only_fields: typeDraft.fields
      .filter((field) => field.read_only === true)
      .map((field) => field.id),
    field_order: typeDraft.fields.map((field) => field.id),
    header_layout: typeDraft.headerLayout,
    collection_name: typeDraft.collectionName.trim() || undefined,
  };
}

export function buildHeaderTemplateJson(typeDraft: TypeDraft) {
  const featuredFields = typeDraft.fields
    .filter((field) => field.displayMode === "featured")
    .map((field) => field.id);
  const visibleFields = typeDraft.fields
    .filter((field) => field.displayMode === "visible")
    .map((field) => field.id);
  const imageFieldId = typeDraft.fields.find((field) => field.kind === "image")?.id ?? null;

  return JSON.stringify({
    ...createDefaultHeaderTemplate(typeDraft.headerLayout === "column" ? "centered_profile" : "default"),
    primaryFieldIds: featuredFields,
    secondaryFieldIds: visibleFields,
    imageFieldId,
  });
}

export function createFieldDraft(field: NoteTypeField, noteType: NoteType): TypeEditorFieldDraft {
  const structuralLocked = isSystemType(noteType.id);

  return {
    ...field,
    displayMode: getNoteTypeFieldDisplayMode(noteType, field.id),
    structuralLocked,
  };
}

export function createEmptyTypeDraft(): TypeDraft {
  return {
    name: "",
    collectionName: "",
    slug: "",
    icon: "document",
    color: "#2aa7ee",
    headerLayout: "inline",
    fields: [],
  };
}

export function createTypeDraftFromNoteType(noteType: NoteType): TypeDraft {
  const definition = parseNoteTypeDefinition(noteType.schema_json);
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);

  return {
    id: noteType.id,
    name: noteType.name,
    collectionName: uiSchema.collection_name ?? getNoteTypeCollectionName(noteType),
    slug: noteType.slug,
    icon: noteType.icon ?? "document",
    color: noteType.color ?? "#2aa7ee",
    headerLayout: uiSchema.header_layout ?? "inline",
    fields: definition.fields.map((field) => createFieldDraft(field, noteType)),
  };
}
