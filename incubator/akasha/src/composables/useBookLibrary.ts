import { computed, onMounted, readonly, shallowRef } from "vue";
import { readEpubBytes } from "../lib/epub";
import {
  base64ToBytes,
  bytesToBase64,
  createEmptyCatalog,
  hashBytes,
  LIBRARY_FILE,
  normalizeCatalog,
  readFileBytes,
  type BookRecord,
  type LibraryCatalog,
  type ReadingProgress,
} from "../lib/bookStorage";

const FALLBACK_LIBRARY_KEY = "akasha:library";

export function useBookLibrary() {
  const catalog = shallowRef<LibraryCatalog>(createEmptyCatalog());
  const loading = shallowRef(false);
  const error = shallowRef<string | null>(null);

  const books = computed(() => catalog.value.books);
  const continueBook = computed(
    () => books.value.find((book) => book.lastOpenedAt) ?? books.value[0] ?? null,
  );
  const hasBooks = computed(() => books.value.length > 0);

  onMounted(() => {
    void loadLibrary();
  });

  async function loadLibrary() {
    loading.value = true;
    error.value = null;
    try {
      catalog.value = await readCatalog();
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось загрузить библиотеку.";
    } finally {
      loading.value = false;
    }
  }

  async function importBook(file: File): Promise<{ record: BookRecord; bytes: Uint8Array }> {
    loading.value = true;
    error.value = null;
    try {
      const bytes = await readFileBytes(file);
      const id = await hashBytes(bytes);
      const fileName = `books/${id}.epub`;
      const parsed = await readEpubBytes(bytes, file.name);
      const now = new Date().toISOString();
      const existing = catalog.value.books.find((book) => book.id === id);
      const record: BookRecord = {
        id,
        title: parsed.title,
        authors: parsed.authors,
        importedAt: existing?.importedAt ?? now,
        updatedAt: now,
        fileName,
        originalFileName: file.name,
        sizeBytes: bytes.length,
        coverDataUrl: parsed.coverDataUrl,
        lastOpenedAt: now,
        progress: existing?.progress ?? null,
      };

      await window.kepler?.userData?.writeBinary(fileName, bytesToBase64(bytes));
      await upsertBook(record);
      return { record, bytes };
    } catch (cause) {
      error.value = cause instanceof Error ? cause.message : "Не удалось импортировать книгу.";
      throw cause;
    } finally {
      loading.value = false;
    }
  }

  async function readBookBytes(book: BookRecord): Promise<Uint8Array> {
    const base64 = await window.kepler?.userData?.readBinary(book.fileName);
    if (!base64) throw new Error("Файл книги не найден в локальной библиотеке.");
    return base64ToBytes(base64);
  }

  async function markOpened(bookId: string) {
    const now = new Date().toISOString();
    await patchBook(bookId, { lastOpenedAt: now, updatedAt: now });
  }

  async function updateProgress(bookId: string, progress: ReadingProgress) {
    await patchBook(bookId, {
      progress,
      updatedAt: progress.updatedAt,
      lastOpenedAt: progress.updatedAt,
    });
  }

  async function upsertBook(record: BookRecord) {
    const nextBooks = [record, ...catalog.value.books.filter((book) => book.id !== record.id)];
    catalog.value = { version: 1, books: nextBooks };
    await saveCatalog(catalog.value);
  }

  async function patchBook(bookId: string, patch: Partial<BookRecord>) {
    const nextBooks = catalog.value.books
      .map((book) => (book.id === bookId ? { ...book, ...patch } : book))
      .sort((left, right) =>
        (right.lastOpenedAt ?? right.importedAt).localeCompare(
          left.lastOpenedAt ?? left.importedAt,
        ),
      );
    catalog.value = { version: 1, books: nextBooks };
    await saveCatalog(catalog.value);
  }

  return {
    books,
    catalog: readonly(catalog),
    continueBook,
    error: readonly(error),
    hasBooks,
    loading: readonly(loading),
    importBook,
    loadLibrary,
    markOpened,
    readBookBytes,
    updateProgress,
  };
}

async function readCatalog(): Promise<LibraryCatalog> {
  const stored = await window.kepler?.userData?.readJson<Partial<LibraryCatalog>>(LIBRARY_FILE);
  if (stored) return normalizeCatalog(stored);

  const fallback = localStorage.getItem(FALLBACK_LIBRARY_KEY);
  if (!fallback) return createEmptyCatalog();
  try {
    return normalizeCatalog(JSON.parse(fallback) as Partial<LibraryCatalog>);
  } catch {
    return createEmptyCatalog();
  }
}

async function saveCatalog(catalog: LibraryCatalog) {
  localStorage.setItem(FALLBACK_LIBRARY_KEY, JSON.stringify(catalog));
  await window.kepler?.userData?.writeJson(LIBRARY_FILE, catalog);
}
