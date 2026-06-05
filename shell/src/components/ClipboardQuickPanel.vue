<script setup lang="ts">
import { computed } from "vue";
import { Clipboard, XCircle } from "@lucide/vue";
import type { ClipboardHistoryItem } from "@shared/ipc-types";

const props = defineProps<{
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

const groupedItems = computed(() => {
  const now = new Date();
  const today = new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime();
  const yesterday = today - 24 * 60 * 60 * 1000;
  const groups: Array<{
    key: string;
    title: string;
    rows: Array<{ item: ClipboardHistoryItem; index: number }>;
  }> = [
    { key: "today", title: "Сегодня", rows: [] },
    { key: "yesterday", title: "Вчера", rows: [] },
    { key: "older", title: "Ранее", rows: [] },
  ];

  props.items.forEach((item, index) => {
    const day = new Date(item.createdAt);
    const dayStart = new Date(day.getFullYear(), day.getMonth(), day.getDate()).getTime();
    if (dayStart >= today) groups[0]!.rows.push({ item, index });
    else if (dayStart >= yesterday) groups[1]!.rows.push({ item, index });
    else groups[2]!.rows.push({ item, index });
  });

  return groups.filter((group) => group.rows.length > 0);
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
    <div class="clipboard-panel__surface">
      <section class="clipboard-panel__list" aria-label="История буфера обмена">
        <div v-if="loading" class="clipboard-panel__empty">Загружаю историю</div>
        <div v-else-if="items.length === 0" class="clipboard-panel__empty">{{ emptyLabel }}</div>

        <template v-else>
          <div v-for="group in groupedItems" :key="group.key" class="clipboard-panel__group">
            <div class="clipboard-panel__section-label">{{ group.title }}</div>
            <ul class="clipboard-panel__results">
              <li
                v-for="{ item, index } in group.rows"
                :key="item.id"
                class="clipboard-panel__row result"
                :class="{ selected: selectedIndex === index }"
                @click="emit('select', index)"
                @dblclick="emit('copy', index)"
              >
                <span class="clipboard-panel__row-icon" aria-hidden="true">
                  <Clipboard :size="15" />
                </span>
                <span class="clipboard-panel__row-body">
                  <span class="clipboard-panel__preview">{{ item.preview }}</span>
                </span>
              </li>
            </ul>
          </div>
        </template>
      </section>

      <section class="clipboard-panel__detail" aria-label="Подробности буфера обмена">
        <template v-if="selectedItem">
          <div class="clipboard-panel__preview-pane">
            <pre>{{ selectedItem.text }}</pre>
          </div>
          <dl class="clipboard-panel__metadata">
            <div class="clipboard-panel__metadata-heading">
              <dt>Информация</dt>
              <dd />
            </div>
            <div>
              <dt>Источник</dt>
              <dd>Kosmos</dd>
            </div>
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
      <div class="clipboard-panel__footer-title">
        <span class="clipboard-panel__footer-icon" aria-hidden="true">
          <Clipboard :size="14" />
        </span>
        <span>История буфера</span>
      </div>
      <div class="clipboard-panel__footer-actions">
        <span>Вставить</span>
        <kbd>Enter</kbd>
        <span class="clipboard-panel__footer-divider" />
        <button
          class="clipboard-panel__footer-button"
          type="button"
          :disabled="items.length === 0"
          @click="emit('clear')"
        >
          <XCircle :size="14" />
          <span>Очистить</span>
        </button>
        <span>Действия</span>
        <kbd>Ctrl</kbd>
        <kbd>K</kbd>
      </div>
    </div>
  </div>
</template>

<style scoped>
.clipboard-panel {
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
  min-height: 0;
}

.clipboard-panel__row,
.clipboard-panel__footer,
.clipboard-panel__footer-title,
.clipboard-panel__footer-actions,
.clipboard-panel__footer-button {
  display: flex;
  align-items: center;
}

.clipboard-panel__surface {
  display: grid;
  min-height: 0;
  grid-template-columns: minmax(286px, 0.78fr) minmax(390px, 1.22fr);
}

.clipboard-panel__list,
.clipboard-panel__detail {
  min-height: 0;
  border: 0;
  border-radius: 0;
  background: transparent;
  overflow: hidden;
}

.clipboard-panel__list {
  border-right: 1px solid color-mix(in srgb, var(--foreground) 9%, transparent);
  overflow-y: auto;
}

.clipboard-panel__group {
  padding: 0 8px 8px;
}

.clipboard-panel__section-label {
  padding: 10px 6px 6px;
  color: color-mix(in srgb, var(--foreground) 54%, transparent);
  font-size: 11px;
  font-weight: 650;
}

.clipboard-panel__results {
  display: grid;
  gap: 1px;
  margin: 0;
  padding: 0;
  list-style: none;
}

.clipboard-panel__row {
  min-height: 38px;
  gap: 8px;
  margin: 1px 0;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  padding: 6px 8px;
}

.clipboard-panel__row:hover {
  background: color-mix(in srgb, oklch(1 0 0) 3.5%, transparent);
}

.clipboard-panel__row.selected {
  border-color: color-mix(in srgb, oklch(1 0 0) 12%, transparent);
  background: color-mix(in srgb, oklch(1 0 0) 8%, transparent);
}

.clipboard-panel__row-icon {
  display: grid;
  width: 24px;
  height: 24px;
  flex: 0 0 auto;
  place-items: center;
  border-radius: 4px;
  background: color-mix(in srgb, var(--foreground) 14%, transparent);
  color: color-mix(in srgb, var(--foreground) 82%, transparent);
}

.clipboard-panel__row-body {
  display: grid;
  min-width: 0;
  flex: 1;
  gap: 6px;
}

.clipboard-panel__preview {
  overflow: hidden;
  color: var(--foreground);
  font-size: 14px;
  font-weight: 650;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.clipboard-panel__detail {
  display: grid;
  grid-template-rows: minmax(0, 1fr) auto;
}

.clipboard-panel__preview-pane {
  min-height: 0;
  overflow: auto;
  padding: 18px 16px;
}

.clipboard-panel__preview-pane pre {
  margin: 0;
  color: var(--foreground);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}

.clipboard-panel__metadata {
  display: grid;
  gap: 0;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  margin: 0;
  padding: 12px 16px;
}

.clipboard-panel__metadata div {
  display: grid;
  grid-template-columns: 120px minmax(0, 1fr);
  gap: 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--foreground) 7%, transparent);
  padding: 8px 0;
}

.clipboard-panel__metadata dt,
.clipboard-panel__metadata dd {
  margin: 0;
  font-size: 12px;
}

.clipboard-panel__metadata dt {
  color: color-mix(in srgb, var(--foreground) 42%, transparent);
  font-weight: 650;
}

.clipboard-panel__metadata dd {
  color: var(--foreground);
  text-align: right;
}

.clipboard-panel__metadata-heading {
  border-bottom: 0 !important;
  padding-top: 0 !important;
}

.clipboard-panel__metadata-heading dt {
  grid-column: 1 / -1;
  color: color-mix(in srgb, var(--foreground) 62%, transparent);
}

.clipboard-panel__metadata-heading dd {
  display: none;
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
  justify-content: space-between;
  min-height: 40px;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 9%, transparent);
  padding: 0 10px;
  color: color-mix(in srgb, var(--foreground) 74%, transparent);
  font-size: 12px;
}

.clipboard-panel__footer-title,
.clipboard-panel__footer-actions,
.clipboard-panel__footer-button {
  gap: 7px;
}

.clipboard-panel__footer-icon {
  display: grid;
  width: 22px;
  height: 22px;
  place-items: center;
  border-radius: 5px;
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  color: var(--accent);
}

.clipboard-panel__footer-actions {
  color: var(--foreground);
  font-weight: 650;
}

.clipboard-panel__footer-actions kbd {
  border: 1px solid color-mix(in srgb, var(--foreground) 14%, transparent);
  border-radius: 4px;
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: color-mix(in srgb, var(--foreground) 70%, transparent);
  padding: 1px 5px 2px;
  font: inherit;
  font-size: 11px;
}

.clipboard-panel__footer-divider {
  width: 1px;
  height: 16px;
  margin: 0 4px;
  background: color-mix(in srgb, var(--foreground) 15%, transparent);
}

.clipboard-panel__footer-button {
  border: 0;
  background: transparent;
  color: color-mix(in srgb, var(--foreground) 74%, transparent);
  padding: 0;
  font: inherit;
}
</style>
