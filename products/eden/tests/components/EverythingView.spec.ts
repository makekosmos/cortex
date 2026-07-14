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
      .toHaveTextContent("Урсула Ле Гуин");
    await expect.element(screen.getByTestId("everything-cover-book-covered")).toBeInTheDocument();
    await expect
      .element(screen.getByTestId("everything-cover-layer-book-covered"))
      .toBeInTheDocument();
    await expect
      .element(screen.getByTestId("everything-cover-fallback-book-fallback"))
      .toHaveTextContent("Без обложки");
    await expect
      .element(screen.getByTestId("everything-card-note-old"))
      .toHaveTextContent("Заметка");
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
    await expect
      .element(screen.getByTestId("everything-empty"))
      .toHaveTextContent("Ничего не найдено");
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

  test("emits the selected entry id", async () => {
    const onOpenEntry = vi.fn();
    const screen = render(EverythingView, {
      props: { entries: mixedEntries(), noteTypes, onOpenEntry },
    });

    await userEvent.click(screen.getByTestId("everything-card-book-covered"));

    expect(onOpenEntry).toHaveBeenCalledOnce();
    expect(onOpenEntry).toHaveBeenCalledWith("book-covered");
  });

  test("renders the empty state", async () => {
    const screen = render(EverythingView, {
      props: { entries: [], noteTypes },
    });

    await expect
      .element(screen.getByTestId("everything-empty"))
      .toHaveTextContent("Здесь пока ничего нет");
    await expect.element(screen.getByTestId("everything-grid")).not.toBeInTheDocument();
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
    await new Promise((resolve) => requestAnimationFrame(resolve));

    const cards = [...grid.querySelectorAll<HTMLElement>('[data-testid^="everything-card-"]')];
    const gridStyle = getComputedStyle(grid);
    const firstVisual = cards[0]!.querySelector<HTMLElement>(".everything-item-visual")!;
    const firstCardStyle = getComputedStyle(cards[0]!);
    const firstVisualStyle = getComputedStyle(firstVisual);
    expect(firstCardStyle.marginBlockEnd).toBe(gridStyle.columnGap);
    expect(firstCardStyle.breakInside).toBe("avoid");
    expect(firstCardStyle.gap).toBe("4px");
    expect(firstVisualStyle.borderTopWidth).toBe("2px");
    const firstTitle = cards[0]!.querySelector<HTMLElement>(".everything-item-title")!;
    const firstTitleStyle = getComputedStyle(firstTitle);
    expect(firstTitleStyle.whiteSpace).toBe("nowrap");
    expect(firstTitleStyle.textOverflow).toBe("ellipsis");
    expect(firstTitle.scrollWidth).toBeGreaterThan(firstTitle.clientWidth);

    const beforeHover = firstVisual.getBoundingClientRect();
    await userEvent.hover(screen.getByRole("searchbox", { name: "Поиск" }));
    expect(getComputedStyle(firstVisual).backgroundColor).toBe("rgba(0, 0, 0, 0)");
    await userEvent.hover(screen.getByTestId("everything-card-card-0"));
    const afterHover = firstVisual.getBoundingClientRect();
    expect(getComputedStyle(firstVisual).backgroundColor).toBe("rgb(1, 2, 3)");
    expect(afterHover.top).toBe(beforeHover.top);
    expect(afterHover.height).toBe(beforeHover.height);

    const rects = cards.map((card) => card.getBoundingClientRect());
    for (let left = 0; left < rects.length; left += 1) {
      for (let right = left + 1; right < rects.length; right += 1) {
        expect(rectsOverlap(rects[left]!, rects[right]!)).toBe(false);
      }
    }

    view.style.width = "320px";
    await new Promise((resolve) => requestAnimationFrame(resolve));
    expect(getComputedStyle(grid).columnCount).toBe("1");
  });
});
