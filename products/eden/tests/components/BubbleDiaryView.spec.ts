import { beforeEach, describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import BubbleDiaryView from "../../src/components/bubbles/BubbleDiaryView.vue";
import {
  LOCAL_BUBBLES_STORAGE_KEY,
  createDraftBubble,
  encodeLocalBubblesStorage,
  formatBubbleDateKey,
} from "../../src/components/bubbles/bubbleDiaryModel";
import { writeEntryTiptapDoc } from "../../src/editor-content/content";
import { SYSTEM_TYPE_JOURNAL_ID } from "../../src/lib/systemTypeDefinitions";

function storedBubbles() {
  return JSON.parse(localStorage.getItem(LOCAL_BUBBLES_STORAGE_KEY) ?? "null")?.bubbles ?? [];
}

function journalEntry(overrides: Partial<Entry> = {}): Entry {
  return {
    id: "journal-1",
    title: "2026-07-01",
    content_json: JSON.stringify(
      writeEntryTiptapDoc({
        type: "doc",
        content: [
          { type: "paragraph", content: [{ type: "text", text: "first block" }] },
          { type: "paragraph", content: [{ type: "text", text: "second block" }] },
        ],
      }),
    ),
    content_loaded: true,
    created_at: 1_780_000_000_000,
    updated_at: 1_780_000_000_000,
    folder_id: null,
    type_id: SYSTEM_TYPE_JOURNAL_ID,
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
    ...overrides,
  };
}

describe("BubbleDiaryView", () => {
  beforeEach(() => {
    localStorage.removeItem(LOCAL_BUBBLES_STORAGE_KEY);
  });

  test("starts as an empty diary timeline", async () => {
    const screen = render(BubbleDiaryView);

    await expect.element(screen.getByTestId("diary-view")).toBeInTheDocument();
    expect(document.querySelector("h1")).toBeNull();
    expect(document.body).not.toHaveTextContent("Eden");
    await expect.element(screen.getByTestId("bubble-filter-trigger")).not.toBeInTheDocument();
    await expect.element(screen.getByText("Пока нет записей")).toBeInTheDocument();
  });

  test("composer adds a local bubble and turns hashtags into chips", async () => {
    const screen = render(BubbleDiaryView);

    await userEvent.type(
      screen.getByTestId("bubble-composer-input"),
      "Новая мысль про дневник #идея",
    );

    await userEvent.click(screen.getByTestId("bubble-composer-submit"));

    await expect.element(screen.getByText("Новая мысль про дневник")).toBeInTheDocument();
    const draftBubble = document.querySelector('[data-testid^="bubble-node-draft-"]');
    expect(draftBubble?.textContent).toContain("идея");
    expect(draftBubble?.textContent).not.toContain("#идея");

    const saved = storedBubbles();
    expect(saved[0].text).toBe("Новая мысль про дневник");
    expect(saved[0].tags).toEqual(["идея"]);
    expect(saved[0].kind).toBe("plain");
    expect(saved[0].date).toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(saved[0].source).toBeUndefined();
  });

  test("draft bubbles keep Tiptap code blocks in stored content", () => {
    const bubble = createDraftBubble(
      {
        type: "doc",
        content: [
          {
            type: "codeBlock",
            content: [{ type: "text", text: "const answer = 42;" }],
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "#idea" }],
          },
        ],
      },
      new Date("2026-07-02T09:41:00"),
      "const answer = 42;\n#idea",
    );

    expect(bubble?.text).toBe("const answer = 42;");
    expect(bubble?.tags).toEqual(["idea"]);
    expect(bubble?.date).toBe("2026-07-02");
    expect(bubble?.contentJson?.content?.[0]?.type).toBe("codeBlock");
  });

  test("restores local bubbles after remount", async () => {
    localStorage.setItem(
      LOCAL_BUBBLES_STORAGE_KEY,
      encodeLocalBubblesStorage([
        {
          id: "draft-persisted",
          time: "09:41",
          text: "Покушал",
          tags: ["еда"],
          kind: "plain",
        },
      ]),
    );

    const screen = render(BubbleDiaryView);

    await expect
      .element(screen.getByTestId("bubble-node-draft-persisted"))
      .toHaveTextContent("Покушал");
  });

  test("ignores pre-release local bubble arrays", async () => {
    localStorage.setItem(
      LOCAL_BUBBLES_STORAGE_KEY,
      JSON.stringify([
        {
          id: "old-draft",
          time: "09:41",
          text: "old",
          tags: [],
        },
      ]),
    );

    const screen = render(BubbleDiaryView);

    await expect.element(screen.getByTestId("bubble-node-old-draft")).not.toBeInTheDocument();
  });

  test("converts old journal blocks into dated bubbles in order", async () => {
    const screen = render(BubbleDiaryView, {
      props: {
        journalEntries: [journalEntry()],
      },
    });

    await expect
      .element(screen.getByTestId("bubble-node-journal-journal-1-0"))
      .toHaveTextContent("first block");
    await expect
      .element(screen.getByTestId("bubble-node-journal-journal-1-1"))
      .toHaveTextContent("second block");
    expect(document.body.textContent?.indexOf("first block")).toBeLessThan(
      document.body.textContent?.indexOf("second block") ?? 0,
    );
    expect(screen.getByTestId("bubble-node-journal-journal-1-0").element()).toHaveTextContent(
      "2026-07-01",
    );
    expect(storedBubbles()[0].date).toBe("2026-07-01");
  });

  test("does not load full content for non-journal entries during legacy import", async () => {
    const previousApi = window.api;
    const loadEntry = vi.fn(async () => undefined);
    window.api = { ...(previousApi ?? {}), loadEntry } as typeof window.api;

    try {
      render(BubbleDiaryView, {
        props: {
          journalEntries: [
            journalEntry({
              id: "regular-note",
              type_id: "note_obj",
              content_loaded: false,
            }),
          ],
        },
      });
      await new Promise((resolve) => setTimeout(resolve, 0));
      expect(loadEntry).not.toHaveBeenCalled();
    } finally {
      window.api = previousApi;
    }
  });

  test("calendar sidebar shows dated local bubbles when enabled", async () => {
    const today = formatBubbleDateKey(new Date());
    localStorage.setItem(
      LOCAL_BUBBLES_STORAGE_KEY,
      encodeLocalBubblesStorage([
        {
          id: "draft-first",
          date: today,
          time: "09:41",
          text: "Первая",
          tags: [],
          kind: "plain",
        },
        {
          id: "draft-second",
          date: today,
          time: "10:12",
          text: "Вторая",
          tags: [],
          kind: "plain",
        },
      ]),
    );

    const screen = render(BubbleDiaryView, {
      props: {
        calendarOpen: true,
      },
    });

    await expect.element(screen.getByTestId("diary-calendar-sidebar")).toBeInTheDocument();
    await expect
      .element(screen.getByTestId(`diary-calendar-day-${today}`))
      .toHaveAttribute("data-entry-count", "2");
  });

  test("does not reimport journal bubbles after import was saved", async () => {
    localStorage.setItem(
      LOCAL_BUBBLES_STORAGE_KEY,
      encodeLocalBubblesStorage([], { journalImported: true }),
    );

    const screen = render(BubbleDiaryView, {
      props: {
        journalEntries: [journalEntry()],
      },
    });

    await expect
      .element(screen.getByTestId("bubble-node-journal-journal-1-0"))
      .not.toBeInTheDocument();
  });

  test("time opens inline editing with update and delete actions", async () => {
    localStorage.setItem(
      LOCAL_BUBBLES_STORAGE_KEY,
      encodeLocalBubblesStorage([
        {
          id: "draft-persisted",
          time: "09:41",
          text: "Покушал",
          tags: ["еда"],
          kind: "plain",
        },
      ]),
    );

    const screen = render(BubbleDiaryView);

    await userEvent.click(screen.getByTestId("bubble-time-draft-persisted"));
    await expect
      .element(screen.getByTestId("bubble-edit-input-draft-persisted"))
      .toBeInTheDocument();

    await userEvent.clear(screen.getByTestId("bubble-edit-input-draft-persisted"));
    await userEvent.type(screen.getByTestId("bubble-edit-input-draft-persisted"), "Ужин #еда");
    await userEvent.click(screen.getByText("Обновить"));

    await expect
      .element(screen.getByTestId("bubble-node-draft-persisted"))
      .toHaveTextContent("Ужин");
    let saved = storedBubbles();
    expect(saved[0].text).toBe("Ужин");
    expect(saved[0].tags).toEqual(["еда"]);

    await userEvent.click(screen.getByTestId("bubble-time-draft-persisted"));
    await userEvent.click(screen.getByText("Удалить"));
    await userEvent.click(screen.getByText("Точно удалить"));

    await expect.element(screen.getByTestId("bubble-node-draft-persisted")).not.toBeInTheDocument();
    saved = storedBubbles();
    expect(saved).toEqual([]);
  });

  test("ball opens kind dropdown and stores selected kind", async () => {
    localStorage.setItem(
      LOCAL_BUBBLES_STORAGE_KEY,
      encodeLocalBubblesStorage([
        {
          id: "draft-persisted",
          time: "09:41",
          text: "Покушал",
          tags: ["еда"],
          kind: "plain",
        },
      ]),
    );

    const screen = render(BubbleDiaryView);
    await expect.element(screen.getByTestId("bubble-node-draft-persisted")).toBeInTheDocument();

    const trigger = document.querySelector('[data-testid="bubble-kind-draft-persisted"] button');
    expect(trigger).toBeInstanceOf(HTMLElement);

    await userEvent.click(trigger as HTMLElement);
    await userEvent.click(screen.getByText("Идея"));

    const saved = storedBubbles();
    expect(saved[0].kind).toBe("idea");
  });
});
