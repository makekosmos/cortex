import { describe, expect, test } from "bun:test";
import { writeEntryTiptapDoc } from "../src/editor-content/content";
import {
  createDraftBubble,
  createJournalBubblesFromEntry,
  decodeLocalBubblesStorage,
  isLegacyDatedJournalEntry,
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
