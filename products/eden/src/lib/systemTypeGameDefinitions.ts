import { parseNoteTypeDefinition, parseNoteTypeUiSchema, type NoteType } from "./typedNotes";

export const SYSTEM_TYPE_GAME_ID = "game_obj";

export const gameSchemaJson = JSON.stringify({
  fields: [
    {
      id: "description",
      label: "Описание",
      kind: "long_text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "user_rating",
      label: "Оценка",
      kind: "number",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "play_status",
      label: "Статус",
      kind: "select",
      required: false,
      visible: true,
      read_only: false,
      options: ["not_started", "in_progress", "completed", "abandoned"],
      system: false,
    },
    {
      id: "genres",
      label: "Жанры",
      kind: "multi_select",
      required: false,
      visible: true,
      read_only: false,
      options: [
        "Action",
        "Adventure",
        "RPG",
        "Strategy",
        "Simulation",
        "Shooter",
        "Puzzle",
        "Platformer",
        "Racing",
        "Sports",
        "Survival",
        "Horror",
        "Sandbox",
        "Indie",
      ],
      system: false,
    },
    {
      id: "cover_image",
      label: "Обложка",
      kind: "image",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "background_image",
      label: "Фон",
      kind: "image",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "related_notes",
      label: "Связанные заметки",
      kind: "relation",
      required: false,
      visible: true,
      read_only: false,
      link_type: "related",
      system: false,
    },
    {
      id: "exe_path",
      label: "Путь к игре",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: true,
    },
    {
      id: "save_path",
      label: "Путь к сейвам",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: true,
    },
    {
      id: "total_playtime_seconds",
      label: "Время игры",
      kind: "number",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "last_played_at",
      label: "Последний запуск",
      kind: "date",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "play_count",
      label: "Запусков",
      kind: "number",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "save_exists",
      label: "Сейв найден",
      kind: "boolean",
      required: false,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "rawg_id",
      label: "RAWG ID",
      kind: "text",
      required: false,
      visible: false,
      read_only: true,
      system: true,
    },
    {
      id: "exe_name",
      label: "Имя exe",
      kind: "text",
      required: false,
      visible: false,
      read_only: true,
      system: true,
    },
  ],
});

export const gameHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: [],
  secondaryFieldIds: ["play_status", "genres", "total_playtime_seconds", "last_played_at"],
  imageFieldId: null,
});

const LEGACY_GAME_FEATURED_FIELDS = [
  "play_status",
  "genres",
  "user_rating",
  "total_playtime_seconds",
  "last_played_at",
  "play_count",
  "save_exists",
] as const;

const LEGACY_GAME_VISIBLE_FIELDS = [
  "description",
  "play_status",
  "genres",
  "user_rating",
  "cover_image",
  "background_image",
  "related_notes",
  "exe_path",
  "save_path",
  "total_playtime_seconds",
  "last_played_at",
  "play_count",
  "save_exists",
] as const;

const GAME_READ_ONLY_FIELDS = [
  "total_playtime_seconds",
  "last_played_at",
  "play_count",
  "save_exists",
  "rawg_id",
  "exe_name",
] as const;

export const gameUiSchemaJson = JSON.stringify({
  featured_fields: [],
  visible_fields: ["play_status", "genres", "total_playtime_seconds", "last_played_at"],
  hidden_fields: [
    "created_at",
    "updated_at",
    "deleted_at",
    "description",
    "user_rating",
    "cover_image",
    "background_image",
    "related_notes",
    "exe_path",
    "save_path",
    "play_count",
    "save_exists",
    "rawg_id",
    "exe_name",
    "sync_source",
  ],
  read_only_fields: [
    "total_playtime_seconds",
    "last_played_at",
    "play_count",
    "save_exists",
    "rawg_id",
    "exe_name",
  ],
  field_order: [
    "play_status",
    "genres",
    "total_playtime_seconds",
    "last_played_at",
    "description",
    "user_rating",
    "play_count",
    "save_exists",
    "cover_image",
    "background_image",
    "related_notes",
    "exe_path",
    "save_path",
    "rawg_id",
    "exe_name",
  ],
  header_layout: "inline",
  default_layout: "page",
  default_template_id: null,
  collection_name: "Игры",
});

function arraysEqual(left: readonly string[] | undefined, right: readonly string[]): boolean {
  if ((left?.length ?? 0) !== right.length) {
    return false;
  }

  return right.every((value, index) => left?.[index] === value);
}

export function shouldUpgradeLegacyGamePresentation(noteType: NoteType): boolean {
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);

  return (
    arraysEqual(uiSchema.featured_fields, LEGACY_GAME_FEATURED_FIELDS) &&
    arraysEqual(uiSchema.visible_fields, LEGACY_GAME_VISIBLE_FIELDS) &&
    arraysEqual(uiSchema.read_only_fields, GAME_READ_ONLY_FIELDS) &&
    (uiSchema.header_layout ?? "inline") === "column"
  );
}

export function shouldUpgradeLegacyGameSchema(noteType: NoteType): boolean {
  try {
    const definition = parseNoteTypeDefinition(noteType.schema_json);
    const genresField = definition.fields.find((field) => field.id === "genres");
    const playStatusField = definition.fields.find((field) => field.id === "play_status");

    return (
      genresField?.kind !== "multi_select" ||
      (genresField.options?.length ?? 0) === 0 ||
      playStatusField?.kind !== "select"
    );
  } catch {
    return true;
  }
}
