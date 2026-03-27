<script lang="ts">
    /**
     * VirtualTable - High-performance virtualized table component
     *
     * Features:
     * - Only renders visible rows (+ buffer)
     * - Server-side sorting and filtering
     * - Smooth scrolling with prefetch
     * - CSS containment for optimal rendering
     */

    import { onMount, onDestroy } from "svelte";
    import { dataService } from "../services/dataService";
    import type { Event, PageRequest, Filter, TableColumn } from "../types";

    // Props using Svelte 5 $props()
    interface Props {
        columns?: TableColumn[];
        rowHeight?: number;
        bufferSize?: number;
        pageSize?: number;
        filters?: Filter[];
        search?: string;
        onRowClick?: (event: Event) => void;
        onSortChange?: (data: {
            column: string;
            direction: "ASC" | "DESC";
        }) => void;
    }

    let {
        columns = [
            {
                key: "occurred_at",
                label: "Date",
                width: 180,
                sortable: true,
                filterable: true,
            },
            {
                key: "event_type",
                label: "Type",
                width: 150,
                sortable: true,
                filterable: true,
            },
            {
                key: "category",
                label: "Category",
                width: 120,
                sortable: true,
                filterable: true,
            },
            {
                key: "summary",
                label: "Summary",
                width: 300,
                sortable: false,
                filterable: false,
            },
            {
                key: "source",
                label: "Source",
                width: 120,
                sortable: true,
                filterable: true,
            },
        ],
        rowHeight = 40,
        bufferSize = 10,
        pageSize = 100,
        filters = [],
        search = "",
        onRowClick,
        onSortChange,
    }: Props = $props();

    // State
    let containerRef: HTMLDivElement | undefined = $state();
    let scrollTop = $state(0);
    let containerHeight = $state(0);
    let totalCount = $state(0);
    let rows: Event[] = $state([]);
    let isLoading = $state(false);
    let error: string | null = $state(null);
    let sortColumn = $state("occurred_at");
    let sortDirection: "ASC" | "DESC" = $state("DESC");

    // Track what range we have loaded
    let loadedStartIndex = $state(0);
    let loadedEndIndex = $state(0);

    // Computed values using $derived
    let totalHeight = $derived(totalCount * rowHeight);
    let visibleRowCount = $derived(
        Math.ceil(containerHeight / rowHeight) + bufferSize * 2,
    );
    let startIndex = $derived(
        Math.max(0, Math.floor(scrollTop / rowHeight) - bufferSize),
    );
    let endIndex = $derived(Math.min(totalCount, startIndex + visibleRowCount));
    let offsetY = $derived(startIndex * rowHeight);

    // Visible rows from loaded data
    let visibleRows = $derived.by(() => {
        const result: Event[] = [];
        for (let i = startIndex; i < endIndex; i++) {
            const row = rows[i - loadedStartIndex];
            if (row) {
                result.push(row);
            }
        }
        return result;
    });

    // Load data for visible range
    async function loadVisibleData() {
        if (isLoading) return;

        // Calculate which pages we need
        const neededStart = Math.floor(startIndex / pageSize) * pageSize;
        const neededEnd = Math.ceil(endIndex / pageSize) * pageSize;

        // Skip if already loaded
        if (
            neededStart >= loadedStartIndex &&
            neededEnd <= loadedEndIndex &&
            rows.length > 0
        ) {
            return;
        }

        isLoading = true;
        error = null;

        try {
            const request: PageRequest = {
                offset: neededStart,
                limit: neededEnd - neededStart,
                sort_column: sortColumn,
                sort_direction: sortDirection,
                filters: filters.length > 0 ? filters : undefined,
                search: search || undefined,
            };

            const response = await dataService.getPage(request);

            rows = response.rows;
            totalCount = response.total_count;
            loadedStartIndex = neededStart;
            loadedEndIndex = neededStart + response.rows.length;
        } catch (e) {
            error = e instanceof Error ? e.message : "Failed to load data";
        } finally {
            isLoading = false;
        }
    }

    // Debounced scroll handler
    let scrollTimeout: ReturnType<typeof setTimeout> | null = null;

    function handleScroll(event: UIEvent) {
        const target = event.target as HTMLDivElement;
        scrollTop = target.scrollTop;

        // Debounce data loading
        if (scrollTimeout) clearTimeout(scrollTimeout);
        scrollTimeout = setTimeout(() => {
            loadVisibleData();
        }, 16); // ~60fps
    }

    // Sort handling
    function handleHeaderClick(column: TableColumn) {
        if (!column.sortable) return;

        if (sortColumn === column.key) {
            sortDirection = sortDirection === "ASC" ? "DESC" : "ASC";
        } else {
            sortColumn = column.key;
            sortDirection = "DESC";
        }

        // Invalidate cache and reload
        dataService.invalidateCache();
        rows = [];
        loadedStartIndex = 0;
        loadedEndIndex = 0;
        loadVisibleData();

        onSortChange?.({ column: sortColumn, direction: sortDirection });
    }

    // Row click
    function handleRowClick(row: Event) {
        onRowClick?.(row);
    }

    // Reload when filters/search change
    // Using a derived key to track changes
    let filterKey = $derived(JSON.stringify(filters) + "|" + (search ?? ""));
    let lastFilterKey = "";

    $effect(() => {
        // Only reload if the key actually changed (not on initial mount)
        if (lastFilterKey !== "" && filterKey !== lastFilterKey) {
            reloadData();
        }
        lastFilterKey = filterKey;
    });

    function reloadData() {
        dataService.invalidateCache();
        rows = [];
        loadedStartIndex = 0;
        loadedEndIndex = 0;
        scrollTop = 0;
        if (containerRef) {
            containerRef.scrollTop = 0;
        }
        loadVisibleData();
    }

    // Initial load and resize observer
    let resizeObserver: ResizeObserver | undefined;

    onMount(() => {
        if (containerRef) {
            containerHeight = containerRef.clientHeight;

            resizeObserver = new ResizeObserver((entries) => {
                for (const entry of entries) {
                    containerHeight = entry.contentRect.height;
                    loadVisibleData();
                }
            });
            resizeObserver.observe(containerRef);

            loadVisibleData();
        }
    });

    onDestroy(() => {
        resizeObserver?.disconnect();
        if (scrollTimeout) clearTimeout(scrollTimeout);
    });

    // Format cell value
    function formatCell(row: Event, column: TableColumn): string {
        const value = row[column.key as keyof Event];

        if (column.key === "occurred_at" || column.key === "created_at") {
            return formatDate(value as string);
        }

        if (value === null || value === undefined) {
            return "-";
        }

        return String(value);
    }

    function formatDate(dateStr: string): string {
        try {
            const date = new Date(dateStr);
            return date.toLocaleString("ru-RU", {
                year: "numeric",
                month: "2-digit",
                day: "2-digit",
                hour: "2-digit",
                minute: "2-digit",
            });
        } catch {
            return dateStr;
        }
    }
