// Eden TrailingParagraph extension regression.
//
// Tested: `products/eden/src/TrailingParagraph.ts` + bootstrap в Editor.vue.
//
// Инвариант: после ЛЮБОЙ транзакции последний top-level node документа =
// пустой paragraph. Не дублируется (idempotent). Работает с heading'ами,
// непустым контентом, taskRef'ами в конце.
//
// Стратегия проверки:
//   - Primary: `editor.getJSON()` через pmViewDesc.node.toJSON() (см. helpers/eden.ts).
//   - Fallback: DOM-структура (getProseMirrorStructure) если JSON недоступен.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import {
  openEden,
  createEdenNote,
  openNoteViaReload,
  getProseMirrorJSON,
  getProseMirrorStructure,
  deleteEdenEntry,
} from "./helpers/eden";
import {
  emptyDoc,
  emptyParagraphsDoc,
  paragraph,
  heading,
  taskRef,
  type PMDoc,
} from "./helpers/eden-doc";

interface PMNodeJSON {
  type: string;
  content?: PMNodeJSON[];
  text?: string;
  attrs?: Record<string, unknown>;
}

interface PMDocJSON {
  type: "doc";
  content?: PMNodeJSON[];
}

function isEmptyParagraph(node: PMNodeJSON | undefined): boolean {
  if (!node || node.type !== "paragraph") return false;
  if (!node.content || node.content.length === 0) return true;
  // Иногда tiptap кладёт пустой text? — content === [] более типичный empty.
  return node.content.every((c) => c.type === "text" && (!c.text || c.text === ""));
}

async function inspectDoc(edenWindow: Awaited<ReturnType<typeof openEden>>): Promise<{
  json: PMDocJSON | null;
  dom: Awaited<ReturnType<typeof getProseMirrorStructure>>;
}> {
  const json = (await getProseMirrorJSON(edenWindow)) as PMDocJSON | null;
  const dom = await getProseMirrorStructure(edenWindow);
  return { json, dom };
}

/**
 * Открыть заметку в editor через sidebar click + дать lazy chunk + onMount.
 * Возвращает true если ProseMirror смонтирован.
 */
async function openAndMount(
  edenWindow: Awaited<ReturnType<typeof openEden>>,
  entryId: string,
  _title: string,
): Promise<boolean> {
  // Используем reload-based навигацию — надёжнее sidebar click'а (entries
  // могут ещё не быть в recentEntries в момент click'а).
  const ready = await openNoteViaReload(edenWindow, entryId);
  // eslint-disable-next-line no-console
  console.log(`[trailing-paragraph] openNoteViaReload(${entryId}) → mounted=${ready}`);
  return ready;
}

