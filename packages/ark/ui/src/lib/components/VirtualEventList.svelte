<script lang="ts">
    /**
     * VirtualEventList - Virtualized event list with card layout
     *
     * Features:
     * - Only renders visible cards (+ buffer)
     * - Server-side pagination
     * - Smooth scrolling with prefetch
     * - CSS containment
     */

    import { onMount, onDestroy } from "svelte";
    import { dataService } from "../services/dataService";
    import type { Event, PageRequest, Filter } from "../types";
    import { getCategoryColor } from "../types";

    // Props using Svelte 5 $props()
    interface Props {
        filters?: Filter[];
        search?: string;
        selectedId?: string | null;
        cardHeight?: number;
        bufferSize?: number;
        pageSize?: number;
        onSelect?: (event: Event) => void;
    }

    let {
        filters = [],
        search = "",
        selectedId = null,
        cardHeight = 88,
        bufferSize = 5,
        pageSize = 50,
        onSelect,
    }: Props = $props();

    // State
    let containerRef: HTMLDivElement | undefined = $state();
    let scrollTop = $state(0);
    let containerHeight = $state(0);
    let totalCount = $state(0);
    let rows: Event[] = $state([]);
    let isLoading = $state(false);
    let error: string | null = $state(null);

    // Loaded range tracking
    let loadedStartIndex = $state(0);
    let loadedEndIndex = $state(0);

    // Computed values using $derived
    let totalHeight = $derived(totalCount * cardHeight);
    let visibleCount = $derived(
        Math.ceil(containerHeight / cardHeight) + bufferSize * 2,
    );
    let startIndex = $derived(
        Math.max(0, Math.floor(scrollTop / cardHeight) - bufferSize),
    );
    let endIndex = $derived(Math.min(totalCount, startIndex + visibleCount));
    let offsetY = $derived(startIndex * cardHeight);

    // Get visible rows from loaded data
    let visibleRows = $derived.by(() => {
        return rows.slice(
            Math.max(0, startIndex - loadedStartIndex),
            Math.min(rows.length, endIndex - loadedStartIndex),
        );
    });

    // Load data for visible range
    async function loadVisibleData() {
        if (isLoading) return;

        const neededStart = Math.floor(startIndex / pageSize) * pageSize;
        const neededEnd = Math.ceil(endIndex / pageSize) * pageSize;

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
                sort_column: "occurred_at",
                sort_direction: "DESC",
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

        if (scrollTimeout) clearTimeout(scrollTimeout);
        scrollTimeout = setTimeout(() => {
            loadVisibleData();
        }, 16);
    }

    function handleSelect(event: Event) {
        onSelect?.(event);
    }

    // Public method for reloading
    export function reload() {
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

    // Reload when filters/search change
    // Using a derived key to track changes
    let filterKey = $derived(JSON.stringify(filters) + "|" + (search ?? ""));
    let lastFilterKey = "";

    $effect(() => {
        // Only reload if the key actually changed (not on initial mount)
        if (lastFilterKey !== "" && filterKey !== lastFilterKey) {
            reload();
        }
        lastFilterKey = filterKey;
    });

    // Resize observer
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

    function formatDate(isoDate: string): string {
        const date = new Date(isoDate);
        const now = new Date();
        const diff = now.getTime() - date.getTime();
        const days = Math.floor(diff / (1000 * 60 * 60 * 24));

        if (days === 0) {
            return date.toLocaleTimeString("en-US", {
                hour: "2-digit",
                minute: "2-digit",
            });
        } else if (days === 1) {
            return "Yesterday";
        } else if (days < 7) {
            return date.toLocaleDateString("en-US", { weekday: "short" });
        } else {
            return date.toLocaleDateString("en-US", {
                month: "short",
                day: "numeric",
            });
        }
    }
</script>

<div class="virtual-list" bind:this={containerRef} onscroll={handleScroll}>
    {#if totalCount === 0 && !isLoading}
        <div class="empty-state">
            <svg
                class="empty-icon"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
            >
                <path
                    d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10"
                />
            </svg>
            <p>No events found</p>
        </div>
    {:else}
        <!-- Spacer for total height -->
        <div class="scroll-spacer" style="height: {totalHeight}px;">
            <!-- Visible cards container -->
            <div
                class="cards-container"
                style="transform: translateY({offsetY}px);"
            >
                {#each visibleRows as event (event.id)}
                    {@const color = getCategoryColor(
                        event.category || "uncategorized",
                    )}
                    {@const isSelected = selectedId === event.id}
                    <button
                        class="event-card"
                        class:selected={isSelected}
                        style="height: {cardHeight}px; --card-color: {color};"
                        onclick={() => handleSelect(event)}
                    >
                        <div class="color-bar" class:active={isSelected}></div>

                        <div class="card-content">
                            <div class="card-header">
                                <span class="event-type"
                                    >{event.event_type}</span
                                >
                                <span class="event-date"
                                    >{formatDate(event.occurred_at)}</span
                                >
                            </div>

                            {#if event.summary}
                                <div class="event-summary">{event.summary}</div>
                            {/if}

                            <div class="event-tags">
                                {#if event.category}
                                    <span class="tag category">
                                        {event.category}
                                    </span>
                                {/if}
                                {#if event.source}
                                    <span class="tag source"
                                        >{event.source}</span
                                    >
                                {/if}
                            </div>
                        </div>
                    </button>
                {/each}
            </div>
        </div>
    {/if}

    {#if isLoading}
        <div class="loading-indicator">
            <div class="spinner"></div>
            <span>Loading...</span>
        </div>
    {/if}

    {#if error}
        <div class="error-message">{error}</div>
    {/if}
</div>

<div class="list-footer">
    {totalCount.toLocaleString()} events
</div>

<style>
    .virtual-list {
        flex: 1;
        overflow-y: auto;
        overflow-x: hidden;
        contain: strict;
        will-change: scroll-position;
        border-left: 1px solid var(--color-border);
    }

    .scroll-spacer {
        position: relative;
        contain: strict;
    }

    .cards-container {
        position: absolute;
        top: 0;
        left: 0;
        right: 0;
        display: flex;
        flex-direction: column;
        will-change: transform;
        contain: layout style;
    }

    .event-card {
        display: flex;
        align-items: stretch;
        padding: 0;
        background: transparent;
        border: none;
        border-bottom: 1px solid var(--color-border);
        text-align: left;
        width: 100%;
        cursor: pointer;
        transition: background-color 0.1s ease;
        contain: layout style;
        content-visibility: auto;
    }

    .event-card:hover {
        background: var(--color-bg-hover);
    }

    .event-card.selected {
        background: var(--color-bg-card);
    }

    .color-bar {
        width: 3px;
        flex-shrink: 0;
        background: var(--card-color);
        opacity: 0.5;
    }

    .color-bar.active {
        opacity: 1;
    }

    .card-content {
        flex: 1;
        min-width: 0;
        display: flex;
        flex-direction: column;
        justify-content: center;
        gap: 2px;
        padding: 10px 12px;
    }

    .card-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        gap: 8px;
    }

    .event-type {
        font-family: var(--font-mono);
        font-size: 13px;
        font-weight: 500;
        color: var(--color-text);
    }

    .event-date {
        font-size: 11px;
        color: var(--color-text-muted);
        white-space: nowrap;
    }

    .event-summary {
        font-size: 12px;
        color: var(--color-text-secondary);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .event-tags {
        display: flex;
        gap: 6px;
        flex-wrap: wrap;
        margin-top: 2px;
    }

    .tag {
        font-size: 10px;
        padding: 1px 5px;
    }

    .tag.category {
        border: 1px solid var(--card-color);
        color: var(--card-color);
    }

    .tag.source {
        color: var(--color-text-muted);
    }

    .empty-state {
        display: flex;
        flex-direction: column;
        align-items: center;
        justify-content: center;
        padding: 48px 16px;
        color: var(--color-text-muted);
        gap: 16px;
    }

    .empty-icon {
        width: 48px;
        height: 48px;
        opacity: 0.5;
    }

    .loading-indicator {
        position: fixed;
        bottom: 60px;
        left: 50%;
        transform: translateX(-50%);
        display: flex;
        align-items: center;
        gap: 8px;
        padding: 8px 16px;
        background: var(--color-bg-secondary);
        border: 1px solid var(--color-border);
        color: var(--color-text-muted);
        font-size: 13px;
    }

    .spinner {
        width: 16px;
        height: 16px;
        border: 2px solid var(--color-border);
        border-top-color: var(--color-accent);
        border-radius: 50%;
        animation: spin 0.8s linear infinite;
    }

    @keyframes spin {
        to {
            transform: rotate(360deg);
        }
    }

    .error-message {
        padding: 12px 16px;
        background: rgba(255, 68, 68, 0.1);
        border: 1px solid var(--color-red);
        color: var(--color-red);
        font-size: 13px;
        margin: 8px 0;
    }

    .list-footer {
        padding: 8px 16px;
        background: var(--color-bg-secondary);
        border-top: 1px solid var(--color-border);
        font-size: 11px;
        color: var(--color-text-muted);
        text-align: center;
        flex-shrink: 0;
    }

    /* Scrollbar */
    .virtual-list::-webkit-scrollbar {
        width: 8px;
    }

    .virtual-list::-webkit-scrollbar-track {
        background: var(--color-bg-secondary);
    }

    .virtual-list::-webkit-scrollbar-thumb {
        background: var(--color-border);
    }

    .virtual-list::-webkit-scrollbar-thumb:hover {
        background: var(--color-text-muted);
    }
</style>
