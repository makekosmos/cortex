<script setup lang="ts">
import { computed } from "vue";
import type { DelphiTask } from "@shared/ipc-types";

interface Props {
  open: boolean;
  query: string;
  tasks: DelphiTask[];
  highlightedIndex: number;
}

const props = defineProps<Props>();
const emit = defineEmits<{
  pick: [task: DelphiTask];
  hover: [index: number];
}>();

const filtered = computed(() => {
  const q = props.query.toLowerCase().trim();
  if (!q) return props.tasks.slice(0, 8);
  return props.tasks.filter((t) => t.title.toLowerCase().includes(q)).slice(0, 8);
});

defineExpose({ filtered });
</script>

<template>
  <div v-if="open" class="mention-menu" role="listbox" aria-label="Задачи Delphi">
    <div v-if="filtered.length === 0" class="mention-menu__empty">
      Задач не найдено. Создай в Delphi.
    </div>
    <button
      v-for="(t, i) in filtered"
      :key="t.id"
      type="button"
      role="option"
      class="mention-menu__item"
      :class="{ 'mention-menu__item--active': i === props.highlightedIndex }"
      :aria-selected="i === props.highlightedIndex"
      @mouseenter="emit('hover', i)"
      @mousedown.prevent="emit('pick', t)"
    >
      <span class="mention-menu__title">{{ t.title || "Без названия" }}</span>
      <span v-if="t.status" class="mention-menu__status">{{ t.status }}</span>
    </button>
  </div>
</template>

<style scoped>
.mention-menu {
  position: absolute;
  top: 100%;
  left: 0;
  margin-top: 6px;
  min-width: 280px;
  max-width: 360px;
  max-height: 280px;
  overflow-y: auto;
  padding: 0.25rem;
  background: var(--popover, var(--background));
  color: var(--popover-foreground, var(--foreground));
  border: 1px solid var(--border);
  border-radius: calc(var(--radius) * 0.85);
  corner-shape: var(--corner-shape);
  /* Shadow построен через color-mix относительно --foreground:
     в dark теме фон тёмный, поэтому смешиваем с черным через прозрачность foreground'а. */
  box-shadow:
    0 12px 32px rgb(0 0 0 / 28%),
    0 4px 12px rgb(0 0 0 / 14%);
  z-index: 80;
}

.mention-menu__empty {
  padding: 0.625rem 0.75rem;
  font-size: 0.8125rem;
  color: color-mix(in srgb, var(--foreground) 50%, transparent);
}

.mention-menu__item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 0.75rem;
  width: 100%;
  padding: 0.5rem 0.625rem;
  background: transparent;
  border: none;
  border-radius: calc(var(--radius) * 0.6);
  cursor: pointer;
  text-align: left;
  color: var(--foreground);
  font-size: 0.8125rem;
}

.mention-menu__item:hover,
.mention-menu__item--active {
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
}

.mention-menu__title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mention-menu__status {
  font-size: 0.6875rem;
  color: color-mix(in srgb, var(--foreground) 55%, transparent);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 0.05rem 0.4rem;
  border-radius: 999px;
  background: color-mix(in srgb, var(--foreground) 8%, transparent);
  flex-shrink: 0;
}
</style>