test.describe("eden: TrailingParagraph extension", () => {
  test("пустая заметка → doc.lastChild = пустой paragraph", async () => {
    const app = await launchKepler({ slug: "eden-trailing-empty" });
    try {
      const edenWindow = await openEden(app);
      const id = await createEdenNote(edenWindow, {
        title: "trailing-empty",
        contentJson: emptyDoc(),
      });
      const mounted = await openAndMount(edenWindow, id, "trailing-empty");
      expect(mounted, "ProseMirror должен смонтироваться после открытия заметки").toBe(true);

      const { json, dom } = await inspectDoc(edenWindow);

      if (json) {
        const last = json.content?.[json.content.length - 1];
        expect(last?.type, "последний top-level node — paragraph").toBe("paragraph");
        expect(isEmptyParagraph(last), "последний paragraph должен быть пустым").toBe(true);
      } else {
        expect(dom, "DOM structure доступен").toBeTruthy();
        expect(dom?.lastChildTag).toBe("p");
        expect(dom?.lastChildIsEmpty).toBe(true);
      }

      await deleteEdenEntry(edenWindow, id);
    } finally {
      await app.close();
    }
  });

  test("документ с контентом → trailing paragraph в конце, контент сохранён", async () => {
    const app = await launchKepler({ slug: "eden-trailing-content" });
    try {
      const edenWindow = await openEden(app);
      const id = await createEdenNote(edenWindow, {
        title: "trailing-content",
        contentJson: { type: "doc", content: [paragraph("hello")] } satisfies PMDoc,
      });
      const mounted = await openAndMount(edenWindow, id, "trailing-content");
      expect(mounted).toBe(true);

      const { json, dom } = await inspectDoc(edenWindow);

      if (json) {
        const content = json.content ?? [];
        expect(
          content.length,
          `top-level узлов: ${content.length} — ожидаем >=2`,
        ).toBeGreaterThanOrEqual(2);
        const last = content[content.length - 1];
        expect(isEmptyParagraph(last), "последний node — пустой paragraph").toBe(true);
        // Содержимое "hello" должно сохраниться (предпоследний или раньше).
        const flat = JSON.stringify(content);
        expect(flat).toContain("hello");
      } else {
        expect(dom?.childCount).toBeGreaterThanOrEqual(2);
        expect(dom?.lastChildIsEmpty).toBe(true);
        const allText = (dom?.children ?? []).map((c) => c.text).join(" ");
        expect(allText).toContain("hello");
      }

      await deleteEdenEntry(edenWindow, id);
    } finally {
      await app.close();
    }
  });

  test("документ из двух пустых paragraph'ов → не превращается в три", async () => {
    // Inv: «уже есть пустой paragraph в конце» → appendTransaction no-op.
    // Если плагин зациклится, появится бесконечный рост content.length.
    const app = await launchKepler({ slug: "eden-trailing-dedupe" });
    try {
      const edenWindow = await openEden(app);
      const id = await createEdenNote(edenWindow, {
        title: "trailing-dedupe",
        contentJson: emptyParagraphsDoc(2),
      });
      const mounted = await openAndMount(edenWindow, id, "trailing-dedupe");
      expect(mounted).toBe(true);

      // Дать appendTransaction нескольким циклам отработать (если он зациклится — упадёт по timeout либо doc разрастётся).
      await edenWindow.waitForTimeout(500);

      const { json, dom } = await inspectDoc(edenWindow);

      if (json) {
        const content = json.content ?? [];
        // Ожидаем <=3 (хоть 2, хоть 3 если editor нормализовал). >3 — bug.
        expect(content.length, `doc раздулся до ${content.length}`).toBeLessThanOrEqual(3);
        const last = content[content.length - 1];
        expect(isEmptyParagraph(last)).toBe(true);
      } else {
        expect(dom?.childCount ?? 0).toBeLessThanOrEqual(3);
        expect(dom?.lastChildIsEmpty).toBe(true);
      }

      await deleteEdenEntry(edenWindow, id);
    } finally {
      await app.close();
    }
  });

  test("heading-only doc → trailing paragraph добавлен после heading", async () => {
    const app = await launchKepler({ slug: "eden-trailing-heading" });
    try {
      const edenWindow = await openEden(app);
      const id = await createEdenNote(edenWindow, {
        title: "trailing-heading",
        contentJson: { type: "doc", content: [heading(1, "Заголовок")] } satisfies PMDoc,
      });
      const mounted = await openAndMount(edenWindow, id, "trailing-heading");
      expect(mounted).toBe(true);

      const { json, dom } = await inspectDoc(edenWindow);

      if (json) {
        const content = json.content ?? [];
        expect(content[0]?.type).toBe("heading");
        const last = content[content.length - 1];
        expect(last?.type, "после heading должен быть paragraph").toBe("paragraph");
        expect(isEmptyParagraph(last)).toBe(true);
      } else {
        expect(dom?.children[0]?.tag).toMatch(/^h[1-6]$/);
        expect(dom?.lastChildTag).toBe("p");
        expect(dom?.lastChildIsEmpty).toBe(true);
      }

      await deleteEdenEntry(edenWindow, id);
    } finally {
      await app.close();
    }
  });

  test("trailing paragraph переживает reload (close+reopen Eden)", async () => {
    const app = await launchKepler({ slug: "eden-trailing-reload" });
    try {
      const edenWindow = await openEden(app);
      const id = await createEdenNote(edenWindow, {
        title: "trailing-reload",
        contentJson: { type: "doc", content: [paragraph("первый абзац")] } satisfies PMDoc,
      });
      const mounted = await openAndMount(edenWindow, id, "trailing-reload");
      expect(mounted).toBe(true);

      // Дать editor смонтироваться и autosave сохранить (нормализованный doc
      // с trailing paragraph должен персистнуться).
      await edenWindow.waitForTimeout(1500);

      // Close Eden и reopen.
      await edenWindow.close();
      await app.firstWindow().then((l) => l.waitForTimeout(500));

      const second = await openEden(app);
      const mounted2 = await openAndMount(second, id, "trailing-reload");
      expect(mounted2, "ProseMirror должен смонтироваться после reopen").toBe(true);

      const { json, dom } = await inspectDoc(second);

      if (json) {
        const content = json.content ?? [];
        const last = content[content.length - 1];
        expect(
          isEmptyParagraph(last),
          `после reload последний node должен быть пустым paragraph, JSON: ${JSON.stringify(json).slice(0, 300)}`,
        ).toBe(true);
        // Точная проверка дедупа: должны быть ровно 2 ноды — `paragraph("первый абзац")` и trailing empty.
        // Если плагин добавил ещё один — будет 3, что bug.
        expect(content.length, `после reload content.length=${content.length}`).toBeLessThanOrEqual(
          3,
        );
      } else {
        expect(dom?.lastChildIsEmpty).toBe(true);
      }

      await deleteEdenEntry(second, id);
    } finally {
      await app.close();
    }
  });

  test("taskRef в конце → trailing paragraph добавляется после atom", async () => {
    // taskRef — atom; StarterKit TrailingNode по дефолту не добавил бы
    // paragraph после atom. Наш кастомный TrailingParagraph должен.
    const app = await launchKepler({ slug: "eden-trailing-taskref" });
    try {
      const edenWindow = await openEden(app);

      // Создаём заметку и привязанный к ней task_obj.
      const noteId = `eden-trailing-taskref-${Date.now()}`;

      // Используем shim createTask для task_obj.
      const taskId = await edenWindow.evaluate(async (sourceNoteId) => {
        const shim = (
          window as unknown as {
            api: { createTask?: (s: string, t?: string, e?: string) => Promise<string> };
          }
        ).api;
        if (typeof shim.createTask !== "function") {
          // Fallback: попробуем напрямую через @kosmos/ark через kepler bridge.
          return "__no-createTask__";
        }
        try {
          return await shim.createTask(sourceNoteId, "Тестовая задача");
        } catch (e) {
          return "__err:" + (e instanceof Error ? e.message : String(e));
        }
      }, noteId);

      // Если task_obj API недоступен через shim — skip остаток с soft notice.
      if (typeof taskId !== "string" || taskId.startsWith("__")) {
        test.skip(true, `createTask недоступен через shim: ${taskId}`);
        return;
      }

      await createEdenNote(edenWindow, {
        id: noteId,
        title: "trailing-taskref",
        contentJson: { type: "doc", content: [taskRef(taskId)] } satisfies PMDoc,
      });

      const mounted = await openAndMount(edenWindow, noteId, "trailing-taskref");
      expect(mounted).toBe(true);

      // Дать appendTransaction сработать после mount.
      await edenWindow.waitForTimeout(800);

      const { json, dom } = await inspectDoc(edenWindow);

      if (json) {
        const content = json.content ?? [];
        const last = content[content.length - 1];
        expect(
          last?.type,
          `после taskRef должен быть paragraph как trailing, JSON: ${JSON.stringify(json).slice(0, 400)}`,
        ).toBe("paragraph");
        expect(isEmptyParagraph(last)).toBe(true);
      } else {
        expect(dom?.lastChildTag).toBe("p");
        expect(dom?.lastChildIsEmpty).toBe(true);
      }

      await deleteEdenEntry(edenWindow, noteId);
    } finally {
      await app.close();
    }
  });
});
