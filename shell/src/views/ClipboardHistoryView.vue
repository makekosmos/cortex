<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, shallowRef } from "vue";
import { Check, Clipboard, Search, Trash2, X, XCircle } from "@lucide/vue";
import { DesktopChrome, DesktopContentSurface } from "@kosmos/visuals";
import type { ClipboardHistoryItem } from "../../shared/ipc-types";
import ClipboardQuickPanel from "../components/ClipboardQuickPanel.vue";

const items = shallowRef<ClipboardHistoryItem[]>([]);
const query = shallowRef("");
const selectedId = shallowRef<string | null>(null);
const status = shallowRef<string | null>(null);
const loading = shallowRef(true);
let stopUpdates: (() => void) | null = null;
let statusTimer: number | null = null;

const filteredItems = computed(() => {
  const needle = query.value.trim().toLocaleLowerCase("ru-RU");
  if (!needle) return items.value;
  return items.value.filter((item) => item.searchText.toLocaleLowerCase("ru-RU").includes(needle));
});

const selectedItem = computed(() => {
  const id = selectedId.value;
  return filteredItems.value.find((item) => item.id === id) ?? filteredItems.value[0] ?? null;
});

const selectedIndex = computed(() => {
  const item = selectedItem.value;
  if (!item) return 0;
  return Math.max(
    0,
    filteredItems.value.findIndex((candidate) => candidate.id === item.id),
  );
});

onMounted(async () => {
  stopUpdates = window.kepler.clipboardHistory.onUpdated(loadItems);
  await loadItems();
  window.addEventListener("keydown", handleKeydown);
});

onBeforeUnmount(() => {
  stopUpdates?.();
  stopUpdates = null;
  window.removeEventListener("keydown", handleKeydown);
  if (statusTimer !== null) window.clearTimeout(statusTimer);
});

async function loadItems(): Promise<void> {
  try {
    items.value = await window.kepler.clipboardHistory.list();
    if (!selectedId.value || !items.value.some((item) => item.id === selectedId.value)) {
      selectedId.value = items.value[0]?.id ?? null;
    }
  } finally {
    loading.value = false;
  }
}

function selectIndex(index: number): void {
  selectedId.value = filteredItems.value[index]?.id ?? null;
}

async function copyItem(id: string | null = selectedItem.value?.id ?? null): Promise<void> {
  if (!id) return;
  const ok = await window.kepler.clipboardHistory.copy(id);
  if (ok) {
    showStatus("Скопировано");
    await loadItems();
  }
}

async function copyItemAt(index: number): Promise<void> {
  await copyItem(filteredItems.value[index]?.id ?? null);
}

async function openItemAt(index: number): Promise<void> {
  const id = filteredItems.value[index]?.id;
  if (!id) return;
  const ok = await window.kepler.clipboardHistory.open(id);
  if (ok) showStatus("Открыто");
}

async function togglePinAt(index: number): Promise<void> {
  const item = filteredItems.value[index];
  if (!item) return;
  const ok = await window.kepler.clipboardHistory.togglePin(item.id);
  if (ok) {
    selectedId.value = item.id;
    showStatus(item.pinned ? "Откреплено" : "Закреплено");
    await loadItems();
  }
}

async function deleteItem(id: string | null = selectedItem.value?.id ?? null): Promise<void> {
  if (!id) return;
  const index = filteredItems.value.findIndex((item) => item.id === id);
  const ok = await window.kepler.clipboardHistory.delete(id);
  if (ok) {
    const next = filteredItems.value[Math.min(index, filteredItems.value.length - 1)] ?? null;
    selectedId.value = next?.id ?? null;
    showStatus("Удалено");
    await loadItems();
  }
}

async function deleteItemAt(index: number): Promise<void> {
  await deleteItem(filteredItems.value[index]?.id ?? null);
}

async function clearItems(): Promise<void> {
  await window.kepler.clipboardHistory.clear();
  showStatus("Незакреплённые записи очищены");
  await loadItems();
}

async function clearAllItems(): Promise<void> {
  await window.kepler.clipboardHistory.clearAll();
  selectedId.value = null;
  showStatus("История очищена");
  await loadItems();
}

function handleKeydown(event: KeyboardEvent): void {
  if (event.defaultPrevented) return;
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLTextAreaElement) {
    if (event.key !== "Escape") return;
  }

  if (event.key === "ArrowDown") {
    event.preventDefault();
    moveSelection(1);
  } else if (event.key === "ArrowUp") {
    event.preventDefault();
    moveSelection(-1);
  } else if (event.key === "Enter") {
    event.preventDefault();
    void copyItem();
  } else if ((event.ctrlKey || event.metaKey) && event.code === "KeyO") {
    event.preventDefault();
    void openItemAt(selectedIndex.value);
  } else if ((event.ctrlKey || event.metaKey) && event.shiftKey && event.code === "KeyP") {
    event.preventDefault();
    void togglePinAt(selectedIndex.value);
  } else if (event.key === "Delete") {
    event.preventDefault();
    void deleteItem();
  } else if (event.key === "Escape") {
    if (query.value) {
      event.preventDefault();
      query.value = "";
    } else {
      void window.kepler.clipboardHistory.hide();
    }
  }
}

