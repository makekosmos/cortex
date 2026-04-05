import { create } from "zustand";

import { format } from "date-fns";

import type { MealEntry, MealType, FoodItem, Macros } from "@/types/nutrition";

import { DEFAULT_GOALS } from "@/types/nutrition";

import { sumMacros, caloriesFromMacros } from "@/utils/macros";

import {
  loadAllNutritionEntries,
  insertNutritionEntry,
  updateNutritionQuantity,
  deleteNutritionEntry,
  getSetting,
  setSetting,
} from "@/db/database";

import { arkSync } from "@/sync/ark-client";

import { mealEntryToArkEvent } from "@/sync/mapper";

import { registerNutritionRefresh } from "@/sync/sync-store";

function uid(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
}

function todayKey(): string {
  return format(new Date(), "yyyy-MM-dd");
}

function loadGoals(): Macros {
  const raw = getSetting("nutrition_goals", "");

  if (!raw) return DEFAULT_GOALS;

  try {
    return JSON.parse(raw);
  } catch {
    return DEFAULT_GOALS;
  }
}

function buildEntriesByDate(
  rows: ReturnType<typeof loadAllNutritionEntries>,
): Record<string, MealEntry[]> {
  const map: Record<string, MealEntry[]> = {};

  for (const row of rows) {
    const entry: MealEntry = {
      id: row.id,

      foodItem: JSON.parse(row.food_json),

      quantity: row.quantity,

      mealType: row.meal_type as MealType,

      loggedAt: row.logged_at,
    };

    (map[row.date] ??= []).push(entry);
  }

  return map;
}

interface NutritionState {
  entriesByDate: Record<string, MealEntry[]>;

  goals: Macros;

  selectedDate: string;

  _hydrated: boolean;

  hydrate: () => void;

  setSelectedDate: (date: string) => void;

  addEntry: (food: FoodItem, quantity: number, mealType: MealType) => void;

  updateEntryQuantity: (id: string, quantity: number) => void;

  removeEntry: (id: string) => void;

  updateGoals: (goals: Macros) => void;

  getEntries: () => MealEntry[];

  getTotals: () => Macros;

  getEntriesByMeal: () => Record<MealType, MealEntry[]>;
}

export const useNutritionStore = create<NutritionState>((set, get) => ({
  entriesByDate: {},

  goals: DEFAULT_GOALS,

  selectedDate: todayKey(),

  _hydrated: false,

  hydrate: () => {
    if (get()._hydrated) return;

    const rows = loadAllNutritionEntries();

    const goals = loadGoals();

    set({ entriesByDate: buildEntriesByDate(rows), goals, _hydrated: true });

    // Register refresh callback for incoming sync changes

    registerNutritionRefresh(() => {
      const freshRows = loadAllNutritionEntries();

      set({ entriesByDate: buildEntriesByDate(freshRows) });
    });
  },

  setSelectedDate: (date) => set({ selectedDate: date }),

  addEntry: (food, quantity, mealType) => {
    const date = get().selectedDate;

    const entry: MealEntry = {
      id: uid(),

      foodItem: food,

      quantity,

      mealType,

      loggedAt: new Date().toISOString(),
    };

    insertNutritionEntry({
      id: entry.id,

      date,

      food_json: JSON.stringify(food),

      quantity,

      meal_type: mealType,

      logged_at: entry.loggedAt,
    });

    set((state) => ({
      entriesByDate: {
        ...state.entriesByDate,

        [date]: [...(state.entriesByDate[date] ?? []), entry],
      },
    }));

    arkSync.sendChange(mealEntryToArkEvent(entry, date, "create"));
  },

  updateEntryQuantity: (id, quantity) => {
    const date = get().selectedDate;

    updateNutritionQuantity(id, quantity);

    const updated = (get().entriesByDate[date] ?? []).find((e) => e.id === id);

    set((state) => ({
      entriesByDate: {
        ...state.entriesByDate,

        [date]: (state.entriesByDate[date] ?? []).map((e) =>
          e.id === id ? { ...e, quantity } : e,
        ),
      },
    }));

    if (updated) {
      arkSync.sendChange(
        mealEntryToArkEvent({ ...updated, quantity }, date, "update"),
      );
    }
  },

  removeEntry: (id) => {
    const date = get().selectedDate;

    const removed = (get().entriesByDate[date] ?? []).find((e) => e.id === id);

    deleteNutritionEntry(id);

    set((state) => ({
      entriesByDate: {
        ...state.entriesByDate,

        [date]: (state.entriesByDate[date] ?? []).filter((e) => e.id !== id),
      },
    }));

    if (removed) {
      arkSync.sendChange(mealEntryToArkEvent(removed, date, "delete"));
    }
  },

  updateGoals: (goals) => {
    const computed = {
      ...goals,
      calories: caloriesFromMacros(goals.protein, goals.fat, goals.carbs),
    };

    setSetting("nutrition_goals", JSON.stringify(computed));

    set({ goals: computed });
  },

  getEntries: () => {
    const state = get();

    return state.entriesByDate[state.selectedDate] ?? [];
  },

  getTotals: () => sumMacros(get().getEntries()),

  getEntriesByMeal: () => {
    const entries = get().getEntries();

    return {
      breakfast: entries.filter((e) => e.mealType === "breakfast"),

      lunch: entries.filter((e) => e.mealType === "lunch"),

      dinner: entries.filter((e) => e.mealType === "dinner"),

      snack: entries.filter((e) => e.mealType === "snack"),
    };
  },
}));
