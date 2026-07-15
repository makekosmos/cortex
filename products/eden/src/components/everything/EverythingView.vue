<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, useTemplateRef, watch } from "vue";
import { ContextMenu, ContextMenuItem, useContextMenu } from "@kosmos/visuals";
import { Trash2 } from "@lucide/vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { SYSTEM_TYPE_BOOK_ID, SYSTEM_TYPE_NOTE_ID } from "@/lib/systemTypes";
import EverythingItemCard from "./EverythingItemCard.vue";

const props = defineProps<{
  entries: Entry[];
  noteTypes: NoteType[];
}>();

const emit = defineEmits<{
  createEntry: [];
  deleteEntry: [entryId: string];
  openEntry: [entryId: string];
}>();

const supportedTypeIds = new Set([SYSTEM_TYPE_NOTE_ID, SYSTEM_TYPE_BOOK_ID]);
const searchQuery = shallowRef("");
const normalizedSearchQuery = computed(() => searchQuery.value.trim().toLocaleLowerCase("ru-RU"));
const isFiltering = computed(() => normalizedSearchQuery.value.length > 0);
const fullTextMatchIds = shallowRef<ReadonlySet<string>>(new Set());
const gridRef = useTemplateRef<HTMLElement>("grid");
const cardMenu = useContextMenu<string>();
const availableColumnCount = shallowRef(1);
let searchTimer: number | null = null;
let searchRequestId = 0;

watch(normalizedSearchQuery, (query) => {
  searchRequestId += 1;
  const requestId = searchRequestId;
  if (searchTimer !== null) window.clearTimeout(searchTimer);
  fullTextMatchIds.value = new Set();
  if (!query) return;

  searchTimer = window.setTimeout(async () => {
    searchTimer = null;
    const results = (await window.api?.searchEntries(query).catch(() => [])) ?? [];
    if (requestId !== searchRequestId) return;
    fullTextMatchIds.value = new Set(results.map((result) => result.entryId));
  }, 300);
});

onBeforeUnmount(() => {
  searchRequestId += 1;
  if (searchTimer !== null) window.clearTimeout(searchTimer);
});

function getAuthor(entry: Entry): string {
  try {
    const headerProps = JSON.parse(entry.header_props_json || "{}");
    return String(headerProps?.author ?? "");
  } catch {
    return "";
  }
}

const visibleEntries = computed(() =>
  props.entries
    .filter((entry) => entry.type_id !== null && supportedTypeIds.has(entry.type_id))
    .filter((entry) => {
      if (!normalizedSearchQuery.value) return true;
      const localMatch =
        `${getEntryDisplayTitle(entry.title, entry.header_props_json)} ${getAuthor(entry)}`
          .toLocaleLowerCase("ru-RU")
          .includes(normalizedSearchQuery.value);
      return localMatch || fullTextMatchIds.value.has(entry.id);
    })
    .sort((left, right) => right.updated_at - left.updated_at),
);
const noteTypesById = computed(
  () =>
    new Map(
      props.noteTypes.map((noteType) => [noteType.id, noteType] satisfies [string, NoteType]),
    ),
);
const entriesById = computed(
  () => new Map(props.entries.map((entry) => [entry.id, entry] satisfies [string, Entry])),
);
const renderedColumnCount = computed(() =>
  Math.min(
    availableColumnCount.value,
    Math.max(1, visibleEntries.value.length + (isFiltering.value ? 0 : 1)),
  ),
);
const masonryColumns = computed(() => {
  const columns = Array.from({ length: renderedColumnCount.value }, () => [] as Entry[]);
  const offset = isFiltering.value ? 0 : 1;
  visibleEntries.value.forEach((entry, index) => {
    columns[(index + offset) % columns.length]!.push(entry);
  });
  return columns;
});

function updateColumnCount(grid: HTMLElement, width: number): void {
  const styles = getComputedStyle(grid);
  const gap = Number.parseFloat(styles.columnGap) || 0;
  const columnWidth =
    Number.parseFloat(styles.getPropertyValue("--everything-column-width")) || 330;
  availableColumnCount.value = Math.max(1, Math.floor((width + gap) / (columnWidth + gap)));
}

function deleteMenuEntry(): void {
  const entryId = cardMenu.payload.value;
  cardMenu.close();
  if (entryId) emit("deleteEntry", entryId);
}

