// Eden clipboard: Ctrl+A → block selection, Ctrl+C → markdown.
//
// Regression test для UX-фичи описанной пользователем 2026-05-22:
// «Ctrl+A выбирает все блоки как drag-select, Ctrl+C по этому выделению
// копирует в markdown. Drag-select + Ctrl+C по конкретным блокам — тоже md».
//
// Воспроизводит реальный пользовательский флоу — focus в title input,
// нажатие Ctrl+A, нажатие Ctrl+C, проверка clipboard через
// Electron clipboard API (browser navigator.clipboard в headless Electron
// под вопросом без user gesture).

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { openEden, openNoteViaReload } from "./helpers/eden";

test.describe("Eden clipboard markdown", () => {
  test("Ctrl+A → Ctrl+C копирует все блоки в markdown", async () => {
    const app = await launchKepler({ slug: "eden-clipboard-ctrl-a" });
    try {
      const edenWindow = await openEden(app);
      const noteId = `eden-clip-${Date.now()}`;
      const contentJson = {
        type: "doc",
        content: [
          {
            type: "heading",
            attrs: { level: 1 },
            content: [{ type: "text", text: "Заголовок" }],
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Первый параграф." }],
          },
          {
            type: "paragraph",
            content: [{ type: "text", text: "Второй параграф." }],
          },
        ],
      };
      await edenWindow.evaluate(
        async ([id, doc]) => {
          await window.api.saveEntry({
            id: id as string,
            title: "clipboard md test",
            type_id: "note_obj",
            content_json: JSON.stringify(doc),
          });
        },
        [noteId, contentJson] as const,
      );

      const mounted = await openNoteViaReload(edenWindow, noteId);
      expect(mounted, "ProseMirror должен смонтироваться после reload").toBe(true);

      // Сценарий 1: focus в editor body, Ctrl+A → Ctrl+C → markdown.
      const pmEl = edenWindow.locator(".ProseMirror").first();
      await pmEl.click();
      await edenWindow.keyboard.press("Control+a");
      // Block selection overlay должен показать selected positions.
      await edenWindow.waitForTimeout(100);
      const selectedCount = await edenWindow.evaluate(() => {
        return document.querySelectorAll(".kepler-block-selected").length;
      });
      // 3 written blocks + 1 trailing paragraph (TrailingParagraph extension
      // добавляет пустой <p> в конце документа автоматически) = 4.
      expect(selectedCount).toBe(4);

      // Ctrl+C → запись в clipboard.
      await edenWindow.keyboard.press("Control+c");
      await edenWindow.waitForTimeout(250);

      const clipboardText = await app.evaluate(({ clipboard }) => clipboard.readText());

      // Markdown должен содержать heading + оба параграфа.
      expect(clipboardText).toContain("# Заголовок");
      expect(clipboardText).toContain("Первый параграф.");
      expect(clipboardText).toContain("Второй параграф.");
      // Подтверждаем что это md (heading `#` префикс), не plain text.
      expect(clipboardText.startsWith("# ")).toBe(true);
    } finally {
      await app.close();
    }
  });

  test("Ctrl+A из title input → block selection + Ctrl+C markdown (не title)", async () => {
    const app = await launchKepler({ slug: "eden-clipboard-ctrl-a-from-title" });
    try {
      const edenWindow = await openEden(app);
      const noteId = `eden-clip-title-${Date.now()}`;
      const contentJson = {
        type: "doc",
        content: [
          {
            type: "paragraph",
            content: [{ type: "text", text: "Привет." }],
          },
        ],
      };
      await edenWindow.evaluate(
        async ([id, doc]) => {
          await window.api.saveEntry({
            id: id as string,
            title: "from title test",
            type_id: "note_obj",
            content_json: JSON.stringify(doc),
          });
        },
        [noteId, contentJson] as const,
      );

      const mounted = await openNoteViaReload(edenWindow, noteId);
      expect(mounted).toBe(true);

      // Focus в title input (не в editor body).
      await edenWindow.locator(".title-input").click();
      // Ctrl+A — наш handler должен перенять (target внутри
      // .editor-wrapper), focus переедет в editor, block-selection все
      // блоки.
      await edenWindow.keyboard.press("Control+a");
      await edenWindow.waitForTimeout(100);
      const selectedCount = await edenWindow.evaluate(
        () => document.querySelectorAll(".kepler-block-selected").length,
      );
      // 1 written + 1 trailing paragraph.
      expect(selectedCount).toBe(2);

      await edenWindow.keyboard.press("Control+c");
      await edenWindow.waitForTimeout(250);
      const clipboardText = await app.evaluate(({ clipboard }) => clipboard.readText());
      expect(clipboardText).toContain("Привет.");
      // Главное что НЕ title ("from title test") — иначе native Ctrl+A
      // в input + Ctrl+C дал бы plain text title.
      expect(clipboardText).not.toContain("from title test");
    } finally {
      await app.close();
    }
  });
});
