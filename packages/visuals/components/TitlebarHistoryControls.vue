<script setup lang="ts">
import { ChevronLeft, ChevronRight } from "@lucide/vue";

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
  <div class="kosmos-titlebar-history-controls inline-flex items-center gap-2">
    <button
      type="button"
      class="inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color,opacity] duration-[120ms] ease-in hover:not-disabled:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:not-disabled:text-(--foreground) disabled:cursor-default disabled:opacity-[0.38]"
      :disabled="backDisabled"
      :title="backTitle"
      :aria-label="backTitle"
      data-testid="titlebar-history-back"
      @click="handleBack"
    >
      <ChevronLeft :size="16" />
    </button>

    <button
      type="button"
      class="inline-flex size-[var(--kosmos-titlebar-control-size,32px)] items-center justify-center rounded-[var(--kosmos-titlebar-control-radius,8px)] text-[color-mix(in_srgb,var(--sidebar-foreground)_72%,transparent)] transition-[background-color,color,opacity] duration-[120ms] ease-in hover:not-disabled:bg-[color-mix(in_srgb,var(--foreground)_8%,transparent)] hover:not-disabled:text-(--foreground) disabled:cursor-default disabled:opacity-[0.38]"
      :disabled="forwardDisabled"
      :title="forwardTitle"
      :aria-label="forwardTitle"
      data-testid="titlebar-history-forward"
      @click="handleForward"
    >
      <ChevronRight :size="16" />
    </button>
  </div>
</template>

<style scoped>
.kosmos-titlebar-history-controls {
  -webkit-app-region: no-drag;
}
</style>
