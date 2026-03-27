import { derived, get, writable } from "svelte/store";
import type { DbStats, Event, PluginConfig } from "./types";

// ============================================================================
// LocalStorage Keys
// ============================================================================

const STORAGE_KEYS = {
    SERVER_URL: "life_server_url",
    API_KEY: "life_api_key",
    VIEW_MODE: "life_view_mode",
    PLUGINS: "life_plugins_v2", // v2: Russian translation
} as const;

// ============================================================================
// Persistence Helpers
// ============================================================================

function loadFromStorage<T>(key: string, defaultValue: T): T {
    try {
        const stored = localStorage.getItem(key);
        if (stored !== null) {
            return JSON.parse(stored) as T;
        }
    } catch {
        // Ignore parsing errors, return default
    }
    return defaultValue;
}

function saveToStorage<T>(key: string, value: T): void {
    try {
        if (value === null || value === undefined) {
            localStorage.removeItem(key);
        } else {
            localStorage.setItem(key, JSON.stringify(value));
        }
    } catch {
        // Ignore storage errors (e.g., quota exceeded)
    }
}

// ============================================================================
// Connection State
// ============================================================================

const initialServerUrl = loadFromStorage<string>(STORAGE_KEYS.SERVER_URL, "http://localhost:8000");
const initialApiKey = loadFromStorage<string | null>(STORAGE_KEYS.API_KEY, null);

export const serverUrl = writable<string>(initialServerUrl);
export const apiKey = writable<string | null>(initialApiKey);
export const dbStats = writable<DbStats | null>(null);
export const isLoading = writable(false);
export const error = writable<string | null>(null);

serverUrl.subscribe((value) => {
    saveToStorage(STORAGE_KEYS.SERVER_URL, value);
});

apiKey.subscribe((value) => {
    saveToStorage(STORAGE_KEYS.API_KEY, value);
});

// ============================================================================
// UI State
// ============================================================================

export const showSettings = writable(false);
export const showPlugins = writable(false);
export const selectedCategory = writable<string | null>(null);
export const selectedEventType = writable<string | null>(null);
export const searchQuery = writable("");
export const events = writable<Event[]>([]);
export const selectedEvent = writable<Event | null>(null);

// ============================================================================
// View Mode (persisted)
// ============================================================================

export type ViewMode = "overview" | "events";

const initialViewMode = (() => {
    const raw = loadFromStorage<string>(STORAGE_KEYS.VIEW_MODE, "overview");
    return raw === "events" ? ("events" as ViewMode) : ("overview" as ViewMode);
})();
export const viewMode = writable<ViewMode>(initialViewMode);

// Persist viewMode changes
viewMode.subscribe((value) => {
    saveToStorage(STORAGE_KEYS.VIEW_MODE, value);
});

// ============================================================================
// Derived Stores
// ============================================================================

export const isConnected = derived(dbStats, ($stats) => $stats !== null);

export const totalEvents = derived(dbStats, ($stats) => $stats?.total_events ?? 0);

export const categoriesWithColors = derived(dbStats, ($stats) => {
    if (!$stats) return [];
    return $stats.categories.map((cat) => ({
        ...cat,
        percentage:
            $stats.total_events > 0 ? ((cat.count / $stats.total_events) * 100).toFixed(1) : "0",
    }));
});

// ============================================================================
// Plugins State
// ============================================================================

const defaultPlugins: PluginConfig[] = [
    {
        id: "toggl_track",
        name: "Toggl Track",
        description: "Импорт записей учёта времени из Toggl Track",
        icon: "clock",
        enabled: false,
        syncIntervalMinutes: 60,
        lastSyncAt: null,
        lastSyncStatus: "never",
        lastSyncMessage: null,
        limits: [
            {
                name: "Лимит запросов (Free)",
                description: "Бесплатный план: 30 запросов в час к API",
                value: "30/час",
            },
            {
                name: "Лимит диапазона дат",
                description: "Reports API v3: максимум 365 дней за запрос",
                value: "365 дней",
            },
            {
                name: "Track API v9",
                description: "Только последние 90 дней, макс 1000 записей",
                value: "90 дней",
            },
        ],
        requiresApiKey: true,
        apiKeyConfigured: false,
    },
];

const initialPlugins = loadFromStorage<PluginConfig[]>(STORAGE_KEYS.PLUGINS, defaultPlugins);
export const plugins = writable<PluginConfig[]>(initialPlugins);

// Persist plugins changes
plugins.subscribe((value) => {
    saveToStorage(STORAGE_KEYS.PLUGINS, value);
});

/**
 * Update a plugin configuration
 */
export function updatePlugin(pluginId: string, updates: Partial<PluginConfig>): void {
    plugins.update((list) => list.map((p) => (p.id === pluginId ? { ...p, ...updates } : p)));
}

/**
 * Get plugin by ID
 */
export function getPlugin(pluginId: string): PluginConfig | undefined {
    return get(plugins).find((p) => p.id === pluginId);
}

// ============================================================================
// Utility Functions
// ============================================================================

/**
 * Clear saved credentials and UI state
 */
export function clearStoredCredentials(): void {
    apiKey.set(null);
    dbStats.set(null);
}

/**
 * Reset all UI state (useful when closing/switching databases)
 */
export function resetUIState(): void {
    selectedCategory.set(null);
    selectedEventType.set(null);
    searchQuery.set("");
    events.set([]);
    selectedEvent.set(null);
    error.set(null);
}
