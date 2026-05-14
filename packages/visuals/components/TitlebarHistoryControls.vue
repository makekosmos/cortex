<script setup lang="ts">
import { ChevronLeft, ChevronRight } from "lucide-vue-next";

interface Props {
  backDisabled?: boolean;
  forwardDisabled?: boolean;
  backTitle?: string;
  forwardTitle?: string;
}

const props = withDefaults(defineProps<Props>(), {
  backDisabled: false,
  forwardDisabled: false,
  backTitle: "Назад",
  forwardTitle: "Вперёд",
});

const emit = defineEmits<{
  back: [];
  forward: [];
}>();

function handleBack() {
  if (props.backDisabled) return;
  emit("back");
}

function handleForward() {
  if (props.forwardDisabled) return;
  emit("forward");
}
</script>

<template>
  <div class="kosmos-titlebar-history-controls">
    <button
      type="button"
      class="kosmos-titlebar-history-controls__button"
      :class="{ 'kosmos-titlebar-history-controls__button--disabled': backDisabled }"
      :disabled="backDisabled"
      :title="backTitle"
      :aria-label="backTitle"
      data-testid="titlebar-history-back"
      @click="handleBack"
    >
      <ChevronLeft :size="14" />
    </button>

    <button
      type="button"
      class="kosmos-titlebar-history-controls__button"
      :class="{ 'kosmos-titlebar-history-controls__button--disabled': forwardDisabled }"
      :disabled="forwardDisabled"
      :title="forwardTitle"
      :aria-label="forwardTitle"
      data-testid="titlebar-history-forward"
      @click="handleForward"
    >
      <ChevronRight :size="14" />
    </button>
  </div>
</template>

<style scoped>
.kosmos-titlebar-history-controls {
  display: inline-flex;
  align-items: center;
  gap: 0.125rem;
  -webkit-app-region: no-drag;
}

.kosmos-titlebar-history-controls__button {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: var(--kosmos-titlebar-control-size, 32px);
  height: var(--kosmos-titlebar-control-size, 32px);
  border-radius: var(--kosmos-titlebar-control-radius, 10px);
  color: color-mix(in srgb, var(--sidebar-foreground) 72%, transparent);
  transition:
    background-color 120ms ease,
    color 120ms ease,
    opacity 120ms ease;
  -webkit-app-region: no-drag;
}

.kosmos-titlebar-history-controls__button:hover:not(:disabled) {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  color: var(--foreground);
}

.kosmos-titlebar-history-controls__button--disabled,
.kosmos-titlebar-history-controls__button:disabled {
  opacity: 0.38;
  cursor: default;
}
</style>
