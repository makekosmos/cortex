<script setup lang="ts">
// EmptyState — стандартный empty-state для list views, trash, search results.
// Используется в Eden (SearchOverlay nothing), Delphi (QuickSearch empty),
// Arrancador (game list empty).
//
// title — заголовок (например «Нет заметок»)
// description — поясняющий текст (опционально)
// Slot `action` — кнопка действия (например «Создать первую заметку»)
// Slot `icon` — кастомный icon (по умолчанию empty)

interface Props {
  title: string;
  description?: string;
  /** Уменьшенный вариант для компактных контейнеров (popover, sidebar). */
  compact?: boolean;
}

withDefaults(defineProps<Props>(), {
  compact: false,
});
</script>

<template>
  <div
    class="flex flex-col items-center justify-center text-center text-[var(--muted-foreground)]"
    :class="compact ? 'gap-2 px-4 py-4' : 'gap-2 px-8 py-8'"
  >
    <div
      v-if="$slots.icon"
      class="mb-2 flex items-center justify-center text-[color-mix(in_srgb,var(--muted-foreground)_70%,transparent)]"
    >
      <slot name="icon" />
    </div>
    <div class="text-[var(--foreground)]" :class="compact ? 'text-sm' : 'text-[0.9375rem]'">
      {{ title }}
    </div>
    <div v-if="description" class="max-w-80 text-[0.8125rem] leading-[1.45]">
      {{ description }}
    </div>
    <div v-if="$slots.action" class="mt-2">
      <slot name="action" />
    </div>
  </div>
</template>
