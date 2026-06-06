<script setup lang="ts">
import { computed, nextTick, onBeforeUpdate, watch } from "vue";
import { Clipboard, File, Image as ImageIcon, Link, Palette, Pin } from "@lucide/vue";
import type { ClipboardHistoryItem } from "@shared/ipc-types";

const props = defineProps<{
  items: ClipboardHistoryItem[];
  selectedItem: ClipboardHistoryItem | null;
  selectedIndex: number;
  loading: boolean;
  emptyLabel: string;
}>();

const TEXT_PREVIEW_LIMIT = 1000;

const emit = defineEmits<{
  select: [index: number];
  copy: [index: number];
  open: [index: number];
  togglePin: [index: number];
  remove: [index: number];
  clear: [];
  clearAll: [];
}>();

const detailDateFormatter = new Intl.DateTimeFormat("ru-RU", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
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

const rowRefs = new Map<number, HTMLElement>();

function setRowRef(index: number, element: Element | null): void {
  if (element instanceof HTMLElement) {
    rowRefs.set(index, element);
    return;
  }
  rowRefs.delete(index);
}

onBeforeUpdate(() => {
  rowRefs.clear();
});

watch(
  () => [props.selectedIndex, props.items.length],
  async () => {
    await nextTick();
    rowRefs.get(props.selectedIndex)?.scrollIntoView({ block: "nearest" });
  },
);

function itemDateTime(item: ClipboardHistoryItem): string {
  return detailDateFormatter.format(new Date(item.createdAt));
}

function itemKindLabel(item: ClipboardHistoryItem): string {
  if (item.kind === "image") return "Изображение";
  if (item.kind === "link") return "Ссылка";
  if (item.kind === "color") return "Цвет";
  if (item.kind === "file") return "Файл";
  return "Текст";
}

function itemDetailLabel(item: ClipboardHistoryItem): string {
  if (item.kind === "image" && item.width && item.height) return `${item.width}×${item.height}`;
  if (item.kind === "file") return item.filePath ?? item.text;
  if (item.kind === "link") return item.url ?? item.text;
  if (item.kind === "color") return item.color ?? item.text;
  return charLabel(item.charCount);
}

function itemMetricLabel(item: ClipboardHistoryItem): string {
  if (item.kind === "image" && item.width && item.height) return "Разрешение";
  if (item.kind === "file") return "Путь";
  if (item.kind === "link") return "URL";
  if (item.kind === "color") return "HEX";
  return "Символы";
}

function previewText(item: ClipboardHistoryItem): string {
  if (item.text.length <= TEXT_PREVIEW_LIMIT) return item.text;
  return item.text.slice(0, TEXT_PREVIEW_LIMIT);
}

function hiddenTextLabel(item: ClipboardHistoryItem): string {
  return `+ ${charLabel(Math.max(0, item.text.length - TEXT_PREVIEW_LIMIT))}`;
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
      <section class="clipboard-panel__list kosmos-scroll" aria-label="История буфера обмена">
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
                :ref="(element) => setRowRef(index, element)"
                :class="{ selected: selectedIndex === index }"
                @click="emit('select', index)"
                @dblclick="emit('copy', index)"
              >
                <span class="clipboard-panel__row-icon" aria-hidden="true">
                  <img
                    v-if="item.kind === 'image' && item.imageDataUrl"
                    class="clipboard-panel__row-thumb"
                    :src="item.imageDataUrl"
                    alt=""
                  />
                  <ImageIcon v-else-if="item.kind === 'image'" :size="13" />
                  <Link v-else-if="item.kind === 'link'" :size="13" />
                  <Palette v-else-if="item.kind === 'color'" :size="13" />
                  <File v-else-if="item.kind === 'file'" :size="13" />
                  <Clipboard v-else :size="13" />
                </span>
                <span class="clipboard-panel__row-body">
                  <span class="clipboard-panel__preview">
                    <Pin v-if="item.pinned" class="clipboard-panel__pin" :size="12" />
                    <span>{{ item.preview }}</span>
                  </span>
                </span>
              </li>
            </ul>
          </div>
        </template>
      </section>

      <section class="clipboard-panel__detail" aria-label="Подробности буфера обмена">
        <template v-if="selectedItem">
          <div
            class="clipboard-panel__preview-pane"
            :class="{ 'clipboard-panel__preview-pane--image': selectedItem.kind === 'image' }"
          >
            <img
              v-if="selectedItem.kind === 'image' && selectedItem.imageDataUrl"
              class="clipboard-panel__image-preview"
              :src="selectedItem.imageDataUrl"
              alt=""
            />
            <div
              v-else-if="selectedItem.kind === 'color'"
              class="clipboard-panel__color-preview"
              :style="{ backgroundColor: selectedItem.color ?? selectedItem.text }"
            >
              <span>{{ selectedItem.color ?? selectedItem.text }}</span>
            </div>
            <div
              v-else
              class="clipboard-panel__text-preview kosmos-scroll"
              :class="{
                'clipboard-panel__text-preview--truncated':
                  selectedItem.text.length > TEXT_PREVIEW_LIMIT,
              }"
            >
              <div
                class="clipboard-panel__text-preview-content"
                :class="{
                  'clipboard-panel__text-preview-content--truncated':
                    selectedItem.text.length > TEXT_PREVIEW_LIMIT,
                }"
              >
                <pre>{{ previewText(selectedItem) }}</pre>
              </div>
              <span
                v-if="selectedItem.text.length > TEXT_PREVIEW_LIMIT"
                class="clipboard-panel__text-overflow-count"
              >
                {{ hiddenTextLabel(selectedItem) }}
              </span>
            </div>
          </div>
          <dl class="clipboard-panel__metadata kosmos-scroll">
            <div v-if="selectedItem.source">
              <dt>Источник</dt>
              <dd class="clipboard-panel__source-value">
                <img
                  v-if="selectedItem.sourceIcon"
                  class="clipboard-panel__source-icon"
                  :src="selectedItem.sourceIcon"
                  alt=""
                />
                <span>{{ selectedItem.source }}</span>
              </dd>
            </div>
            <div>
              <dt>Тип</dt>
              <dd>{{ itemKindLabel(selectedItem) }}</dd>
            </div>
            <div>
              <dt>{{ itemMetricLabel(selectedItem) }}</dt>
              <dd>{{ itemDetailLabel(selectedItem) }}</dd>
            </div>
            <div>
              <dt>Закреплено</dt>
              <dd>{{ selectedItem.pinned ? "Да" : "Нет" }}</dd>
            </div>
            <div>
              <dt>Скопировано</dt>
              <dd>{{ itemDateTime(selectedItem) }}</dd>
            </div>
          </dl>
        </template>
        <div v-else class="clipboard-panel__empty clipboard-panel__empty--detail">
          <Clipboard :size="28" />
          <span>Выбери запись слева</span>
        </div>
      </section>
    </div>
  </div>
