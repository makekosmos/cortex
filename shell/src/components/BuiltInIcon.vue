<script setup lang="ts">
import type { Component } from "vue";
import { HelpCircle } from "lucide-vue-next";

const props = withDefaults(
    defineProps<{
        icon?: Component | null;
        from?: string;
        to?: string;
        size?: number;
        strokeWidth?: number;
    }>(),
    {
        icon: null,
        from: "oklch(0.72 0.14 240)",
        to: "oklch(0.45 0.18 260)",
        size: 13,
        strokeWidth: 2,
    },
);
</script>

<template>
    <span class="builtin-icon" :style="{ backgroundImage: `linear-gradient(to bottom left, ${props.from}, ${props.to})` }">
        <component :is="props.icon ?? HelpCircle" :size="props.size" :stroke-width="props.strokeWidth" />
    </span>
</template>

<style scoped>
.builtin-icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    border-radius: 6px;
    color: oklch(0.96 0 0);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
    flex-shrink: 0;
}
</style>
