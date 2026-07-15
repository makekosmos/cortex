import { describe, expect, test } from "bun:test";
import { lookupOpenLibraryIsbn } from "./book-metadata-open-library";

function json(value: unknown, status = 200): Response {
  return new Response(JSON.stringify(value), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("Open Library book metadata", () => {
  test("maps edition data and resolves author names on the fixed origin", async () => {
    const urls: string[] = [];
    const redirects: Array<RequestRedirect | undefined> = [];
    const fetcher: typeof fetch = async (input, init) => {
      const url = String(input);
      urls.push(url);
      redirects.push(init?.redirect);
      if (url.endsWith("/isbn/9780140328721.json")) {
        return new Response(null, {
          status: 302,
          headers: { location: "/books/OL7353617M.json" },
        });
      }
      if (url.endsWith("/authors/OL34184A.json")) return json({ name: "Roald Dahl" });
      return json({
        title: "Fantastic Mr. Fox",
        authors: [{ key: "/authors/OL34184A" }],
        publishers: ["Puffin"],
        publish_date: "October 1, 1988",
        number_of_pages: 96,
        languages: [{ key: "/languages/eng" }],
        covers: [15152634],
      });
    };

    await expect(lookupOpenLibraryIsbn("9780140328721", { fetcher })).resolves.toEqual({
      title: "Fantastic Mr. Fox",
      author: "Roald Dahl",
      cover_image: "https://covers.openlibrary.org/b/id/15152634-L.jpg?default=false",
      isbn: "9780140328721",
      page_count: 96,
      language: "eng",
      publisher: "Puffin",
      published_date: "October 1, 1988",
    });
    expect(urls).toEqual([
      "https://openlibrary.org/isbn/9780140328721.json",
      "https://openlibrary.org/books/OL7353617M.json",
      "https://openlibrary.org/authors/OL34184A.json",
    ]);
    expect(redirects).toEqual(["manual", "manual", "manual"]);
  });

  test("rejects redirects outside the fixed Open Library origin", async () => {
    const fetcher: typeof fetch = async () =>
      new Response(null, {
        status: 302,
        headers: { location: "https://example.com/book.json" },
      });

    await expect(lookupOpenLibraryIsbn("9780140328721", { fetcher })).rejects.toThrow(
      "внешний адрес",
    );
  });

  test("returns null for a missing edition and rejects invalid ISBN", async () => {
    const fetcher: typeof fetch = async () => json({}, 404);
    await expect(lookupOpenLibraryIsbn("9780140328721", { fetcher })).resolves.toBeNull();
    await expect(lookupOpenLibraryIsbn("not-an-isbn", { fetcher })).rejects.toThrow(
      "Некорректный ISBN",
    );
  });
});
