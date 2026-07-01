import { describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import TiptapEditor from "../../src/editor-tiptap/TiptapEditor.vue";
import { EMPTY_DOC, makeEntry, markdownEntry } from "./editor-test-helpers";
import { writeEntryMarkdown } from "../../src/editor-content/content";

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
  test("body skeleton uses the same content shell as the editor body", async () => {
    render(TiptapEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave: vi.fn(async () => null), bodyLoading: true },
    });

    await expect.poll(() => document.querySelector(".tiptap-body-skeleton")).not.toBeNull();
    const shell = document.querySelector(".tiptap-body-shell");
    const skeleton = document.querySelector(".tiptap-body-skeleton");

    expect(shell).not.toBeNull();
    expect(skeleton).not.toBeNull();
    expect(shell?.contains(skeleton)).toBe(true);
    expect(document.querySelector(".tiptap-editor-content")).not.toBeNull();
  });

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

  test("markdown heading shortcut turns hashes into a TipTap heading", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await focusBody();
    await userEvent.keyboard("## Heading");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    const firstBlock = parseSavedContent(saved).doc?.content?.[0];
    expect(firstBlock?.type).toBe("heading");
    expect(firstBlock?.attrs?.level).toBe(2);
  });

  test("Backspace at heading start resets it to paragraph", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: {
        entry: makeEntry({
          type: "tiptap",
          version: 1,
          doc: {
            type: "doc",
            content: [
              {
                type: "heading",
                attrs: { level: 2 },
                content: [{ type: "text", text: "Heading" }],
              },
            ],
          },
        }),
        onSave,
        zenMode: false,
      },
    });

    await focusBody();
    const heading = document.querySelector<HTMLElement>(".tiptap-editor-content h2");
    if (!heading) throw new Error("heading not found");
    placeCursorBeforeText(heading, "Heading");
    await userEvent.keyboard("{Backspace}");

    await expect.poll(() => onSave.mock.calls.length, { timeout: 4000 }).toBeGreaterThan(0);
    const saved = onSave.mock.calls.at(-1)?.[0] as Entry;
    expect(parseSavedContent(saved).doc?.content?.[0]?.type).toBe("paragraph");
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
        entry: markdownEntry(
          '```rs\nfn main() {\n  let name = "Rust";\n  println!("Привет, {}!", name);\n}\n```',
        ),
        onSave,
        zenMode: false,
      },
    });

    const languageButton = () =>
      document.querySelector<HTMLButtonElement>(".tiptap-code-language-host button");
    const copyButton = () => document.querySelector<HTMLButtonElement>(".tiptap-code-copy");

    await expect.poll(languageButton).not.toBeNull();
    expect(languageButton()?.textContent).toContain("Rust");
    await expect.poll(() => shikiHighlightedSpanCount(), { timeout: 4000 }).toBeGreaterThan(1);

    await userEvent.click(copyButton()!);
    expect(writeText).toHaveBeenCalledWith(
      '```rust\nfn main() {\n  let name = "Rust";\n  println!("Привет, {}!", name);\n}\n```',
    );

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

  test("partial code block selection is copied as plain text", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: {
        entry: markdownEntry('```ts\nconst name = "eden"\n```'),
        onSave,
        zenMode: false,
      },
    });

    await focusBody();
    await expect
      .poll(() => document.querySelector<HTMLElement>(".tiptap-code-block code"))
      .toBeTruthy();
    const code = document.querySelector<HTMLElement>(".tiptap-code-block code");
    if (!code) throw new Error("code block not found");

    selectTextInside(code, "name");
    const clipboardData = new DataTransfer();
    const event = new ClipboardEvent("copy", {
      bubbles: true,
      cancelable: true,
      clipboardData,
    });

    tiptapBody().dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    expect(clipboardData.getData("text/plain")).toBe("name");
    expect(clipboardData.getData("text/html")).toBe("");
  });

  test("pasted fenced markdown becomes a code block", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave, zenMode: false },
    });

    await focusBody();
    const clipboardData = new DataTransfer();
    clipboardData.setData("text/plain", "```typescript\nconst jhon = () => return something\n```");
    const event = new ClipboardEvent("paste", {
      bubbles: true,
      cancelable: true,
      clipboardData,
    });

    tiptapBody().dispatchEvent(event);

    expect(event.defaultPrevented).toBe(true);
    await expect
      .poll(() => document.querySelector<HTMLElement>(".tiptap-code-block code")?.textContent)
      .toContain("const jhon");
    await expect
      .poll(
        () =>
          document.querySelector<HTMLButtonElement>(".tiptap-code-language-host button")
            ?.textContent,
      )
      .toContain("TypeScript");
  });

  test("code block line wrapping can be toggled", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: {
        entry: markdownEntry(
          '```ts\n  const veryLongLine = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";\n```',
        ),
        onSave,
        zenMode: false,
      },
    });

    const block = () => document.querySelector<HTMLElement>(".tiptap-code-block");
    const scroll = () => document.querySelector<HTMLElement>(".tiptap-code-scroll");
    const wrapButton = () => document.querySelector<HTMLButtonElement>(".tiptap-code-wrap");

    await expect.poll(block).not.toBeNull();
    await expect.poll(scroll).not.toBeNull();
    await expect.poll(wrapButton).not.toBeNull();
    await expect.poll(() => wrapButton()?.hidden).toBe(false);
    expect(scroll()?.classList.contains("kosmos-scroll")).toBe(true);
    expect(block()?.classList.contains("is-wrapped")).toBe(true);

    scroll()?.dispatchEvent(new Event("mouseenter", { bubbles: true }));
    expect(scroll()?.dataset.scrolling).toBe("1");

    const blockWidth = block()!.offsetWidth;
    await userEvent.click(wrapButton()!);
    expect(block()?.classList.contains("is-wrapped")).toBe(false);
    expect(block()!.offsetWidth).toBe(blockWidth);
    await expect
      .poll(() =>
        [...document.querySelectorAll<HTMLElement>(".tiptap-code-block code span")].some((span) =>
          ["block", "inline-block"].includes(getComputedStyle(span).display),
        ),
      )
      .toBe(false);

    await userEvent.click(wrapButton()!);
    expect(block()?.classList.contains("is-wrapped")).toBe(true);
  });

  test("code block is wrapped by default without horizontal overflow", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: {
        entry: markdownEntry("```ts\nconst ok = true;\n```"),
        onSave,
        zenMode: false,
      },
    });

    const wrapButton = () => document.querySelector<HTMLButtonElement>(".tiptap-code-wrap");
    const block = () => document.querySelector<HTMLElement>(".tiptap-code-block");
    await expect.poll(wrapButton).not.toBeNull();
    await expect.poll(block).not.toBeNull();
    expect(block()?.classList.contains("is-wrapped")).toBe(true);
  });

  test("code block that becomes long while editing stays expanded", async () => {
    const onSave = vi.fn(async () => null);
    render(TiptapEditor, {
      props: {
        entry: markdownEntry("```ts\nline 1\n```"),
        onSave,
        zenMode: false,
      },
    });

    const block = () => document.querySelector<HTMLElement>(".tiptap-code-block");
    const code = () => document.querySelector<HTMLElement>(".tiptap-code-block code");
    const expandButton = () => document.querySelector<HTMLButtonElement>(".tiptap-code-expand");

    await expect.poll(code).not.toBeNull();
    await userEvent.click(code()!);
    await userEvent.keyboard(
      Array.from({ length: 20 }, (_, index) => `{Enter}line ${index + 2}`).join(""),
    );

    await expect.poll(() => block()?.classList.contains("is-collapsible")).toBe(true);
    expect(block()?.classList.contains("is-expanded")).toBe(true);
    expect(expandButton()?.hidden).toBe(true);
  });

  test("long code block preview expands from code click", async () => {
    const onSave = vi.fn(async () => null);
    const code = Array.from(
      { length: 22 },
      (_, index) =>
        `line ${index + 1}: const veryLongLine = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";`,
    ).join("\n");
    render(TiptapEditor, {
      props: {
        entry: markdownEntry(`\`\`\`ts\n${code}\n\`\`\``),
        onSave,
        zenMode: false,
      },
    });

    const block = () => document.querySelector<HTMLElement>(".tiptap-code-block");
    const scroll = () => document.querySelector<HTMLElement>(".tiptap-code-scroll");
    const wrapButton = () => document.querySelector<HTMLButtonElement>(".tiptap-code-wrap");
    const expandButton = () => document.querySelector<HTMLButtonElement>(".tiptap-code-expand");

    await expect.poll(() => block()?.classList.contains("is-collapsible")).toBe(true);
    await expect.poll(expandButton).not.toBeNull();
    expect(expandButton()?.hidden).toBe(false);
    expect(expandButton()?.textContent).toBe("");
    expect(expandButton()?.ariaLabel).toBe("Показать полностью");
    expect(wrapButton()?.hidden).toBe(true);

    await userEvent.click(scroll()!);
    expect(block()?.classList.contains("is-expanded")).toBe(true);
    expect(expandButton()?.hidden).toBe(true);
    await expect.poll(() => wrapButton()?.hidden).toBe(false);
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

function selectTextInside(root: HTMLElement, needle: string): void {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let textNode = walker.nextNode() as Text | null;
  while (textNode) {
    const start = textNode.data.indexOf(needle);
    if (start >= 0) {
      const range = document.createRange();
      range.setStart(textNode, start);
      range.setEnd(textNode, start + needle.length);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      return;
    }
    textNode = walker.nextNode() as Text | null;
  }
  throw new Error(`Text "${needle}" not found`);
}

function placeCursorBeforeText(root: HTMLElement, needle: string): void {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let textNode = walker.nextNode() as Text | null;
  while (textNode) {
    const start = textNode.data.indexOf(needle);
    if (start >= 0) {
      const range = document.createRange();
      range.setStart(textNode, start);
      range.collapse(true);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      return;
    }
    textNode = walker.nextNode() as Text | null;
  }
  throw new Error(`Text "${needle}" not found`);
}
