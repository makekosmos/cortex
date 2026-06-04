import { describe, expect, test } from "bun:test";
import { createClipboardHistoryStore } from "../../shell/electron/clipboard-history-store";

describe("clipboard history store", () => {
  test("records normalized text newest first", () => {
    let now = 1000;
    const store = createClipboardHistoryStore({ now: () => now++ });

    store.record("  первый текст  ");
    store.record("второй\r\nтекст");

    expect(store.list()).toMatchObject([
      { text: "второй\nтекст", charCount: 12 },
      { text: "первый текст", charCount: 12 },
    ]);
  });

  test("moves duplicates to the top instead of cloning them", () => {
    let now = 10;
    const store = createClipboardHistoryStore({ now: () => now++ });

    const first = store.record("alpha");
    store.record("beta");
    const repeated = store.record("alpha");

    expect(repeated?.id).toBe(first?.id);
    expect(store.list().map((item) => item.text)).toEqual(["alpha", "beta"]);
    expect(store.list()[0].createdAt).toBe(12);
  });

  test("ignores empty text and respects max items", () => {
    const store = createClipboardHistoryStore({ maxItems: 2, now: () => 1 });

    store.record(" ");
    store.record("one");
    store.record("two");
    store.record("three");

    expect(store.list().map((item) => item.text)).toEqual(["three", "two"]);
  });

  test("removes and clears items", () => {
    const store = createClipboardHistoryStore({ now: () => 1 });
    const item = store.record("copy me");

    expect(item).not.toBeNull();
    expect(store.remove(item!.id)).toBe(true);
    expect(store.remove(item!.id)).toBe(false);

    store.record("next");
    store.clear();

    expect(store.list()).toEqual([]);
  });
});
