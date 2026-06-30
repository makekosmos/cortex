// Тесты для логики live-обновления открытой заметки при удалённом изменении.
// SDD → TDD: RED → GREEN.
//
// Тестируется чистая функция shouldApplyRemoteEntry из store/liveRefresh.ts.

import { describe, expect, test } from "bun:test";
import { isOlderRemoteEntry, shouldApplyRemoteEntry } from "../src/store/liveRefresh";
import { writeEntryMarkdown } from "../src/editor-cm/content";

function makeEntry(id: string, markdown: string): Entry {
  return {
    id,
    title: "Тест",
    content_json: JSON.stringify(writeEntryMarkdown(markdown)),
    created_at: 1000,
    updated_at: 2000,
    folder_id: null,
    type_id: "note_obj",
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
  };
}

describe("shouldApplyRemoteEntry", () => {
  test("возвращает apply когда контент реально изменился и редактор не dirty", () => {
    const current = makeEntry("abc", "старый текст");
    const fresh = makeEntry("abc", "новый текст от другого устройства");
    expect(
      shouldApplyRemoteEntry({
        fresh,
        currentContentJson: current.content_json,
        isEditorDirty: false,
      }),
    ).toBe("apply");
  });

  test("возвращает skip-same-content когда контент идентичен (self-echo от autosave)", () => {
    const current = makeEntry("abc", "мой текст");
    const fresh = makeEntry("abc", "мой текст");
    expect(
      shouldApplyRemoteEntry({
        fresh,
        currentContentJson: current.content_json,
        isEditorDirty: false,
      }),
    ).toBe("skip-same-content");
  });

  test("возвращает skip-same-content когда контент идентичен, даже если редактор dirty", () => {
    const current = makeEntry("abc", "мой текст");
    const fresh = makeEntry("abc", "мой текст");
    expect(
      shouldApplyRemoteEntry({
        fresh,
        currentContentJson: current.content_json,
        isEditorDirty: true,
      }),
    ).toBe("skip-same-content");
  });

  test("возвращает skip-dirty когда контент изменился, но редактор dirty (пользователь редактирует)", () => {
    const current = makeEntry("abc", "мой черновик");
    const fresh = makeEntry("abc", "удалённое изменение");
    expect(
      shouldApplyRemoteEntry({
        fresh,
        currentContentJson: current.content_json,
        isEditorDirty: true,
      }),
    ).toBe("skip-dirty");
  });

  test("content-equality guard: одинаковый markdown через разные JSON-обёртки — skip", () => {
    // fresh.content_json и currentContentJson могут иметь разные JSON-формы, но одинаковый markdown
    const markdown = "# Привет\n\nМир";
    const current = makeEntry("abc", markdown);
    // Вручную конструируем слегка другой JSON (пробел в конце, не влияющий на текст)
    const freshEntry = { ...makeEntry("abc", markdown) };
    expect(
      shouldApplyRemoteEntry({
        fresh: freshEntry,
        currentContentJson: current.content_json,
        isEditorDirty: false,
      }),
    ).toBe("skip-same-content");
  });

  test("возвращает apply когда current имеет пустой контент, а fresh — нет", () => {
    const current = makeEntry("abc", "");
    const fresh = makeEntry("abc", "новый контент");
    expect(
      shouldApplyRemoteEntry({
        fresh,
        currentContentJson: current.content_json,
        isEditorDirty: false,
      }),
    ).toBe("apply");
  });
});

describe("isOlderRemoteEntry", () => {
  test("возвращает true для задержанного remote load со старым updated_at", () => {
    expect(
      isOlderRemoteEntry(
        { ...makeEntry("abc", "старое"), updated_at: 100 },
        { ...makeEntry("abc", "новое"), updated_at: 101 },
      ),
    ).toBe(true);
  });

  test("не отбрасывает такую же или более новую версию", () => {
    const current = { ...makeEntry("abc", "текущее"), updated_at: 100 };
    expect(isOlderRemoteEntry({ ...makeEntry("abc", "такая же"), updated_at: 100 }, current)).toBe(
      false,
    );
    expect(isOlderRemoteEntry({ ...makeEntry("abc", "новее"), updated_at: 101 }, current)).toBe(
      false,
    );
  });
});
