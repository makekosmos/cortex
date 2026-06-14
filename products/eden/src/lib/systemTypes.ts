import { parseNoteTypeDefinition, parseNoteTypeUiSchema, type NoteType } from "./typedNotes";

export const SYSTEM_TYPE_NOTE_ID = "note_obj";
export const SYSTEM_TYPE_GAME_ID = "game_obj";
export const SYSTEM_TYPE_IMAGE_ID = "image_obj";
export const SYSTEM_TYPE_PERSON_ID = "person_obj";
export const SYSTEM_TYPE_WORKOUT_ID = "system-type-workout";
export const SYSTEM_TYPE_EXERCISE_ID = "system-type-exercise";
export const SYSTEM_TYPE_JOURNAL_ID = "system-type-journal";

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

const gameHeaderTemplateJson = JSON.stringify({
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

const gameUiSchemaJson = JSON.stringify({
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

function shouldUpgradeLegacyGamePresentation(noteType: NoteType): boolean {
  const uiSchema = parseNoteTypeUiSchema(noteType.ui_schema_json);

  return (
    arraysEqual(uiSchema.featured_fields, LEGACY_GAME_FEATURED_FIELDS) &&
    arraysEqual(uiSchema.visible_fields, LEGACY_GAME_VISIBLE_FIELDS) &&
    arraysEqual(uiSchema.read_only_fields, GAME_READ_ONLY_FIELDS) &&
    (uiSchema.header_layout ?? "inline") === "column"
  );
}

function shouldUpgradeLegacyGameSchema(noteType: NoteType): boolean {
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

const workoutSchemaJson = JSON.stringify({
  fields: [
    { id: "date", label: "Дата", kind: "date", required: true },
    { id: "duration_min", label: "Длительность (мин)", kind: "number", required: false },
    { id: "volume_kg", label: "Объем (кг)", kind: "number", required: false },
    { id: "exercise_count", label: "Упражнений", kind: "number", required: false },
    { id: "source", label: "Источник", kind: "text", required: false },
    { id: "external_id", label: "Внешний ID", kind: "text", required: false },
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

export const SYSTEM_TYPE_JOURNAL: NoteType = {
  id: SYSTEM_TYPE_JOURNAL_ID,
  name: "Дневник",
  slug: "journal",
  icon: "document-text",
  color: "#a855f7",
  schema_json: noteSchemaJson,
  header_template_json: noteHeaderTemplateJson,
  ui_schema_json: JSON.stringify({
    featured_fields: [],
    visible_fields: [],
    hidden_fields: ["description", "related_notes", "created_at", "updated_at", "deleted_at"],
    read_only_fields: [],
    field_order: ["description", "related_notes"],
    header_layout: "inline",
    default_layout: "page",
    default_template_id: null,
    collection_name: "Дневник",
  }),
  created_at: 0,
  updated_at: 0,
};

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
    hidden_fields: ["created_at", "updated_at", "deleted_at", "external_id"],
    read_only_fields: [],
    field_order: ["date", "duration_min", "volume_kg", "exercise_count", "source", "external_id"],
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
  SYSTEM_TYPE_JOURNAL,
  SYSTEM_TYPE_IMAGE,
  SYSTEM_TYPE_PERSON,
  SYSTEM_TYPE_GAME,
  SYSTEM_TYPE_WORKOUT,
  SYSTEM_TYPE_EXERCISE,
];

export function normalizeSystemNoteType(noteType: NoteType): NoteType {
  const systemType = SYSTEM_TYPES.find((candidate) => candidate.id === noteType.id);
  if (systemType && noteType.id !== SYSTEM_TYPE_GAME_ID) {
    return {
      ...systemType,
      created_at: noteType.created_at || systemType.created_at,
      updated_at: noteType.updated_at || systemType.updated_at,
    };
  }

  if (noteType.id !== SYSTEM_TYPE_GAME_ID) {
    return noteType;
  }

  if (
    shouldUpgradeLegacyGameSchema(noteType) ||
    !noteType.ui_schema_json?.trim() ||
    shouldUpgradeLegacyGamePresentation(noteType)
  ) {
    return {
      ...noteType,
      schema_json: gameSchemaJson,
      header_template_json: gameHeaderTemplateJson,
      ui_schema_json: gameUiSchemaJson,
    };
  }

  return noteType;
}

export function isSystemType(noteTypeId: string): boolean {
  return (
    noteTypeId === SYSTEM_TYPE_NOTE_ID ||
    noteTypeId === SYSTEM_TYPE_JOURNAL_ID ||
    noteTypeId === SYSTEM_TYPE_IMAGE_ID ||
    noteTypeId === SYSTEM_TYPE_PERSON_ID ||
    noteTypeId === SYSTEM_TYPE_GAME_ID ||
    noteTypeId === SYSTEM_TYPE_WORKOUT_ID ||
    noteTypeId === SYSTEM_TYPE_EXERCISE_ID
  );
}
