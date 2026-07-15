import { describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import { nextTick } from "vue";
import EverythingView from "../../src/components/everything/EverythingView.vue";
import {
  SYSTEM_TYPE_BOOK,
  SYSTEM_TYPE_BOOK_ID,
  SYSTEM_TYPE_COLLECTION_ID,
  SYSTEM_TYPE_IMAGE_ID,
  SYSTEM_TYPE_JOURNAL_ID,
  SYSTEM_TYPE_NOTE,
  SYSTEM_TYPE_NOTE_ID,
} from "../../src/lib/systemTypes";

const COVER_DATA_URI =
  "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='120' height='180'%3E%3Crect width='120' height='180' fill='orange'/%3E%3C/svg%3E";

function entry(
  id: string,
  typeId: string,
  updatedAt: number,
  overrides: Partial<Entry> = {},
): Entry {
  return {
    id,
    title: id,
    content_json: JSON.stringify({ kind: "markdown", markdown: "" }),
    content_loaded: false,
    created_at: updatedAt - 1,
    updated_at: updatedAt,
    folder_id: null,
    type_id: typeId,
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
    ...overrides,
  };
}

function mixedEntries(): Entry[] {
  return [
    entry("note-old", SYSTEM_TYPE_NOTE_ID, 100, { title: "Старая заметка" }),
    entry("book-covered", SYSTEM_TYPE_BOOK_ID, 400, {
      title: "Книга с обложкой",
      header_props_json: JSON.stringify({ author: "Урсула Ле Гуин", cover_image: "cover" }),
    }),
    entry("book-fallback", SYSTEM_TYPE_BOOK_ID, 300, { title: "Книга без обложки" }),
    entry("task", "task_obj", 700, { title: "Скрытая задача" }),
    entry("journal", SYSTEM_TYPE_JOURNAL_ID, 600, { title: "Скрытый журнал" }),
    entry("collection", SYSTEM_TYPE_COLLECTION_ID, 500, { title: "Скрытая коллекция" }),
    entry("cover", SYSTEM_TYPE_IMAGE_ID, 800, {
      header_props_json: JSON.stringify({ image: COVER_DATA_URI }),
    }),
  ];
}

const noteTypes = [SYSTEM_TYPE_NOTE, SYSTEM_TYPE_BOOK];

function cardIds(): string[] {
  return [...document.querySelectorAll<HTMLElement>('[data-testid^="everything-card-"]')].map(
    (card) => card.dataset.testid?.replace("everything-card-", "") ?? "",
  );
}

function rectsOverlap(left: DOMRect, right: DOMRect): boolean {
  return (
    left.left < right.right - 0.5 &&
    left.right > right.left + 0.5 &&
    left.top < right.bottom - 0.5 &&
    left.bottom > right.top + 0.5
  );
}

describe("EverythingView", () => {
  test("shows only notes and books sorted by latest update", async () => {
    const screen = render(EverythingView, {
      props: { entries: mixedEntries(), noteTypes },
    });

    await expect.element(screen.getByTestId("everything-grid")).toBeInTheDocument();
    expect(cardIds()).toEqual(["book-covered", "book-fallback", "note-old"]);
    await expect
      .element(screen.getByTestId("everything-card-book-covered"))
      .not.toHaveTextContent("Урсула Ле Гуин");
    await expect.element(screen.getByTestId("everything-cover-book-covered")).toBeInTheDocument();
    await expect
      .element(screen.getByTestId("everything-cover-layer-book-covered"))
      .toBeInTheDocument();
    await expect
      .element(screen.getByTestId("everything-cover-fallback-book-fallback"))
      .toHaveTextContent("Без обложки");
    await expect
      .element(screen.getByTestId("everything-card-note-old"))
      .not.toHaveTextContent("Заметка");
    await expect.element(screen.getByTestId("everything-card-task")).not.toBeInTheDocument();
    await expect.element(screen.getByTestId("everything-card-journal")).not.toBeInTheDocument();
    await expect.element(screen.getByTestId("everything-card-collection")).not.toBeInTheDocument();
  });

  test("filters by title and book author", async () => {
    const screen = render(EverythingView, {
      props: { entries: mixedEntries(), noteTypes },
    });
    const search = screen.getByRole("searchbox", { name: "Поиск" });

    await userEvent.fill(search, "Урсула");
    expect(cardIds()).toEqual(["book-covered"]);

    await userEvent.fill(search, "Старая");
    expect(cardIds()).toEqual(["note-old"]);

    await userEvent.fill(search, "Нет совпадений");
    await expect.element(screen.getByTestId("everything-empty")).not.toBeInTheDocument();
    await expect.element(screen.getByTestId("everything-grid")).not.toBeInTheDocument();
  });

  test("adds full-text matches after the immediate local results", async () => {
    const originalApi = window.api;
    let resolveSearch!: (results: SearchResult[]) => void;
    const searchEntries = vi.fn(
      () => new Promise<SearchResult[]>((resolve) => (resolveSearch = resolve)),
    );
    window.api = { ...originalApi, searchEntries } as typeof window.api;

    try {
      const screen = render(EverythingView, {
        props: { entries: mixedEntries(), noteTypes },
      });
      const search = screen.getByRole("searchbox", { name: "Поиск" });
      const query = mixedEntries()[0]!.title.split(" ")[0]!;

      await userEvent.fill(search, query);
      expect(cardIds()).toEqual(["note-old"]);
      expect(searchEntries).not.toHaveBeenCalled();

      await vi.waitFor(() =>
        expect(searchEntries).toHaveBeenCalledWith(query.toLocaleLowerCase("ru-RU")),
      );
      resolveSearch([
        { entryId: "book-fallback", file: "", line: 1, text: "body match" },
        { entryId: "task", file: "", line: 1, text: "hidden type" },
      ]);
      await vi.waitFor(() => expect(cardIds()).toEqual(["book-fallback", "note-old"]));
    } finally {
      window.api = originalApi;
    }
  });

  test("loads and caches a visible note preview", async () => {
    const originalApi = window.api;
    const originalObserver = window.IntersectionObserver;
    let reveal: (() => void) | undefined;
    let observerOptions: IntersectionObserverInit | undefined;
    let previewUpdatedAt = 900;
    let previewText = `# Preview heading\n\nPreview body ${"word ".repeat(250)}`;
    const loadEntry = vi.fn(async () =>
      entry("note-preview", SYSTEM_TYPE_NOTE_ID, previewUpdatedAt, {
        title: "Заметка с телом",
        content_loaded: true,
        content_json: JSON.stringify({
          type: "markdown",
          version: 1,
          text: previewText,
        }),
      }),
    );
    window.api = { ...originalApi, loadEntry } as typeof window.api;
    window.IntersectionObserver = class {
      constructor(callback: IntersectionObserverCallback, options?: IntersectionObserverInit) {
        observerOptions = options;
        reveal = () => callback([{ isIntersecting: true } as IntersectionObserverEntry], this);
      }
      observe() {}
      disconnect() {}
      unobserve() {}
      takeRecords() {
        return [];
      }
      root = null;
      rootMargin = "0px";
      thresholds = [0];
    } as typeof IntersectionObserver;

    try {
      const first = render(EverythingView, {
        props: {
          entries: [entry("note-preview", SYSTEM_TYPE_NOTE_ID, 900)],
          noteTypes,
        },
      });
      expect(loadEntry).not.toHaveBeenCalled();
      expect(observerOptions?.root).toBe(await first.getByTestId("everything-view").element());
      expect(observerOptions?.rootMargin).toBe("50% 0px");
      reveal?.();
      reveal?.();
      expect(loadEntry).not.toHaveBeenCalled();
      await expect
        .element(first.getByTestId("everything-preview-note-preview"))
        .toHaveTextContent("Preview heading Preview body");
      const preview = (await first
        .getByTestId("everything-preview-note-preview")
        .element()) as HTMLElement;
      const previewStyle = getComputedStyle(preview);
      expect(preview.textContent!.length).toBeLessThanOrEqual(801);
      expect(
        Number.parseFloat(previewStyle.height) / Number.parseFloat(previewStyle.lineHeight),
      ).toBe(5);
      expect(getComputedStyle(preview).maskImage).toContain("linear-gradient");
      expect(loadEntry).toHaveBeenCalledOnce();
      expect(loadEntry).toHaveBeenCalledWith("note-preview", { contentOnly: true });
      first.unmount();

      const second = render(EverythingView, {
        props: {
          entries: [entry("note-preview", SYSTEM_TYPE_NOTE_ID, 900)],
          noteTypes,
        },
      });
      await expect
        .element(second.getByTestId("everything-preview-note-preview"))
        .toBeInTheDocument();
      expect(loadEntry).toHaveBeenCalledOnce();
      second.unmount();

      previewUpdatedAt = 901;
      previewText = "# Refreshed preview\n\nNew body";
      const refreshed = render(EverythingView, {
        props: {
          entries: [entry("note-preview", SYSTEM_TYPE_NOTE_ID, 901)],
          noteTypes,
        },
      });
      reveal?.();
      await expect
        .element(refreshed.getByTestId("everything-preview-note-preview"))
        .toHaveTextContent("Refreshed preview New body");
      expect(loadEntry).toHaveBeenCalledTimes(2);
    } finally {
      window.api = originalApi;
      window.IntersectionObserver = originalObserver;
    }
  });

  test("uses the typographic fallback after a cover load error", async () => {
    const screen = render(EverythingView, {
      props: { entries: mixedEntries(), noteTypes },
    });
    const cover = (await screen
      .getByTestId("everything-cover-book-covered")
      .element()) as HTMLImageElement;

    cover.dispatchEvent(new Event("error"));
    await nextTick();

    await expect
      .element(screen.getByTestId("everything-cover-fallback-book-covered"))
      .toHaveTextContent("Без обложки");
  });

  test("uses the dominant cover color for the spine", async () => {
    const screen = render(EverythingView, {
      props: { entries: mixedEntries(), noteTypes },
    });
    const layer = (await screen
      .getByTestId("everything-cover-layer-book-covered")
      .element()) as HTMLElement;

    await expect
      .poll(() => layer.style.getPropertyValue("--book-cover-spine-color"))
      .toBe("rgb(255 165 0)");
  });

  test("emits the selected entry id", async () => {
    const onOpenEntry = vi.fn();
    const screen = render(EverythingView, {
      props: { entries: mixedEntries(), noteTypes, onOpenEntry },
    });

    await userEvent.click(screen.getByTestId("everything-card-book-covered"));

    expect(onOpenEntry).toHaveBeenCalledOnce();
    expect(onOpenEntry).toHaveBeenCalledWith("book-covered");
  });

  test("opens the Visuals context menu and emits deletion", async () => {
    const onDeleteEntry = vi.fn();
    const screen = render(EverythingView, {
      props: { entries: mixedEntries(), noteTypes, onDeleteEntry },
    });

    const card = await screen.getByTestId("everything-card-book-covered").element();
    card.dispatchEvent(
      new MouseEvent("contextmenu", { bubbles: true, clientX: 120, clientY: 160 }),
    );
    await nextTick();

    const deleteItem = screen.getByRole("menuitem", { name: "Удалить" });
    await expect.element(deleteItem).toBeVisible();
    expect((await deleteItem.element()).querySelector("svg")).not.toBeNull();
    await userEvent.click(deleteItem);

    expect(onDeleteEntry).toHaveBeenCalledOnce();
    expect(onDeleteEntry).toHaveBeenCalledWith("book-covered");
  });

  test("renders the add card and emits default entry creation", async () => {
    const onCreateEntry = vi.fn();
    const screen = render(EverythingView, {
      props: { entries: [], noteTypes, onCreateEntry },
    });

    await expect
      .element(screen.getByTestId("everything-add-card"))
      .toHaveAccessibleName("Добавить заметку");
    await userEvent.click(screen.getByTestId("everything-add-card"));
    expect(onCreateEntry).toHaveBeenCalledOnce();
  });

  test("keeps column gaps equal, cards indivisible, and rectangles separate", async () => {
    const entries = Array.from({ length: 8 }, (_, index) =>
      entry(
        `card-${index}`,
        index % 2 === 0 ? SYSTEM_TYPE_BOOK_ID : SYSTEM_TYPE_NOTE_ID,
        100 - index,
        {
          title:
            index === 0
              ? "Очень длинное название карточки, которое точно не помещается в одну строку"
              : `Карточка ${index}`,
        },
      ),
    );
    const screen = render(EverythingView, { props: { entries, noteTypes } });
    const view = (await screen.getByTestId("everything-view").element()) as HTMLElement;
    const grid = (await screen.getByTestId("everything-grid").element()) as HTMLElement;
    view.style.setProperty("--space-1", "4px");
    view.style.setProperty("--space-4", "16px");
    view.style.setProperty("--border", "currentColor");
    view.style.setProperty("--surface", "rgb(1, 2, 3)");
    view.style.width = "760px";
    await expect.poll(() => grid.children.length).toBe(2);

    const cards = [...grid.querySelectorAll<HTMLElement>('[data-testid^="everything-card-"]')];
    const addCard = (await screen.getByTestId("everything-add-card").element()) as HTMLElement;
    const firstCard = (await screen.getByTestId("everything-card-card-0").element()) as HTMLElement;
    const belowAddCard = (await screen
      .getByTestId("everything-card-card-1")
      .element()) as HTMLElement;
    const gridStyle = getComputedStyle(grid);
    const firstVisual = firstCard.querySelector<HTMLElement>(".everything-item-visual")!;
    const firstCardStyle = getComputedStyle(firstCard);
    const firstVisualStyle = getComputedStyle(firstVisual);
    expect(firstCardStyle.marginBlockEnd).toBe(gridStyle.columnGap);
    expect(firstCardStyle.breakInside).toBe("avoid");
    expect(firstCardStyle.gap).toBe("4px");
    expect(firstVisualStyle.borderTopWidth).toBe("2px");
    expect(grid.firstElementChild?.firstElementChild).toBe(addCard);
    expect(
      Math.abs(addCard.getBoundingClientRect().width - addCard.getBoundingClientRect().height),
    ).toBeLessThan(1);
    expect(firstCard.getBoundingClientRect().left).toBeGreaterThan(
      addCard.getBoundingClientRect().right,
    );
    expect(belowAddCard.getBoundingClientRect().left).toBe(addCard.getBoundingClientRect().left);
    expect(belowAddCard.getBoundingClientRect().top).toBeGreaterThanOrEqual(
      addCard.getBoundingClientRect().bottom,
    );
    const firstTitle = firstCard.querySelector<HTMLElement>(".everything-item-title")!;
    const firstTitleStyle = getComputedStyle(firstTitle);
    expect(firstTitleStyle.whiteSpace).toBe("nowrap");
    expect(firstTitleStyle.textOverflow).toBe("ellipsis");
    expect(firstTitle.scrollWidth).toBeGreaterThan(firstTitle.clientWidth);

    const beforeHoverTop = firstVisual.offsetTop;
    const beforeHover = firstVisual.getBoundingClientRect();
    await userEvent.hover(screen.getByRole("searchbox", { name: "Поиск" }));
    await expect.poll(() => getComputedStyle(firstVisual).backgroundColor).toBe("rgba(0, 0, 0, 0)");
    await userEvent.hover(screen.getByTestId("everything-card-card-0"));
    const afterHover = firstVisual.getBoundingClientRect();
    await expect.poll(() => getComputedStyle(firstVisual).backgroundColor).toBe("rgb(1, 2, 3)");
    expect(firstVisual.offsetTop).toBe(beforeHoverTop);
    expect(afterHover.height).toBe(beforeHover.height);

    const rects = cards.map((card) => card.getBoundingClientRect());
    for (let left = 0; left < rects.length; left += 1) {
      for (let right = left + 1; right < rects.length; right += 1) {
        expect(rectsOverlap(rects[left]!, rects[right]!)).toBe(false);
      }
    }

    view.style.width = "320px";
    await expect.poll(() => grid.children.length).toBe(1);
  });
});
