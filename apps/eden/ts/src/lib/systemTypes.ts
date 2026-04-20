import type { NoteType } from "@/lib/typedNotes";

export const SYSTEM_TYPE_NOTE_ID = "note_obj";
export const SYSTEM_TYPE_GAME_ID = "game_obj";
export const SYSTEM_TYPE_WORKOUT_ID = "system-type-workout";
export const SYSTEM_TYPE_EXERCISE_ID = "system-type-exercise";

const noteSchemaJson = JSON.stringify({
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
      id: "related_notes",
      label: "Связанные заметки",
      kind: "relation",
      required: false,
      visible: true,
      read_only: false,
      link_type: "related",
      system: false,
    },
  ],
});

const noteHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: [],
  secondaryFieldIds: [],
  imageFieldId: null,
});

const noteUiSchemaJson = JSON.stringify({
  featured_fields: [],
  visible_fields: [],
  hidden_fields: ["description", "related_notes", "created_at", "updated_at", "deleted_at"],
  read_only_fields: [],
  field_order: ["description", "related_notes"],
  header_layout: "inline",
  default_layout: "page",
  default_template_id: null,
  collection_name: "Заметки",
});

const gameSchemaJson = JSON.stringify({
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
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
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

const gameHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: ["genres", "play_status", "user_rating"],
  secondaryFieldIds: ["total_playtime_seconds", "last_played_at", "play_count", "save_exists"],
  imageFieldId: "cover_image",
});

const gameUiSchemaJson = JSON.stringify({
  featured_fields: [
    "play_status",
    "genres",
    "user_rating",
    "total_playtime_seconds",
    "last_played_at",
    "play_count",
    "save_exists",
  ],
  visible_fields: [
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
  ],
  hidden_fields: ["created_at", "updated_at", "deleted_at", "rawg_id", "exe_name", "sync_source"],
  read_only_fields: [
    "total_playtime_seconds",
    "last_played_at",
    "play_count",
    "save_exists",
    "rawg_id",
    "exe_name",
  ],
  field_order: [
    "description",
    "play_status",
    "genres",
    "user_rating",
    "total_playtime_seconds",
    "last_played_at",
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
  header_layout: "column",
  default_layout: "page",
  default_template_id: null,
  collection_name: "Игры",
});

const workoutSchemaJson = JSON.stringify({
  fields: [
    { id: "date", label: "Дата", kind: "date", required: true },
    { id: "duration_min", label: "Длительность (мин)", kind: "number", required: false },
    { id: "volume_kg", label: "Объем (кг)", kind: "number", required: false },
    { id: "exercise_count", label: "Упражнений", kind: "number", required: false },
    { id: "source", label: "Источник", kind: "text", required: false },
    { id: "hevy_id", label: "Hevy ID", kind: "text", required: false },
  ],
});

const workoutHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: ["date", "duration_min"],
  secondaryFieldIds: ["volume_kg", "exercise_count"],
  imageFieldId: null,
});

const exerciseSchemaJson = JSON.stringify({
  fields: [
    { id: "exercise_name", label: "Упражнение", kind: "text", required: true },
    {
      id: "exercise_type",
      label: "Тип",
      kind: "select",
      required: false,
      options: ["weight_reps", "reps_only", "distance_duration", "duration"],
    },
    {
      id: "equipment",
      label: "Оборудование",
      kind: "select",
      required: false,
      options: ["barbell", "dumbbell", "machine", "cable", "bodyweight", "none", "other"],
    },
    { id: "muscle_group", label: "Группа мышц", kind: "text", required: false },
    { id: "sets_summary", label: "Подходы", kind: "long_text", required: false },
    { id: "best_set", label: "Лучший подход", kind: "text", required: false },
    { id: "total_volume_kg", label: "Объем (кг)", kind: "number", required: false },
    { id: "notes", label: "Заметки", kind: "long_text", required: false },
  ],
});

const exerciseHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: ["exercise_name", "muscle_group"],
  secondaryFieldIds: ["sets_summary", "best_set"],
  imageFieldId: null,
});

export const SYSTEM_TYPE_NOTE: NoteType = {
  id: SYSTEM_TYPE_NOTE_ID,
  name: "Заметка",
  slug: "note_obj",
  icon: "document-text",
  color: "#2aa7ee",
  schema_json: noteSchemaJson,
  header_template_json: noteHeaderTemplateJson,
  ui_schema_json: noteUiSchemaJson,
  created_at: 0,
  updated_at: 0,
};

export const SYSTEM_TYPE_GAME: NoteType = {
  id: SYSTEM_TYPE_GAME_ID,
  name: "Игра",
  slug: "game_obj",
  icon: "game-controller",
  color: "#ef4444",
  schema_json: gameSchemaJson,
  header_template_json: gameHeaderTemplateJson,
  ui_schema_json: gameUiSchemaJson,
  created_at: 0,
  updated_at: 0,
};

export const SYSTEM_TYPE_WORKOUT: NoteType = {
  id: SYSTEM_TYPE_WORKOUT_ID,
  name: "Тренировка",
  slug: "workout",
  icon: "barbell",
  color: "#f97316",
  schema_json: workoutSchemaJson,
  header_template_json: workoutHeaderTemplateJson,
  ui_schema_json: JSON.stringify({
    featured_fields: ["date", "duration_min"],
    visible_fields: ["date", "duration_min", "volume_kg", "exercise_count", "source"],
    hidden_fields: ["created_at", "updated_at", "deleted_at", "hevy_id"],
    read_only_fields: [],
    field_order: ["date", "duration_min", "volume_kg", "exercise_count", "source", "hevy_id"],
    header_layout: "inline",
    default_layout: "page",
    default_template_id: null,
    collection_name: "Тренировки",
  }),
  created_at: 0,
  updated_at: 0,
};

export const SYSTEM_TYPE_EXERCISE: NoteType = {
  id: SYSTEM_TYPE_EXERCISE_ID,
  name: "Упражнение",
  slug: "exercise",
  icon: "fitness",
  color: "#22c55e",
  schema_json: exerciseSchemaJson,
  header_template_json: exerciseHeaderTemplateJson,
  ui_schema_json: JSON.stringify({
    featured_fields: ["exercise_name", "muscle_group"],
    visible_fields: [
      "exercise_name",
      "exercise_type",
      "equipment",
      "muscle_group",
      "sets_summary",
      "best_set",
      "total_volume_kg",
      "notes",
    ],
    hidden_fields: ["created_at", "updated_at", "deleted_at"],
    read_only_fields: [],
    field_order: [
      "exercise_name",
      "exercise_type",
      "equipment",
      "muscle_group",
      "sets_summary",
      "best_set",
      "total_volume_kg",
      "notes",
    ],
    header_layout: "inline",
    default_layout: "page",
    default_template_id: null,
    collection_name: "Упражнения",
  }),
  created_at: 0,
  updated_at: 0,
};

export const SYSTEM_TYPES: NoteType[] = [
  SYSTEM_TYPE_NOTE,
  SYSTEM_TYPE_GAME,
  SYSTEM_TYPE_WORKOUT,
  SYSTEM_TYPE_EXERCISE,
];

export function isSystemType(noteTypeId: string): boolean {
  return (
    noteTypeId === SYSTEM_TYPE_NOTE_ID ||
    noteTypeId === SYSTEM_TYPE_GAME_ID ||
    noteTypeId === SYSTEM_TYPE_WORKOUT_ID ||
    noteTypeId === SYSTEM_TYPE_EXERCISE_ID
  );
}
