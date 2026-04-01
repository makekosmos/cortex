export type ExerciseType = "weight_reps" | "reps_only" | "distance_duration" | "duration";

export type EquipmentCategory =
  | "barbell"
  | "dumbbell"
  | "machine"
  | "cable"
  | "bodyweight"
  | "none"
  | "other";

export type SetIndicator = "normal" | "warmup" | "dropset";

export type MuscleGroup =
  | "chest"
  | "back"
  | "biceps"
  | "triceps"
  | "shoulders"
  | "forearms"
  | "lats"
  | "upper_back"
  | "lower_back"
  | "quads"
  | "hamstrings"
  | "glutes"
  | "calves"
  | "abdominals"
  | "cardio"
  | "other";

export interface PersonalRecord {
  type: string;
  value: number;
}

export interface HevyExerciseSet {
  id: number;
  index: number;
  indicator: SetIndicator;
  weightKg: number | null;
  reps: number | null;
  distanceMeters: number | null;
  durationSeconds: number | null;
  rpe: number | null;
  personalRecords: PersonalRecord[];
}

export interface HevyExercise {
  id: string;
  title: string;
  exerciseTemplateId: string;
  exerciseType: ExerciseType;
  equipmentCategory: EquipmentCategory;
  muscleGroup: MuscleGroup;
  otherMuscles: string[];
  sets: HevyExerciseSet[];
  notes: string;
  restSeconds: number;
  supersetId: string | null;
  warmupSetCount: number;
  normalSetCount: number;
}

export interface HevyWorkout {
  id: string;
  shortId: string;
  index: number;
  name: string;
  description: string;
  startTime: number;
  endTime: number;
  createdAt: string;
  updatedAt: string;
  exercises: HevyExercise[];
  estimatedVolumeKg: number;
  nthWorkout: number;
}

export interface HevyAccount {
  id: string;
  username: string;
  email: string;
  fullName: string;
  profilePic: string | null;
  followerCount: number;
  followingCount: number;
  createdAt: string;
  lastWorkoutAt: string | null;
}

export type HevyLoginResult = { ok: true; authToken: string } | { ok: false; error: string };

export interface HevyAuthStatus {
  loggedIn: boolean;
  username: string | null;
}

function parseSet(raw: Record<string, unknown>): HevyExerciseSet {
  const prs = Array.isArray(raw.prs) ? raw.prs : [];
  const personalRecords = Array.isArray(raw.personalRecords) ? raw.personalRecords : [];

  return {
    id: raw.id as number,
    index: raw.index as number,
    indicator: (raw.indicator as SetIndicator) || "normal",
    weightKg: (raw.weight_kg as number) ?? null,
    reps: (raw.reps as number) ?? null,
    distanceMeters: (raw.distance_meters as number) ?? null,
    durationSeconds: (raw.duration_seconds as number) ?? null,
    rpe: (raw.rpe as number) ?? null,
    personalRecords: [
      ...prs.map((pr: Record<string, unknown>) => ({
        type: pr.type as string,
        value: pr.value as number,
      })),
      ...personalRecords.map((pr: Record<string, unknown>) => ({
        type: pr.type as string,
        value: pr.value as number,
      })),
    ],
  };
}

function parseExercise(raw: Record<string, unknown>): HevyExercise {
  const sets = Array.isArray(raw.sets) ? raw.sets.map(parseSet) : [];

  return {
    id: raw.id as string,
    title: (raw.title as string) || "",
    exerciseTemplateId: (raw.exercise_template_id as string) || "",
    exerciseType: (raw.exercise_type as ExerciseType) || "weight_reps",
    equipmentCategory: (raw.equipment_category as EquipmentCategory) || "other",
    muscleGroup: (raw.muscle_group as MuscleGroup) || "other",
    otherMuscles: Array.isArray(raw.other_muscles) ? raw.other_muscles : [],
    sets,
    notes: (raw.notes as string) || "",
    restSeconds: (raw.rest_seconds as number) || 0,
    supersetId: (raw.superset_id as string) ?? null,
    warmupSetCount: (raw.warmup_set_count as number) || 0,
    normalSetCount: (raw.normal_set_count as number) || 0,
  };
}

export function parseHevyWorkout(raw: Record<string, unknown>): HevyWorkout {
  const exercises = Array.isArray(raw.exercises) ? raw.exercises.map(parseExercise) : [];

  return {
    id: raw.id as string,
    shortId: (raw.short_id as string) || "",
    index: (raw.index as number) || 0,
    name: (raw.name as string) || "",
    description: (raw.description as string) || "",
    startTime: (raw.start_time as number) || 0,
    endTime: (raw.end_time as number) || 0,
    createdAt: (raw.created_at as string) || "",
    updatedAt: (raw.updated_at as string) || "",
    exercises,
    estimatedVolumeKg: (raw.estimated_volume_kg as number) || 0,
    nthWorkout: (raw.nth_workout as number) || 0,
  };
}

export function parseHevyAccount(raw: Record<string, unknown>): HevyAccount {
  return {
    id: raw.id as string,
    username: (raw.username as string) || "",
    email: (raw.email as string) || "",
    fullName: (raw.full_name as string) || "",
    profilePic: (raw.profile_pic as string) ?? null,
    followerCount: (raw.follower_count as number) || 0,
    followingCount: (raw.following_count as number) || 0,
    createdAt: (raw.created_at as string) || "",
    lastWorkoutAt: (raw.last_workout_at as string) ?? null,
  };
}
