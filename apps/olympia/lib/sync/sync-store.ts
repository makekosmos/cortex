/**
 * Zustand store for sync state and control.
 *
 * Persists serverUrl, apiKey, deviceId to AsyncStorage.
 * Wires the ArkSyncClient to the workout, routine, and body-weight stores.
 */

import { create } from 'zustand';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { arkSync, type ArkChange } from './ark-client';
import {
  arkEventToWorkout,
  arkEventToRoutine,
  arkEventToBodyWeight,
} from './mapper';
import {
  getDatabase,
  saveWorkout,
  deleteWorkout,
  saveRoutine,
  deleteRoutine,
  saveBodyWeight,
} from '../database';
import { claimPairingCode } from './pairing';

// ---------------------------------------------------------------------------
// AsyncStorage keys
// ---------------------------------------------------------------------------

const KEY_SERVER_URL = 'ark_sync_server_url';
const KEY_API_KEY = 'ark_sync_api_key';
const KEY_DEVICE_ID = 'ark_sync_device_id';
const KEY_LAST_SYNC = 'ark_sync_last_at';

// ---------------------------------------------------------------------------
// UUID generation (no extra deps)
// ---------------------------------------------------------------------------

function generateDeviceId(): string {
  const s4 = () =>
    Math.floor((1 + Math.random()) * 0x10000)
      .toString(16)
      .substring(1);
  return `rn-${s4()}${s4()}-${s4()}`;
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

interface SyncState {
  serverUrl: string;
  apiKey: string;
  deviceId: string;
  isConnected: boolean;
  isSyncing: boolean;
  isPairing: boolean;
  pairingError: string | null;
  lastSyncAt: string | null;
  isPaired: boolean;

  hydrate: () => Promise<void>;
  connect: () => void;
  disconnect: () => void;
  pair: (serverBaseUrl: string, code: string) => Promise<void>;
  unpair: () => void;
}

let unsubStatus: (() => void) | null = null;
let unsubChange: (() => void) | null = null;

export const useSyncStore = create<SyncState>((set, get) => ({
  serverUrl: '',
  apiKey: '',
  deviceId: '',
  isConnected: false,
  isSyncing: false,
  isPairing: false,
  pairingError: null,
  lastSyncAt: null,
  isPaired: false,

  hydrate: async () => {
    const [serverUrl, apiKey, deviceId, lastSyncAt] = await Promise.all([
      AsyncStorage.getItem(KEY_SERVER_URL).then((v) => v ?? ''),
      AsyncStorage.getItem(KEY_API_KEY).then((v) => v ?? ''),
      AsyncStorage.getItem(KEY_DEVICE_ID).then((v) => v ?? ''),
      AsyncStorage.getItem(KEY_LAST_SYNC).then((v) => v || null),
    ]);

    let finalDeviceId = deviceId;
    if (!finalDeviceId) {
      finalDeviceId = generateDeviceId();
      await AsyncStorage.setItem(KEY_DEVICE_ID, finalDeviceId);
    }

    const isPaired = !!(serverUrl && apiKey);
    set({ serverUrl, apiKey, deviceId: finalDeviceId, lastSyncAt, isPaired });

    // Auto-connect if already paired
    if (isPaired) {
      setTimeout(() => get().connect(), 0);
    }
  },

  connect: () => {
    const { serverUrl, apiKey, deviceId } = get();
    if (!serverUrl || !apiKey) return;

    set({ isSyncing: true });

    // Clean up previous listeners
    unsubStatus?.();
    unsubChange?.();

    unsubStatus = arkSync.onStatus((connected) => {
      set({ isConnected: connected, isSyncing: false });
      if (connected) {
        const now = new Date().toISOString();
        AsyncStorage.setItem(KEY_LAST_SYNC, now).catch(() => {});
        set({ lastSyncAt: now });
      }
    });

    unsubChange = arkSync.onChange((change) => {
      applyIncomingChange(change);
    });

    arkSync.connect(serverUrl, apiKey, deviceId, 'Olympia Mobile');
  },

  disconnect: () => {
    arkSync.disconnect();
    unsubStatus?.();
    unsubChange?.();
    unsubStatus = null;
    unsubChange = null;
    set({ isConnected: false, isSyncing: false });
  },

  pair: async (serverBaseUrl, code) => {
    set({ isPairing: true, pairingError: null });
    try {
      const result = await claimPairingCode(serverBaseUrl, code, 'Olympia Mobile');
      await Promise.all([
        AsyncStorage.setItem(KEY_SERVER_URL, result.server_url),
        AsyncStorage.setItem(KEY_API_KEY, result.api_key),
        AsyncStorage.setItem(KEY_DEVICE_ID, result.device_id),
      ]);
      set({
        serverUrl: result.server_url,
        apiKey: result.api_key,
        deviceId: result.device_id,
        isPaired: true,
        isPairing: false,
      });
      // Auto-connect immediately after pairing
      get().connect();
    } catch (e) {
      const msg = e instanceof Error ? e.message : 'Неизвестная ошибка';
      set({ isPairing: false, pairingError: msg });
      throw e;
    }
  },

  unpair: () => {
    get().disconnect();
    AsyncStorage.multiRemove([KEY_SERVER_URL, KEY_API_KEY]).catch(() => {});
    set({
      serverUrl: '',
      apiKey: '',
      isPaired: false,
      lastSyncAt: null,
    });
  },
}));

// ---------------------------------------------------------------------------
// Refresh callbacks (registered from outside to avoid circular imports)
// ---------------------------------------------------------------------------

let _workoutRefresh: (() => void) | null = null;
let _routineRefresh: (() => void) | null = null;
let _bodyWeightRefresh: (() => void) | null = null;

export function registerWorkoutRefresh(fn: () => void) {
  _workoutRefresh = fn;
}

export function registerRoutineRefresh(fn: () => void) {
  _routineRefresh = fn;
}

export function registerBodyWeightRefresh(fn: () => void) {
  _bodyWeightRefresh = fn;
}

// ---------------------------------------------------------------------------
// Apply incoming changes to local SQLite + trigger store refresh
// ---------------------------------------------------------------------------

async function applyIncomingChange(change: ArkChange) {
  const eventType = (change.data as Record<string, unknown>).event_type as string | undefined;

  try {
    if (eventType === 'workout') {
      if (change.change_type === 'delete') {
        await deleteWorkout(change.event_id);
      } else {
        const workout = arkEventToWorkout(change);
        if (workout) {
          // Delete existing first (upsert pattern)
          await deleteWorkout(workout.id).catch(() => {});
          await saveWorkout(
            {
              id: workout.id,
              title: workout.title,
              started_at: workout.started_at,
              finished_at: workout.finished_at ?? new Date().toISOString(),
              notes: workout.notes ?? '',
              duration_seconds: workout.duration_seconds ?? 0,
            },
            workout.exercises.map((ex) => ({
              id: ex.id,
              exercise_id: ex.exercise_id,
              sort_order: ex.sort_order,
              notes: ex.notes ?? '',
              sets: ex.sets.map((s) => ({
                id: s.id,
                set_index: s.set_index,
                set_type: s.set_type,
                weight_kg: s.weight_kg,
                reps: s.reps,
                completed: s.completed,
                rpe: s.rpe,
              })),
            })),
          );
        }
      }
      _workoutRefresh?.();
    } else if (eventType === 'routine') {
      if (change.change_type === 'delete') {
        await deleteRoutine(change.event_id);
      } else {
        const routine = arkEventToRoutine(change);
        if (routine) {
          // Delete existing first (upsert pattern)
          await deleteRoutine(routine.id).catch(() => {});
          await saveRoutine(
            { id: routine.id, title: routine.title },
            routine.exercises.map((ex) => ({
              id: ex.id,
              exercise_id: ex.exercise_id,
              sort_order: ex.sort_order,
              target_sets: ex.target_sets ?? 3,
              target_reps: ex.target_reps ?? '',
              target_weight_kg: ex.target_weight_kg,
              notes: ex.notes ?? '',
            })),
          );
        }
      }
      _routineRefresh?.();
    } else if (eventType === 'body_weight') {
      if (change.change_type === 'delete') {
        const database = await getDatabase();
        await database.runAsync('DELETE FROM body_weight_log WHERE id = ?', [change.event_id]);
      } else {
        const entry = arkEventToBodyWeight(change);
        if (entry) {
          await saveBodyWeight(entry);
        }
      }
      _bodyWeightRefresh?.();
    }
  } catch (e) {
    console.warn('[ArkSync] failed to apply incoming change:', e);
  }
}
