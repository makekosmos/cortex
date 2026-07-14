<script setup lang="ts">
import { computed, onBeforeUnmount, shallowRef, watch } from "vue";
import { getEntryDisplayTitle } from "@/lib/entryTitles";
import { SYSTEM_TYPE_BOOK_ID, SYSTEM_TYPE_NOTE_ID } from "@/lib/systemTypes";
import EverythingItemCard from "./EverythingItemCard.vue";

const props = defineProps<{
  entries: Entry[];
  noteTypes: NoteType[];
}>();

const emit = defineEmits<{
  openEntry: [entryId: string];
}>();

const supportedTypeIds = new Set([SYSTEM_TYPE_NOTE_ID, SYSTEM_TYPE_BOOK_ID]);
const searchQuery = shallowRef("");
const normalizedSearchQuery = computed(() => searchQuery.value.trim().toLocaleLowerCase("ru-RU"));
const isFiltering = computed(() => normalizedSearchQuery.value.length > 0);
const fullTextMatchIds = shallowRef<ReadonlySet<string>>(new Set());
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

      <div v-if="visibleEntries.length" class="everything-grid" data-testid="everything-grid">
        <EverythingItemCard
          v-for="entry in visibleEntries"
          :key="entry.id"
          :entry="entry"
          :note-type="noteTypesById.get(entry.type_id ?? '') ?? null"
          :entries-by-id="entriesById"
          @open-entry="emit('openEntry', $event)"
        />
      </div>

      <section
        v-else-if="!isFiltering"
        class="everything-empty"
        data-testid="everything-empty"
        aria-live="polite"
      >
        <h2 class="everything-empty-title">Здесь пока ничего нет</h2>
        <p class="everything-empty-description">
          Создай заметку или книгу — она появится на странице «Всё».
        </p>
      </section>
    </div>
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

  column-width: 330px;
  column-gap: var(--everything-gap);
}

.everything-empty {
  display: flex;
  min-height: 16rem;
  align-items: center;
  justify-content: center;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-8);
  border: 1px dashed var(--border);
  border-radius: var(--radius-xl);
  background: var(--card);
  text-align: center;
}

.everything-empty-title {
  color: var(--foreground);
  font-size: var(--kosmos-text-subheading-size);
  font-weight: var(--kosmos-text-subheading-weight);
  line-height: var(--kosmos-text-subheading-line-height);
}

.everything-empty-description {
  max-width: 28rem;
  color: var(--muted-foreground);
  font-size: var(--kosmos-text-body-size);
  line-height: 1.5;
}

@container (max-width: 34rem) {
  .everything-grid {
    column-width: auto;
    column-count: 1;
  }
}
</style>