function moveSelection(delta: number): void {
  const rows = filteredItems.value;
  if (rows.length === 0) return;
  const current = selectedItem.value;
  const currentIndex = current ? rows.findIndex((item) => item.id === current.id) : -1;
  const nextIndex = Math.max(0, Math.min(rows.length - 1, currentIndex + delta));
  selectedId.value = rows[nextIndex]?.id ?? null;
}

function showStatus(message: string): void {
  status.value = message;
  if (statusTimer !== null) window.clearTimeout(statusTimer);
  statusTimer = window.setTimeout(() => {
    status.value = null;
    statusTimer = null;
  }, 1800);
}
</script>

<template>
  <DesktopChrome platform="windows" title="Буфер обмена">
    <DesktopContentSurface class="clipboard-history">
      <header class="clipboard-history__header">
        <div class="clipboard-history__title-row">
          <div class="clipboard-history__title-icon" aria-hidden="true">
            <Clipboard :size="18" />
          </div>
          <div class="clipboard-history__title-copy">
            <h1>Буфер обмена</h1>
            <p>{{ items.length }} записей в этой сессии</p>
          </div>
        </div>
        <div v-if="status" class="clipboard-history__status" aria-live="polite">
          <Check :size="14" />
          <span>{{ status }}</span>
        </div>
      </header>

      <label class="clipboard-history__search">
        <Search :size="16" aria-hidden="true" />
        <span class="clipboard-history__sr-only">Искать в буфере обмена</span>
        <input v-model="query" type="search" placeholder="Искать в буфере" autocomplete="off" />
        <button
          v-if="query"
          class="clipboard-history__icon-button"
          type="button"
          title="Очистить поиск"
          @click="query = ''"
        >
          <X :size="15" />
        </button>
      </label>

      <main class="clipboard-history__body">
        <ClipboardQuickPanel
          :items="filteredItems"
          :selected-item="selectedItem"
          :selected-index="selectedIndex"
          :loading="loading"
          :empty-label="items.length === 0 ? 'История пока пустая' : 'Ничего не найдено'"
          @select="selectIndex"
          @copy="copyItemAt"
          @open="openItemAt"
          @toggle-pin="togglePinAt"
          @remove="deleteItemAt"
          @clear="clearItems"
          @clear-all="clearAllItems"
        />
      </main>

      <footer class="clipboard-history__footer">
        <div class="clipboard-history__hints">
          <span>Enter — скопировать</span>
          <span>Ctrl+O — открыть</span>
          <span>Ctrl+Shift+P — закрепить</span>
          <span>Delete — удалить</span>
          <span>Esc — закрыть</span>
        </div>
        <button
          class="clipboard-history__clear"
          type="button"
          :disabled="items.length === 0"
          @click="clearAllItems"
        >
          <Trash2 :size="15" />
          <span>Удалить всё</span>
        </button>
        <button
          class="clipboard-history__clear"
          type="button"
          :disabled="items.length === 0"
          @click="clearItems"
        >
          <XCircle :size="15" />
          <span>Очистить</span>
        </button>
      </footer>
    </DesktopContentSurface>
  </DesktopChrome>
</template>

<style scoped>
.clipboard-history {
  display: grid;
  grid-template-rows: auto auto minmax(0, 1fr) auto;
  min-height: 0;
  min-width: 0;
  gap: 12px;
  padding: 18px;
  overflow: hidden;
  background: color-mix(in srgb, var(--background) 94%, transparent);
}

.clipboard-history__header,
.clipboard-history__title-row,
.clipboard-history__search,
.clipboard-history__row,
.clipboard-history__detail-toolbar,
.clipboard-history__actions,
.clipboard-history__footer,
.clipboard-history__hints,
.clipboard-history__status,
.clipboard-history__clear,
.clipboard-history__action {
  display: flex;
  align-items: center;
}

.clipboard-history__header {
  justify-content: space-between;
  gap: 12px;
}

.clipboard-history__title-row {
  min-width: 0;
  gap: 10px;
}

.clipboard-history__title-icon {
  display: grid;
  width: 32px;
  height: 32px;
  flex: 0 0 auto;
  place-items: center;
  border: 1px solid color-mix(in srgb, var(--accent) 42%, var(--border));
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--accent) 12%, transparent);
  color: var(--accent);
}

.clipboard-history__title-copy {
  min-width: 0;
}

.clipboard-history__title-copy h1,
.clipboard-history__title-copy p {
  margin: 0;
}

.clipboard-history__title-copy h1 {
  font-size: 17px;
  font-weight: 760;
}

.clipboard-history__title-copy p {
  margin-top: 2px;
  color: var(--muted-foreground);
  font-size: 12px;
}

.clipboard-history__status {
  max-width: 220px;
  gap: 6px;
  border: 1px solid color-mix(in srgb, var(--accent) 50%, var(--border));
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--accent) 12%, var(--background));
  color: var(--foreground);
  padding: 7px 10px;
  font-size: 12px;
  white-space: nowrap;
}

