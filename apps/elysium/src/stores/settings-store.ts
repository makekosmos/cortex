import { create } from 'zustand';
import { getSetting, setSetting } from '@/db/database';

export type FoodSource = 'fatsecret' | 'openfoodfacts';

interface SettingsState {
  foodSource: FoodSource;
  _hydrated: boolean;

  hydrate: () => void;
  setFoodSource: (source: FoodSource) => void;
}

export const useSettingsStore = create<SettingsState>((set, get) => ({
  foodSource: 'fatsecret',
  _hydrated: false,

  hydrate: () => {
    if (get()._hydrated) return;
    const source = getSetting('food_source', 'fatsecret') as FoodSource;
    set({ foodSource: source, _hydrated: true });
  },

  setFoodSource: (source) => {
    setSetting('food_source', source);
    set({ foodSource: source });
  },
}));
