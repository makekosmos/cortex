import type { NoteType } from "./typedNotes";

export const SYSTEM_TYPE_IMAGE_ID = "image_obj";
export const SYSTEM_TYPE_PERSON_ID = "person_obj";

const imageSchemaJson = JSON.stringify({
  fields: [
    {
      id: "image",
      label: "Изображение",
      kind: "image",
      required: true,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "file_name",
      label: "Имя файла",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "mime_type",
      label: "MIME-тип",
      kind: "text",
      required: false,
      visible: false,
      read_only: true,
      system: true,
    },
    {
      id: "size_bytes",
      label: "Размер (байт)",
      kind: "number",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "width",
      label: "Ширина",
      kind: "number",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "height",
      label: "Высота",
      kind: "number",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "resolution",
      label: "Разрешение",
      kind: "text",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "source_path",
      label: "Исходный путь",
      kind: "text",
      required: false,
      visible: false,
      read_only: true,
      system: true,
    },
    {
      id: "alt_text",
      label: "Alt-текст",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
  ],
});

const imageHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: ["image"],
  secondaryFieldIds: ["file_name", "resolution", "size_bytes"],
  imageFieldId: "image",
});

const imageUiSchemaJson = JSON.stringify({
  featured_fields: ["file_name", "resolution"],
  visible_fields: ["image", "file_name", "size_bytes", "width", "height", "resolution", "alt_text"],
  hidden_fields: ["created_at", "updated_at", "deleted_at", "mime_type", "source_path"],
  read_only_fields: ["mime_type", "source_path", "size_bytes", "width", "height", "resolution"],
  field_order: [
    "image",
    "file_name",
    "size_bytes",
    "width",
    "height",
    "resolution",
    "alt_text",
    "mime_type",
    "source_path",
  ],
  header_layout: "inline",
  default_layout: "page",
  default_template_id: null,
  collection_name: "Изображения",
});

const personSchemaJson = JSON.stringify({
  fields: [
    {
      id: "first_name",
      label: "Имя",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "last_name",
      label: "Фамилия",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "patronymic",
      label: "Отчество",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "birth_date",
      label: "Дата рождения",
      kind: "date",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "photo",
      label: "Фотография",
      kind: "relation",
      required: false,
      visible: true,
      read_only: false,
      multiple: false,
      allowed_object_types: [SYSTEM_TYPE_IMAGE_ID],
      system: false,
    },
  ],
});

const personHeaderTemplateJson = JSON.stringify({
  kind: "centered_profile",
  primaryFieldIds: ["first_name", "last_name", "patronymic"],
  secondaryFieldIds: ["birth_date", "photo"],
  imageFieldId: "photo",
});

const personUiSchemaJson = JSON.stringify({
  featured_fields: ["first_name", "last_name", "patronymic"],
  visible_fields: ["first_name", "last_name", "patronymic", "birth_date", "photo"],
  hidden_fields: ["created_at", "updated_at", "deleted_at"],
  read_only_fields: [],
  field_order: ["photo", "first_name", "last_name", "patronymic", "birth_date"],
  header_layout: "column",
  default_layout: "page",
  default_template_id: null,
  collection_name: "Люди",
});

export const SYSTEM_TYPE_IMAGE: NoteType = {
  id: SYSTEM_TYPE_IMAGE_ID,
  name: "Изображение",
  slug: "image",
  icon: "image",
  color: "#38bdf8",
  schema_json: imageSchemaJson,
  header_template_json: imageHeaderTemplateJson,
  ui_schema_json: imageUiSchemaJson,
  created_at: 0,
  updated_at: 0,
};

export const SYSTEM_TYPE_PERSON: NoteType = {
  id: SYSTEM_TYPE_PERSON_ID,
  name: "Человек",
  slug: "person",
  icon: "user",
  color: "#14b8a6",
  schema_json: personSchemaJson,
  header_template_json: personHeaderTemplateJson,
  ui_schema_json: personUiSchemaJson,
  created_at: 0,
  updated_at: 0,
};
