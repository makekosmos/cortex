import { create } from 'zustand';
import { generateId } from '../id';
import { Exercise, ActiveWorkoutExercise, ActiveWorkoutSet } from '../types';
import { saveWorkout, getLastSetsForExercise } from '../database';

interface WorkoutStore {
  isActive: boolean;
  workoutId: string;
  title: string;
  startedAt: Date | null;
  exercises: ActiveWorkoutExercise[];
  activeExerciseId: string | null;
  restTimerEnd: Date | null;
  restTimerDuration: number;
  nextSupersetGroup: number;

  startWorkout: (title?: string) => void;
  startFromRoutine: (title: string, exercises: { exercise: Exercise; targetSets: number; targetWeight: number | null }[]) => Promise<void>;
  setTitle: (title: string) => void;
  setActiveExercise: (id: string) => void;
  addExercise: (exercise: Exercise) => Promise<void>;
  removeExercise: (exerciseId: string) => void;
  moveExercise: (fromIndex: number, toIndex: number) => void;
  addSet: (exerciseId: string) => void;
  removeSet: (exerciseId: string, setId: string) => void;
  updateSet: (exerciseId: string, setId: string, field: string, value: any) => void;
  toggleSetCompleted: (exerciseId: string, setId: string) => void;
  setExerciseRest: (exerciseId: string, seconds: number | null) => void;
  linkSuperset: (exerciseIds: string[]) => void;
  unlinkSuperset: (exerciseId: string) => void;
  getRestForExercise: (exerciseId: string, globalDefault: number) => number;
  startRestTimer: (seconds: number) => void;
  clearRestTimer: () => void;
  finishWorkout: () => Promise<void>;
  cancelWorkout: () => void;
}