watch(gridRef, (grid, _previous, onCleanup) => {
  if (!grid) return;
  updateColumnCount(grid, grid.clientWidth);
  let resizeFrame: number | null = null;
  const observer = new ResizeObserver(([entry]) => {
    if (resizeFrame !== null) window.cancelAnimationFrame(resizeFrame);
    resizeFrame = window.requestAnimationFrame(() => {
      resizeFrame = null;
      updateColumnCount(grid, entry!.contentRect.width);
    });
  });
  observer.observe(grid);
  onCleanup(() => {
    observer.disconnect();
    if (resizeFrame !== null) window.cancelAnimationFrame(resizeFrame);
  });
});
</script>

<template>
  <section class="everything-view kosmos-scroll" data-testid="everything-view">
    <div class="everything-content">
      <header class="everything-header">
        <input
          v-model="searchQuery"
          class="everything-search"
          type="search"
          aria-label="Поиск"
          placeholder="Поиск"
        />
      </header>

      <div
        v-if="visibleEntries.length || !isFiltering"
        ref="grid"
        class="everything-grid"
        data-testid="everything-grid"
        :style="{
          gridTemplateColumns: `repeat(${masonryColumns.length}, minmax(0, 1fr))`,
        }"
      >
        <div
          v-for="(column, columnIndex) in masonryColumns"
          :key="columnIndex"
          class="everything-column"
        >
          <button
            v-if="!isFiltering && columnIndex === 0"
            type="button"
            class="everything-add-card"
            aria-label="Добавить заметку"
            data-testid="everything-add-card"
            @click="emit('createEntry')"
          >
            <span aria-hidden="true">+</span>
          </button>

          <EverythingItemCard
            v-for="entry in column"
            :key="entry.id"
            :entry="entry"
            :note-type="noteTypesById.get(entry.type_id ?? '') ?? null"
            :entries-by-id="entriesById"
            @context-menu="cardMenu.open($event, entry.id)"
            @open-entry="emit('openEntry', $event)"
          />
        </div>
      </div>
    </div>

    <ContextMenu
      :open="cardMenu.isOpen.value"
      :x="cardMenu.x.value"
      :y="cardMenu.y.value"
      :elevated="false"
      @close="cardMenu.close"
    >
      <ContextMenuItem destructive @click="deleteMenuEntry">
        <Trash2 :size="15" aria-hidden="true" />
        Удалить
      </ContextMenuItem>
    </ContextMenu>
  </section>
</template>

<style scoped>
.everything-view {
  container-type: inline-size;
  height: 100%;
  min-width: 0;
  overflow-y: auto;
  padding: var(--space-8) var(--space-4) var(--space-12);
}

.everything-content {
  width: 100%;
}

.everything-header {
  margin-block-end: var(--space-6);
}

.everything-search {
  width: 100%;
  height: 2.5rem;
  padding-inline: var(--space-4);
  border: 2px solid var(--card, transparent);
  border-radius: var(--radius-md);
  outline: none;
  background: var(--card);
  color: var(--foreground);
  font: inherit;
  font-size: var(--kosmos-text-body-size);
}

.everything-search::placeholder {
  color: var(--muted-foreground);
}

.everything-search:focus {
  border-color: var(--accent);
}

.everything-grid {
  --everything-gap: var(--space-4);
  --everything-column-width: 330px;

  display: grid;
  column-gap: var(--everything-gap);
}

.everything-column {
  min-width: 0;
}

.everything-add-card {
  position: relative;
  display: block;
  width: 100%;
  margin-block-end: var(--everything-gap);
  break-inside: avoid;
  padding: 0;
  border: 2px solid var(--card);
  border-radius: var(--radius-sm);
  background: var(--card);
  color: var(--muted-foreground);
  font-size: 2rem;
  line-height: 1;
  transition:
    background-color 300ms ease,
    border-color 300ms ease,
    color 300ms ease;
}

.everything-add-card::before {
  display: block;
  padding-block-start: 100%;
  content: "";
}

.everything-add-card > span {
  position: absolute;
  display: grid;
  place-items: center;
  inset: 0;
}

.everything-add-card:hover {
  border-color: color-mix(in srgb, var(--foreground) 18%, var(--border));
  background: var(--surface);
  color: var(--foreground);
}

.everything-add-card:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
}
</style>
