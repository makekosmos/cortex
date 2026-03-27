import { create } from 'zustand';
import AsyncStorage from '@react-native-async-storage/async-storage';

interface SettingsState {
  units: 'kg' | 'lbs';
  restTimerSeconds: number;
  theme: 'dark' | 'light';
  loaded: boolean;
  setUnits: (units: 'kg' | 'lbs') => void;
  setRestTimer: (seconds: number) => void;
  setTheme: (theme: 'dark' | 'light') => void;
  loadSettings: () => Promise<void>;
}

const STORAGE_KEY = 'workout-tracker-settings';

export const useSettingsStore = create<SettingsState>((set, get) => ({
  units: 'kg',
  restTimerSeconds: 90,
  theme: 'dark',
  loaded: false,

  setUnits: (units) => {
    set({ units });
    persistSettings(get());
  },
  setRestTimer: (seconds) => {
    set({ restTimerSeconds: seconds });
    persistSettings(get());
  },
  setTheme: (theme) => {
    set({ theme });
    persistSettings(get());
  },
  loadSettings: async () => {
    try {
      const raw = await AsyncStorage.getItem(STORAGE_KEY);
      if (raw) {
        const parsed = JSON.parse(raw);
        set({ ...parsed, loaded: true });
      } else {
        set({ loaded: true });
      }
    } catch {
      set({ loaded: true });
    }
  },
}));

function persistSettings(state: SettingsState) {
  AsyncStorage.setItem(STORAGE_KEY, JSON.stringify({
    units: state.units,
    restTimerSeconds: state.restTimerSeconds,
    theme: state.theme,
  })).catch(() => {});
}
