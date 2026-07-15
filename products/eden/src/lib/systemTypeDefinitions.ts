import type { NoteType } from "./typedNotes";
import { BOOK_LANGUAGE_OPTIONS } from "./bookLanguages";
import {
  SYSTEM_TYPE_GAME_ID,
  gameHeaderTemplateJson,
  gameSchemaJson,
  gameUiSchemaJson,
} from "./systemTypeGameDefinitions";
import { SYSTEM_TYPE_IMAGE, SYSTEM_TYPE_PERSON } from "./systemTypeVisualDefinitions";

export {
  SYSTEM_TYPE_GAME_ID,
  gameHeaderTemplateJson,
  gameSchemaJson,
  gameUiSchemaJson,
  shouldUpgradeLegacyGamePresentation,
  shouldUpgradeLegacyGameSchema,
} from "./systemTypeGameDefinitions";
export {
  SYSTEM_TYPE_IMAGE,
  SYSTEM_TYPE_IMAGE_ID,
  SYSTEM_TYPE_PERSON,
  SYSTEM_TYPE_PERSON_ID,
} from "./systemTypeVisualDefinitions";

export const SYSTEM_TYPE_NOTE_ID = "note_obj";
export const SYSTEM_TYPE_BOOK_ID = "book_obj";
export const SYSTEM_TYPE_WORKOUT_ID = "system-type-workout";
export const SYSTEM_TYPE_EXERCISE_ID = "system-type-exercise";
export const SYSTEM_TYPE_JOURNAL_ID = "system-type-journal";
export const SYSTEM_TYPE_COLLECTION_ID = "collection_obj";

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

const bookSchemaJson = JSON.stringify({
  fields: [
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
      id: "author",
      label: "Автор",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "isbn",
      label: "ISBN",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "page_count",
      label: "Страниц",
      kind: "number",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "language",
      label: "Язык",
      kind: "select",
      options: BOOK_LANGUAGE_OPTIONS,
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "publisher",
      label: "Издательство",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "published_date",
      label: "Дата издания",
      kind: "text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
    {
      id: "source_url",
      label: "Источник",
      kind: "url",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
  ],
});

const bookHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: ["author"],
  secondaryFieldIds: [],
  imageFieldId: "cover_image",
});

const bookUiSchemaJson = JSON.stringify({
  featured_fields: ["author"],
  visible_fields: [
    "cover_image",
    "author",
    "isbn",
    "page_count",
    "language",
    "publisher",
    "published_date",
    "source_url",
  ],
  hidden_fields: ["created_at", "updated_at", "deleted_at"],
  read_only_fields: [],
  field_order: [
    "cover_image",
    "author",
    "isbn",
    "page_count",
    "language",
    "publisher",
    "published_date",
    "source_url",
  ],
  header_layout: "inline",
  default_layout: "page",
  default_template_id: null,
  collection_name: "Книги",
});

const collectionSchemaJson = JSON.stringify({
  fields: [
    {
      id: "object_type_id",
      label: "Тип объектов",
      kind: "text",
      required: true,
      visible: true,
      read_only: true,
      system: true,
    },
    {
      id: "description",
      label: "Описание",
      kind: "long_text",
      required: false,
      visible: true,
      read_only: false,
      system: false,
    },
  ],
});

const collectionHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: ["object_type_id"],
  secondaryFieldIds: [],
  imageFieldId: null,
});

const collectionUiSchemaJson = JSON.stringify({
  featured_fields: ["object_type_id"],
  visible_fields: ["object_type_id"],
  hidden_fields: ["description", "created_at", "updated_at", "deleted_at"],
  read_only_fields: ["object_type_id"],
  field_order: ["object_type_id", "description"],
  header_layout: "inline",
  default_layout: "page",
  default_template_id: null,
  collection_name: "Коллекции",
});

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
    collection_name: "Дневники",
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

export const SYSTEM_TYPE_BOOK: NoteType = {
  id: SYSTEM_TYPE_BOOK_ID,
  name: "Книга",
  slug: "book_obj",
  icon: "book",
  color: null,
  schema_json: bookSchemaJson,
  header_template_json: bookHeaderTemplateJson,
  ui_schema_json: bookUiSchemaJson,
  created_at: 0,
  updated_at: 0,
};

export const SYSTEM_TYPE_COLLECTION: NoteType = {
  id: SYSTEM_TYPE_COLLECTION_ID,
  name: "Коллекция",
  slug: "collection",
  icon: "folder",
  color: "#a855f7",
  schema_json: collectionSchemaJson,
  header_template_json: collectionHeaderTemplateJson,
  ui_schema_json: collectionUiSchemaJson,
  created_at: 0,
  updated_at: 0,
};

const SYSTEM_TYPE_GAME: NoteType = {
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

const SYSTEM_TYPE_WORKOUT: NoteType = {
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

const SYSTEM_TYPE_EXERCISE: NoteType = {
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
  SYSTEM_TYPE_BOOK,
  SYSTEM_TYPE_COLLECTION,
  SYSTEM_TYPE_JOURNAL,
  SYSTEM_TYPE_IMAGE,
  SYSTEM_TYPE_PERSON,
  SYSTEM_TYPE_GAME,
  SYSTEM_TYPE_WORKOUT,
  SYSTEM_TYPE_EXERCISE,
];
