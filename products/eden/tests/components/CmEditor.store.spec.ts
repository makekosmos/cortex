import { describe, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { nextTick } from "vue";
import { deferred, markdownEntry } from "./cm-editor-test-helpers";
import { useEdenStore } from "../../src/store/eden";

describe("CmEditor store flows", () => {
  test("store handleSave сохраняет body после optimistic updateEntryDraft", async () => {
    // Regression: 2026-06-15. updateEntryDraft changed entries before handleSave,
    // so handleSave compared against the optimistic draft and skipped ARK save.
    setActivePinia(createPinia());
    const store = useEdenStore();
    const original = markdownEntry("");
    const draft: Entry = {
      ...original,
      content_json: JSON.stringify({ type: "markdown", version: 1, text: "persist me" }),
      updated_at: original.updated_at + 1,
    };
    const saveEntry = vi.fn(
      async () => ({ ok: true, entryId: draft.id }) satisfies SaveEntryResult,
    );
    (window as unknown as { api: Partial<Window["api"]> }).api = { saveEntry };

    store.entries = [original];
    store.currentEntry = original;
    store.updateEntryDraft(draft);
    await store.handleSave(draft);

    expect(saveEntry).toHaveBeenCalledTimes(1);
    expect(saveEntry.mock.calls[0]?.[0]).toMatchObject({
      id: draft.id,
      content_json: draft.content_json,
    });
  });

  test("store ignores stale save completion after a newer optimistic draft", async () => {
    // Regression: 2026-06-16. A save that started before a newer local draft
    // must not push its older body back into currentEntry.
    setActivePinia(createPinia());
    const original = markdownEntry("");
    original.updated_at = 100;
    const olderDraft: Entry = {
      ...original,
      content_json: JSON.stringify({ type: "markdown", version: 1, text: "old save" }),
      updated_at: 101,
    };
    const newerDraft: Entry = {
      ...original,
      content_json: JSON.stringify({ type: "markdown", version: 1, text: "new local draft" }),
      updated_at: 102,
    };
    const pendingSave = deferred<SaveEntryResult>();
    const saveEntry = vi.fn(() => pendingSave.promise);
    (window as unknown as { api: Partial<Window["api"]> }).api = { saveEntry };

    const store = useEdenStore();
    store.entries = [original];
    store.currentEntry = original;

    const savePromise = store.handleSave(olderDraft);
    await expect.poll(() => saveEntry.mock.calls.length, { timeout: 4000 }).toBe(1);

    store.updateEntryDraft(newerDraft);
    pendingSave.resolve({ ok: true, entryId: olderDraft.id });
    await savePromise;

    expect(store.currentEntry?.content_json).toBe(newerDraft.content_json);
    expect(store.entries.find((entry) => entry.id === original.id)?.content_json).toBe(
      newerDraft.content_json,
    );
  });

  test("store createNewEntry waits for initial empty save before mounting editor", async () => {
    // Regression: 2026-06-16. The initial empty save must not race the first
    // editor autosave with user text.
    setActivePinia(createPinia());
    const initialSave = deferred<SaveEntryResult>();
    const saveEntry = vi.fn(() => initialSave.promise);
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        saveEntry,
        saveNoteType: vi.fn(async () => ({ ok: true })),
      },
    });

    const store = useEdenStore();
    const createPromise = store.createNewEntry();

    await expect.poll(() => saveEntry.mock.calls.length, { timeout: 4000 }).toBe(1);
    expect(store.currentEntry).toBeNull();

    const newEntry = saveEntry.mock.calls[0]?.[0] as Entry;
    initialSave.resolve({ ok: true, entryId: newEntry.id });
    await createPromise;

    expect(store.currentEntry?.id).toBe(newEntry.id);
  });

  test("store navigateTo сразу показывает preview entry вместо пустой страницы", async () => {
    setActivePinia(createPinia());

    const entry = markdownEntry("Preview body");
    const loadEntry = vi.fn(() => new Promise<Entry | undefined>(() => undefined));
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        loadEntry,
      },
    });

    const store = useEdenStore();
    store.entries = [entry];
    store.currentEntry = null;

    void store.navigateTo(entry.id);

    expect(store.activeScreen).toBe("notes");
    expect(store.loadingEntryId).toBe(entry.id);
    expect(store.currentEntry?.id).toBe(entry.id);
    expect(store.currentEntry?.title).toBe(entry.title);
    expect(loadEntry).not.toHaveBeenCalled();
  });

  test("store navigateTo защищает от stale resolve при быстрых кликах по заметкам", async () => {
    setActivePinia(createPinia());

    const first: Entry = { ...markdownEntry("First preview"), id: "note-1", title: "First" };
    const second: Entry = { ...markdownEntry("Second preview"), id: "note-2", title: "Second" };
    const firstLoad = deferred<Entry | undefined>();
    const secondLoad = deferred<Entry | undefined>();
    const saveEntry = vi.fn(async () => ({ ok: true, entryId: first.id }));
    const loadEntry = vi.fn((entryId: string) => {
      if (entryId === first.id) return firstLoad.promise;
      if (entryId === second.id) return secondLoad.promise;
      return;
    });
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        saveEntry,
        loadEntry,
      },
    });

    const store = useEdenStore();
    store.entries = [first, second];
    store.currentEntry = null;

    void store.navigateTo(first.id);
    void store.navigateTo(second.id);

    await expect.poll(() => loadEntry.mock.calls.length, { timeout: 4000 }).toBe(2);
    expect(store.loadingEntryId).toBe(second.id);
    expect(store.currentEntry?.id).toBe(second.id);

    const resolvedSecond: Entry = {
      ...second,
      title: "Loaded second",
      content_json: JSON.stringify({ type: "markdown", version: 1, text: "second body" }),
    };
    secondLoad.resolve(resolvedSecond);
    await expect.poll(() => store.currentEntry?.title, { timeout: 4000 }).toBe("Loaded second");
    expect(store.loadingEntryId).toBe(null);

    const resolvedFirst: Entry = {
      ...first,
      title: "Loaded first",
      content_json: JSON.stringify({ type: "markdown", version: 1, text: "first body" }),
    };
    firstLoad.resolve(resolvedFirst);
    await nextTick();

    expect(store.currentEntry?.id).toBe(second.id);
    expect(store.currentEntry?.title).toBe("Loaded second");
    expect(store.loadingEntryId).toBe(null);
  });

  test("store clears pending load when activeScreen switches to settings", async () => {
    setActivePinia(createPinia());

    const entry = markdownEntry("Preview before settings");
    const load = deferred<Entry | undefined>();
    const loadEntry = vi.fn(() => load.promise);
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        loadEntry,
      },
    });

    const store = useEdenStore();
    store.entries = [entry];
    store.currentEntry = null;

    void store.navigateTo(entry.id);
    await expect.poll(() => loadEntry.mock.calls.length, { timeout: 4000 }).toBe(1);

    store.activeScreen = "settings";
    await nextTick();

    expect(store.loadingEntryId).toBe(null);

    load.resolve({
      ...entry,
      title: "Loaded after settings",
      content_json: JSON.stringify({ type: "markdown", version: 1, text: "body" }),
    });
    await nextTick();

    expect(store.currentEntry?.id).toBe(entry.id);
    expect(store.currentEntry?.title).toBe(entry.title);
    expect(store.loadingEntryId).toBe(null);
  });

  test("store initApp не открывает lastEntryId на старте", async () => {
    setActivePinia(createPinia());

    const entry = markdownEntry("Saved body");
    window.localStorage.setItem("eden:nav:lastEntryId", entry.id);
    Object.defineProperty(window, "kepler", {
      configurable: true,
      value: {
        ark: {
          request: vi.fn(async () => null),
          subscribe: vi.fn(() => () => undefined),
        },
      },
    });
    const loadEntry = vi.fn(async () => entry);
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
        getVaultPath: vi.fn(async () => "D:/tmp/eden"),
        getRecentVaultPaths: vi.fn(async () => ["D:/tmp/eden"]),
        getSidebarConfig: vi.fn(async () => ({ widget: { hidden: false, width: 280 } })),
        listEntries: vi.fn(async () => [entry]),
        listNoteTypes: vi.fn(async () => []),
        ensureCollectionObjects: vi.fn(async () => []),
        saveEntry: vi.fn(async () => ({ ok: true, entryId: entry.id })),
        saveNoteType: vi.fn(async () => ({ ok: true })),
        deleteNoteType: vi.fn(async () => undefined),
        selectFolder: vi.fn(async () => null),
        setVaultPath: vi.fn(async () => undefined),
        loadEntry,
      },
    });

    const store = useEdenStore();
    await store.initApp();

    await expect.poll(() => store.isHydratingVault).toBe(false);
    expect(store.entries.map((candidate) => candidate.id)).toEqual([entry.id]);
    expect(store.currentEntry).toBeNull();
    expect(loadEntry).not.toHaveBeenCalled();
  });
});
