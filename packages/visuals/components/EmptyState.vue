<script setup lang="ts">
// EmptyState — стандартный empty-state для list views, trash, search results.
// Используется в Eden (SearchOverlay nothing), Delphi (QuickSearch empty),
// Arrancador (game list empty), Horologion (ListView no entries).
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
    class="kosmos-empty-state"
    :class="{ 'kosmos-empty-state--compact': compact }"
  >
    <div v-if="$slots.icon" class="kosmos-empty-state__icon">
      <slot name="icon" />
    </div>
    <div class="kosmos-empty-state__title">{{ title }}</div>
    <div v-if="description" class="kosmos-empty-state__description">
      {{ description }}
    </div>
    <div v-if="$slots.action" class="kosmos-empty-state__action">
      <slot name="action" />
    </div>
  </div>
</template>

<style scoped>
.kosmos-empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 2rem 1.5rem;
  text-align: center;
  color: var(--muted-foreground);
}

.kosmos-empty-state--compact {
  padding: 1rem 0.75rem;
  gap: 0.25rem;
}

.kosmos-empty-state__icon {
  display: flex;
  align-items: center;
  justify-content: center;
  color: color-mix(in srgb, var(--muted-foreground) 70%, transparent);
  margin-bottom: 0.25rem;
}

.kosmos-empty-state__title {
  font-size: 0.9375rem;
  color: var(--foreground);
}

.kosmos-empty-state--compact .kosmos-empty-state__title {
  font-size: 0.875rem;
}

.kosmos-empty-state__description {
  font-size: 0.8125rem;
  line-height: 1.45;
  max-width: 320px;
}

.kosmos-empty-state__action {
  margin-top: 0.5rem;
}
</style>
