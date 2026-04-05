import { create } from "zustand";

import { getExercises as dbGetExercises } from "../database";

interface ExercisesState {
  exercises: any[];

  loaded: boolean;

  load: () => Promise<void>;

  reload: () => Promise<void>;
}

export const useExercisesStore = create<ExercisesState>((set, get) => ({
  exercises: [],

  loaded: false,

  load: async () => {
    if (get().loaded) return;

    const data = await dbGetExercises();

    set({ exercises: data, loaded: true });
  },

  reload: async () => {
    const data = await dbGetExercises();

    set({ exercises: data, loaded: true });
  },
}));
