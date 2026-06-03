<script setup lang="ts">
import { BookMarked, LibraryBig, Plus } from "@lucide/vue";
import { computed } from "vue";
import BookCard from "./BookCard.vue";
import type { BookRecord } from "../lib/bookStorage";
import type { LibrarySection } from "../lib/libraryView";

const props = defineProps<{
  activeSection: LibrarySection;
  books: BookRecord[];
  continueBook: BookRecord | null;
  loading: boolean;
  searchQuery: string;
}>();

const emit = defineEmits<{
  importRequest: [];
  openBook: [book: BookRecord];
}>();

const normalizedQuery = computed(() => props.searchQuery.trim().toLowerCase());
const searchedBooks = computed(() => {
  if (!normalizedQuery.value) return props.books;
  return props.books.filter((book) => {
    const haystack = [book.title, ...book.authors, book.originalFileName].join(" ").toLowerCase();
    return haystack.includes(normalizedQuery.value);
  });
});
const visibleBooks = computed(() => {
  if (props.activeSection === "reading") {
    return searchedBooks.value.filter((book) => (book.progress?.percentage ?? 0) > 0.001);
  }
  if (props.activeSection === "finished") {
    return searchedBooks.value.filter((book) => (book.progress?.percentage ?? 0) >= 0.98);
  }
  return searchedBooks.value;
});
const continueBooks = computed(() => {
  const first = props.continueBook ? [props.continueBook] : [];
  const rest = props.books.filter((book) => book.id !== props.continueBook?.id);
  return [...first, ...rest].slice(0, 3);
});
const shelfTitle = computed(() => {
  if (props.activeSection === "reading") return "Читаю";
  if (props.activeSection === "finished") return "Завершённые";
  if (normalizedQuery.value) return "Найдено";
  return "Библиотека";
});
</script>

<template>
  <main class="library-view kosmos-scroll">
    <section class="library-view__hero">
      <h1 class="library-view__home-title">Главная</h1>
      <div class="library-view__today">
        <BookMarked :size="14" aria-hidden="true" />
        <span>Сегодня</span>
        <span>{{ continueBook ? "можно продолжить чтение" : "добавьте первую книгу" }}</span>
      </div>
    </section>

    <section v-if="books.length === 0" class="library-view__empty">
      <LibraryBig :size="42" aria-hidden="true" />
      <h1 class="library-view__empty-title">Добавьте первую книгу</h1>
      <p class="library-view__empty-copy">
        EPUB сохранится в локальной библиотеке Akasha, и его можно будет открыть снова с того же
        места.
      </p>
      <button
        class="library-view__primary"
        type="button"
        :disabled="loading"
        @click="emit('importRequest')"
      >
        <Plus :size="16" aria-hidden="true" />
        <span>{{ loading ? "Добавляю" : "Добавить EPUB" }}</span>
      </button>
    </section>

    <section
      v-if="continueBooks.length > 0"
      class="library-view__continue"
      aria-label="Продолжить чтение"
    >
      <h2 class="library-view__heading">Продолжить</h2>
      <div class="library-view__continue-row kosmos-scroll">
        <BookCard
          v-for="book in continueBooks"
          :key="book.id"
          :book="book"
          variant="compact"
          @click="emit('openBook', book)"
        />
      </div>
    </section>

    <section v-if="books.length > 0" class="library-view__shelf" aria-label="Библиотека">
      <div class="library-view__section-head">
        <h2 class="library-view__heading">{{ shelfTitle }}</h2>
        <button
          class="library-view__secondary"
          type="button"
          :disabled="loading"
          @click="emit('importRequest')"
        >
          <Plus :size="16" aria-hidden="true" />
          <span>{{ loading ? "Добавляю" : "Добавить" }}</span>
        </button>
      </div>

      <div v-if="visibleBooks.length > 0" class="library-view__grid">
        <BookCard
          v-for="book in visibleBooks"
          :key="book.id"
          :book="book"
          variant="tile"
          @click="emit('openBook', book)"
        />
      </div>
      <div v-else class="library-view__no-results">Ничего не найдено</div>
    </section>
  </main>
</template>

<style scoped>
.library-view {
  flex: 1;
  min-width: 0;
  min-height: 0;
  overflow-x: hidden;
  overflow-y: auto;
  padding: 64px 32px 56px;
  background: var(--background);
}

.library-view__hero,
.library-view__continue,
.library-view__shelf {
  width: min(100%, 1120px);
  margin: 0 auto;
}

.library-view__hero {
  padding-bottom: 10px;
}

.library-view__home-title {
  margin: 0;
  font-family: "Source Serif 4", serif;
  font-size: 34px;
  font-weight: 700;
  letter-spacing: 0;
  line-height: 1.05;
}

.library-view__today {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-top: 8px;
  padding-bottom: 14px;
  border-bottom: 1px solid var(--border);
  color: var(--muted-foreground);
  font-size: 12px;
}

.library-view__today span:first-of-type {
  color: var(--accent);
  font-weight: 700;
}

.library-view__section-head {
  display: flex;
  align-items: flex-end;
  justify-content: space-between;
  gap: 18px;
  margin-bottom: 18px;
}

.library-view__heading,
.library-view__empty-title,
.library-view__empty-copy {
  margin: 0;
}

.library-view__continue {
  padding: 8px 0 26px;
}

.library-view__heading {
  font-size: 22px;
  font-weight: 720;
  letter-spacing: 0;
}

.library-view__continue-row {
  display: flex;
  gap: 18px;
  margin-top: 12px;
  overflow-x: auto;
  overflow-y: hidden;
  padding: 2px 2px 10px;
}

.library-view__primary,
.library-view__secondary {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 34px;
  padding: 0 13px;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  cursor: pointer;
  white-space: nowrap;
}

.library-view__primary {
  background: var(--primary);
  color: var(--primary-foreground);
}

.library-view__secondary {
  background: var(--card);
  color: var(--foreground);
}

.library-view__primary:hover:not(:disabled),
.library-view__secondary:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--accent) 60%, var(--border));
}

.library-view__primary:disabled,
.library-view__secondary:disabled {
  opacity: 0.65;
  cursor: wait;
}

.library-view__grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
  gap: 28px 26px;
}

.library-view__empty {
  display: flex;
  align-items: center;
  flex-direction: column;
  justify-content: center;
  min-height: 420px;
  color: var(--muted-foreground);
  text-align: center;
}

.library-view__empty-title {
  margin-top: 16px;
  color: var(--foreground);
  font-size: 24px;
  font-weight: 720;
  letter-spacing: 0;
}

.library-view__empty-copy {
  max-width: 430px;
  margin: 8px 0 20px;
  line-height: 1.5;
}

.library-view__no-results {
  display: flex;
  align-items: center;
  min-height: 180px;
  color: var(--muted-foreground);
  font-size: 14px;
}

@media (max-width: 720px) {
  .library-view {
    padding: 54px 20px 44px;
  }

  .library-view__section-head {
    align-items: flex-start;
    flex-direction: column;
  }

  .library-view__home-title {
    font-size: 30px;
  }

  .library-view__grid {
    grid-template-columns: 1fr;
  }
}
</style>
