import { describe, expect, test, vi } from "vitest";
import { userEvent } from "vitest/browser";
import { render } from "vitest-browser-vue";
import { defineComponent, ref } from "vue";
import CmEditor from "../../src/editor-cm/CmEditor.vue";
import { SYSTEM_TYPE_NOTE, SYSTEM_TYPE_PERSON } from "../../src/lib/systemTypes";

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

async function typeInEditor(text: string): Promise<void> {
  await userEvent.click(cmContent());
  await userEvent.keyboard(text);
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

  test("без vimMode использует обычную CodeMirror-каретку без кастомного fat layer", async () => {
    render(CmEditor, {
      props: { entry: makeEntry(EMPTY_DOC), onSave: vi.fn(async () => null), zenMode: false },
    });

    await expect.poll(() => document.querySelector(".cm-content")).not.toBeNull();
    await typeInEditor("а");

    // Zennotes non-Vim path leaves cursor ownership to CodeMirror itself.
    // Eden must not mount its old geometry-based fat cursor layer here.
    expect(document.querySelector(".cm-fat-cursorLayer")).toBeNull();
    expect(document.querySelector(".cm-fat-cursor")).toBeNull();
    const thinCursor = document.querySelector<HTMLElement>(".cm-cursor-primary");
    if (!thinCursor) throw new Error(".cm-cursor-primary не найден");
    expect(getComputedStyle(thinCursor).display).not.toBe("none");
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

    await userEvent.click(document.querySelector(".typed-object-header__type-dropdown button")!);
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
});
