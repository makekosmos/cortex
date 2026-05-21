<script setup lang="ts">
import type { Component } from "vue";
import { HelpCircle } from "lucide-vue-next";

const props = withDefaults(
  defineProps<{
    icon?: Component | null;
    /** Монохромный глиф (fill="white" в svg) поверх gradient'а, 18×18
            в squircle 20×20. Для брендовых иконок extension'ов. */
    svgSrc?: string | null;
    from?: string;
    to?: string;
    /** Цвет глифа (Lucide). По умолчанию белый — на тёмных gradient'ах.
            Для светлых backgrounds передавай тёмный. */
    iconColor?: string;
    size?: number;
    strokeWidth?: number;
  }>(),
  {
    icon: null,
    svgSrc: null,
    from: "oklch(0.72 0.14 240)",
    to: "oklch(0.45 0.18 260)",
    iconColor: "oklch(0.96 0 0)",
    size: 13,
    strokeWidth: 2,
  },
);
</script>

<template>
  <span
    class="builtin-icon"
    :style="{
      backgroundImage: `linear-gradient(to bottom left, ${props.from}, ${props.to})`,
      color: props.iconColor,
    }"
  >
    <img v-if="props.svgSrc" :src="props.svgSrc" class="svg-glyph" alt="" />
    <component
      v-else
      :is="props.icon ?? HelpCircle"
      :size="props.size"
      :stroke-width="props.strokeWidth"
    />
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
  box-shadow: inset 0 0 0 1px color-mix(in srgb, oklch(1 0 0) 6%, transparent);
  flex-shrink: 0;
  overflow: hidden;
}

.svg-glyph {
  width: 18px;
  height: 18px;
  display: block;
}
</style>
