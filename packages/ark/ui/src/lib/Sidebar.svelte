<script lang="ts">
    import { SvelteSet } from "svelte/reactivity";
    import type { CategoryStats } from "./types";
    import { getCategoryColor } from "./types";

    interface Props {
        categories: CategoryStats[];
        selectedCategory: string | null;
        selectedEventType: string | null;
        totalEvents: number;
        onSelectCategory?: (category: string | null) => void;
        onSelectEventType?: (
            data: { category: string; eventType: string } | null,
        ) => void;
    }

    let {
        categories = [],
        selectedCategory = null,
        selectedEventType = null,
        totalEvents = 0,
        onSelectCategory,
        onSelectEventType,
    }: Props = $props();

    let expandedCategories = new SvelteSet<string>();

    function toggleCategory(category: string) {
        if (expandedCategories.has(category)) {
            expandedCategories.delete(category);
        } else {
            expandedCategories.add(category);
        }
    }
</script>

<aside
    class="w-[280px] bg-bg-secondary border-r border-border flex flex-col shrink-0"
>
    <div
        class="flex items-center justify-between p-4 pb-3 border-b border-border"
    >
        <h3
            class="text-xs font-semibold uppercase tracking-wide text-text-muted"
        >
            Categories
        </h3>
        <button
            class="text-xs px-2 py-1 border border-border rounded text-text-muted
             hover:border-accent hover:text-accent transition-all
             {selectedCategory !== null ? 'opacity-100' : 'opacity-0'}"
            onclick={() => onSelectCategory?.(null)}
        >
            Clear
        </button>
    </div>

    <div class="flex-1 overflow-y-auto p-2">
        <!-- All Events -->
        <button
            class="flex items-center gap-2 w-full px-3 py-2.5 rounded-lg text-left transition-all
             border border-transparent mb-2 pb-3 border-b border-border rounded-b-none
             {selectedCategory === null
                ? 'bg-bg-card border-accent'
                : 'hover:bg-bg-card'}"
            onclick={() => onSelectCategory?.(null)}
        >
            <span class="flex-1 text-sm text-text">All Events</span>
            <span class="font-mono text-xs text-text-muted"
                >{totalEvents.toLocaleString()}</span
            >
        </button>

        <!-- Categories -->
        {#each categories as cat (cat.category)}
            {@const color = getCategoryColor(cat.category)}
            {@const isExpanded = expandedCategories.has(cat.category)}
            {@const isSelected = selectedCategory === cat.category}

            <div class="mb-1">
                <div class="flex items-center gap-1">
                    <button
                        class="flex items-center gap-2 flex-1 px-3 py-2.5 rounded-lg text-left transition-all
                   border border-transparent
                   {isSelected && !selectedEventType
                            ? 'bg-bg-card'
                            : 'hover:bg-bg-card'}"
                        style={isSelected && !selectedEventType
                            ? `border-color: ${color}`
                            : ""}
                        onclick={() => onSelectCategory?.(cat.category)}
                    >
                        <span
                            class="w-2 h-2 rounded-full shrink-0"
                            style="background: {color}"
                        ></span>
                        <span class="flex-1 text-sm text-text capitalize"
                            >{cat.category}</span
                        >
                        <span class="font-mono text-xs text-text-muted"
                            >{cat.count.toLocaleString()}</span
                        >
                    </button>

                    <button
                        class="w-5 h-5 flex items-center justify-center text-text-muted hover:text-text
                   transition-transform {isExpanded ? 'rotate-90' : ''}"
                        aria-label="Expand {cat.category}"
                        onclick={() => toggleCategory(cat.category)}
                    >
                        <svg
                            class="w-3 h-3"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="2"
                        >
                            <path d="M9 5l7 7-7 7" />
                        </svg>
                    </button>
                </div>

                {#if isExpanded}
                    <div class="pl-6 mt-1 mb-2">
                        {#each Object.entries(cat.event_types).sort((a, b) => b[1] - a[1]) as [eventType, count] (eventType)}
                            <button
                                class="flex items-center justify-between w-full px-2.5 py-1.5 rounded text-left transition-all
                       border border-transparent
                       {selectedCategory === cat.category &&
                                selectedEventType === eventType
                                    ? 'bg-bg-card'
                                    : 'hover:bg-bg-card'}"
                                style={selectedCategory === cat.category &&
                                selectedEventType === eventType
                                    ? `border-color: ${color}`
                                    : ""}
                                onclick={() =>
                                    onSelectEventType?.({
                                        category: cat.category,
                                        eventType,
                                    })}
                            >
                                <span
                                    class="font-mono text-xs text-text-secondary"
                                    >{eventType}</span
                                >
                                <span
                                    class="font-mono text-[10px] text-text-muted"
                                    >{count}</span
                                >
                            </button>
                        {/each}
                    </div>
                {/if}
            </div>
        {/each}
    </div>
</aside>
