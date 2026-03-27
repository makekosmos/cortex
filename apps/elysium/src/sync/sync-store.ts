/**
 * Zustand store for sync state and control.
 *
 * Persists serverUrl, apiKey, deviceId to SQLite settings table.
 * Wires the ArkSyncClient to the nutrition and water stores.
 */

import { create } from 'zustand';
import { getSetting, setSetting } from '@/db/database';
import { arkSync, type ArkChange } from './ark-client';
import { arkEventToMealEntry, arkEventToWaterEntry } from './mapper';
import {
  insertNutritionEntry,
  deleteNutritionEntry,
  updateNutritionQuantity,
  insertWaterEntry,
  deleteWaterEntry,
} from '@/db/database';
import { claimPairingCode } from './pairing';

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
  pendingChanges: number;
  isPaired: boolean;

  hydrate: () => void;
  configureServer: (url: string, key: string) => void;
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
  pendingChanges: 0,
  isPaired: false,

  hydrate: () => {
    const serverUrl = getSetting('sync_server_url', '');
    const apiKey = getSetting('sync_api_key', '');
    let deviceId = getSetting('sync_device_id', '');
    if (!deviceId) {
      deviceId = generateDeviceId();
      setSetting('sync_device_id', deviceId);
    }
    const lastSyncAt = getSetting('sync_last_at', '') || null;
    const isPaired = !!(serverUrl && apiKey);
    set({ serverUrl, apiKey, deviceId, lastSyncAt, isPaired });

    // Auto-connect if already paired
    if (isPaired) {
      setTimeout(() => get().connect(), 0);
    }
  },

  configureServer: (url, key) => {
    setSetting('sync_server_url', url);
    setSetting('sync_api_key', key);
    set({ serverUrl: url, apiKey: key, isPaired: !!(url && key) });
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
        setSetting('sync_last_at', now);
        set({ lastSyncAt: now });
      }
    });

    unsubChange = arkSync.onChange((change) => {
      applyIncomingChange(change, set);
    });

    arkSync.connect(serverUrl, apiKey, deviceId, 'Elysium Mobile');
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
      const result = await claimPairingCode(serverBaseUrl, code, 'Elysium Mobile');
      setSetting('sync_server_url', result.server_url);
      setSetting('sync_api_key', result.api_key);
      setSetting('sync_device_id', result.device_id);
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
    setSetting('sync_server_url', '');
    setSetting('sync_api_key', '');
    set({
      serverUrl: '',
      apiKey: '',
      isPaired: false,
      lastSyncAt: null,
    });
  },
}));

// ---------------------------------------------------------------------------
// Apply incoming changes to local SQLite + trigger store refresh
// ---------------------------------------------------------------------------

// We keep references to the zustand stores lazily to avoid circular imports.
let _nutritionRefresh: (() => void) | null = null;
let _waterRefresh: (() => void) | null = null;

export function registerNutritionRefresh(fn: () => void) {
  _nutritionRefresh = fn;
}

export function registerWaterRefresh(fn: () => void) {
  _waterRefresh = fn;
}

function applyIncomingChange(
  change: ArkChange,
  _set: (partial: Partial<SyncState>) => void,
) {
  const eventType = (change.data as Record<string, unknown>).event_type as string | undefined;

  if (eventType === 'nutrition.meal') {
    if (change.change_type === 'delete') {
      deleteNutritionEntry(change.event_id);
    } else {
      const parsed = arkEventToMealEntry(change);
      if (parsed) {
        const { entry, date } = parsed;
        if (change.change_type === 'update') {
          updateNutritionQuantity(entry.id, entry.quantity);
        } else {
          insertNutritionEntry({
            id: entry.id,
            date,
            food_json: JSON.stringify(entry.foodItem),
            quantity: entry.quantity,
            meal_type: entry.mealType,
            logged_at: entry.loggedAt,
          });
        }
      }
    }
    _nutritionRefresh?.();
  } else if (eventType === 'nutrition.water') {
    if (change.change_type === 'delete') {
      deleteWaterEntry(change.event_id);
    } else {
      const parsed = arkEventToWaterEntry(change);
      if (parsed) {
        const { entry, date } = parsed;
        insertWaterEntry({
          id: entry.id,
          date,
          amount: entry.amount,
          time: entry.time,
        });
      }
    }
    _waterRefresh?.();
  }
}
