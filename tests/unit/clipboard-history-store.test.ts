import { describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  classifyClipboardText,
  createClipboardHistoryStore,
} from "../../platform/desktop/electron/clipboard-history-store";

describe("clipboard history store", () => {
  test("records normalized text newest first", () => {
    let now = 1000;
    const store = createClipboardHistoryStore({ now: () => now++ });

    store.record("  первый текст  ");
    store.record("второй\r\nтекст");

    expect(store.list()).toMatchObject([
      { kind: "text", text: "второй\nтекст", charCount: 12, pinned: false },
      { kind: "text", text: "первый текст", charCount: 12, pinned: false },
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
    expect(store.list()[0].createdAt).toBe(10);
    expect(store.list()[0].updatedAt).toBe(12);
  });

  test("classifies links and colors", () => {
    expect(classifyClipboardText("https://example.com/docs")).toMatchObject({
      kind: "link",
      url: "https://example.com/docs",
    });
    expect(classifyClipboardText("ff5c00")).toMatchObject({
      kind: "color",
      color: "#FF5C00",
    });
    expect(classifyClipboardText("rgb(255, 92, 0)")).toMatchObject({
      kind: "color",
      color: "#FF5C00",
    });
  });

  test("records link, color, and file entries", () => {
    let now = 30;
    const store = createClipboardHistoryStore({ now: () => now++ });

    store.record("https://example.com/docs");
    store.record("#ff5c00");
    store.recordFile("D:\\Personal\\note.md");

    expect(store.list()).toMatchObject([
      {
        kind: "file",
        preview: "note.md",
        filePath: "D:\\Personal\\note.md",
        searchText: "note.md D:\\Personal\\note.md",
      },
      { kind: "color", text: "#FF5C00", color: "#FF5C00" },
      { kind: "link", url: "https://example.com/docs" },
    ]);
  });

  test("records image entries and moves duplicate images to the top", () => {
    let now = 20;
    const store = createClipboardHistoryStore({ now: () => now++ });

    const first = store.recordImage({
      dataUrl: "data:image/png;base64,abc",
      width: 1280,
      height: 720,
    });
    store.record("text");
    const repeated = store.recordImage({
      dataUrl: "data:image/png;base64,abc",
      width: 1280,
      height: 720,
    });

    expect(repeated?.id).toBe(first?.id);
    expect(store.list()).toMatchObject([
      {
        kind: "image",
        preview: "Изображение 1280×720",
        width: 1280,
        height: 720,
        createdAt: 20,
        updatedAt: 22,
      },
      { kind: "text", text: "text" },
    ]);
  });

  test("records source metadata for new clipboard entries", () => {
    let now = 50;
    const store = createClipboardHistoryStore({ now: () => now++ });

    const sharexIcon = "data:image/png;base64,sharex";
    const explorerIcon = "data:image/png;base64,explorer";

    store.record("from text", { source: "ShareX", sourceIcon: sharexIcon });
    store.recordFile("D:\\Personal\\image.png", { source: "Explorer", sourceIcon: explorerIcon });
    store.recordImage({
      dataUrl: "data:image/png;base64,source",
      width: 1400,
      height: 800,
      source: "ShareX",
      sourceIcon: sharexIcon,
    });

    expect(store.list()).toMatchObject([
      { kind: "image", source: "ShareX", sourceIcon: sharexIcon },
      { kind: "file", source: "Explorer", sourceIcon: explorerIcon },
      { kind: "text", source: "ShareX", sourceIcon: sharexIcon },
    ]);
  });

  test("ignores empty text and respects max items", () => {
    const store = createClipboardHistoryStore({ maxItems: 2, now: () => 1 });

    store.record(" ");
    store.record("one");
    store.record("two");
    store.record("three");

    expect(store.list().map((item) => item.text)).toEqual(["three", "two"]);
  });

  test("keeps pinned items at the top and preserves them on normal clear", () => {
    let now = 40;
    const store = createClipboardHistoryStore({ now: () => now++ });

    const pinned = store.record("keep");
    store.record("remove");
    expect(pinned).not.toBeNull();
    store.togglePin(pinned!.id);
    store.record("newer");

    expect(store.list().map((item) => item.text)).toEqual(["keep", "newer", "remove"]);

    store.clear();
    expect(store.list()).toMatchObject([{ text: "keep", pinned: true }]);

    store.clearAll();
    expect(store.list()).toEqual([]);
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

  test("persists entries and settings across store instances", () => {
    const dir = mkdtempSync(path.join(tmpdir(), "kosmos-clipboard-"));
    try {
      const storagePath = path.join(dir, "clipboard-history.json");
      const first = createClipboardHistoryStore({ storagePath, now: () => 100 });
      first.record("persist me");
      first.updateSettings({ retentionDays: 12, maxBytes: 1024 * 1024 });

      const second = createClipboardHistoryStore({ storagePath, now: () => 200 });

      expect(second.settings()).toEqual({ retentionDays: 12, maxBytes: 1024 * 1024 });
      expect(second.list()).toMatchObject([{ text: "persist me" }]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });

  test("prunes expired unpinned items but keeps pinned entries", () => {
    let now = 0;
    const store = createClipboardHistoryStore({ now: () => now });

    now = 1_000;
    const old = store.record("old");
    expect(old).not.toBeNull();
    store.togglePin(old!.id);
    now = 2_000;
    store.record("unpinned old");
    now = 4 * 24 * 60 * 60 * 1000;

    store.updateSettings({ retentionDays: 1 });

    expect(store.list().map((item) => item.text)).toEqual(["old"]);
  });

  test("prunes oldest unpinned items when storage budget is exceeded", () => {
    let now = 10;
    const store = createClipboardHistoryStore({ now: () => now++ });

    const pinned = store.record("pin " + "x".repeat(80));
    expect(pinned).not.toBeNull();
    store.togglePin(pinned!.id);
    store.record("first " + "a".repeat(80));
    store.record("second " + "b".repeat(80));

    const pinnedBytes = store.list().find((item) => item.pinned)?.storageBytes ?? 0;
    store.updateSettings({ maxBytes: pinnedBytes + 80 });

    expect(store.list().map((item) => item.text)).toEqual([pinned!.text]);
  });
});
