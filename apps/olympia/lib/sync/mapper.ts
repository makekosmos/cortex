/**
 * Maps between Olympia domain objects and Ark event format.
 *
 * Ark events follow the schema:
 *   event_type, data (json), category, occurred_at, summary, source, source_id, tags
 */

import type {
  Workout,
  WorkoutExercise,
  WorkoutSet,
  Routine,
  RoutineExercise,
  BodyWeightEntry,
} from "../types";

import type { ArkChange } from "./ark-client";

// ---------------------------------------------------------------------------

// Workout (with nested exercises + sets)

// ---------------------------------------------------------------------------

export interface WorkoutWithDetails extends Workout {
  exercises: (WorkoutExercise & {
    exercise_name?: string;

    sets: WorkoutSet[];
  })[];
}

export function workoutToArkEvent(
  workout: WorkoutWithDetails,

  changeType: "create" | "update" | "delete" = "create",
): ArkChange {
  const totalVolume = workout.exercises.reduce((vol, ex) => {
    return (
      vol +
      ex.sets.reduce((sv, s) => {
        if (s.completed && s.weight_kg && s.reps)
          return sv + s.weight_kg * s.reps;

        return sv;
      }, 0)
    );
  }, 0);

  return {
    event_id: workout.id,

    change_type: changeType,

    data: {
      event_type: "workout",

      category: "fitness",

      source: "olympia",

      source_id: workout.id,

      occurred_at: workout.started_at,

      summary: `${workout.title} (${workout.exercises.length} упр.)`,

      tags: ["fitness", "workout"],

      data: {
        title: workout.title,

        started_at: workout.started_at,

        finished_at: workout.finished_at,

        notes: workout.notes,

        duration_seconds: workout.duration_seconds,

        total_volume_kg: totalVolume,

        exercises: workout.exercises.map((ex) => ({
          id: ex.id,

          exercise_id: ex.exercise_id,

          exercise_name: ex.exercise_name,

          sort_order: ex.sort_order,

          notes: ex.notes,

          sets: ex.sets.map((s) => ({
            id: s.id,

            set_index: s.set_index,

            set_type: s.set_type,

            weight_kg: s.weight_kg,

            reps: s.reps,

            duration_seconds: s.duration_seconds,

            distance_meters: s.distance_meters,

            completed: s.completed,

            rpe: s.rpe,
          })),
        })),
      },
    },
  };
}

export function arkEventToWorkout(event: ArkChange): WorkoutWithDetails | null {
  const d = event.data as Record<string, unknown>;

  const inner = d.data as Record<string, unknown> | undefined;

  if (!inner) return null;

  const exercises = ((inner.exercises as any[]) ?? []).map((ex: any) => ({
    id: ex.id,

    workout_id: (d.source_id as string) || event.event_id,

    exercise_id: ex.exercise_id,

    exercise_name: ex.exercise_name,

    sort_order: ex.sort_order ?? 0,

    notes: ex.notes ?? null,

    sets: ((ex.sets as any[]) ?? []).map((s: any) => ({
      id: s.id,

      workout_exercise_id: ex.id,

      set_index: s.set_index ?? 0,

      set_type: s.set_type ?? "normal",

      weight_kg: s.weight_kg ?? null,

      reps: s.reps ?? null,

      duration_seconds: s.duration_seconds ?? null,

      distance_meters: s.distance_meters ?? null,

      completed: s.completed ?? false,

      rpe: s.rpe ?? null,
    })),
  }));

  return {
    id: (d.source_id as string) || event.event_id,

    title: (inner.title as string) ?? "",

    started_at:
      (inner.started_at as string) ??
      (d.occurred_at as string) ??
      new Date().toISOString(),

    finished_at: (inner.finished_at as string) ?? null,

    notes: (inner.notes as string) ?? null,

    duration_seconds: (inner.duration_seconds as number) ?? null,

    exercises,
  };
}

// ---------------------------------------------------------------------------

// Routine (with nested exercises)

// ---------------------------------------------------------------------------

export interface RoutineWithExercises extends Routine {
  exercises: (RoutineExercise & { exercise_name?: string })[];
}

export function routineToArkEvent(
  routine: RoutineWithExercises,

  changeType: "create" | "update" | "delete" = "create",
): ArkChange {
  return {
    event_id: routine.id,

    change_type: changeType,

    data: {
      event_type: "routine",

      category: "fitness",

      source: "olympia",

      source_id: routine.id,

      occurred_at: routine.created_at,

      summary: `${routine.title} (${routine.exercises.length} упр.)`,

      tags: ["fitness", "routine"],

      data: {
        title: routine.title,

        created_at: routine.created_at,

        updated_at: routine.updated_at,

        exercises: routine.exercises.map((ex) => ({
          id: ex.id,

          exercise_id: ex.exercise_id,

          exercise_name: ex.exercise_name,

          sort_order: ex.sort_order,

          target_sets: ex.target_sets,

          target_reps: ex.target_reps,

          target_weight_kg: ex.target_weight_kg,

          notes: ex.notes,
        })),
      },
    },
  };
}

export function arkEventToRoutine(
  event: ArkChange,
): RoutineWithExercises | null {
  const d = event.data as Record<string, unknown>;

  const inner = d.data as Record<string, unknown> | undefined;

  if (!inner) return null;

  const exercises = ((inner.exercises as any[]) ?? []).map((ex: any) => ({
    id: ex.id,

    routine_id: (d.source_id as string) || event.event_id,

    exercise_id: ex.exercise_id,

    exercise_name: ex.exercise_name,

    sort_order: ex.sort_order ?? 0,

    target_sets: ex.target_sets ?? null,

    target_reps: ex.target_reps ?? null,

    target_weight_kg: ex.target_weight_kg ?? null,

    notes: ex.notes ?? null,
  }));

  return {
    id: (d.source_id as string) || event.event_id,

    title: (inner.title as string) ?? "",

    created_at: (inner.created_at as string) ?? new Date().toISOString(),

    updated_at: (inner.updated_at as string) ?? null,

    exercises,
  };
}

// ---------------------------------------------------------------------------

// Body Weight

// ---------------------------------------------------------------------------

export function bodyWeightToArkEvent(
  entry: BodyWeightEntry,

  changeType: "create" | "update" | "delete" = "create",
): ArkChange {
  return {
    event_id: entry.id,

    change_type: changeType,

    data: {
      event_type: "body_weight",

      category: "health",

      source: "olympia",

      source_id: entry.id,

      occurred_at: entry.date,

      summary: `${entry.weight_kg} кг`,

      tags: ["health", "body_weight"],

      data: {
        date: entry.date,

        weight_kg: entry.weight_kg,

        notes: entry.notes,
      },
    },
  };
}

export function arkEventToBodyWeight(event: ArkChange): BodyWeightEntry | null {
  const d = event.data as Record<string, unknown>;

  const inner = d.data as Record<string, unknown> | undefined;

  if (!inner) return null;

  return {
    id: (d.source_id as string) || event.event_id,

    date:
      (inner.date as string) ??
      (d.occurred_at as string) ??
      new Date().toISOString().split("T")[0],

    weight_kg: (inner.weight_kg as number) ?? 0,

    notes: (inner.notes as string) ?? null,
  };
}
