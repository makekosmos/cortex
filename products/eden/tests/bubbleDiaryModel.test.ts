import { describe, expect, test } from "bun:test";
import { writeEntryTiptapDoc } from "../src/editor-content/content";
import {
  createDraftBubble,
  createJournalBubblesFromEntry,
  decodeLocalBubblesStorage,
  formatBubbleOccurrenceLabel,
  isLegacyDatedJournalEntry,
  normalizeBubbleThreads,
  parseBubbleDraft,
} from "../src/components/bubbles/bubbleDiaryModel";

function entry(overrides: Partial<Entry> = {}): Entry {
  return {
    id: "entry-1",
    title: "2021-01-21",
    content_json: JSON.stringify(
      writeEntryTiptapDoc({
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "старый дневниковый текст" }],
          },
        ],
      }),
    ),
    content_loaded: true,
    created_at: Date.UTC(2021, 0, 21, 9, 0),
    updated_at: Date.UTC(2021, 0, 21, 9, 0),
    folder_id: null,
    type_id: "note_obj",
    header_layout: "default",
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
    ...overrides,
  };
}

describe("bubbleDiaryModel legacy journal migration", () => {
  test("imports dated legacy note_obj entries as diary bubbles", () => {
    // Regression: 2026-07-03. Старые дневниковые заметки могли быть note_obj.
    const legacy = entry();

    expect(isLegacyDatedJournalEntry(legacy)).toBe(true);
    expect(createJournalBubblesFromEntry(legacy)).toMatchObject([
      {
        id: "journal-entry-1-0",
        date: "2021-01-21",
        time: "2021-01-21",
        text: "старый дневниковый текст",
      },
    ]);
  });

  test("sorts later legacy blocks above earlier blocks and drops empty blocks", () => {
    const legacy = entry({
      content_json: JSON.stringify(
        writeEntryTiptapDoc({
          type: "doc",
          content: [
            { type: "paragraph", content: [{ type: "text", text: "первая" }] },
            { type: "paragraph" },
            { type: "paragraph", content: [{ type: "text", text: "вторая" }] },
          ],
        }),
      ),
    });

    expect(createJournalBubblesFromEntry(legacy).map((bubble) => bubble.text)).toEqual([
      "первая",
      "вторая",
    ]);
    expect(createJournalBubblesFromEntry(legacy).map((bubble) => bubble.sortKey)).toEqual([
      Date.UTC(2021, 0, 21),
      Date.UTC(2021, 0, 21) + 2,
    ]);
  });

  test("repairs sort order for already imported legacy bubbles", () => {
    const decoded = decodeLocalBubblesStorage({
      version: 1,
      bubbles: [
        {
          id: "journal-entry-1-2",
          date: "2021-01-21",
          time: "2021-01-21",
          sortKey: Date.UTC(2021, 0, 21) - 2,
          text: "вторая",
          tags: [],
          kind: "plain",
        },
      ],
    });

    expect(decoded[0]?.sortKey).toBe(Date.UTC(2021, 0, 21) + 2);
  });

  test("trims trailing spaces and collapses empty lines", () => {
    expect(parseBubbleDraft("  первая   строка  \n\n\n  вторая   строка   ").text).toBe(
      "первая строка\nвторая строка",
    );
  });

  test("drops empty tiptap blocks from rendered draft content", () => {
    const bubble = createDraftBubble(
      {
        type: "doc",
        content: [
          { type: "paragraph", content: [{ type: "text", text: "первая" }] },
          { type: "paragraph" },
        ],
      },
      new Date(Date.UTC(2026, 6, 1, 12, 0)),
      "первая\n\n",
    );

    expect(bubble?.text).toBe("первая");
    expect(bubble?.contentJson?.content).toHaveLength(1);
    expect(bubble?.contentJson?.content?.[0]?.content?.[0]?.text).toBe("первая");
  });
});

describe("bubble occurrence labels", () => {
  test("uses local calendar boundaries for today, yesterday, and older dates", () => {
    const now = new Date(2026, 0, 1, 0, 15);
    expect(formatBubbleOccurrenceLabel(new Date(2026, 0, 1, 0, 1), now)).toBe("00:01");
    expect(formatBubbleOccurrenceLabel(new Date(2025, 11, 31, 23, 59), now)).toBe("Вчера, 23:59");
    expect(formatBubbleOccurrenceLabel(new Date(2025, 6, 2, 9, 5), now)).toBe("2 июл 2025, 09:05");

    const sameYearNow = new Date(2026, 6, 10, 12, 0);
    expect(formatBubbleOccurrenceLabel(new Date(2026, 5, 1, 8, 7), sameYearNow)).toBe(
      "1 июн, 08:07",
    );
  });

  test("changes a pre-midnight label to yesterday after local midnight", () => {
    const occurrence = new Date(2026, 6, 10, 23, 58);
    expect(formatBubbleOccurrenceLabel(occurrence, new Date(2026, 6, 10, 23, 59))).toBe("23:58");
    expect(formatBubbleOccurrenceLabel(occurrence, new Date(2026, 6, 11, 0, 1))).toBe(
      "Вчера, 23:58",
    );
  });
});

describe("one-level bubble threads", () => {
  const node = (id: string, sortKey: number) => ({
    id,
    time: "00:00",
    sortKey,
    text: id,
    tags: [],
    kind: "plain" as const,
  });

  test("groups roots newest-first and replies oldest-first", () => {
    const result = normalizeBubbleThreads(
      [node("root", 10), node("reply-2", 12), node("reply-1", 11), node("new-root", 20)],
      [
        { id: "l2", sourceObjectId: "reply-2", targetObjectId: "root" },
        { id: "l1", sourceObjectId: "reply-1", targetObjectId: "root" },
      ],
    );
    expect(result.bubbles.map(({ id, parentId }) => [id, parentId])).toEqual([
      ["new-root", undefined],
      ["root", undefined],
      ["reply-1", "root"],
      ["reply-2", "root"],
    ]);
  });

  test("keeps malformed deeper and cyclic relationships visible as roots", () => {
    const result = normalizeBubbleThreads(
      [node("a", 1), node("b", 2), node("c", 3), node("orphan", 4)],
      [
        { id: "ab", sourceObjectId: "a", targetObjectId: "b" },
        { id: "ba", sourceObjectId: "b", targetObjectId: "a" },
        { id: "cb", sourceObjectId: "c", targetObjectId: "b" },
        { id: "missing", sourceObjectId: "orphan", targetObjectId: "gone" },
      ],
    );
    expect(result.bubbles).toHaveLength(4);
    expect(result.bubbles.every((bubble) => bubble.parentId === undefined)).toBe(true);
    expect(new Set(result.invalidLinkIds)).toEqual(new Set(["ab", "ba", "cb", "missing"]));
  });
});
