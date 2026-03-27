export interface Exercise {
  id: string;
  name: string;
  force: string | null;
  level: string;
  mechanic: string | null;
  equipment: string | null;
  primary_muscles: string[];
  secondary_muscles: string[];
  instructions: string[];
  images: string[];
  category: string | null;
  is_custom: boolean;
}

export interface Workout {
  id: string;
  title: string;
  started_at: string;
  finished_at: string | null;
  notes: string | null;
  duration_seconds: number | null;
}

export interface WorkoutExercise {
  id: string;
  workout_id: string;
  exercise_id: string;
  sort_order: number;
  notes: string | null;
  exercise?: Exercise;
  sets?: WorkoutSet[];
}

export interface WorkoutSet {
  id: string;
  workout_exercise_id: string;
  set_index: number;
  set_type: 'normal' | 'warmup' | 'dropset' | 'failure';
  weight_kg: number | null;
  reps: number | null;
  duration_seconds: number | null;
  distance_meters: number | null;
  completed: boolean;
  rpe: number | null;
}

export interface Routine {
  id: string;
  title: string;
  created_at: string;
  updated_at: string | null;
}

export interface RoutineExercise {
  id: string;
  routine_id: string;
  exercise_id: string;
  sort_order: number;
  target_sets: number | null;
  target_reps: string | null;
  target_weight_kg: number | null;
  notes: string | null;
  exercise?: Exercise;
}

export interface BodyWeightEntry {
  id: string;
  date: string;
  weight_kg: number;
  notes: string | null;
}

// For the active workout in progress
export interface ActiveWorkoutExercise {
  id: string;
  exercise: Exercise;
  sort_order: number;
  notes: string;
  sets: ActiveWorkoutSet[];
  supersetGroup: number | null; // same number = same superset
  restSeconds: number | null; // null = use global default
}

export interface ActiveWorkoutSet {
  id: string;
  set_index: number;
  set_type: 'normal' | 'warmup' | 'dropset' | 'failure';
  weight_kg: string; // string for input handling
  reps: string;
  completed: boolean;
  rpe: number | null;
  // Placeholders from previous workout
  prev_weight_kg: number | null;
  prev_reps: number | null;
}

export type MuscleGroup =
  | 'abdominals'
  | 'abductors'
  | 'adductors'
  | 'biceps'
  | 'calves'
  | 'chest'
  | 'forearms'
  | 'glutes'
  | 'hamstrings'
  | 'lats'
  | 'lower back'
  | 'middle back'
  | 'neck'
  | 'quadriceps'
  | 'shoulders'
  | 'traps'
  | 'triceps';

export const MUSCLE_GROUPS: MuscleGroup[] = [
  'abdominals', 'abductors', 'adductors', 'biceps', 'calves',
  'chest', 'forearms', 'glutes', 'hamstrings', 'lats',
  'lower back', 'middle back', 'neck', 'quadriceps',
  'shoulders', 'traps', 'triceps',
];

export const EQUIPMENT_LIST = [
  'barbell', 'dumbbell', 'kettlebells', 'machine', 'cable',
  'body only', 'bands', 'foam roll', 'e-z curl bar',
  'medicine ball', 'exercise ball', 'other',
];

export const MUSCLE_NAME_RU: Record<string, string> = {
  'abdominals': 'Пресс',
  'abductors': 'Отводящие',
  'adductors': 'Приводящие',
  'biceps': 'Бицепс',
  'calves': 'Икры',
  'chest': 'Грудь',
  'forearms': 'Предплечья',
  'glutes': 'Ягодицы',
  'hamstrings': 'Бицепс бедра',
  'lats': 'Широчайшие',
  'lower back': 'Поясница',
  'middle back': 'Средняя спина',
  'neck': 'Шея',
  'quadriceps': 'Квадрицепс',
  'shoulders': 'Плечи',
  'traps': 'Трапеции',
  'triceps': 'Трицепс',
};

export function muscleRu(name: string): string {
  return MUSCLE_NAME_RU[name.toLowerCase()] || name;
}

export function musclesRu(names: string[]): string {
  return names.map(muscleRu).join(', ');
}
