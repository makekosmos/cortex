import { describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, nextTick, ref } from "vue";
import CmEditor from "../../src/editor-cm/CmEditor.vue";
import { SYSTEM_TYPE_NOTE, SYSTEM_TYPE_PERSON } from "../../src/lib/systemTypes";
import { useEdenStore } from "../../src/store/eden";

function makeEntry(contentJson: Record<string, unknown>): Entry {
  return {
    id: "cm-test-1",
    title: "Тест",
    content_json: JSON.stringify(contentJson),
    created_at: Date.now(),
    updated_at: Date.now(),
    folder_id: null,
    type_id: null,
    header_layout: null,
    header_props_json: "{}",
    schema_version: 1,
    deleted_at: null,
  };
}

const EMPTY_DOC = { type: "doc", content: [{ type: "paragraph" }] };

function cmContent(): HTMLElement {
  const el = document.querySelector<HTMLElement>(".cm-editor-container .cm-content");
  if (!el)
    throw new Error(".cm-editor-container .cm-content не найден — CodeMirror не смонтирован");
  return el;
}

function cmContainer(): HTMLElement {
  const el = document.querySelector<HTMLElement>(".cm-editor-container");
  if (!el) throw new Error(".cm-editor-container не найден — CodeMirror не смонтирован");
  return el;
}

