import type { BookRecord } from "./bookStorage";

export type LibrarySection = "home" | "all" | "reading" | "finished";

export function countBooksInSection(books: BookRecord[], section: LibrarySection): number {
  if (section === "reading") {
    return books.filter((book) => (book.progress?.percentage ?? 0) > 0.001).length;
  }
  if (section === "finished") {
    return books.filter((book) => (book.progress?.percentage ?? 0) >= 0.98).length;
  }
  return books.length;
}
