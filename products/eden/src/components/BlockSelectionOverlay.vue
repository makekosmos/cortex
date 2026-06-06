<script setup lang="ts">
// BlockSelectionOverlay — rubber-band rectangle для drag-selection блоков.
// Absolute-positioned, mount'ится в editor container (relative parent).
// pointer-events: none — не блокирует mouse handlers на editor.
//
// Style мирок Anytype: rgba(55, 122, 255, 0.25) bg + 1px solid #2aa7ee
// border + radius 2px. Через CSS var override родитель может переопределить
// цвет (Eden подсовывает свой orange-tint? — нет, для selection blue
// сильнее работает на restraint vs accent).

import type { DragRect } from "@/composables/useBlockSelection";

defineProps<{ rect: DragRect | null }>();
</script>

<template>
  <div
    v-if="rect"
    class="block-selection-rect"
    :style="{
      left: `${rect.x}px`,
      top: `${rect.y}px`,
      width: `${rect.width}px`,
      height: `${rect.height}px`,
    }"
    aria-hidden="true"
  />
</template>

<style scoped>
.block-selection-rect {
  position: absolute;
  z-index: 20;
  background: var(--block-selection-bg, rgba(55, 122, 255, 0.25));
  border: 1px solid var(--block-selection-border, #2aa7ee);
  border-radius: 2px;
  pointer-events: none;
  /* GPU acceleration на translate3d не используем — left/top работают
     одинаково быстро для коротких frame-to-frame изменений drag'а. */
}
</style>
