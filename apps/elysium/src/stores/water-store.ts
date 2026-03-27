import { create } from 'zustand';
import { format } from 'date-fns';
import {
  loadAllWaterEntries,
  insertWaterEntry,
  deleteWaterEntry,
  getSetting,
  setSetting,
} from '@/db/database';
import { arkSync } from '@/sync/ark-client';
import { waterEntryToArkEvent } from '@/sync/mapper';
import { registerWaterRefresh } from '@/sync/sync-store';

function todayKey(): string {
  return format(new Date(), 'yyyy-MM-dd');
}

interface WaterEntry {
  id: string;
  amount: number;
  time: string;
}

interface WaterState {
  entriesByDate: Record<string, WaterEntry[]>;
  goal: number;
  _hydrated: boolean;

  hydrate: () => void;
  addWater: (amount: number) => void;
  removeWater: (id: string) => void;
  setGoal: (ml: number) => void;
  getToday: () => WaterEntry[];
  getTodayTotal: () => number;
}

export const useWaterStore = create<WaterState>((set, get) => ({
  entriesByDate: {},
  goal: 2500,
  _hydrated: false,

  hydrate: () => {
    if (get()._hydrated) return;
    const rows = loadAllWaterEntries();
    const map: Record<string, WaterEntry[]> = {};
    for (const row of rows) {
      (map[row.date] ??= []).push({ id: row.id, amount: row.amount, time: row.time });
    }
    const goal = parseInt(getSetting('water_goal', '2500'), 10);
    set({ entriesByDate: map, goal, _hydrated: true });

    // Register refresh callback for incoming sync changes
    registerWaterRefresh(() => {
      const freshRows = loadAllWaterEntries();
      const freshMap: Record<string, WaterEntry[]> = {};
      for (const row of freshRows) {
        (freshMap[row.date] ??= []).push({ id: row.id, amount: row.amount, time: row.time });
      }
      set({ entriesByDate: freshMap });
    });
  },

  addWater: (amount) => {
    const date = todayKey();
    const entry: WaterEntry = {
      id: Date.now().toString(36) + Math.random().toString(36).slice(2, 6),
      amount,
      time: new Date().toISOString(),
    };
    insertWaterEntry({ id: entry.id, date, amount, time: entry.time });
    set((s) => ({
      entriesByDate: {
        ...s.entriesByDate,
        [date]: [...(s.entriesByDate[date] ?? []), entry],
      },
    }));
    arkSync.sendChange(waterEntryToArkEvent(entry, date, 'create'));
  },

  removeWater: (id) => {
    const date = todayKey();
    const removed = (get().entriesByDate[date] ?? []).find((e) => e.id === id);
    deleteWaterEntry(id);
    set((s) => ({
      entriesByDate: {
        ...s.entriesByDate,
        [date]: (s.entriesByDate[date] ?? []).filter((e) => e.id !== id),
      },
    }));
    if (removed) {
      arkSync.sendChange(waterEntryToArkEvent(removed, date, 'delete'));
    }
  },

  setGoal: (ml) => {
    setSetting('water_goal', String(ml));
    set({ goal: ml });
  },

  getToday: () => get().entriesByDate[todayKey()] ?? [],

  getTodayTotal: () =>
    (get().entriesByDate[todayKey()] ?? []).reduce((sum, e) => sum + e.amount, 0),
}));
