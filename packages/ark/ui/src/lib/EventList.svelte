<script lang="ts">
    import type { Event as DataEvent } from "./types";
    import { getCategoryColor } from "./types";

    interface Props {
        events: DataEvent[];
        loading: boolean;
        selectedId: string | null;
        onSelect?: (event: DataEvent) => void;
        onLoadMore?: () => void;
    }

    let {
        events = [],
        loading = false,
        selectedId = null,
        onSelect,
        onLoadMore,
    }: Props = $props();

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

    function handleScroll(e: UIEvent) {
        const target = e.target as HTMLElement;
        const scrollBottom =
            target.scrollHeight - target.scrollTop - target.clientHeight;
        if (scrollBottom < 100 && !loading) {
            onLoadMore?.();
        }
    }
</script>

<div
    class="flex flex-col gap-2 p-4 overflow-y-auto h-full"
    onscroll={handleScroll}
>
    {#if events.length === 0 && !loading}
        <div
            class="flex flex-col items-center justify-center py-12 text-text-muted gap-4"
        >
            <svg
                class="w-12 h-12 opacity-50"
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
        {#each events as event (event.id)}
            {@const color = getCategoryColor(event.category || "uncategorized")}
            {@const isSelected = selectedId === event.id}
            <button
                class="flex gap-3 p-3 bg-bg-card border border-border rounded-lg text-left w-full transition-all
               hover:bg-bg-hover
               {isSelected ? 'bg-bg-hover shadow-[0_0_0_1px]' : ''}"
                style={isSelected
                    ? `border-color: ${color}; box-shadow-color: ${color}`
                    : ""}
                onclick={() => onSelect?.(event)}
            >
                <div
                    class="w-1 min-h-full rounded opacity-70"
                    style="background: {color}"
                    class:opacity-100={isSelected}
                ></div>

                <div class="flex-1 min-w-0">
                    <div class="flex justify-between items-center gap-2 mb-1">
                        <span class="font-mono text-sm font-medium text-text"
                            >{event.event_type}</span
                        >
                        <span class="text-xs text-text-muted whitespace-nowrap"
                            >{formatDate(event.occurred_at)}</span
                        >
                    </div>

                    {#if event.summary}
                        <div class="text-sm text-text-secondary truncate mb-1">
                            {event.summary}
                        </div>
                    {/if}

                    <div class="flex gap-2 flex-wrap">
                        {#if event.category}
                            <span
                                class="text-[10px] px-1.5 py-0.5 rounded border"
                                style="color: {color}; border-color: {color}"
                            >
                                {event.category}
                            </span>
                        {/if}
                        {#if event.source}
                            <span
                                class="text-[10px] px-1.5 py-0.5 bg-bg-secondary rounded text-text-muted"
                            >
                                {event.source}
                            </span>
                        {/if}
                    </div>
                </div>
            </button>
        {/each}
    {/if}

    {#if loading}
        <div class="flex items-center justify-center gap-2 p-4 text-text-muted">
            <div
                class="w-4 h-4 border-2 border-border border-t-accent rounded-full animate-spin"
            ></div>
            <span>Loading...</span>
        </div>
    {/if}
</div>
