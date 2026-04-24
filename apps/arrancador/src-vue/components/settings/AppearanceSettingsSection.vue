<script setup lang="ts">
import { Monitor, Moon, Sun } from "lucide-vue-next";
import type { Theme } from "../../composables/useTheme";

const props = withDefaults(
  defineProps<{
    sectionId?: string;
    theme: Theme;
  }>(),
  {
    sectionId: undefined,
  },
);

const emit = defineEmits<{
  updateTheme: [theme: Theme];
}>();

function themeButtonClass(current: Theme, expected: Theme) {
  return current === expected
    ? "border-primary bg-primary text-primary-foreground"
    : "border-border/70 bg-card/70 text-card-foreground hover:bg-accent/70";
}
</script>

<template>
  <section :id="props.sectionId" class="space-y-4">
    <div class="flex items-center gap-2">
      <Monitor class="h-5 w-5" />
      <h2 class="text-base font-semibold sm:text-lg">Оформление</h2>
    </div>

    <div class="rounded-2xl border border-border/70 bg-card/90 p-4 shadow-sm">
      <div class="mb-3 text-sm font-medium">Тема</div>
      <div class="flex flex-wrap gap-2">
        <button
          type="button"
          class="inline-flex flex-1 items-center justify-center gap-2 rounded-xl border px-4 py-2 text-sm transition-colors sm:flex-none"
          :class="themeButtonClass(props.theme, 'light')"
          @click="emit('updateTheme', 'light')"
        >
          <Sun class="h-4 w-4" />
          Светлая
        </button>
        <button
          type="button"
          class="inline-flex flex-1 items-center justify-center gap-2 rounded-xl border px-4 py-2 text-sm transition-colors sm:flex-none"
          :class="themeButtonClass(props.theme, 'dark')"
          @click="emit('updateTheme', 'dark')"
        >
          <Moon class="h-4 w-4" />
          Тёмная
        </button>
        <button
          type="button"
          class="inline-flex w-full items-center justify-center gap-2 rounded-xl border px-4 py-2 text-sm transition-colors sm:w-auto"
          :class="themeButtonClass(props.theme, 'system')"
          @click="emit('updateTheme', 'system')"
        >
          <Monitor class="h-4 w-4" />
          Системная
        </button>
      </div>
    </div>
  </section>
</template>
