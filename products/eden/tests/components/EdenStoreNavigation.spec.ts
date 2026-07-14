import { beforeEach, describe, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { writeEntryMarkdown } from "../../src/editor-content/content";
import { SYSTEM_TYPE_NOTE_ID } from "../../src/lib/systemTypes";
import { useEdenStore } from "../../src/store/eden";

function cachedEntry(id: string): Entry {
  return {
    id,
    title: "Cached",
    content_json: JSON.stringify(writeEntryMarkdown("already loaded")),
    created_at: 1000,
    updated_at: 2000,
    folder_id: null,
    type_id: SYSTEM_TYPE_NOTE_ID,
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
    content_loaded: true,
  };
}

beforeEach(() => {
  localStorage.clear();
  setActivePinia(createPinia());
});

describe("Eden store navigation", () => {
  test("uses already loaded entries without loadEntry or skeleton state", async () => {
    const loadEntry = vi.fn(async () => null);
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        loadEntry,
      },
    });

    const eden = useEdenStore();
    const entry = cachedEntry("cached-entry");
    eden.entries = [entry];

    await eden.navigateTo(entry.id);

    expect(loadEntry).not.toHaveBeenCalled();
    expect(eden.loadingEntryId).toBeNull();
    expect(eden.currentEntry?.id).toBe(entry.id);
    expect(eden.currentEntry?.content_json).toBe(entry.content_json);
  });

  test("always starts on Everything even with stale navigation storage", () => {
    localStorage.setItem("eden:nav:lastScreen", "diary");
    localStorage.setItem("eden:nav:lastEntryId", "old-entry");

    const eden = useEdenStore();

    expect(eden.activeScreen).toBe("notes");
    expect(eden.activeNoteTypeId).toBeNull();
    expect(eden.currentEntry).toBeNull();
  });

  test("openEverything clears the selected entry, collection, and pending load", () => {
    const eden = useEdenStore();
    const entry = cachedEntry("current-entry");

    eden.activeScreen = "type-collection";
    eden.activeNoteTypeId = "book_obj";
    eden.currentEntry = entry;
    eden.loadingEntryId = entry.id;

    eden.openEverything();

    expect(eden.activeScreen).toBe("notes");
    expect(eden.activeNoteTypeId).toBeNull();
    expect(eden.currentEntry).toBeNull();
    expect(eden.loadingEntryId).toBeNull();
  });
});
