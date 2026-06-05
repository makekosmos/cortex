<script setup lang="ts">
import { Clipboard, Copy, Trash2, XCircle } from "@lucide/vue";
import type { ClipboardHistoryItem } from "@shared/ipc-types";

defineProps<{
  items: ClipboardHistoryItem[];
  selectedIndex: number;
  loading: boolean;
  emptyLabel: string;
}>();

const emit = defineEmits<{
  select: [index: number];
  copy: [index: number];
  remove: [index: number];
  clear: [];
}>();

const formatter = new Intl.DateTimeFormat("ru-RU", {
  hour: "2-digit",
  minute: "2-digit",
});

function itemTime(item: ClipboardHistoryItem): string {
  return formatter.format(new Date(item.createdAt));
}

function charLabel(count: number): string {
  const mod10 = count % 10;
  const mod100 = count % 100;
  if (mod10 === 1 && mod100 !== 11) return `${count} символ`;
  if (mod10 >= 2 && mod10 <= 4 && (mod100 < 12 || mod100 > 14)) return `${count} символа`;
  return `${count} символов`;
}
</script>

<template>
  <div class="clipboard-panel">
    <div class="clipboard-panel__header">
      <div class="clipboard-panel__title">
        <span class="clipboard-panel__icon" aria-hidden="true">
          <Clipboard :size="16" />
        </span>
        <span>Буфер обмена</span>
      </div>
      <button
        class="clipboard-panel__clear"
        type="button"
        :disabled="items.length === 0"
        title="Очистить историю"
        @click="emit('clear')"
      >
        <XCircle :size="14" />
        <span>Очистить</span>
      </button>
    </div>

    <div v-if="loading" class="clipboard-panel__empty">Загружаю историю</div>
    <div v-else-if="items.length === 0" class="clipboard-panel__empty">{{ emptyLabel }}</div>

    <ul v-else class="clipboard-panel__results">
      <li
        v-for="(item, index) in items"
        :key="item.id"
        class="clipboard-panel__row result"
        :class="{ selected: selectedIndex === index }"
        @click="emit('select', index)"
        @dblclick="emit('copy', index)"
      >
        <span class="clipboard-panel__row-body">
          <span class="clipboard-panel__preview">{{ item.preview }}</span>
          <span class="clipboard-panel__meta">
            <span>{{ itemTime(item) }}</span>
            <span>{{ charLabel(item.charCount) }}</span>
          </span>
        </span>
        <button
          class="clipboard-panel__icon-button"
          type="button"
          title="Скопировать"
          @click.stop="emit('copy', index)"
        >
          <Copy :size="14" />
        </button>
        <button
          class="clipboard-panel__icon-button"
          type="button"
          title="Удалить"
          @click.stop="emit('remove', index)"
        >
          <Trash2 :size="14" />
        </button>
      </li>
    </ul>

    <div class="clipboard-panel__footer">
      <span>Enter — скопировать</span>
      <span>Delete — удалить</span>
      <span>Esc — закрыть</span>
    </div>
  </div>
</template>

<style scoped>
.clipboard-panel {
  display: grid;
  min-height: 0;
  gap: 8px;
}

.clipboard-panel__header,
.clipboard-panel__title,
.clipboard-panel__clear,
.clipboard-panel__row,
.clipboard-panel__meta,
.clipboard-panel__footer,
.clipboard-panel__icon-button {
  display: flex;
  align-items: center;
}

.clipboard-panel__header {
  justify-content: space-between;
  padding: 2px 14px 0;
}

.clipboard-panel__title {
  gap: 8px;
  color: color-mix(in srgb, var(--foreground) 72%, transparent);
  font-size: 12px;
  font-weight: 650;
  text-transform: uppercase;
}

.clipboard-panel__icon {
  display: inline-grid;
  width: 24px;
  height: 24px;
  place-items: center;
  border: 1px solid color-mix(in srgb, var(--accent) 38%, transparent);
  border-radius: 6px;
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  color: var(--accent);
}

.clipboard-panel__clear,
.clipboard-panel__icon-button {
  border: 1px solid transparent;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 54%, transparent);
}

.clipboard-panel__clear {
  gap: 6px;
  border-radius: 6px;
  padding: 5px 8px;
  font-size: 12px;
}

.clipboard-panel__clear:not(:disabled):hover,
.clipboard-panel__icon-button:hover {
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  color: var(--foreground);
}

.clipboard-panel__clear:disabled {
  opacity: 0.42;
}

.clipboard-panel__results {
  display: grid;
  gap: 1px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.clipboard-panel__row {
  min-height: 66px;
  gap: 8px;
  margin: 1px 0;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  padding: 10px 14px;
}

.clipboard-panel__row:hover {
  background: color-mix(in srgb, oklch(1 0 0) 3.5%, transparent);
}

.clipboard-panel__row.selected {
  border-color: color-mix(in srgb, oklch(1 0 0) 12%, transparent);
  background: color-mix(in srgb, oklch(1 0 0) 8%, transparent);
}

.clipboard-panel__row-body {
  display: grid;
  min-width: 0;
  flex: 1;
  gap: 6px;
}

.clipboard-panel__preview {
  display: -webkit-box;
  overflow: hidden;
  color: var(--foreground);
  font-size: 14px;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.clipboard-panel__meta,
.clipboard-panel__footer {
  color: color-mix(in srgb, var(--foreground) 46%, transparent);
  font-size: 11px;
}

.clipboard-panel__meta {
  gap: 10px;
}

.clipboard-panel__icon-button {
  width: 28px;
  height: 28px;
  flex: 0 0 auto;
  justify-content: center;
  border-radius: 6px;
  padding: 0;
}

.clipboard-panel__empty {
  display: grid;
  min-height: 220px;
  place-items: center;
  color: color-mix(in srgb, var(--foreground) 42%, transparent);
  padding: 32px 22px;
  text-align: center;
  font-size: 13px;
}

.clipboard-panel__footer {
  flex-wrap: wrap;
  gap: 10px;
  padding: 4px 14px 0;
}
</style>
