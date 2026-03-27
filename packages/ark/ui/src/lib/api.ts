import { get } from "svelte/store";
import type {
    DbStats,
    Event,
    Filter,
    PageRequest,
    PageResponse,
    QueryParams,
    CategoryStats,
} from "./types";
import { apiKey, serverUrl } from "./stores";

function getApiConfig(): { baseUrl: string; key: string } {
    const baseUrl = get(serverUrl).replace(/\/+$/, "");
    const key = get(apiKey);
    if (!key) {
        throw new Error("Не задан ключ доступа (API key)");
    }
    return { baseUrl, key };
}

async function apiFetch<T>(path: string, init: RequestInit = {}): Promise<T> {
    const { baseUrl, key } = getApiConfig();
    const url = `${baseUrl}${path}`;

    const headers = new Headers(init.headers);
    headers.set("Authorization", `Bearer ${key}`);
    if (!headers.has("Content-Type") && init.body) {
        headers.set("Content-Type", "application/json");
    }

    const res = await fetch(url, { ...init, headers });
    if (!res.ok) {
        let detail = `${res.status} ${res.statusText}`;
        try {
            const body = (await res.json()) as { detail?: unknown };
            if (body?.detail) detail = String(body.detail);
        } catch {
            // ignore
        }
        throw new Error(detail);
    }
    return (await res.json()) as T;
}

// ============================================================================
// Connection / Stats
// ============================================================================

export async function getDbStats(): Promise<DbStats> {
    return apiFetch<DbStats>("/db/stats");
}

export async function getCategoryDetails(category: string): Promise<CategoryStats> {
    return apiFetch<CategoryStats>(`/categories/${encodeURIComponent(category)}`);
}

// ============================================================================
// Legacy Query (simple list)
// ============================================================================

export async function queryEvents(params: QueryParams): Promise<Event[]> {
    const url = new URL("/events", get(serverUrl));
    Object.entries(params).forEach(([k, v]) => {
        if (v !== undefined && v !== null) url.searchParams.set(k, String(v));
    });
    // Override with authenticated request but keep querystring.
    return apiFetch<Event[]>(url.pathname + url.search);
}

// ============================================================================
// Optimized Paginated Queries
// ============================================================================

export async function getPage(request: PageRequest): Promise<PageResponse> {
    return apiFetch<PageResponse>("/events/page", {
        method: "POST",
        body: JSON.stringify(request),
    });
}

export async function getTotalCount(filters?: Filter[], search?: string): Promise<number> {
    return apiFetch<number>("/events/count", {
        method: "POST",
        body: JSON.stringify({ filters, search }),
    });
}

// ============================================================================
// Full-Text Search
// ============================================================================

export async function searchEvents(query: string, limit = 50): Promise<Event[]> {
    const url = new URL("/search", get(serverUrl));
    url.searchParams.set("query", query);
    url.searchParams.set("limit", String(limit));
    return apiFetch<Event[]>(url.pathname + url.search);
}

// ============================================================================
// Plugins
// ============================================================================

export interface PluginServerInfo {
    id: string;
    name: string;
    description: string;
    icon: string;
    requires_api_key: boolean;
    api_key_configured: boolean;
    last_sync_at: string | null;
    last_sync_status: string;
    last_sync_message: string | null;
}

export async function getPlugins(): Promise<PluginServerInfo[]> {
    return apiFetch<PluginServerInfo[]>("/plugins");
}

export async function syncPlugin(
    pluginId: string,
    payload: { days?: number | null; full_sync?: boolean } = {},
): Promise<Record<string, unknown>> {
    return apiFetch<Record<string, unknown>>(`/plugins/${encodeURIComponent(pluginId)}/sync`, {
        method: "POST",
        body: JSON.stringify(payload),
    });
}
