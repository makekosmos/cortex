export interface Event {
    id: string;
    event_type: string;
    category: string | null;
    occurred_at: string;
    data: Record<string, unknown>;
    summary: string | null;
    source: string | null;
    tags: string[];
    created_at: string;
}

export interface CategoryStats {
    category: string;
    count: number;
    event_types: Record<string, number>;
}

export interface DayStats {
    date: string;
    count: number;
}

export interface DbStats {
    total_events: number;
    total_entities: number;
    categories: CategoryStats[];
    recent_days: DayStats[];
}

export interface QueryParams {
    event_type?: string;
    category?: string;
    search?: string;
    limit?: number;
    offset?: number;
}

// ============================================================================
// Optimized Pagination Types
// ============================================================================

export type FilterOperator =
    | "Equals"
    | "NotEquals"
    | "Contains"
    | "GreaterThan"
    | "LessThan"
    | "Between"
    | "In";

export interface Filter {
    column: string;
    operator: FilterOperator;
    value: unknown;
}

export interface PageRequest {
    offset: number;
    limit: number;
    sort_column?: string;
    sort_direction?: "ASC" | "DESC";
    filters?: Filter[];
    search?: string;
}

export interface PageResponse {
    rows: Event[];
    total_count: number;
    has_more: boolean;
    query_time_ms: number;
}

export interface TimeSeriesPoint {
    timestamp: string;
    value: number;
    count: number;
}

export interface CategoryCount {
    category: string;
    count: number;
}

// ============================================================================
// Virtual Table Types
// ============================================================================

export interface VirtualTableState {
    scrollTop: number;
    visibleStartIndex: number;
    visibleEndIndex: number;
    totalHeight: number;
}

export interface TableColumn {
    key: string;
    label: string;
    width: number;
    sortable: boolean;
    filterable: boolean;
}

// Category colors for visualization
export const CATEGORY_COLORS: Record<string, string> = {
    health: "#ff4444",
    nutrition: "#00d4aa",
    fitness: "#0088ff",
    work: "#8855ff",
    productivity: "#f59e0b",
    media: "#ffcc00",
    finance: "#00ff88",
    location: "#ff8800",
    journal: "#ff88ff",
    social: "#88ccff",
    medical: "#ff6666",
    uncategorized: "#555555",
};

export function getCategoryColor(category: string): string {
    return CATEGORY_COLORS[category.toLowerCase()] || CATEGORY_COLORS.uncategorized;
}

// ============================================================================
// Plugin Types
// ============================================================================

export interface PluginLimit {
    name: string;
    description: string;
    value: string;
}

export interface PluginConfig {
    id: string;
    name: string;
    description: string;
    icon: string;
    enabled: boolean;
    syncIntervalMinutes: number;
    lastSyncAt: string | null;
    lastSyncStatus: "success" | "error" | "never" | "running";
    lastSyncMessage: string | null;
    limits: PluginLimit[];
    requiresApiKey: boolean;
    apiKeyConfigured: boolean;
}

export interface PluginSyncResult {
    created: number;
    updated: number;
    skipped: number;
    error: string | null;
}
