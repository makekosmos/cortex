<script setup lang="ts">
import { useId } from "vue";

type TooltipPlacement = "top-start" | "top" | "top-end";

withDefaults(
  defineProps<{
    text: string;
    placement?: TooltipPlacement;
    focusable?: boolean;
  }>(),
  {
    placement: "top",
    focusable: false,
  },
);

const tooltipId = useId();
</script>

<template>
  <span
    class="kosmos-tooltip"
    :class="`kosmos-tooltip--${placement}`"
    :tabindex="focusable ? 0 : undefined"
    :aria-label="focusable ? text : undefined"
    :aria-describedby="tooltipId"
  >
    <slot />
    <span :id="tooltipId" class="kosmos-tooltip__content" role="tooltip">{{ text }}</span>
  </span>
</template>

<style scoped>
.kosmos-tooltip {
  position: relative;
  display: inline-flex;
}

.kosmos-tooltip__content {
  position: absolute;
  z-index: 100;
  bottom: calc(100% + 7px);
  left: 50%;
  width: max-content;
  max-width: 220px;
  padding: 6px 8px;
  border: 1px solid var(--border);
  border-radius: 6px;
  background: color-mix(in srgb, var(--foreground) 14%, var(--background));
  color: var(--foreground);
  font-size: 0.6875rem;
  line-height: 1.35;
  opacity: 0;
  pointer-events: none;
  transform: translate(-50%, 2px);
  transition:
    opacity 100ms ease,
    transform 100ms ease;
  white-space: nowrap;
}

.kosmos-tooltip--top-start .kosmos-tooltip__content {
  left: 0;
  transform: translate(0, 2px);
}

.kosmos-tooltip--top-end .kosmos-tooltip__content {
  right: 0;
  left: auto;
  transform: translate(0, 2px);
}

.kosmos-tooltip:hover .kosmos-tooltip__content,
.kosmos-tooltip:focus-visible .kosmos-tooltip__content {
  opacity: 1;
  transform: translate(-50%, 0);
}

.kosmos-tooltip--top-start:hover .kosmos-tooltip__content,
.kosmos-tooltip--top-start:focus-visible .kosmos-tooltip__content,
.kosmos-tooltip--top-end:hover .kosmos-tooltip__content,
.kosmos-tooltip--top-end:focus-visible .kosmos-tooltip__content {
  transform: translate(0, 0);
}
</style>