.clipboard-history__search {
  min-width: 0;
  gap: 8px;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--muted-foreground);
  padding: 8px 10px;
}

.clipboard-history__search input {
  min-width: 0;
  flex: 1;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--foreground);
  font-size: 13px;
}

.clipboard-history__search input::placeholder {
  color: var(--muted-foreground);
}

.clipboard-history__icon-button,
.clipboard-history__action,
.clipboard-history__clear {
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--foreground) 6%, transparent);
  color: var(--foreground);
}

.clipboard-history__icon-button {
  display: grid;
  width: 24px;
  height: 24px;
  place-items: center;
  padding: 0;
}

.clipboard-history__body {
  display: block;
  min-height: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  overflow: hidden;
}

.clipboard-history__body :deep(.clipboard-panel) {
  height: 100%;
}

.clipboard-history__list,
.clipboard-history__detail {
  min-height: 0;
  border: 1px solid var(--border);
  border-radius: var(--radius-card);
  background: color-mix(in srgb, var(--foreground) 4%, transparent);
  overflow: hidden;
}

.clipboard-history__list {
  display: grid;
  align-content: start;
  overflow-y: auto;
  padding: 6px;
}

.clipboard-history__row {
  width: 100%;
  min-height: 62px;
  justify-content: flex-start;
  border: 1px solid transparent;
  border-radius: calc(var(--radius-card) - 2px);
  background: transparent;
  padding: 9px 10px;
  text-align: left;
}

.clipboard-history__row:hover,
.clipboard-history__row--active {
  border-color: color-mix(in srgb, var(--accent) 40%, var(--border));
  background: color-mix(in srgb, var(--accent) 13%, transparent);
}

.clipboard-history__row-main {
  display: grid;
  min-width: 0;
  gap: 7px;
}

.clipboard-history__row-preview {
  display: -webkit-box;
  overflow: hidden;
  color: var(--foreground);
  font-size: 13px;
  line-height: 1.35;
  -webkit-box-orient: vertical;
  -webkit-line-clamp: 2;
}

.clipboard-history__row-meta,
.clipboard-history__detail-meta,
.clipboard-history__hints {
  color: var(--muted-foreground);
  font-size: 11px;
}

.clipboard-history__row-meta {
  display: flex;
  gap: 10px;
}

.clipboard-history__detail {
  display: grid;
  grid-template-rows: auto minmax(0, 1fr);
}

.clipboard-history__detail-toolbar {
  justify-content: space-between;
  gap: 10px;
  border-bottom: 1px solid var(--border);
  padding: 10px;
}

.clipboard-history__detail-meta {
  display: grid;
  min-width: 0;
  gap: 2px;
}

.clipboard-history__detail-meta strong {
  color: var(--foreground);
  font-size: 13px;
}

.clipboard-history__actions {
  flex: 0 0 auto;
  gap: 8px;
}

.clipboard-history__action,
.clipboard-history__clear {
  justify-content: center;
  gap: 7px;
  min-height: 30px;
  padding: 0 10px;
  font-size: 12px;
}

.clipboard-history__action--primary {
  border-color: color-mix(in srgb, var(--accent) 48%, var(--border));
  background: color-mix(in srgb, var(--accent) 16%, transparent);
}

.clipboard-history__action:not(:disabled):hover,
.clipboard-history__clear:not(:disabled):hover,
.clipboard-history__icon-button:hover {
  background: color-mix(in srgb, var(--foreground) 10%, transparent);
}

.clipboard-history__clear:disabled {
  cursor: default;
  opacity: 0.45;
}

.clipboard-history__preview {
  width: 100%;
  height: 100%;
  min-height: 0;
  resize: none;
  border: 0;
  outline: 0;
  background: transparent;
  color: var(--foreground);
  padding: 14px;
  font-size: 13px;
  line-height: 1.45;
  white-space: pre-wrap;
}

.clipboard-history__empty {
  display: grid;
  min-height: 160px;
  place-items: center;
  gap: 10px;
  color: var(--muted-foreground);
  padding: 24px;
  text-align: center;
  font-size: 13px;
}

.clipboard-history__empty--detail {
  min-height: 0;
}

.clipboard-history__footer {
  justify-content: space-between;
  gap: 12px;
}

.clipboard-history__hints {
  min-width: 0;
  flex-wrap: wrap;
  gap: 10px;
}

.clipboard-history__clear {
  flex: 0 0 auto;
}

.clipboard-history__sr-only {
  position: absolute;
  width: 1px;
  height: 1px;
  overflow: hidden;
  clip: rect(0, 0, 0, 0);
  white-space: nowrap;
}

@media (max-width: 680px) {
  .clipboard-history {
    padding: 14px;
  }

  .clipboard-history__header,
  .clipboard-history__footer {
    align-items: flex-start;
    flex-direction: column;
  }

  .clipboard-history__body {
    grid-template-columns: 1fr;
  }

  .clipboard-history__detail {
    min-height: 220px;
  }
}
</style>