export const useWorkoutStore = create<WorkoutStore>((set, get) => ({
  isActive: false,
  workoutId: '',
  title: '',
  startedAt: null,
  exercises: [],
  activeExerciseId: null,
  restTimerEnd: null,
  restTimerDuration: 0,
  nextSupersetGroup: 1,

  startWorkout: (title?: string) => {
    set({
      isActive: true,
      workoutId: generateId(),
      title: title || `Тренировка ${new Date().toLocaleDateString('ru-RU')}`,
      startedAt: new Date(),
      exercises: [],
      activeExerciseId: null,
      restTimerEnd: null,
      nextSupersetGroup: 1,
    });
  },

  startFromRoutine: async (title, routineExercises) => {
    const exercises: ActiveWorkoutExercise[] = [];

    for (let i = 0; i < routineExercises.length; i++) {
      const { exercise, targetSets, targetWeight } = routineExercises[i];
      const lastSets = await getLastSetsForExercise(exercise.id);

      const sets: ActiveWorkoutSet[] = [];
      const numSets = targetSets || 3;
      for (let s = 0; s < numSets; s++) {
        const prev = lastSets[s];
        sets.push({
          id: generateId(),
          set_index: s,
          set_type: 'normal',
          weight_kg: targetWeight?.toString() || '',
          reps: '',
          completed: false,
          rpe: null,
          prev_weight_kg: prev?.weight_kg ?? null,
          prev_reps: prev?.reps ?? null,
        });
      }

      exercises.push({
        id: generateId(),
        exercise,
        sort_order: i,
        notes: '',
        sets,
        supersetGroup: null,
        restSeconds: null,
      });
    }

    set({
      isActive: true,
      workoutId: generateId(),
      title,
      startedAt: new Date(),
      exercises,
      activeExerciseId: exercises[0]?.id || null,
      restTimerEnd: null,
      nextSupersetGroup: 1,
    });
  },

  setTitle: (title) => set({ title }),

  setActiveExercise: (id) => set({ activeExerciseId: id }),

  addExercise: async (exercise) => {
    const { exercises } = get();
    const lastSets = await getLastSetsForExercise(exercise.id);

    const defaultSets: ActiveWorkoutSet[] = Array.from({ length: 3 }, (_, i) => {
      const prev = lastSets[i];
      return {
        id: generateId(),
        set_index: i,
        set_type: 'normal' as const,
        weight_kg: '',
        reps: '',
        completed: false,
        rpe: null,
        prev_weight_kg: prev?.weight_kg ?? null,
        prev_reps: prev?.reps ?? null,
      };
    });

    const newEx: ActiveWorkoutExercise = {
      id: generateId(),
      exercise,
      sort_order: exercises.length,
      notes: '',
      sets: defaultSets,
      supersetGroup: null,
      restSeconds: null,
    };

    set({
      exercises: [...exercises, newEx],
      activeExerciseId: newEx.id,
    });
  },

  removeExercise: (exerciseId) => {
    const { exercises, activeExerciseId } = get();
    const filtered = exercises.filter((e) => e.id !== exerciseId);
    set({
      exercises: filtered,
      activeExerciseId: activeExerciseId === exerciseId
        ? (filtered[0]?.id || null)
        : activeExerciseId,
    });
  },

  moveExercise: (fromIndex, toIndex) => {
    const exercises = [...get().exercises];
    const [moved] = exercises.splice(fromIndex, 1);
    exercises.splice(toIndex, 0, moved);
    exercises.forEach((e, i) => (e.sort_order = i));
    set({ exercises });
  },

  addSet: (exerciseId) => {
    set({
      exercises: get().exercises.map((ex) => {
        if (ex.id !== exerciseId) return ex;
        return {
          ...ex,
          sets: [
            ...ex.sets,
            {
              id: generateId(),
              set_index: ex.sets.length,
              set_type: 'normal' as const,
              weight_kg: '',
              reps: '',
              completed: false,
              rpe: null,
              prev_weight_kg: null,
              prev_reps: null,
            },
          ],
        };
      }),
    });
  },

  removeSet: (exerciseId, setId) => {
    set({
      exercises: get().exercises.map((ex) => {
        if (ex.id !== exerciseId) return ex;
        const sets = ex.sets
          .filter((s) => s.id !== setId)
          .map((s, i) => ({ ...s, set_index: i }));
        return { ...ex, sets };
      }),
    });
  },

  updateSet: (exerciseId, setId, field, value) => {
    set({
      exercises: get().exercises.map((ex) => {
        if (ex.id !== exerciseId) return ex;
        return {
          ...ex,
          sets: ex.sets.map((s) => {
            if (s.id !== setId) return s;
            return { ...s, [field]: value };
          }),
        };
      }),
    });
  },

  toggleSetCompleted: (exerciseId, setId) => {
    set({
      exercises: get().exercises.map((ex) => {
        if (ex.id !== exerciseId) return ex;
        return {
          ...ex,
          sets: ex.sets.map((s) => {
            if (s.id !== setId) return s;
            return { ...s, completed: !s.completed };
          }),
        };
      }),
    });
  },

  setExerciseRest: (exerciseId, seconds) => {
    set({
      exercises: get().exercises.map((ex) =>
        ex.id === exerciseId ? { ...ex, restSeconds: seconds } : ex
      ),
    });
  },

  getRestForExercise: (exerciseId, globalDefault) => {
    const ex = get().exercises.find((e) => e.id === exerciseId);
    return ex?.restSeconds ?? globalDefault;
  },

  linkSuperset: (exerciseIds) => {
    const { nextSupersetGroup, exercises } = get();
    set({
      exercises: exercises.map((ex) =>
        exerciseIds.includes(ex.id) ? { ...ex, supersetGroup: nextSupersetGroup } : ex
      ),
      nextSupersetGroup: nextSupersetGroup + 1,
    });
  },

  unlinkSuperset: (exerciseId) => {
    set({
      exercises: get().exercises.map((ex) =>
        ex.id === exerciseId ? { ...ex, supersetGroup: null } : ex
      ),
    });
  },

  startRestTimer: (seconds) => {
    set({
      restTimerEnd: new Date(Date.now() + seconds * 1000),
      restTimerDuration: seconds,
    });
  },

  clearRestTimer: () => {
    set({ restTimerEnd: null, restTimerDuration: 0 });
  },

  finishWorkout: async () => {
    const state = get();
    if (!state.startedAt) return;

    const now = new Date();
    const durationSeconds = Math.floor((now.getTime() - state.startedAt.getTime()) / 1000);

    const workoutData = {
      id: state.workoutId,
      title: state.title,
      started_at: state.startedAt.toISOString(),
      finished_at: now.toISOString(),
      notes: '',
      duration_seconds: durationSeconds,
    };

    const exercisesData = state.exercises
      .filter((ex) => ex.sets.some((s) => s.completed))
      .map((ex) => ({
        id: ex.id,
        exercise_id: ex.exercise.id,
        sort_order: ex.sort_order,
        notes: ex.notes,
        sets: ex.sets
          .filter((s) => s.completed)
          .map((s) => ({
            id: s.id,
            set_index: s.set_index,
            set_type: s.set_type,
            weight_kg: s.weight_kg ? parseFloat(s.weight_kg) : null,
            reps: s.reps ? parseInt(s.reps, 10) : null,
            completed: true,
            rpe: s.rpe,
          })),
      }));

    await saveWorkout(workoutData, exercisesData);

    set({
      isActive: false,
      workoutId: '',
      title: '',
      startedAt: null,
      exercises: [],
      activeExerciseId: null,
      restTimerEnd: null,
      restTimerDuration: 0,
      nextSupersetGroup: 1,
    });
  },

  cancelWorkout: () => {
    set({
      isActive: false,
      workoutId: '',
      title: '',
      startedAt: null,
      exercises: [],
      activeExerciseId: null,
      restTimerEnd: null,
      restTimerDuration: 0,
      nextSupersetGroup: 1,
    });
  },
}));
