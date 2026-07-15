import { describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import TiptapEditor from "../../src/editor-tiptap/TiptapEditor.vue";
import { EMPTY_DOC, makeEntry, markdownEntry } from "./editor-test-helpers";
import { readEntryMarkdown, writeEntryMarkdown } from "../../src/editor-content/content";
import { SYSTEM_TYPE_BOOK, SYSTEM_TYPE_BOOK_ID } from "../../src/lib/systemTypes";
import { LIVELIB_LIKE_BOOK_PAGE } from "../fixtures/bookMetadataPages";

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
  test("lays out a book cover left of its editable object details", async () => {
    const onSave = vi.fn(async () => null);
    const entry = {
      ...makeEntry(EMPTY_DOC),
      id: "book-layout",
      title: "Море внутри",
      type_id: SYSTEM_TYPE_BOOK_ID,
      header_layout: "inline",
      header_props_json: JSON.stringify({
        author: "Fyodor Dostoevsky, Richard Pevear, Larissa Volokhonsky",
        cover_image:
          "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='200' height='300'%3E%3Crect width='200' height='300' fill='navy'/%3E%3C/svg%3E",
      }),
    };
    const screen = render(TiptapEditor, {
      props: { entry, noteTypes: [SYSTEM_TYPE_BOOK], onSave, zenMode: false },
    });

    const header = screen.getByTestId("typed-note-header");
    await expect.element(header).toHaveClass("is-book");
    expect(
      getComputedStyle(document.querySelector<HTMLElement>(".typed-object-header__inner")!)
        .columnGap,
    ).toBe("36px");
    expect(document.querySelector(".tiptap-title-input")).toBeNull();
    const objectTitle = screen.getByRole("textbox", { name: "Название объекта" });
    await expect.element(objectTitle).toHaveValue("Море внутри");
    expect((await objectTitle.element()).tagName).toBe("TEXTAREA");
    const authorField = screen.getByTestId("typed-note-field-author");
    await expect
      .element(authorField)
      .toHaveValue("Fyodor Dostoevsky, Richard Pevear, Larissa Volokhonsky");
    await vi.waitFor(async () => {
      const textarea = (await authorField.element()) as HTMLTextAreaElement;
      expect(textarea.getBoundingClientRect().height).toBeGreaterThan(24);
      expect(parseFloat(textarea.style.height)).toBe(textarea.scrollHeight);
    });
    const typeButton = document.querySelector<HTMLElement>(
      '[data-testid="typed-note-field-__object_type"] > button',
    );
    expect(typeButton).not.toBeNull();
    expect(getComputedStyle(typeButton!).fontSize).toBe("14px");

    expect(document.querySelector(".typed-object-header__visual")).not.toBeNull();
    expect(document.querySelector(".book-cover__layer")).not.toBeNull();
    expect(document.querySelector('[data-testid="typed-note-field-cover_image"]')).toBeNull();

    const longTitle = "Новое очень длинное название книги, которое переносится на следующую строку";
    await userEvent.fill(objectTitle, longTitle);
    const titleElement = await objectTitle.element();
    const titleStyle = getComputedStyle(titleElement);
    expect(titleElement.getBoundingClientRect().height).toBeGreaterThan(
      Number.parseFloat(titleStyle.lineHeight) * 1.5,
    );
    await userEvent.click(tiptapBody());
    await vi.waitFor(() =>
      expect((onSave.mock.calls.at(-1)?.[0] as Entry | undefined)?.title).toBe(longTitle),
    );
  });

  test("adds a missing book cover from a URL or local image", async () => {
    const onSave = vi.fn(async () => null);
    const entry = {
      ...makeEntry(EMPTY_DOC),
      id: "book-cover-picker",
      title: "Без обложки",
      type_id: SYSTEM_TYPE_BOOK_ID,
      header_layout: "inline",
      header_props_json: JSON.stringify({ author: "Автор" }),
    };
    const screen = render(TiptapEditor, {
      props: { entry, noteTypes: [SYSTEM_TYPE_BOOK], onSave, zenMode: false },
    });

    const coverButton = screen.getByRole("button", { name: "Изменить обложку" });
    await expect.element(coverButton).toHaveTextContent("+");
    expect(document.querySelector(".book-cover__layer")).toBeNull();
    await userEvent.click(coverButton);
    await expect.element(screen.getByRole("dialog")).toBeVisible();

    const urlInput = screen.getByRole("textbox", { name: "Ссылка на изображение" });
    await userEvent.fill(urlInput, "https://example.com/cover.jpg");
    await userEvent.click(screen.getByRole("button", { name: "Сохранить ссылку" }));
    await vi.waitFor(() => {
      const saved = onSave.mock.calls.at(-1)?.[0] as Entry | undefined;
      expect(JSON.parse(saved?.header_props_json ?? "{}").cover_image).toBe(
        "https://example.com/cover.jpg",
      );
    });

    await userEvent.click(coverButton);
    const previousKepler = window.kepler;
    const writeBinary = vi.fn(async () => undefined);
    window.kepler = {
      ...(previousKepler ?? {}),
      userData: {
        ...(previousKepler?.userData ?? {}),
        writeBinary,
        path: vi.fn(async () => "C:\\Kosmos\\Eden"),
      },
    } as NonNullable<typeof window.kepler>;
    try {
      const dropzone = await screen
        .getByRole("region", { name: "Загрузка обложки книги" })
        .element();
      const transfer = new DataTransfer();
      transfer.items.add(
        new File([new Uint8Array([137, 80, 78, 71])], "cover.png", { type: "image/png" }),
      );
      dropzone.dispatchEvent(new DragEvent("drop", { bubbles: true, dataTransfer: transfer }));
      await vi.waitFor(() => {
        const saved = onSave.mock.calls.at(-1)?.[0] as Entry | undefined;
        expect(JSON.parse(saved?.header_props_json ?? "{}").cover_image).toMatch(
          /^C:\\Kosmos\\Eden\\book-covers\\book-cover-picker-[0-9a-f-]+\.png$/,
        );
      });
      expect(writeBinary).toHaveBeenCalledWith(
        expect.stringMatching(/^book-covers\/book-cover-picker-[0-9a-f-]+\.png$/),
        "iVBORw==",
      );
    } finally {
      window.kepler = previousKepler;
    }
  });

  test("deletes a replaced local cover only after the new path is saved", async () => {
    const onSave = vi
      .fn()
      .mockResolvedValueOnce({ ok: false, reason: "write_failed" })
      .mockResolvedValueOnce({ ok: true });
    const deleteFile = vi.fn(async () => true);
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      userData: {
        ...(previousKepler?.userData ?? {}),
        deleteFile,
        path: vi.fn(async () => "C:\\Kosmos\\Eden"),
      },
    } as NonNullable<typeof window.kepler>;

    try {
      const entry = {
        ...makeEntry(EMPTY_DOC),
        id: "book-cover-cleanup",
        title: "Книга",
        type_id: SYSTEM_TYPE_BOOK_ID,
        header_layout: "inline",
        header_props_json: JSON.stringify({
          author: "Автор",
          cover_image: "C:\\Kosmos\\Eden\\book-covers\\old.png",
        }),
      };
      const screen = render(TiptapEditor, {
        props: { entry, noteTypes: [SYSTEM_TYPE_BOOK], onSave, zenMode: false },
      });
      const coverButton = screen.getByRole("button", { name: "Изменить обложку" });

      await userEvent.click(coverButton);
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка на изображение" }),
        "https://example.com/first.jpg",
      );
      await userEvent.click(screen.getByRole("button", { name: "Сохранить ссылку" }));
      await vi.waitFor(() => expect(onSave).toHaveBeenCalledTimes(1), { timeout: 4000 });
      expect(deleteFile).not.toHaveBeenCalled();

      await userEvent.click(coverButton);
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка на изображение" }),
        "https://example.com/second.jpg",
      );
      await userEvent.click(screen.getByRole("button", { name: "Сохранить ссылку" }));
      await vi.waitFor(() => expect(deleteFile).toHaveBeenCalledTimes(1), { timeout: 4000 });
      expect(deleteFile).toHaveBeenCalledWith("book-covers/old.png");
    } finally {
      window.kepler = previousKepler;
    }
  });

  test("replaces selected book metadata and preserves the Markdown body", async () => {
    const onSave = vi.fn(async () => null);
    const previousKepler = window.kepler;
    window.kepler = {
      ...(previousKepler ?? {}),
      bookMetadata: {
        fetchPage: vi.fn(async () => LIVELIB_LIKE_BOOK_PAGE),
      },
    } as NonNullable<typeof window.kepler>;

    try {
      const entry = {
        ...markdownEntry("Личные заметки о книге"),
        id: "book-metadata-import",
        title: "Старое название",
        type_id: SYSTEM_TYPE_BOOK_ID,
        header_layout: "inline",
        header_props_json: JSON.stringify({
          author: "Мой автор",
          cover_image: "C:\\Kosmos\\Eden\\book-covers\\mine.jpg",
        }),
      };
      const screen = render(TiptapEditor, {
        props: { entry, noteTypes: [SYSTEM_TYPE_BOOK], onSave, zenMode: false },
      });

      await userEvent.click(screen.getByRole("button", { name: "Получить данные" }));
      await userEvent.fill(
        screen.getByRole("textbox", { name: "Ссылка или ISBN" }),
        LIVELIB_LIKE_BOOK_PAGE.finalUrl,
      );
      await userEvent.click(screen.getByRole("button", { name: "Найти" }));
      await expect.element(screen.getByTestId("book-metadata-preview")).toBeVisible();
      await userEvent.click(screen.getByRole("button", { name: "Применить" }));

      await vi.waitFor(() => expect(onSave).toHaveBeenCalledTimes(1), { timeout: 4000 });
      const saved = onSave.mock.calls[0]?.[0] as Entry;
      const savedProps = JSON.parse(saved.header_props_json ?? "{}") as Record<string, unknown>;
      expect(saved.title).toBe("Марафон в рай");
      expect(savedProps).toMatchObject({
        author: "Артур Кларк",
        cover_image: "https://www.livelib.ru/storage/covers/marafon-v-raj.jpg",
        isbn: "9780306406157",
        page_count: 352,
        language: "Русский",
        publisher: "АСТ",
        published_date: "2024",
        source_url: LIVELIB_LIKE_BOOK_PAGE.finalUrl,
      });
      expect(readEntryMarkdown(saved.content_json)).toBe("Личные заметки о книге");
    } finally {
      window.kepler = previousKepler;
    }
  });

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

  test("code block handles editor keys without leaving the block", async () => {
    const onSave = vi.fn(async () => null);

    render(TiptapEditor, {
      props: {
        entry: markdownEntry("```ts\nif (ok) {\n  run()\n}\n```"),
        onSave,
        zenMode: false,
      },
    });

    await focusBody();
    const code = document.querySelector<HTMLElement>(".tiptap-code-block code");
    if (!code) throw new Error("code block not found");

    placeCursorAfterText(code, "if (ok) {");
    await userEvent.keyboard("{Enter}");
    expect(code.textContent).toContain("if (ok) {\n  ");

    await userEvent.keyboard("{Tab}");
    expect(code.textContent).toContain("if (ok) {\n    ");
    expect(document.activeElement).toBe(tiptapBody());

    await userEvent.keyboard("/");
    expect(document.querySelector(".tiptap-slash-menu-anchor")).toBeNull();
    expect(code.textContent).toContain("    /");
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

function placeCursorAfterText(root: HTMLElement, needle: string): void {
  const offset = root.textContent?.indexOf(needle) ?? -1;
  if (offset < 0) throw new Error(`Text "${needle}" not found`);
  placeCursorAtTextOffset(root, offset + needle.length);
}

function placeCursorAtTextOffset(root: HTMLElement, offset: number): void {
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  let remaining = offset;
  let textNode = walker.nextNode() as Text | null;
  while (textNode) {
    if (remaining <= textNode.data.length) {
      const range = document.createRange();
      range.setStart(textNode, remaining);
      range.collapse(true);
      const selection = window.getSelection();
      selection?.removeAllRanges();
      selection?.addRange(range);
      return;
    }
    remaining -= textNode.data.length;
    textNode = walker.nextNode() as Text | null;
  }
  throw new Error(`Text offset ${offset} not found`);
}
