import { create } from 'zustand';
import type { FoodItem } from '@/types/nutrition';
import {
  loadCustomFoods,
  insertCustomFood,
  loadRecentFoods,
  upsertRecentFood,
} from '@/db/database';

function uid(): string {
  return Date.now().toString(36) + Math.random().toString(36).slice(2, 8);
}

interface FoodState {
  customFoods: FoodItem[];
  recentFoods: FoodItem[];
  _hydrated: boolean;

  hydrate: () => void;
  allFoods: () => FoodItem[];
  addCustomFood: (food: Omit<FoodItem, 'id'>) => FoodItem;
  markUsed: (food: FoodItem) => void;
  getRecent: () => FoodItem[];
  searchFoods: (query: string) => FoodItem[];
}

export const useFoodStore = create<FoodState>((set, get) => ({
  customFoods: [],
  recentFoods: [],
  _hydrated: false,

  hydrate: () => {
    if (get()._hydrated) return;
    const customRows = loadCustomFoods();
    const customFoods: FoodItem[] = customRows.map((r) => ({
      id: r.id,
      name: r.name,
      brand: r.brand ?? undefined,
      servingSize: r.serving_size,
      servingUnit: r.serving_unit,
      macros: JSON.parse(r.macros_json),
    }));
    const recentRows = loadRecentFoods();
    const recentFoods: FoodItem[] = recentRows.map((r) => JSON.parse(r.food_json));
    set({ customFoods, recentFoods, _hydrated: true });
  },

  allFoods: () => get().customFoods,

  addCustomFood: (food) => {
    const existing = get().customFoods.find(
      (f) => f.name.toLowerCase() === food.name?.toLowerCase(),
    );
    if (existing) return existing;
    const newFood: FoodItem = { ...food, id: uid() };
    insertCustomFood({
      id: newFood.id,
      name: newFood.name,
      brand: newFood.brand ?? null,
      serving_size: newFood.servingSize,
      serving_unit: newFood.servingUnit,
      macros_json: JSON.stringify(newFood.macros),
    });
    set((state) => ({ customFoods: [...state.customFoods, newFood] }));
    return newFood;
  },

  markUsed: (food) => {
    upsertRecentFood({
      id: food.id,
      food_json: JSON.stringify(food),
      used_at: new Date().toISOString(),
    });
    set((state) => {
      const filtered = state.recentFoods.filter(
        (f) => f.name.toLowerCase() !== food.name.toLowerCase(),
      );
      return { recentFoods: [food, ...filtered].slice(0, 30) };
    });
  },

  getRecent: () => get().recentFoods,

  searchFoods: (query) => {
    const q = query.toLowerCase().trim();
    const all = [...get().recentFoods, ...get().customFoods];
    const seen = new Set<string>();
    return all.filter((f) => {
      const key = f.name.toLowerCase();
      if (seen.has(key)) return false;
      seen.add(key);
      return !q || key.includes(q);
    });
  },
}));
