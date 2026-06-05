<script setup lang="ts">
import { Clipboard, Copy, Pin, Trash2, XCircle } from "@lucide/vue";
import type { ClipboardHistoryItem } from "@shared/ipc-types";

defineProps<{
  items: ClipboardHistoryItem[];
  selectedItem: ClipboardHistoryItem | null;
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

    <div class="clipboard-panel__surface">
      <section class="clipboard-panel__list" aria-label="История буфера обмена">
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
                <span>Текст</span>
                <span>{{ itemTime(item) }}</span>
                <span>{{ charLabel(item.charCount) }}</span>
              </span>
            </span>
          </li>
        </ul>
      </section>

      <section class="clipboard-panel__detail" aria-label="Подробности буфера обмена">
        <template v-if="selectedItem">
          <div class="clipboard-panel__detail-toolbar">
            <div class="clipboard-panel__detail-title">
              <strong>Текст</strong>
              <span>{{ itemTime(selectedItem) }}</span>
            </div>
            <div class="clipboard-panel__actions">
              <button
                class="clipboard-panel__action clipboard-panel__action--primary"
                type="button"
                title="Скопировать"
                @click="emit('copy', selectedIndex)"
              >
                <Copy :size="14" />
                <span>Скопировать</span>
              </button>
              <button class="clipboard-panel__action" type="button" title="Закрепить" disabled>
                <Pin :size="14" />
              </button>
              <button
                class="clipboard-panel__action"
                type="button"
                title="Удалить"
                @click="emit('remove', selectedIndex)"
              >
                <Trash2 :size="14" />
              </button>
            </div>
          </div>
          <div class="clipboard-panel__preview-pane">
            <pre>{{ selectedItem.text }}</pre>
          </div>
          <dl class="clipboard-panel__metadata">
            <div>
              <dt>Тип</dt>
              <dd>Текст</dd>
            </div>
            <div>
              <dt>Символы</dt>
              <dd>{{ selectedItem.charCount }}</dd>
            </div>
            <div>
              <dt>Скопировано</dt>
              <dd>{{ itemTime(selectedItem) }}</dd>
            </div>
          </dl>
        </template>
        <div v-else class="clipboard-panel__empty clipboard-panel__empty--detail">
          <Clipboard :size="28" />
          <span>Выбери запись слева</span>
        </div>
      </section>
    </div>

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
.clipboard-panel__actions,
.clipboard-panel__action {
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
.clipboard-panel__action {
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
.clipboard-panel__action:not(:disabled):hover {
  background: color-mix(in srgb, var(--foreground) 7%, transparent);
  color: var(--foreground);
}

.clipboard-panel__clear:disabled {
  opacity: 0.42;
}

.clipboard-panel__surface {
  display: grid;
  min-height: 0;
  grid-template-columns: minmax(300px, 0.86fr) minmax(360px, 1.14fr);
  gap: 12px;
  padding: 0 12px;
}

.clipboard-panel__list,
.clipboard-panel__detail {
  min-height: 0;
  border: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  border-radius: 8px;
  background: color-mix(in srgb, var(--foreground) 3.5%, transparent);
  overflow: hidden;
}

.clipboard-panel__list {
  overflow-y: auto;
}

.clipboard-panel__results {
  display: grid;
  gap: 1px;
  margin: 0;
  padding: 6px;
  list-style: none;
}

.clipboard-panel__row {
  min-height: 72px;
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

.clipboard-panel__detail {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
}

.clipboard-panel__detail-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  border-bottom: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  padding: 10px 12px;
}

.clipboard-panel__detail-title {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.clipboard-panel__detail-title strong {
  color: var(--foreground);
  font-size: 13px;
}

.clipboard-panel__detail-title span {
  color: color-mix(in srgb, var(--foreground) 46%, transparent);
  font-size: 11px;
}

.clipboard-panel__actions {
  flex: 0 0 auto;
  gap: 6px;
}

.clipboard-panel__action {
  justify-content: center;
  min-height: 30px;
  gap: 7px;
  border-color: color-mix(in srgb, var(--foreground) 9%, transparent);
  border-radius: 7px;
  padding: 0 10px;
  font-size: 12px;
}

.clipboard-panel__action--primary {
  border-color: color-mix(in srgb, var(--accent) 45%, transparent);
  background: color-mix(in srgb, var(--accent) 15%, transparent);
  color: var(--foreground);
}

.clipboard-panel__action:disabled {
  opacity: 0.35;
}

.clipboard-panel__preview-pane {
  min-height: 0;
  overflow: auto;
  padding: 14px;
}

.clipboard-panel__preview-pane pre {
  margin: 0;
  color: var(--foreground);
  font-family: var(--font-sans);
  font-size: 13px;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}

.clipboard-panel__metadata {
  display: grid;
  gap: 8px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  margin: 0;
  padding: 10px 12px;
}

.clipboard-panel__metadata div {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr);
  gap: 10px;
}

.clipboard-panel__metadata dt,
.clipboard-panel__metadata dd {
  margin: 0;
  font-size: 12px;
}

.clipboard-panel__metadata dt {
  color: color-mix(in srgb, var(--foreground) 42%, transparent);
}

.clipboard-panel__metadata dd {
  color: var(--foreground);
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

.clipboard-panel__empty--detail {
  min-height: 0;
}

.clipboard-panel__footer {
  flex-wrap: wrap;
  gap: 10px;
  padding: 4px 14px 0;
}
</style>
