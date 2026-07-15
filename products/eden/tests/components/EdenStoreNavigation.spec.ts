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

  test("soft-deletes an entry and removes it from Everything after the local write", async () => {
    const deleteEntry = vi.fn(async () => ({ ok: true as const, entryId: "deleted-entry" }));
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: { deleteEntry },
    });
    const eden = useEdenStore();
    eden.entries = [cachedEntry("deleted-entry"), cachedEntry("kept-entry")];

    await expect(eden.deleteEntry("deleted-entry")).resolves.toBe(true);
    expect(deleteEntry).toHaveBeenCalledWith("deleted-entry");
    expect(eden.entries.map((entry) => entry.id)).toEqual(["kept-entry"]);

    deleteEntry.mockResolvedValueOnce({
      ok: false,
      reason: "entry_not_found",
      message: "not found",
    });
    await expect(eden.deleteEntry("kept-entry")).resolves.toBe(false);
    expect(eden.entries.map((entry) => entry.id)).toEqual(["kept-entry"]);
  });

  test("opens a new note before persistence and adds it to Everything after save", async () => {
    let resolveSave!: (result: { ok: true; entryId: string }) => void;
    const saveEntry = vi.fn(
      () =>
        new Promise<{ ok: true; entryId: string }>((resolve) => {
          resolveSave = resolve;
        }),
    );
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        saveEntry,
        saveNoteType: vi.fn(async () => ({ ok: true })),
      },
    });

    const eden = useEdenStore();
    const creation = eden.createNewEntry();
    const createdId = eden.currentEntry?.id;

    expect(createdId).toBeTruthy();
    expect(eden.loadingEntryId).toBe(createdId);
    expect(eden.entries).toHaveLength(0);

    await vi.waitFor(() => expect(saveEntry).toHaveBeenCalledOnce());
    resolveSave({ ok: true, entryId: createdId! });
    await creation;

    expect(eden.loadingEntryId).toBeNull();
    expect(eden.currentEntry?.id).toBe(createdId);
    expect(eden.entries.map((entry) => entry.id)).toEqual([createdId]);
  });
});
