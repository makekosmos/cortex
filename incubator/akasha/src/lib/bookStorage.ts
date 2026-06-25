export interface ReadingProgress {
  chapterId: string;
  blockId: string;
  percentage: number;
  updatedAt: string;
}

export interface BookRecord {
  id: string;
  title: string;
  authors: string[];
  importedAt: string;
  updatedAt: string;
  fileName: string;
  originalFileName: string;
  sizeBytes: number;
  coverDataUrl: string | null;
  lastOpenedAt: string | null;
  progress: ReadingProgress | null;
}

export interface LibraryCatalog {
  version: 1;
  books: BookRecord[];
}

export const LIBRARY_FILE = "library.json";

export function createEmptyCatalog(): LibraryCatalog {
  return { version: 1, books: [] };
}

export async function readFileBytes(file: File): Promise<Uint8Array> {
  return new Uint8Array(await file.arrayBuffer());
}

export async function hashBytes(bytes: Uint8Array): Promise<string> {
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

export function bytesToBase64(bytes: Uint8Array): string {
  let binary = "";
  const chunkSize = 0x8000;
  for (let index = 0; index < bytes.length; index += chunkSize) {
    binary += String.fromCharCode(...bytes.subarray(index, index + chunkSize));
  }
  return btoa(binary);
}

export function base64ToBytes(base64: string): Uint8Array {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let index = 0; index < binary.length; index += 1) {
    bytes[index] = binary.charCodeAt(index);
  }
  return bytes;
}

export function normalizeCatalog(value: Partial<LibraryCatalog> | null): LibraryCatalog {
  if (!value || !Array.isArray(value.books)) return createEmptyCatalog();
  return {
    version: 1,
    books: value.books
      .map(normalizeBookRecord)
      .filter((book): book is BookRecord => book !== null)
      .sort(sortByLastOpened),
  };
}

function normalizeBookRecord(value: unknown): BookRecord | null {
  if (!value || typeof value !== "object") return null;
  const record = value as Partial<BookRecord>;
  if (typeof record.id !== "string" || typeof record.title !== "string") return null;
  if (typeof record.fileName !== "string" || !record.fileName) return null;

  return {
    id: record.id,
    title: record.title,
    authors: Array.isArray(record.authors)
      ? record.authors.filter((author): author is string => typeof author === "string")
      : [],
    importedAt:
      typeof record.importedAt === "string" ? record.importedAt : new Date().toISOString(),
    updatedAt: typeof record.updatedAt === "string" ? record.updatedAt : new Date().toISOString(),
    fileName: record.fileName,
    originalFileName:
      typeof record.originalFileName === "string"
        ? record.originalFileName
        : `${record.title}.epub`,
    sizeBytes: typeof record.sizeBytes === "number" ? record.sizeBytes : 0,
    coverDataUrl: typeof record.coverDataUrl === "string" ? record.coverDataUrl : null,
    lastOpenedAt: typeof record.lastOpenedAt === "string" ? record.lastOpenedAt : null,
    progress: normalizeProgress(record.progress),
  };
}

function normalizeProgress(value: unknown): ReadingProgress | null {
  if (!value || typeof value !== "object") return null;
  const progress = value as Partial<ReadingProgress>;
  if (typeof progress.chapterId !== "string" || typeof progress.blockId !== "string") return null;
  return {
    chapterId: progress.chapterId,
    blockId: progress.blockId,
    percentage:
      typeof progress.percentage === "number" ? Math.min(1, Math.max(0, progress.percentage)) : 0,
    updatedAt:
      typeof progress.updatedAt === "string" ? progress.updatedAt : new Date().toISOString(),
  };
}

function sortByLastOpened(left: BookRecord, right: BookRecord): number {
  const leftDate = left.lastOpenedAt ?? left.importedAt;
  const rightDate = right.lastOpenedAt ?? right.importedAt;
  return rightDate.localeCompare(leftDate);
}