</template>

<style scoped>
.clipboard-panel {
  display: grid;
  height: 100%;
  grid-template-rows: minmax(0, 1fr);
  min-height: 0;
}

.clipboard-panel__row {
  display: flex;
  align-items: center;
}

.clipboard-panel__surface {
  display: grid;
  overflow: hidden;
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
  min-width: 0;
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
  min-width: 0;
  min-height: 30px;
  gap: 8px;
  margin: 1px 0;
  border: 1px solid transparent;
  border-radius: 6px;
  background: transparent;
  padding: 4px 8px;
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
  width: 18px;
  height: 18px;
  flex: 0 0 auto;
  place-items: center;
  color: color-mix(in srgb, var(--foreground) 82%, transparent);
}

.clipboard-panel__row-thumb {
  width: 18px;
  height: 18px;
  border-radius: 3px;
  object-fit: cover;
}

.clipboard-panel__row-body {
  display: flex;
  min-width: 0;
  flex: 1;
  overflow: hidden;
}

.clipboard-panel__preview {
  display: flex;
  width: 100%;
  min-width: 0;
  align-items: center;
  gap: 5px;
  overflow: hidden;
  color: var(--foreground);
  font-size: 14px;
  font-weight: 650;
  line-height: 1.25;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.clipboard-panel__preview span {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}

.clipboard-panel__pin {
  flex: 0 0 auto;
  color: var(--accent);
}

.clipboard-panel__detail {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
}

.clipboard-panel__preview-pane {
  height: 216px;
  min-height: 0;
  overflow: hidden;
  padding: 18px 16px;
}

.clipboard-panel__preview-pane--image {
  display: grid;
  place-items: center;
}

.clipboard-panel__image-preview {
  max-width: 100%;
  max-height: 216px;
  border-radius: 6px;
  object-fit: contain;
}

.clipboard-panel__color-preview {
  display: grid;
  width: min(100%, 420px);
  min-height: 180px;
  place-items: end start;
  border-radius: 8px;
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--foreground) 15%, transparent);
}

.clipboard-panel__color-preview span {
  border-radius: 5px;
  background: color-mix(in srgb, var(--background) 88%, transparent);
  color: var(--foreground);
  margin: 12px;
  padding: 6px 8px;
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
}

.clipboard-panel__text-preview {
  position: relative;
  height: 100%;
  overflow-y: auto;
}

.clipboard-panel__text-preview pre {
  margin: 0;
  color: var(--foreground);
  font-family: var(--font-mono);
  font-size: 12px;
  font-weight: 700;
  line-height: 1.45;
  white-space: pre-wrap;
  word-break: break-word;
}

.clipboard-panel__text-preview-content {
  position: relative;
}

.clipboard-panel__text-preview-content--truncated::after {
  position: absolute;
  right: 0;
  bottom: 0;
  left: 0;
  display: block;
  height: 64px;
  background: linear-gradient(
    180deg,
    color-mix(in srgb, var(--main-background-color) 0%, transparent) 0%,
    color-mix(in srgb, var(--main-background-color) 72%, transparent) 62%,
    var(--main-background-color) 100%
  );
  content: "";
  pointer-events: none;
}

.clipboard-panel__text-overflow-count {
  display: block;
  color: color-mix(in srgb, var(--foreground) 58%, transparent);
  font-size: 12px;
  font-weight: 700;
  padding-top: 10px;
  pointer-events: none;
}

.clipboard-panel__metadata {
  display: grid;
  min-height: 0;
  gap: 0;
  border-top: 1px solid color-mix(in srgb, var(--foreground) 8%, transparent);
  margin: 0;
  overflow-y: auto;
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

.clipboard-panel__source-value {
  display: inline-flex;
  min-width: 0;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.clipboard-panel__source-icon {
  width: 16px;
  height: 16px;
  flex: 0 0 auto;
  border-radius: 3px;
  object-fit: cover;
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
</style>