</script>

<div class="virtual-table">
    <!-- Header -->
    <div class="table-header">
        {#each columns as column (column.key)}
            <div
                class="header-cell"
                class:sortable={column.sortable}
                class:sorted={sortColumn === column.key}
                style="width: {column.width}px; min-width: {column.width}px;"
                onclick={() => handleHeaderClick(column)}
                onkeydown={(e) =>
                    e.key === "Enter" && handleHeaderClick(column)}
                role="columnheader"
                tabindex={column.sortable ? 0 : -1}
            >
                <span class="header-label">{column.label}</span>
                {#if column.sortable}
                    <span class="sort-indicator">
                        {#if sortColumn === column.key}
                            {sortDirection === "ASC" ? "▲" : "▼"}
                        {/if}
                    </span>
                {/if}
            </div>
        {/each}
    </div>

    <!-- Scrollable body -->
    <div class="table-body" bind:this={containerRef} onscroll={handleScroll}>
        <!-- Spacer for total height -->
        <div class="scroll-spacer" style="height: {totalHeight}px;">
            <!-- Visible rows container -->
            <div
                class="rows-container"
                style="transform: translateY({offsetY}px);"
            >
                {#each visibleRows as row, i (row.id)}
                    <div
                        class="table-row"
                        class:even={(startIndex + i) % 2 === 0}
                        style="height: {rowHeight}px;"
                        onclick={() => handleRowClick(row)}
                        onkeydown={(e) =>
                            e.key === "Enter" && handleRowClick(row)}
                        role="row"
                        tabindex="0"
                    >
                        {#each columns as column (column.key)}
                            <div
                                class="table-cell"
                                style="width: {column.width}px; min-width: {column.width}px;"
                                title={formatCell(row, column)}
                            >
                                {formatCell(row, column)}
                            </div>
                        {/each}
                    </div>
                {/each}
            </div>
        </div>
    </div>

    <!-- Status bar -->
    <div class="table-footer">
        {#if isLoading}
            <span class="status loading">Loading...</span>
        {:else if error}
            <span class="status error">{error}</span>
        {:else}
            <span class="status">
                Showing {startIndex + 1}-{Math.min(endIndex, totalCount)} of {totalCount.toLocaleString()}
                rows
            </span>
        {/if}
    </div>
</div>

<style>
    .virtual-table {
        display: flex;
        flex-direction: column;
        height: 100%;
        background: var(--bg-primary, #1a1a2e);
        border: 1px solid var(--border-color, #2a2a4a);
        border-radius: 8px;
        overflow: hidden;
        contain: strict;
    }

    .table-header {
        display: flex;
        background: var(--bg-secondary, #16213e);
        border-bottom: 1px solid var(--border-color, #2a2a4a);
        flex-shrink: 0;
    }

    .header-cell {
        padding: 12px 16px;
        font-weight: 600;
        font-size: 13px;
        color: var(--text-secondary, #8892b0);
        text-transform: uppercase;
        letter-spacing: 0.5px;
        display: flex;
        align-items: center;
        gap: 8px;
        user-select: none;
        border-right: 1px solid var(--border-color, #2a2a4a);
    }

    .header-cell:last-child {
        border-right: none;
        flex: 1;
    }

    .header-cell.sortable {
        cursor: pointer;
    }

    .header-cell.sortable:hover {
        background: var(--bg-hover, #1f2847);
    }

    .header-cell.sorted {
        color: var(--accent-color, #64ffda);
    }

    .sort-indicator {
        font-size: 10px;
        opacity: 0.7;
    }

    .table-body {
        flex: 1;
        overflow-y: auto;
        overflow-x: hidden;
        contain: strict;
        will-change: scroll-position;
    }

    .scroll-spacer {
        position: relative;
        contain: strict;
    }

    .rows-container {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        will-change: transform;
        contain: layout style;
    }

    .table-row {
        display: flex;
        border-bottom: 1px solid var(--border-color, #2a2a4a);
        cursor: pointer;
        transition: background-color 0.1s ease;
        contain: layout style;
        content-visibility: auto;
    }

    .table-row:hover {
        background: var(--bg-hover, #1f2847);
    }

    .table-row:focus {
        outline: 2px solid var(--accent-color, #64ffda);
        outline-offset: -2px;
    }

    .table-row.even {
        background: var(--bg-secondary, #16213e);
    }

    .table-row.even:hover {
        background: var(--bg-hover, #1f2847);
    }

    .table-cell {
        padding: 10px 16px;
        font-size: 14px;
        color: var(--text-primary, #ccd6f6);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        border-right: 1px solid var(--border-color, #2a2a4a);
        display: flex;
        align-items: center;
    }

    .table-cell:last-child {
        border-right: none;
        flex: 1;
    }

    .table-footer {
        padding: 8px 16px;
        background: var(--bg-secondary, #16213e);
        border-top: 1px solid var(--border-color, #2a2a4a);
        font-size: 12px;
        color: var(--text-secondary, #8892b0);
        flex-shrink: 0;
    }

    .status.loading {
        color: var(--accent-color, #64ffda);
    }

    .status.error {
        color: var(--error-color, #ff6b6b);
    }

    /* Scrollbar styling */
    .table-body::-webkit-scrollbar {
        width: 8px;
    }

    .table-body::-webkit-scrollbar-track {
        background: var(--bg-secondary, #16213e);
    }

    .table-body::-webkit-scrollbar-thumb {
        background: var(--border-color, #2a2a4a);
        border-radius: 4px;
    }

    .table-body::-webkit-scrollbar-thumb:hover {
        background: var(--text-secondary, #8892b0);
    }
</style>
