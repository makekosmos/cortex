<script setup lang="ts">
import { ArrowLeft } from "@lucide/vue";
import { DesktopChrome } from "@kosmos/visuals";
import { computed, onBeforeUnmount, shallowRef, useTemplateRef } from "vue";
import BookLibrarySidebar from "./BookLibrarySidebar.vue";
import BookLibraryView from "./BookLibraryView.vue";
import ReaderToolbar from "./ReaderToolbar.vue";
import ReaderView from "./ReaderView.vue";
import { useBookLibrary } from "../composables/useBookLibrary";
import { useEpubReader } from "../composables/useEpubReader";
import type { BookRecord, ReadingProgress } from "../lib/bookStorage";
import type { LibrarySection } from "../lib/libraryView";

type AkashaMode = "library" | "reader";

const mode = shallowRef<AkashaMode>("library");
const librarySection = shallowRef<LibrarySection>("home");
const librarySearchQuery = shallowRef("");
const activeBook = shallowRef<BookRecord | null>(null);
const fileInput = useTemplateRef<HTMLInputElement>("fileInput");
const progressTimer = shallowRef<ReturnType<typeof window.setTimeout> | null>(null);

const library = useBookLibrary();
const reader = useEpubReader();

const books = computed(() => library.books.value);
const continueBook = computed(() => library.continueBook.value);
const readerSettings = computed(() => reader.settings.value);
const readerBook = computed(() => reader.book.value);
const activeBlockId = computed(() => reader.activeBlockId.value);
const activeChapterId = computed(() => reader.activeChapterId.value);
const visibleError = computed(() => library.error.value ?? reader.error.value);
const isBusy = computed(() => library.loading.value || reader.loading.value);

function pickBookFile() {
  fileInput.value?.click();
}

async function onFileChange(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;

  const { record, bytes } = await library.importBook(file);
  activeBook.value = record;
  await reader.openBytes(bytes, record.originalFileName, record.id, record.progress);
  mode.value = "reader";
}

async function openBook(book: BookRecord) {
  const bytes = await library.readBookBytes(book);
  activeBook.value = book;
  await library.markOpened(book.id);
  await reader.openBytes(bytes, book.originalFileName, book.id, book.progress);
  mode.value = "reader";
}

function showLibrary() {
  mode.value = "library";
}

function onReaderLocation(location: { chapterId: string; blockId: string; percentage: number }) {
  reader.setActiveLocation(location);
  const book = activeBook.value;
  if (!book) return;

  if (progressTimer.value) window.clearTimeout(progressTimer.value);
  progressTimer.value = window.setTimeout(() => {
    const progress: ReadingProgress = {
      ...location,
      updatedAt: new Date().toISOString(),
    };
    void library.updateProgress(book.id, progress);
  }, 700);
}

onBeforeUnmount(() => {
  if (progressTimer.value) window.clearTimeout(progressTimer.value);
});
</script>

<template>
  <DesktopChrome
    class="akasha-shell"
    :class="{ 'akasha-shell--overlay-titlebar': mode === 'library' }"
  >
    <template #titlebar-leading>
      <div class="akasha-titlebar__leading">
        <button
          v-if="mode === 'reader'"
          class="akasha-titlebar__back"
          type="button"
          title="Библиотека"
          @click="showLibrary"
        >
          <ArrowLeft :size="16" aria-hidden="true" />
        </button>
      </div>
    </template>

    <template #titlebar-trailing>
      <ReaderToolbar
        v-if="mode === 'reader'"
        :font-family="readerSettings.fontFamily"
        :font-size="readerSettings.fontSize"
        @update-font-family="reader.setFontFamily"
        @update-font-size="reader.setFontSize"
      />
    </template>

    <template v-if="mode === 'library'" #sidebar>
      <BookLibrarySidebar
        :active-section="librarySection"
        :books="books"
        :loading="isBusy"
        :search-query="librarySearchQuery"
        @import-request="pickBookFile"
        @update:active-section="librarySection = $event"
        @update:search-query="librarySearchQuery = $event"
      />
    </template>

    <div class="akasha-app">
      <div v-if="visibleError" class="akasha-error" role="alert">
        {{ visibleError }}
      </div>

      <BookLibraryView
        v-if="mode === 'library'"
        :active-section="librarySection"
        :books="books"
        :continue-book="continueBook"
        :loading="isBusy"
        :search-query="librarySearchQuery"
        @import-request="pickBookFile"
        @open-book="openBook"
      />
      <ReaderView
        v-else
        :active-block-id="activeBlockId"
        :active-chapter-id="activeChapterId"
        :book="readerBook"
        :settings="readerSettings"
        @active-chapter-change="reader.setActiveChapter"
        @active-location-change="onReaderLocation"
        @jump="reader.setActiveChapter"
      />

      <input
        ref="fileInput"
        class="akasha-file-input"
        type="file"
        accept=".epub,application/epub+zip"
        @change="onFileChange"
      />
    </div>
  </DesktopChrome>
</template>

<style scoped>
.akasha-titlebar__leading {
  display: flex;
  align-items: center;
}

.akasha-titlebar__leading {
  min-width: 0;
  gap: 8px;
}

.akasha-titlebar__back {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border);
  border-radius: var(--radius-input);
  background: var(--background);
  color: var(--foreground);
  cursor: pointer;
  -webkit-app-region: no-drag;
}

.akasha-titlebar__back {
  width: 30px;
  height: 30px;
}

.akasha-titlebar__back:hover {
  background: var(--muted);
}

.akasha-app {
  display: flex;
  flex: 1;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
}

.akasha-file-input {
  display: none;
}
</style>
