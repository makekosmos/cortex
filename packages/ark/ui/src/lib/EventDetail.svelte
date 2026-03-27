<script lang="ts">
    import type { Event } from "./types";
    import { getCategoryColor } from "./types";

    interface Props {
        event: Event | null;
    }

    let { event = null }: Props = $props();

    function formatDateTime(isoDate: string): string {
        const date = new Date(isoDate);
        return date.toLocaleString("en-US", {
            weekday: "short",
            year: "numeric",
            month: "short",
            day: "numeric",
            hour: "2-digit",
            minute: "2-digit",
        });
    }

    function formatJson(data: unknown): string {
        return JSON.stringify(data, null, 2);
    }
</script>

<div class="p-6 h-full overflow-y-auto">
    {#if event}
        {@const color = getCategoryColor(event.category || "uncategorized")}

        <!-- Header -->
        <div class="flex items-center gap-3 mb-4 pb-4 border-b border-border">
            <div class="font-mono text-xl font-semibold text-text">
                {event.event_type}
            </div>
            {#if event.category}
                <span
                    class="text-xs px-2 py-1 rounded border capitalize"
                    style="color: {color}; border-color: {color}"
                >
                    {event.category}
                </span>
            {/if}
        </div>

        <!-- Summary -->
        {#if event.summary}
            <div class="text-base text-text-secondary mb-6 leading-relaxed">
                {event.summary}
            </div>
        {/if}

        <!-- Time -->
        <div class="mb-6">
            <h4 class="text-xs font-semibold uppercase tracking-wide text-text-muted mb-2">Time</h4>
            <p class="text-text">{formatDateTime(event.occurred_at)}</p>
        </div>

        <!-- Data -->
        {#if Object.keys(event.data).length > 0}
            <div class="mb-6">
                <h4 class="text-xs font-semibold uppercase tracking-wide text-text-muted mb-2">
                    Данные
                </h4>
                <pre
                    class="font-mono text-xs bg-bg-secondary p-4 rounded-lg overflow-x-auto border border-border"
                    style="color: {color}">{formatJson(event.data)}</pre>
            </div>
        {/if}

        <!-- Tags -->
        {#if event.tags.length > 0}
            <div class="mb-6">
                <h4 class="text-xs font-semibold uppercase tracking-wide text-text-muted mb-2">
                    Теги
                </h4>
                <div class="flex flex-wrap gap-2">
                    {#each event.tags as tag (tag)}
                        <span class="text-xs px-2 py-1 bg-bg-secondary rounded text-text-secondary"
                            >{tag}</span
                        >
                    {/each}
                </div>
            </div>
        {/if}

        <!-- Meta -->
        <div class="mt-8 pt-4 border-t border-border flex flex-col gap-2">
            {#if event.source}
                <div class="flex justify-between items-center">
                    <span class="text-xs text-text-muted">Source</span>
                    <span class="text-xs text-text-secondary">{event.source}</span>
                </div>
            {/if}
            <div class="flex justify-between items-center">
                <span class="text-xs text-text-muted">ID</span>
                <span class="font-mono text-[10px] text-text-secondary">{event.id}</span>
            </div>
            <div class="flex justify-between items-center">
                <span class="text-xs text-text-muted">Created</span>
                <span class="text-xs text-text-secondary">{formatDateTime(event.created_at)}</span>
            </div>
        </div>
    {:else}
        <div class="flex flex-col items-center justify-center h-full text-text-muted gap-4">
            <svg
                class="w-12 h-12 opacity-50"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                stroke-width="1.5"
            >
                <path
                    d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"
                />
            </svg>
            <p>Select an event to view details</p>
        </div>
    {/if}
</div>
