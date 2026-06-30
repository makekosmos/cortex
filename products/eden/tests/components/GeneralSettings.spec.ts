import { afterEach, beforeEach, describe, expect, test, vi } from "vitest";
import { createPinia, setActivePinia } from "pinia";
import { createApp, defineComponent, h, nextTick, vaporInteropPlugin, type App } from "vue";
import GeneralSettings from "../../src/components/settings/GeneralSettings.vue";
import { writeEntryMarkdown } from "../../src/editor-cm/content";
import { useAutoTipTapMigration } from "../../src/editor-tiptap/useAutoTipTapMigration";
import { useEdenStore } from "../../src/store/eden";
import { markdownEntry } from "./cm-editor-test-helpers";

let app: App<Element> | null = null;
let container: HTMLDivElement | null = null;

function makeMarkdownEntry(id: string, text: string): Entry {
  return {
    ...markdownEntry(text),
    id,
    title: `Entry ${id}`,
    content_json: JSON.stringify(writeEntryMarkdown(text)),
  };
}

function installApi(
  entries: Entry[],
  failOnceIds = new Set<string>(),
  mutateAfterListAll?: (persisted: Map<string, Entry>) => void,
) {
  const persisted = new Map(entries.map((entry) => [entry.id, entry]));
  const failed = new Set<string>();
  const saveEntry = vi.fn(async (entry: Entry): Promise<SaveEntryResult> => {
    if (failOnceIds.has(entry.id) && !failed.has(entry.id)) {
      failed.add(entry.id);
      return { ok: false, reason: "write_failed", message: "write failed" };
    }
    persisted.set(entry.id, entry);
    return { ok: true, entryId: entry.id };
  });
  const loadEntry = vi.fn(async (id: string) => persisted.get(id) ?? undefined);
  const listAllEntries = vi.fn(async () => {
    const snapshot = [...persisted.values()];
    mutateAfterListAll?.(persisted);
    return snapshot;
  });

  Object.defineProperty(window, "api", {
    configurable: true,
    writable: true,
    value: {
      getEdenVisibleObjectTypeIds: vi.fn(async () => []),
      setEdenVisibleObjectTypeIds: vi.fn(async (ids: string[]) => ids),
      saveEntry,
      loadEntry,
      listEntries: vi.fn(async () => [...persisted.values()]),
      listAllEntries,
      listNoteTypes: vi.fn(async () => []),
      ensureCollectionObjects: vi.fn(async () => []),
    },
  });

  return { saveEntry, loadEntry, listAllEntries };
}

function byTestId<T extends HTMLElement>(id: string): T {
  const el = document.querySelector<T>(`[data-testid="${id}"]`);
  if (!el) throw new Error(`${id} not found`);
  return el;
}

function mountGeneralSettings(entries: Entry[], failOnceIds?: Set<string>) {
  if (!container) throw new Error("container not initialized");
  const pinia = createPinia();
  setActivePinia(pinia);
  const eden = useEdenStore();
  eden.entries = entries;
  eden.noteTypes = [];
  const api = installApi(entries, failOnceIds);
  app = createApp(GeneralSettings);
  app.use(pinia).use(vaporInteropPlugin);
  app.mount(container);
  return { eden, api };
}

function mountAutoMigration(
  entries: Entry[],
  failOnceIds?: Set<string>,
  configureEden?: (eden: ReturnType<typeof useEdenStore>) => void,
  mutateAfterListAll?: (persisted: Map<string, Entry>) => void,
) {
  if (!container) throw new Error("container not initialized");
  const pinia = createPinia();
  setActivePinia(pinia);
  const eden = useEdenStore();
  eden.entries = entries;
  eden.noteTypes = [];
  eden.vaultPath = "test-vault";
  eden.isInitializing = false;
  eden.isHydratingVault = false;
  const api = installApi(entries, failOnceIds, mutateAfterListAll);
  configureEden?.(eden);
  const Harness = defineComponent({
    setup() {
      useAutoTipTapMigration(eden);
      return () => h("div");
    },
  });
  app = createApp(Harness);
  app.use(pinia).use(vaporInteropPlugin);
  app.mount(container);
  return { eden, api };
}

beforeEach(() => {
  vi.resetModules();
  localStorage.clear();
  document.body.innerHTML = "";
  container = document.createElement("div");
  document.body.appendChild(container);
});

afterEach(() => {
  app?.unmount();
  app = null;
  container?.remove();
  container = null;
});

describe("GeneralSettings", () => {
  test("does not expose manual TipTap migration controls", async () => {
    mountGeneralSettings([makeMarkdownEntry("entry-a", "# Title")]);
    await nextTick();

    expect(document.querySelector('[data-testid="eden-tiptap-migrate-button"]')).toBeNull();
    expect(document.querySelector('[data-testid="eden-tiptap-migrate-confirm"]')).toBeNull();
  });
});

describe("TipTap auto migration", () => {
  test("uses all database entries, not the current visible Eden list", async () => {
    const visibleEntry = makeMarkdownEntry("visible", "visible");
    const hiddenEntry = makeMarkdownEntry("hidden-by-filter", "hidden");
    const { eden, api } = mountAutoMigration([visibleEntry, hiddenEntry]);
    eden.entries = [visibleEntry];

    await expect.poll(() => api.saveEntry).toHaveBeenCalledTimes(2);
    expect(api.saveEntry.mock.calls.map((call) => (call[0] as Entry).id).sort()).toEqual([
      "hidden-by-filter",
      "visible",
    ]);
  });

  test("reports a failed auto migration save", async () => {
    const entry = makeMarkdownEntry("entry-fail", "retry me");
    const { api } = mountAutoMigration([entry], new Set(["entry-fail"]));

    await expect.poll(() => api.saveEntry).toHaveBeenCalledTimes(1);
    expect(api.saveEntry.mock.calls[0]?.[0].id).toBe("entry-fail");
  });

  test("skips an entry that changed in DB after the migration snapshot", async () => {
    const entry = makeMarkdownEntry("entry-changed", "old body");
    const { api } = mountAutoMigration([entry], undefined, undefined, (persisted) => {
      persisted.set(entry.id, makeMarkdownEntry(entry.id, "new body"));
    });

    await expect.poll(() => api.loadEntry).toHaveBeenCalledTimes(1);
    expect(api.saveEntry).not.toHaveBeenCalled();
  });

  test("skips the open dirty entry", async () => {
    const entry = makeMarkdownEntry("entry-dirty", "local draft");
    const { eden, api } = mountAutoMigration([entry], undefined, (eden) => {
      eden.currentEntry = entry;
      eden.updateEntryDraft({
        ...entry,
        content_json: JSON.stringify(writeEntryMarkdown("local draft changed")),
        updated_at: entry.updated_at + 1,
      });
    });
    api.saveEntry.mockClear();

    await expect.poll(() => api.listAllEntries).toHaveBeenCalledTimes(1);
    const staleMarkdown = JSON.stringify(writeEntryMarkdown("local draft"));
    expect(
      api.saveEntry.mock.calls.some(
        (call) =>
          (call[0] as Entry).content_json.includes('"type":"tiptap"') ||
          (call[0] as Entry).content_json === staleMarkdown,
      ),
    ).toBe(false);
    expect(eden.currentEntry?.content_json).toContain("local draft changed");
  });
});
