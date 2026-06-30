import { describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import TiptapEditor from "../../src/editor-tiptap/TiptapEditor.vue";
import { EMPTY_DOC, makeEntry, markdownEntry } from "./cm-editor-test-helpers";
import { writeEntryMarkdown } from "../../src/editor-cm/content";

function tiptapBody(): HTMLElement {
  const el = document.querySelector<HTMLElement>(".tiptap-editor-content .ProseMirror");
  if (!el) throw new Error(".tiptap-editor-content .ProseMirror не найден");
  return el;
}

function titleInput(): HTMLInputElement {
  const el = document.querySelector<HTMLInputElement>(".tiptap-title-input");
  if (!el) throw new Error(".tiptap-title-input не найден");
  return el;
}

async function focusBody(): Promise<void> {
  await expect
    .poll(() => document.querySelector(".tiptap-editor-content .ProseMirror"))
    .not.toBeNull();
  await userEvent.click(tiptapBody());
  await expect.poll(() => document.activeElement === tiptapBody()).toBe(true);
}

function parseSavedContent(entry: Entry): {
  type?: string;
  doc?: { content?: Array<{ type?: string; attrs?: Record<string, unknown> }> };
} {
  return JSON.parse(entry.content_json) as {
    type?: string;
    doc?: {
      content?: Array<{ type?: string; attrs?: Record<string, unknown> }>;
    };
  };
}

describe("TiptapEditor component", () => {
  test("редактирование body автосохраняет content_json как type:tiptap", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await focusBody();
    await userEvent.keyboard("привет tiptap");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const parsed = parseSavedContent(saved);
    expect(parsed.type).toBe("tiptap");
    expect(tiptapBody().textContent).toContain("привет tiptap");
  });

  test("failed save keeps persisted baseline dirty and retries draft on unmount", async () => {
    // Regression: 2026-06-30. SaveEntryResult ok:false is not persisted.
    const onSave = vi
      .fn()
      .mockResolvedValueOnce({
        ok: false,
        reason: "duplicate_title",
        message: "duplicate",
      })
      .mockResolvedValueOnce({ ok: true, entryId: "entry-1" });
    const screen = render(TiptapEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await focusBody();
    await userEvent.keyboard("retry after failed save");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBe(1);
    screen.unmount();

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBe(2);
    const retried = onSave.mock.calls[1]?.[0] as Entry;
    expect(JSON.stringify(parseSavedContent(retried))).toContain("retry after failed save");
  });

  test("structural TipTap-only body changes are saved", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: { entry: markdownEntry("a"), onSave, zenMode: false },
    });

    await focusBody();
    await userEvent.keyboard("{End}{Enter}");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const parsed = parseSavedContent(saved);
    expect(parsed.doc?.content?.length).toBeGreaterThan(1);
  });

  test("изменение title сохраняется по blur", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await userEvent.clear(titleInput());
    await userEvent.type(titleInput(), "Новый заголовок");
    await userEvent.click(tiptapBody());

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    expect(saved.title).toBe("Новый заголовок");
    expect(parseSavedContent(saved).type).toBe("tiptap");
  });

  test("readerMode запрещает редактирование и не вызывает save", async () => {
    const onSave = vi.fn(async () => null);
    const entry = makeEntry(EMPTY_DOC);
    render(TiptapEditor, {
      props: { entry, onSave, zenMode: false, readerMode: true },
    });

    expect(titleInput().readOnly).toBe(true);
    await expect
      .poll(() =>
        document
          .querySelector(".tiptap-editor-content .ProseMirror")
          ?.getAttribute("contenteditable"),
      )
      .toBe("false");

    await userEvent.click(tiptapBody());
    await userEvent.keyboard("нельзя");
    await userEvent.keyboard("/");

    await expect.poll(() => tiptapBody().textContent).not.toContain("нельзя");
    expect(document.querySelector(".tiptap-slash-menu-anchor")).toBeNull();

    await new Promise((resolve) => setTimeout(resolve, 500));
    expect(onSave).not.toHaveBeenCalled();
  });

  test("slash command открывает меню и применяет команду task list", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await focusBody();
    await userEvent.keyboard("/");

    const taskOption = () =>
      [...document.querySelectorAll<HTMLButtonElement>(".kosmos-dd__option")].find((button) =>
        button.textContent?.includes("Задача"),
      ) ?? null;

    await expect.poll(taskOption).not.toBeNull();
    await userEvent.click(taskOption()!);

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const parsed = parseSavedContent(saved);
    expect(parsed.type).toBe("tiptap");
    expect(parsed.doc?.content?.[0]?.type).toBe("taskList");
  });

  test("открывает markdown с link/image после TipTap-конвертации", async () => {
    const onSave = vi.fn(async () => null);
    const entry = {
      ...makeEntry(EMPTY_DOC),
      content_json: JSON.stringify(
        writeEntryMarkdown("[ссылка](https://example.com)\n\n![alt](https://cdn.test/pic.png)"),
      ),
    };

    render(TiptapEditor, {
      props: { entry, onSave, zenMode: false },
    });

    await expect
      .poll(() => document.querySelector<HTMLAnchorElement>(".tiptap-editor-content a")?.href)
      .toBe("https://example.com/");
    await expect
      .poll(() => document.querySelector<HTMLImageElement>(".tiptap-editor-content img")?.src)
      .toBe("https://cdn.test/pic.png");
  });

  test("code block показывает язык, подсветку и copy action", async () => {
    const writeText = vi.fn(async () => undefined);
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: { writeText },
    });
    const onSave = vi.fn(async () => null);

    render(TiptapEditor, {
      props: {
        entry: markdownEntry("```js\nconst jhon = () => return something\n```"),
        onSave,
        zenMode: false,
      },
    });

    const languageButton = () =>
      document.querySelector<HTMLButtonElement>(".tiptap-code-language-host button");
    const copyButton = () => document.querySelector<HTMLButtonElement>(".tiptap-code-copy");

    await expect.poll(languageButton).not.toBeNull();
    expect(languageButton()?.textContent).toContain("JavaScript");
    await expect.poll(() => shikiHighlightedSpanCount(), { timeout: 4000 }).toBeGreaterThan(1);

    await userEvent.click(copyButton()!);
    expect(writeText).toHaveBeenCalledWith("const jhon = () => return something");

    await userEvent.click(languageButton()!);
    const typeScriptOption = () =>
      [...document.querySelectorAll<HTMLButtonElement>(".kosmos-dd__option")].find((button) =>
        button.textContent?.includes("TypeScript"),
      ) ?? null;
    await expect.poll(typeScriptOption).not.toBeNull();
    await userEvent.click(typeScriptOption()!);

    await expect
      .poll(
        () => {
          const saved = onSave.mock.calls.at(-1)?.[0] as Entry | undefined;
          return saved ? parseSavedContent(saved).doc?.content?.[0]?.attrs?.language : "";
        },
        { timeout: 4000 },
      )
      .toBe("typescript");
  });

  test("code block подсвечивает Rust", async () => {
    const onSave = vi.fn(async () => null);

    render(TiptapEditor, {
      props: {
        entry: markdownEntry('```rs\nfn main() { println!("hi"); }\n```'),
        onSave,
        zenMode: false,
      },
    });

    await expect
      .poll(
        () =>
          document.querySelector<HTMLButtonElement>(".tiptap-code-language-host button")
            ?.textContent,
      )
      .toContain("Rust");
    await expect.poll(() => shikiHighlightedSpanCount(), { timeout: 4000 }).toBeGreaterThan(0);
  });
});

function shikiHighlightedSpanCount(): number {
  return document.querySelectorAll(".tiptap-code-block code span[style*='color']").length;
}
