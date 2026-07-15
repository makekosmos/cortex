import { describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import BookMetadataImportModal from "../../src/components/books/BookMetadataImportModal.vue";
import { extractBookMetadata, normalizeIsbn } from "../../src/lib/bookMetadata";
import { LIVELIB_LIKE_BOOK_PAGE, SCHEMA_ORG_BOOK_PAGE } from "../fixtures/bookMetadataPages";

describe("book metadata extraction", () => {
  test("validates ISBN checksums and normalizes ISBN-10 to ISBN-13", () => {
    expect(normalizeIsbn("0-306-40615-2")).toBe("9780306406157");
    expect(normalizeIsbn("978-0-306-40615-8")).toBe("");
  });

  test("extracts a LiveLib-like page without a domain-specific adapter", () => {
    expect(extractBookMetadata(LIVELIB_LIKE_BOOK_PAGE)).toEqual({
      title: "Марафон в рай",
      author: "Артур Кларк",
      cover_image: "https://www.livelib.ru/storage/covers/marafon-v-raj.jpg",
      isbn: "9780306406157",
      page_count: 352,
      language: "Русский",
      publisher: "АСТ",
      published_date: "2024",
      source_url: LIVELIB_LIKE_BOOK_PAGE.finalUrl,
    });
  });

  test("prefers Schema.org Book metadata", () => {
    expect(extractBookMetadata(SCHEMA_ORG_BOOK_PAGE)).toEqual({
      title: "Solaris",
      author: "Stanisław Lem",
      cover_image: "https://books.example/covers/solaris.jpg",
      isbn: "9780306406157",
      page_count: 224,
      language: "Польский",
      publisher: "Wydawnictwo Literackie",
      published_date: "1961",
      source_url: SCHEMA_ORG_BOOK_PAGE.finalUrl,
    });
  });
});

describe("BookMetadataImportModal", () => {
  test("explains why an invalid source URL cannot be loaded", async () => {
    const screen = render(BookMetadataImportModal, {
      props: { open: true, currentTitle: "", currentHeaderProps: {} },
    });

    await userEvent.fill(screen.getByRole("textbox", { name: "Ссылка или ISBN" }), "http://local");
    await expect
      .element(screen.getByRole("alert"))
      .toHaveTextContent("HTTPS-ссылку или корректный ISBN");
    await expect.element(screen.getByRole("button", { name: "Найти" })).toBeDisabled();
    screen.unmount();
  });

  test("looks up an already saved ISBN without fetching a source page", async () => {
    const fetchPage = vi.fn();
    const lookupIsbn = vi.fn(async () => ({
      title: "Fantastic Mr. Fox",
      isbn: "9780140328721",
      publisher: "Puffin",
    }));
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      bookMetadata: { fetchPage, lookupIsbn },
    } as NonNullable<typeof window.kepler>;

    const screen = render(BookMetadataImportModal, {
      props: {
        open: true,
        currentTitle: "",
        currentHeaderProps: { isbn: "978-0-14-032872-1" },
      },
    });

    try {
      await expect
        .element(screen.getByRole("textbox", { name: "Ссылка или ISBN" }))
        .toHaveValue("9780140328721");
      await userEvent.click(screen.getByRole("button", { name: "Найти" }));
      await expect.element(screen.getByTestId("book-metadata-preview")).toBeVisible();
      expect(lookupIsbn).toHaveBeenCalledWith("9780140328721");
      expect(fetchPage).not.toHaveBeenCalled();
    } finally {
      screen.unmount();
      window.kepler = previousKepler;
    }
  });

  test("accepts a pasted ISBN", async () => {
    const lookupIsbn = vi.fn(async () => ({ title: "Matilda", isbn: "9780140328721" }));
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      bookMetadata: { fetchPage: vi.fn(), lookupIsbn },
    } as NonNullable<typeof window.kepler>;
    const screen = render(BookMetadataImportModal, {
      props: { open: true, currentTitle: "", currentHeaderProps: {} },
    });

    try {
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка или ISBN" }),
        "978-0-14-032872-1",
      );
      await userEvent.click(screen.getByRole("button", { name: "Найти" }));
      await expect.element(screen.getByTestId("book-metadata-preview")).toBeVisible();
      expect(lookupIsbn).toHaveBeenCalledWith("9780140328721");
    } finally {
      screen.unmount();
      window.kepler = previousKepler;
    }
  });

  test("selects overwrite fields by default and omits unchecked metadata", async () => {
    let resolvePage!: (page: typeof LIVELIB_LIKE_BOOK_PAGE) => void;
    const pageWithoutCover = {
      ...LIVELIB_LIKE_BOOK_PAGE,
      html: LIVELIB_LIKE_BOOK_PAGE.html.replace(/\s*<meta property="og:image"[^>]+>/, ""),
    };
    const fetchPage = vi.fn(
      () =>
        new Promise<typeof LIVELIB_LIKE_BOOK_PAGE>((resolve) => {
          resolvePage = resolve;
        }),
    );
    const onApply = vi.fn();
    const onClose = vi.fn();
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      bookMetadata: { fetchPage },
    } as NonNullable<typeof window.kepler>;

    const screen = render(BookMetadataImportModal, {
      props: {
        open: true,
        currentTitle: "Моё название",
        currentHeaderProps: { author: "Мой автор" },
        onApply,
        onClose,
      },
    });

    try {
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка или ISBN" }),
        LIVELIB_LIKE_BOOK_PAGE.finalUrl,
      );
      const findButton = screen.getByRole("button", { name: "Найти" });
      await userEvent.click(findButton);

      await expect.element(findButton).toBeDisabled();
      expect(fetchPage).toHaveBeenCalledWith(LIVELIB_LIKE_BOOK_PAGE.finalUrl);
      resolvePage(pageWithoutCover);

      await expect.element(screen.getByTestId("book-metadata-preview")).toBeVisible();
      const overwriteRows = [
        ...document.querySelectorAll(".book-metadata-import__row.is-overwrite"),
      ].map((row) => row.textContent?.replace(/\s+/g, " ").trim() ?? "");
      expect(overwriteRows).toHaveLength(2);
      expect(overwriteRows.some((row) => row.includes("Название"))).toBe(true);
      expect(overwriteRows.some((row) => row.includes("Автор"))).toBe(true);
      await userEvent.click(screen.getByRole("checkbox", { name: "Применить поле «Автор»" }));

      await userEvent.click(screen.getByRole("button", { name: "Применить" }));
      const expected = extractBookMetadata(pageWithoutCover);
      delete expected.author;
      expect(onApply).toHaveBeenCalledWith(expected);
      expect(onClose).toHaveBeenCalledOnce();
    } finally {
      screen.unmount();
      window.kepler = previousKepler;
    }
  });

  test("fills missing fields from Open Library while keeping source-page values", async () => {
    const sourcePage = {
      finalUrl: "https://books.example/matilda",
      html: "<main><h1>Локальное название</h1><dl><dt>ISBN</dt><dd>9780140328721</dd></dl></main>",
    };
    const fetchPage = vi.fn(async () => sourcePage);
    const lookupIsbn = vi.fn(async () => ({
      title: "Fantastic Mr. Fox",
      author: "Roald Dahl",
      cover_image: "https://covers.openlibrary.org/b/id/15152634-L.jpg?default=false",
      isbn: "9780140328721",
      page_count: 96,
      language: "eng",
      publisher: "Puffin",
      published_date: "October 1, 1988",
    }));
    const onApply = vi.fn();
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      bookMetadata: { fetchPage, lookupIsbn },
    } as NonNullable<typeof window.kepler>;

    const screen = render(BookMetadataImportModal, {
      props: { open: true, currentTitle: "", currentHeaderProps: {}, onApply },
    });

    try {
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка или ISBN" }),
        sourcePage.finalUrl,
      );
      await userEvent.click(screen.getByRole("button", { name: "Найти" }));
      await expect.element(screen.getByTestId("book-metadata-preview")).toBeVisible();
      expect(lookupIsbn).toHaveBeenCalledWith("9780140328721");
      await userEvent.click(screen.getByRole("button", { name: "Применить" }));
      expect(onApply).toHaveBeenCalledWith({
        title: "Локальное название",
        author: "Roald Dahl",
        cover_image: "https://covers.openlibrary.org/b/id/15152634-L.jpg?default=false",
        isbn: "9780140328721",
        page_count: 96,
        language: "Английский",
        publisher: "Puffin",
        published_date: "October 1, 1988",
        source_url: sourcePage.finalUrl,
      });
    } finally {
      screen.unmount();
      window.kepler = previousKepler;
    }
  });

  test("keeps extracted metadata when Open Library is unavailable", async () => {
    const fetchPage = vi.fn(async () => LIVELIB_LIKE_BOOK_PAGE);
    const lookupIsbn = vi.fn(async () => {
      throw new Error("offline");
    });
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      bookMetadata: { fetchPage, lookupIsbn },
    } as NonNullable<typeof window.kepler>;

    const screen = render(BookMetadataImportModal, {
      props: { open: true, currentTitle: "", currentHeaderProps: {} },
    });

    try {
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка или ISBN" }),
        LIVELIB_LIKE_BOOK_PAGE.finalUrl,
      );
      await userEvent.click(screen.getByRole("button", { name: "Найти" }));
      await expect.element(screen.getByTestId("book-metadata-preview")).toBeVisible();
      await expect.element(screen.getByText("АСТ", { exact: true })).toBeVisible();
    } finally {
      screen.unmount();
      window.kepler = previousKepler;
    }
  });

  test("shows a transport error without applying anything", async () => {
    const fetchPage = vi.fn(async () => {
      throw new Error("offline");
    });
    const onApply = vi.fn();
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      bookMetadata: { fetchPage },
    } as NonNullable<typeof window.kepler>;

    const screen = render(BookMetadataImportModal, {
      props: {
        open: true,
        currentTitle: "",
        currentHeaderProps: {},
        onApply,
      },
    });

    try {
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка или ISBN" }),
        LIVELIB_LIKE_BOOK_PAGE.finalUrl,
      );
      await userEvent.click(screen.getByRole("button", { name: "Найти" }));

      await expect
        .element(screen.getByRole("alert"))
        .toHaveTextContent("Не удалось загрузить страницу");
      await expect.element(screen.getByRole("button", { name: "Применить" })).toBeDisabled();
      expect(onApply).not.toHaveBeenCalled();
    } finally {
      screen.unmount();
      window.kepler = previousKepler;
    }
  });
});