function markdownEntry(text: string): Entry {
  return makeEntry({ type: "markdown", version: 1, text });
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

async function typeInEditor(text: string): Promise<void> {
  await userEvent.click(cmContent());
  await userEvent.keyboard(text);
}

async function waitForBodyFocus(): Promise<void> {
  await expect.poll(() => document.querySelector(".cm-editor.cm-focused")).not.toBeNull();
}

const VimToggleFixture = defineComponent({
  components: { CmEditor },
  setup() {
    const vimMode = ref(false);
    return {
      entry: makeEntry(EMPTY_DOC),
      vimMode,
      onSave: vi.fn(async () => null),
      enableVim: () => {
        vimMode.value = true;
      },
    };
  },
  template: `
    <div>
      <button type="button" data-testid="enable-vim" @click="enableVim">Включить Vim</button>
      <CmEditor
        :entry="entry"
        :on-save="onSave"
        :zen-mode="false"
        :vim-mode="vimMode"
      />
    </div>
  `,
});

describe("CmEditor component", () => {
  test("монтируется и показывает контент заметки", async () => {
    const entry = makeEntry({
      type: "doc",
      content: [
        {
          type: "heading",
          attrs: { level: 1 },
          content: [{ type: "text", text: "Привет" }],
        },
      ],
    });
    const screen = render(CmEditor, {
      props: { entry, onSave: vi.fn(async () => null), zenMode: false },
    });

    await expect.element(screen.getByTestId("cm-editor-host")).toBeInTheDocument();
    await expect.poll(() => cmContent().textContent).toContain("Привет");
  });

  test("loading state оставляет обычную страницу заметки и заменяет только body на Skeleton", async () => {
    const entry = markdownEntry("Текст ещё грузится");
    const screen = render(CmEditor, {
      props: {
        entry,
        onSave: vi.fn(async () => null),
        zenMode: false,
        bodyLoading: true,
      },
    });

    await expect.element(screen.getByTestId("cm-editor-host")).toBeInTheDocument();
    await expect.element(screen.getByText("Тест")).toBeInTheDocument();
    await expect.element(screen.getByTestId("cm-editor-body-skeleton")).toBeInTheDocument();
    expect(document.querySelector(".cm-editor-container--loading")).not.toBeNull();
  });

  test("открытие и закрытие без правок не вызывает save и не двигает updated_at", async () => {
    const onSave = vi.fn(async () => null);
    const screen = render(CmEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    screen.unmount();

    expect(onSave).not.toHaveBeenCalled();
  });

  test("ввод текста эмитит liveCharCount > 0", async () => {
    const counts: number[] = [];
    render(CmEditor, {
      props: {
        entry: makeEntry(EMPTY_DOC),
        onSave: vi.fn(async () => null),
        zenMode: false,
        onLiveCharCount: (n: number) => counts.push(n),
      },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await typeInEditor("абв");
    await expect.poll(() => counts.some((n) => n > 0)).toBe(true);
  });

  test("ввод текста сразу эмитит body draft до autosave", async () => {
    // Regression: 2026-06-16. Live CodeMirror body must be the local source of
    // truth while debounced remote save is still pending.
    const drafts: Entry[] = [];
    const onSave = vi.fn(async () => null);
    render(CmEditor, {
      props: {
        entry: makeEntry(EMPTY_DOC),
        onSave,
        zenMode: false,
        onEntryDraftChange: (entry: Entry) => drafts.push(entry),
      },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await typeInEditor("ж");

    expect(onSave).not.toHaveBeenCalled();
    expect(drafts.length).toBeGreaterThan(0);
    const parsed = JSON.parse(drafts.at(-1)?.content_json ?? "{}") as { text?: string };
    expect(parsed.text).toContain("ж");
  });

  test("в режиме писателя после открытия body получает каретку на первой строке", async () => {
    const onSave = vi.fn(async () => null);
    render(CmEditor, {
      props: {
        entry: markdownEntry("первая строка\nвторая строка"),
        onSave,
        zenMode: false,
        readerMode: false,
      },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await waitForBodyFocus();
    await userEvent.keyboard("А");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const parsed = JSON.parse(saved.content_json) as { text?: string };
    expect(parsed.text).toMatch(/^Апервая строка/);
  });

  test("переключение из чтеца в писателя ставит каретку на первую строку", async () => {
    const onSave = vi.fn(async () => null);
    const Fixture = defineComponent({
      components: { CmEditor },
      setup() {
        const readerMode = ref(true);
        return {
          entry: markdownEntry("первая строка\nвторая строка"),
          readerMode,
          onSave,
          enableWriter: () => {
            readerMode.value = false;
          },
        };
      },
      template: `
        <div>
          <button type="button" data-testid="enable-writer" @click="enableWriter">Писатель</button>
          <CmEditor :entry="entry" :on-save="onSave" :reader-mode="readerMode" />
        </div>
      `,
    });

    const screen = render(Fixture);
    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await userEvent.click(screen.getByTestId("enable-writer"));
    await waitForBodyFocus();
    await userEvent.keyboard("А");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const parsed = JSON.parse(saved.content_json) as { text?: string };
    expect(parsed.text).toMatch(/^Апервая строка/);
  });

  test("клик по пустому полю под header переводит ввод на последнюю строку", async () => {
    const onSave = vi.fn(async () => null);
    render(CmEditor, {
      props: {
        entry: markdownEntry("первая строка\nпоследняя строка"),
        onSave,
        zenMode: false,
      },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await userEvent.click(cmContainer());
    await userEvent.keyboard(" конец");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const parsed = JSON.parse(saved.content_json) as { text?: string };
    expect(parsed.text).toBe("первая строка\nпоследняя строка конец");
  });

  test("в режиме чтеца markdown-маркер заголовка скрыт", async () => {
    render(CmEditor, {
      props: {
        entry: markdownEntry("# Заголовок\n\nтекст"),
        onSave: vi.fn(async () => null),
        zenMode: false,
        readerMode: true,
      },
    });

    await expect.poll(() => cmContent().textContent).toContain("Заголовок");
    expect(cmContent().textContent).not.toContain("# Заголовок");
  });

  test("без vimMode использует обычную CodeMirror-каретку без кастомного fat layer", async () => {
    render(CmEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave: vi.fn(async () => null), zenMode: false },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await waitForBodyFocus();
    await typeInEditor("а");

    // Zennotes non-Vim path leaves cursor ownership to CodeMirror itself.
    // Eden must not mount its old geometry-based fat cursor layer here.
    expect(document.querySelector(".cm-fat-cursorLayer")).toBeNull();
    expect(document.querySelector(".cm-fat-cursor")).toBeNull();
    expect(document.querySelector(".cm-editor.cm-focused")).not.toBeNull();
    const thinCursor = document.querySelector<HTMLElement>(".cm-cursor-primary");
    if (!thinCursor) throw new Error(".cm-cursor-primary не найден");
  });

  test("при vimMode использует zennotes-style fat cursor из codemirror-vim", async () => {
    render(CmEditor, {
      props: {
        entry: makeEntry(EMPTY_DOC),
        onSave: vi.fn(async () => null),
        zenMode: false,
        vimMode: true,
      },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await expect.poll(() => document.querySelector(".cm-fat-cursorLayer")).toBeNull();
    await userEvent.click(cmContent());
    await userEvent.keyboard("{Escape}");

    await expect.poll(() => document.querySelector<HTMLElement>(".cm-fat-cursor")).not.toBeNull();
    const cursor = document.querySelector<HTMLElement>(".cm-fat-cursor");
    if (!cursor) throw new Error(".cm-fat-cursor не найден");
    const style = getComputedStyle(cursor);
    expect(style.backgroundColor).not.toBe("rgba(0, 0, 0, 0)");
  });

  test("false→true reconfigure включает vim fat cursor без Eden fat layer", async () => {
    const screen = render(VimToggleFixture);

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await typeInEditor("а");

    expect(document.querySelector(".cm-fat-cursorLayer")).toBeNull();
    expect(document.querySelector(".cm-fat-cursor")).toBeNull();

    await userEvent.click(screen.getByTestId("enable-vim"));
    await userEvent.click(cmContent());
    await userEvent.keyboard("{Escape}");

    await expect.poll(() => document.querySelector(".cm-fat-cursorLayer")).toBeNull();
    await expect.poll(() => document.querySelector(".cm-fat-cursor")).not.toBeNull();
  });

  test("в vimMode команда :w вызывает onSave", async () => {
    const onSave = vi.fn(async () => null);
    render(CmEditor, {
      props: {
        entry: makeEntry(EMPTY_DOC),
        onSave,
        zenMode: false,
        vimMode: true,
      },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await userEvent.click(cmContent());
    await userEvent.keyboard("а");
    await userEvent.keyboard("{Escape}:w{Enter}");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
  });

  test("live preview: '- [ ]' на неактивной строке становится чекбоксом, клик переключает в [x]", async () => {
    const onSave = vi.fn(async () => null);
    render(CmEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    // Печатаем таск-строку и уводим курсор на следующую строку,
    // чтобы строка с чекбоксом стала неактивной для live preview.
    await typeInEditor("- [[ ] задача");
    await userEvent.keyboard("{Enter}");

    await expect
      .poll(() => document.querySelector<HTMLInputElement>("input.cm-task-checkbox-input"))
      .not.toBeNull();

    const checkbox = document.querySelector<HTMLInputElement>("input.cm-task-checkbox-input");
    if (!checkbox) throw new Error("чекбокс не найден");
    checkbox.click();

    // Тоггл должен дойти до дока и автосейва: content_json содержит "[x]".
    await expect
      .poll(
        () => {
          const last = onSave.mock.calls.at(-1)?.[0] as Entry | undefined;
          return last ? last.content_json : "";
        },
        { timeout: 4000 },
      )
      .toContain("[x]");
  });

  test("смена типа не откатывается unmount flush'ом CM editor", async () => {
    // Regression: 2026-06-13. Optimistic type change remounts keyed CmEditor;
    // old instance's onBeforeUnmount flush must not save the previous type back.
    const saved: Entry[] = [];
    const Parent = defineComponent({
      components: { CmEditor },
      setup() {
        const entry = ref(makeEntry(EMPTY_DOC));
        entry.value.type_id = SYSTEM_TYPE_NOTE.id;
        return {
          entry,
          noteTypes: [SYSTEM_TYPE_NOTE, SYSTEM_TYPE_PERSON],
          onSave: vi.fn(async (nextEntry: Entry) => {
            saved.push(nextEntry);
            entry.value = nextEntry;
            return { ok: true };
          }),
          onDraft(nextEntry: Entry) {
            entry.value = nextEntry;
          },
        };
      },
      template: `
        <CmEditor
          :key="entry.id + ':' + (entry.type_id ?? '')"
          :entry="entry"
          :note-types="noteTypes"
          :on-save="onSave"
          @entry-draft-change="onDraft"
        />
      `,
    });

    render(Parent);
    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();

    await expect
      .poll(() => document.querySelector("[data-testid='typed-note-field-__object_type'] button"))
      .not.toBeNull();
    await userEvent.click(
      document.querySelector("[data-testid='typed-note-field-__object_type'] button")!,
    );
    await userEvent.click(
      [...document.querySelectorAll<HTMLButtonElement>(".kosmos-dd__option")].find((button) =>
        button.textContent?.includes(SYSTEM_TYPE_PERSON.name),
      )!,
    );

    await expect.poll(() => saved.at(-1)?.type_id, { timeout: 4000 }).toBe(SYSTEM_TYPE_PERSON.id);
    expect(saved.map((entry) => entry.type_id)).not.toContain(SYSTEM_TYPE_NOTE.id);
  });

  test("автосейв вызывает onSave с markdown content_json, содержащим набранный текст", async () => {
    const onSave = vi.fn(async () => null);
    render(CmEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await typeInEditor("привет мир");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const parsed = JSON.parse(saved.content_json) as { type?: string; text?: string };
    expect(parsed.type).toBe("markdown");
    expect(parsed.text).toContain("привет мир");
  });

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
    const loadEntry = vi.fn((entryId: string) => {
      if (entryId === first.id) return firstLoad.promise;
      if (entryId === second.id) return secondLoad.promise;
      return Promise.resolve(undefined);
    });
    Object.defineProperty(window, "api", {
      configurable: true,
      writable: true,
      value: {
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
