import { describe, expect, test } from "bun:test";
import { writeEntryTiptapDoc } from "../src/editor-content/content";
import {
  createJournalBubblesFromEntry,
  isLegacyDatedJournalEntry,
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
});
