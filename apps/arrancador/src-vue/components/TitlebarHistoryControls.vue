<script setup lang="ts">
import { ChevronLeft, ChevronRight } from "lucide-vue-next";

defineProps<{
  backDisabled: boolean;
  forwardDisabled: boolean;
  backTitle: string;
  forwardTitle: string;
}>();

const emit = defineEmits<{
  back: [];
  forward: [];
}>();
</script>

<template>
  <div class="kepler-titlebar-history-controls">
    <button
      type="button"
      :disabled="backDisabled"
      :title="backTitle"
      :aria-label="backTitle"
      class="kepler-titlebar-history-controls__button"
      :class="{ 'kepler-titlebar-history-controls__button--disabled': backDisabled }"
      @click="emit('back')"
    >
      <ChevronLeft class="h-4 w-4" />
    </button>

    <button
      type="button"
      :disabled="forwardDisabled"
      :title="forwardTitle"
      :aria-label="forwardTitle"
      class="kepler-titlebar-history-controls__button"
      :class="{
        'kepler-titlebar-history-controls__button--disabled': forwardDisabled,
      }"
      @click="emit('forward')"
    >
      <ChevronRight class="h-4 w-4" />
    </button>
  </div>
</template>
