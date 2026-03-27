<script lang="ts">
    import * as d3 from "d3";
    import type { HierarchyRectangularNode } from "d3";
    import type { CategoryStats } from "./types";
    import { getCategoryColor } from "./types";

    interface Props {
        categories: CategoryStats[];
        totalEvents: number;
        onSelect?: (category: string) => void;
    }

    let { categories = [], totalEvents = 0, onSelect }: Props = $props();

    let hoveredCategory: string | null = $state(null);
    let hoveredStats: CategoryStats | null = $state(null);
    let hoveredBlock: TreemapNode | null = $state(null);

    interface TreemapNode {
        id: string;
        category: string;
        value: number;
        color: string;
        x: number;
        y: number;
        w: number;
        h: number;
    }

    interface HierarchyData {
        name: string;
        value?: number;
        children?: HierarchyData[];
    }

    const SIZE = 400;
    const TOOLTIP_WIDTH = 192;
    const TOOLTIP_GAP = 8;
    const CHAR_WIDTH = 7; // approximate width of a character in px at text-xs
    const CHAR_HEIGHT = 14; // approximate height of a character
    const PADDING = 8; // internal padding

    type TextLayout = "horizontal" | "vertical" | "ellipsis" | "hidden";

    function getTextLayout(text: string, w: number, h: number): TextLayout {
        const textWidth = text.length * CHAR_WIDTH;
        const textHeight = CHAR_HEIGHT;

        // Check if fits horizontally
        if (textWidth <= w - PADDING * 2 && textHeight <= h - PADDING * 2) {
            return "horizontal";
        }

        // Check if fits vertically
        if (textHeight <= w - PADDING * 2 && textWidth <= h - PADDING * 2) {
            return "vertical";
        }

        // Check if ellipsis fits (minimum 3 chars + "...")
        if (w >= PADDING * 2 + CHAR_WIDTH * 4 && h >= PADDING * 2 + CHAR_HEIGHT) {
            return "ellipsis";
        }

        // Too small for any text
        return "hidden";
    }

    function truncateText(text: string, maxWidth: number): string {
        const maxChars = Math.floor((maxWidth - PADDING * 2) / CHAR_WIDTH) - 3;
        if (maxChars <= 0) return "...";
        return text.slice(0, maxChars) + "...";
    }

    let blocks = $derived.by(() => {
        if (categories.length === 0) return [];

        const hierarchyData: HierarchyData = {
            name: "root",
            children: categories.map((cat) => ({
                name: cat.category,
                value: cat.count,
            })),
        };

        const root = d3
            .hierarchy(hierarchyData)
            .sum((d) => d.value ?? 0)
            .sort((a, b) => (b.value ?? 0) - (a.value ?? 0));

        const treemap = d3.treemap<HierarchyData>().size([SIZE, SIZE]).padding(0).round(true);

        const treemapRoot = treemap(root);

        const result: TreemapNode[] = [];
        for (const leaf of treemapRoot.leaves() as HierarchyRectangularNode<HierarchyData>[]) {
            result.push({
                id: leaf.data.name,
                category: leaf.data.name,
                value: leaf.value ?? 0,
                color: getCategoryColor(leaf.data.name),
                x: leaf.x0,
                y: leaf.y0,
                w: leaf.x1 - leaf.x0,
                h: leaf.y1 - leaf.y0,
            });
        }

        return result;
    });

    // Tooltip position based on block location
    let tooltipStyle = $derived.by(() => {
        if (!hoveredBlock) return "";

        const blockCenterX = hoveredBlock.x + hoveredBlock.w / 2;
        const blockCenterY = hoveredBlock.y + hoveredBlock.h / 2;
        const isLeftSide = blockCenterX < SIZE / 2;

        // Horizontal position
        let left: number;
        if (isLeftSide) {
            // Show tooltip on the right of the block
            left = hoveredBlock.x + hoveredBlock.w + TOOLTIP_GAP;
        } else {
            // Show tooltip on the left of the block
            left = hoveredBlock.x - TOOLTIP_WIDTH - TOOLTIP_GAP;
        }

        // Vertical position - center on block, but clamp to container
        let top = blockCenterY;

        return `left: ${left}px; top: ${top}px; transform: translateY(-50%);`;
    });

    function getPastelColor(hex: string): string {
        // Convert hex to RGB, then create a muted/pastel version with transparency
        const r = parseInt(hex.slice(1, 3), 16);
        const g = parseInt(hex.slice(3, 5), 16);
        const b = parseInt(hex.slice(5, 7), 16);
        // Return rgba with 25% opacity for subtle pastel effect
        return `rgba(${r}, ${g}, ${b}, 0.25)`;
    }

    function handleMouseEnter(block: TreemapNode) {
        hoveredCategory = block.category;
        hoveredBlock = block;
        hoveredStats = categories.find((c) => c.category === block.category) || null;
    }

    function handleMouseLeave() {
        hoveredCategory = null;
        hoveredBlock = null;
        hoveredStats = null;
    }

    function handleClick(category: string) {
        onSelect?.(category);
    }
