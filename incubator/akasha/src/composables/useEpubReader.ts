import { computed, onMounted, readonly, shallowRef, watch } from "vue";
import { readEpubBytes, type EpubBook } from "../lib/epub";
import type { ReadingProgress } from "../lib/bookStorage";

export interface ReaderSettings {
  fontFamily: string;
  fontSize: number;
  selectedBookId: string | null;
}

const STATE_FILE = "reader-state.json";
const DEFAULT_SETTINGS: ReaderSettings = {
  fontFamily: "Source Serif 4",
  fontSize: 18,
  selectedBookId: null,
};

export function useEpubReader() {
  const book = shallowRef<EpubBook | null>(null);
  const activeChapterId = shallowRef<string | null>(null);
  const activeBlockId = shallowRef<string | null>(null);
  const settings = shallowRef<ReaderSettings>({ ...DEFAULT_SETTINGS });
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);

  const hasBook = computed(() => book.value !== null);
  const activeChapterTitle = computed(() => {
    const currentBook = book.value;
    if (!currentBook || !activeChapterId.value) return null;
    return (
      currentBook.chapters.find((chapter) => chapter.id === activeChapterId.value)?.title ?? null
    );
  });

  onMounted(async () => {
    settings.value = await loadSettings();
  });

  watch(
    settings,
    (value) => {
      void saveSettings(value);
    },
    { deep: true },
  );

  async function openBytes(
    bytes: Uint8Array,
    fallbackName: string,
    bookId: string,
    progress?: ReadingProgress | null,
  ) {
    loading.value = true;
    error.value = null;
    try {
      const nextBook = await readEpubBytes(bytes, fallbackName);
      const initialBlock = progress
        ? (nextBook.blocks.find((block) => block.id === progress.blockId) ?? nextBook.blocks[0])
        : nextBook.blocks[0];
      book.value = nextBook;
      activeChapterId.value = initialBlock?.chapterId ?? nextBook.chapters[0]?.id ?? null;
      activeBlockId.value = initialBlock?.id ?? null;
      settings.value = { ...settings.value, selectedBookId: bookId };
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось открыть EPUB.";
    } finally {
      loading.value = false;
    }
  }

  function setFontFamily(fontFamily: string) {
    settings.value = { ...settings.value, fontFamily };
  }

  function setFontSize(fontSize: number) {
    settings.value = { ...settings.value, fontSize: Math.min(28, Math.max(14, fontSize)) };
  }

  function setActiveChapter(chapterId: string) {
    activeChapterId.value = chapterId;
    const chapterBlock = book.value?.blocks.find((block) => block.chapterId === chapterId);
    activeBlockId.value = chapterBlock?.id ?? activeBlockId.value;
  }

  function setActiveLocation(location: { chapterId: string; blockId: string }) {
    activeChapterId.value = location.chapterId;
    activeBlockId.value = location.blockId;
  }

  return {
    book: readonly(book),
    activeBlockId: readonly(activeBlockId),
    activeChapterId: readonly(activeChapterId),
    activeChapterTitle,
    error: readonly(error),
    hasBook,
    loading: readonly(loading),
    settings: readonly(settings),
    openBytes,
    setActiveLocation,
    setActiveChapter,
    setFontFamily,
    setFontSize,
  };
}

async function loadSettings(): Promise<ReaderSettings> {
  const stored = await window.kepler?.userData?.readJson<unknown>(STATE_FILE);
  if (!stored) {
    const fallback = localStorage.getItem("akasha:reader-state");
    if (!fallback) return { ...DEFAULT_SETTINGS };
    try {
      return normalizeSettings(JSON.parse(fallback));
    } catch {
      return { ...DEFAULT_SETTINGS };
    }
  }
  return normalizeSettings(stored);
}

async function saveSettings(settings: ReaderSettings) {
  localStorage.setItem("akasha:reader-state", JSON.stringify(settings));
  await window.kepler?.userData?.writeJson(STATE_FILE, settings);
}

function normalizeSettings(value: unknown): ReaderSettings {
  const settings = value && typeof value === "object" ? (value as Partial<ReaderSettings>) : {};
  return {
    fontFamily:
      typeof settings.fontFamily === "string" ? settings.fontFamily : DEFAULT_SETTINGS.fontFamily,
    fontSize:
      typeof settings.fontSize === "number"
        ? Math.min(28, Math.max(14, settings.fontSize))
        : DEFAULT_SETTINGS.fontSize,
    selectedBookId: typeof settings.selectedBookId === "string" ? settings.selectedBookId : null,
  };
}
