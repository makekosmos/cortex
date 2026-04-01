import type { NoteType } from "@/lib/typedNotes";

export const SYSTEM_TYPE_WORKOUT_ID = "system-type-workout";
export const SYSTEM_TYPE_EXERCISE_ID = "system-type-exercise";

const workoutSchemaJson = JSON.stringify({
  fields: [
    { id: "date", label: "Дата", kind: "date", required: true },
    { id: "duration_min", label: "Длительность (мин)", kind: "number", required: false },
    { id: "volume_kg", label: "Объём (кг)", kind: "number", required: false },
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
    { id: "total_volume_kg", label: "Объём (кг)", kind: "number", required: false },
    { id: "notes", label: "Заметки", kind: "long_text", required: false },
  ],
});

const exerciseHeaderTemplateJson = JSON.stringify({
  kind: "default",
  primaryFieldIds: ["exercise_name", "muscle_group"],
  secondaryFieldIds: ["sets_summary", "best_set"],
  imageFieldId: null,
});

export const SYSTEM_TYPE_WORKOUT: NoteType = {
  id: SYSTEM_TYPE_WORKOUT_ID,
  name: "Тренировка",
  slug: "workout",
  icon: "barbell",
  color: "#f97316",
  schema_json: workoutSchemaJson,
  header_template_json: workoutHeaderTemplateJson,
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
  created_at: 0,
  updated_at: 0,
};

export const SYSTEM_TYPES: NoteType[] = [SYSTEM_TYPE_WORKOUT, SYSTEM_TYPE_EXERCISE];

export function isSystemType(noteTypeId: string): boolean {
  return noteTypeId === SYSTEM_TYPE_WORKOUT_ID || noteTypeId === SYSTEM_TYPE_EXERCISE_ID;
}