</script>

<div class="relative flex items-center justify-center h-full p-8 select-none font-mono">
    <!-- Treemap container -->
    <div
        class="relative outline outline-2 outline-border"
        style="width: {SIZE}px; height: {SIZE}px;"
    >
        {#each blocks as block (block.id)}
            {@const isHovered = hoveredCategory === block.category}
            {@const touchesLeft = block.x === 0}
            {@const touchesTop = block.y === 0}
            {@const touchesRight = block.x + block.w >= SIZE}
            {@const touchesBottom = block.y + block.h >= SIZE}
            {@const layout = getTextLayout(block.category, block.w, block.h)}
            <button
                class="absolute flex flex-col items-center justify-center box-border
                       transition-colors duration-100 cursor-pointer overflow-hidden"
                style="
                    left: {block.x}px;
                    top: {block.y}px;
                    width: {block.w}px;
                    height: {block.h}px;
                    background-color: {isHovered ? getPastelColor(block.color) : 'transparent'};
                    border-style: solid;
                    border-color: var(--color-border);
                    border-width: {touchesTop ? 0 : 1}px {touchesRight ? 0 : 1}px {touchesBottom
                    ? 0
                    : 1}px {touchesLeft ? 0 : 1}px;
                "
                aria-label="{block.category}: {block.value} events"
                onmouseenter={() => handleMouseEnter(block)}
                onmouseleave={handleMouseLeave}
                onclick={() => handleClick(block.category)}
            >
                {#if layout === "horizontal"}
                    <span
                        class="font-medium text-xs leading-tight transition-colors duration-100"
                        style="color: {isHovered ? block.color : 'var(--color-text-secondary)'};"
                    >
                        {block.category}
                    </span>
                {:else if layout === "vertical"}
                    <span
                        class="font-medium text-xs leading-tight transition-colors duration-100"
                        style="color: {isHovered
                            ? block.color
                            : 'var(--color-text-secondary)'}; writing-mode: vertical-rl; text-orientation: mixed;"
                    >
                        {block.category}
                    </span>
                {:else if layout === "ellipsis"}
                    <span
                        class="font-medium text-xs leading-tight transition-colors duration-100"
                        style="color: {isHovered ? block.color : 'var(--color-text-secondary)'};"
                    >
                        {truncateText(block.category, block.w)}
                    </span>
                {/if}
            </button>
        {/each}

        <!-- Tooltip -->
        {#if hoveredStats && hoveredBlock}
            {@const color = getCategoryColor(hoveredStats.category)}
            <div
                class="absolute w-48 bg-bg-card p-3 z-50 border border-border pointer-events-none"
                style={tooltipStyle}
            >
                <div class="flex justify-between items-center pb-2 mb-2 border-b border-border">
                    <span class="font-medium text-sm" style="color: {color};">
                        {hoveredStats.category}
                    </span>
                    <span class="font-mono text-sm font-semibold text-white">
                        {hoveredStats.count.toLocaleString()}
                    </span>
                </div>

                <div class="text-[11px] text-zinc-500 mb-2">
                    {((hoveredStats.count / totalEvents) * 100).toFixed(1)}% of total
                </div>

                {#if Object.keys(hoveredStats.event_types).length > 0}
                    <div class="space-y-0.5">
                        {#each Object.entries(hoveredStats.event_types)
                            .sort((a, b) => b[1] - a[1])
                            .slice(0, 5) as [type, count] (type)}
                            <div class="flex justify-between text-[11px]">
                                <span class="text-zinc-500 truncate mr-2">{type}</span>
                                <span class="text-zinc-400 tabular-nums">{count}</span>
                            </div>
                        {/each}
                        {#if Object.keys(hoveredStats.event_types).length > 5}
                            <div class="text-[10px] text-zinc-600 pt-1">
                                +{Object.keys(hoveredStats.event_types).length - 5} more
                            </div>
                        {/if}
                    </div>
                {/if}
            </div>
        {/if}
    </div>
</div>
