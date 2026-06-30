// Тесты для hasUserVisibleEntryChanges.
// SDD → TDD: RED → GREEN.
//
// Проверяем, что:
// - Заметка, отличающаяся ТОЛЬКО нормализационными дефолтами (header_layout=null→resolved,
//   header_props_json с добавленными дефолтными полями) — НЕ считается изменённой.
// - Реальная смена title, тела, типа, header_props — считается изменённой.
//
// Функция тестируется через src/store/entryChanges.ts (чистая, не зависит от Vue).

import { describe, expect, test } from "bun:test";
import { hasUserVisibleEntryChanges } from "../src/store/entryChanges";
import { writeEntryMarkdown, writeEntryTiptapDoc } from "../src/editor-content/content";
import { SYSTEM_TYPE_NOTE, SYSTEM_TYPE_PERSON } from "../src/lib/systemTypes";

function makeNoteEntry(overrides: Partial<Entry> = {}): Entry {
  return {
    id: "test-id",
    title: "Тестовая заметка",
    content_json: JSON.stringify(writeEntryMarkdown("Текст заметки")),
    created_at: 1000,
    updated_at: 2000,
    folder_id: null,
    type_id: "note_obj",
    header_layout: null,
    header_props_json: null,
    schema_version: 1,
    deleted_at: null,
    ...overrides,
  };
}

const NOTE_TYPES = [SYSTEM_TYPE_NOTE, SYSTEM_TYPE_PERSON];

describe("hasUserVisibleEntryChanges", () => {
  // --- Нормализационные случаи (НЕ должны считаться изменением) ---

  test("header_layout=null vs resolved 'inline' для NOTE — НЕ изменение", () => {
    const base = makeNoteEntry({ header_layout: null });
    const draft = makeNoteEntry({ header_layout: "inline" });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(false);
  });

  test("header_layout=null vs resolved 'column' для PERSON — НЕ изменение", () => {
    const base = makeNoteEntry({
      type_id: "person_obj",
      header_layout: null,
    });
    const draft = makeNoteEntry({
      type_id: "person_obj",
      header_layout: "column",
    });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(false);
  });

  test("header_props_json=null vs нормализованные дефолтные поля типа — НЕ изменение", () => {
    const base = makeNoteEntry({ header_props_json: null });
    // Editor после safeParseHeaderProps добавляет дефолтные поля SYSTEM_TYPE_NOTE:
    // description, related_notes
    const draft = makeNoteEntry({
      header_props_json: JSON.stringify({ description: "", related_notes: [] }),
    });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(false);
  });

  test("header_props_json='{}' vs нормализованные дефолтные поля — НЕ изменение", () => {
    const base = makeNoteEntry({ header_props_json: "{}" });
    const draft = makeNoteEntry({
      header_props_json: JSON.stringify({ description: "", related_notes: [] }),
    });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(false);
  });

  test("Person: header_props с именами vs без — если оба дефолтные пустые — НЕ изменение", () => {
    const base = makeNoteEntry({
      type_id: "person_obj",
      header_props_json: null,
    });
    // safeParseHeaderProps для Person добавляет дефолтные пустые поля
    const draft = makeNoteEntry({
      type_id: "person_obj",
      header_props_json: JSON.stringify({
        first_name: "",
        last_name: "",
        patronymic: "",
        birth_date: "",
        photo: "",
      }),
    });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(false);
  });

  // --- Реальные изменения (ДОЛЖНЫ считаться изменением) ---

  test("смена title — это изменение", () => {
    const base = makeNoteEntry({ title: "Старое" });
    const draft = makeNoteEntry({ title: "Новое" });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(true);
  });

  test("смена тела — это изменение", () => {
    const base = makeNoteEntry({
      content_json: JSON.stringify(writeEntryMarkdown("старый")),
    });
    const draft = makeNoteEntry({
      content_json: JSON.stringify(writeEntryMarkdown("новый")),
    });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(true);
  });

  test("структурная TipTap-смена с тем же markdown — это изменение", () => {
    const base = makeNoteEntry({
      content_json: JSON.stringify(writeEntryMarkdown("a")),
    });
    const draft = makeNoteEntry({
      content_json: JSON.stringify(
        writeEntryTiptapDoc({
          type: "doc",
          content: [
            { type: "paragraph", content: [{ type: "text", text: "a" }] },
            { type: "paragraph" },
          ],
        }),
      ),
    });

    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(true);
  });

  test("смена type_id — это изменение", () => {
    const base = makeNoteEntry({ type_id: "note_obj" });
    const draft = makeNoteEntry({ type_id: "person_obj" });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(true);
  });

  test("реальная смена header_props (заполненное имя) — это изменение", () => {
    const base = makeNoteEntry({
      type_id: "person_obj",
      header_props_json: JSON.stringify({
        first_name: "",
        last_name: "",
        patronymic: "",
      }),
    });
    const draft = makeNoteEntry({
      type_id: "person_obj",
      header_props_json: JSON.stringify({
        first_name: "Иван",
        last_name: "Иванов",
        patronymic: "",
      }),
    });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(true);
  });

  test("реальная смена header_layout пользователем — это изменение", () => {
    const base = makeNoteEntry({ header_layout: "inline" });
    const draft = makeNoteEntry({ header_layout: "column" });
    expect(hasUserVisibleEntryChanges(draft, base, NOTE_TYPES)).toBe(true);
  });

  test("идентичные записи — НЕ изменение", () => {
    const entry = makeNoteEntry();
    expect(hasUserVisibleEntryChanges(entry, entry, NOTE_TYPES)).toBe(false);
  });
});
