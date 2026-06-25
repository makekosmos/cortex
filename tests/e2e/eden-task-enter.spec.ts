// Eden TaskRef Enter behavior — regression test для bug «Enter создаёт
// 2 пустых блока вместо новой task'и».
//
// Воспроизводит:
//   1. Создать заметку
//   2. Вставить TaskRef через `/задача` (или прямой insert)
//   3. Напечатать title
//   4. Press Enter
//   5. Verify: ровно 1 новая TaskRef нода ниже, без extra paragraph

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

test.describe("Eden TaskRef Enter", () => {
  test("Enter в title с текстом создаёт ровно одну новую task", async () => {
    const app = await launchKepler({ slug: "eden-task-enter" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await waitForBackendReady(launcher);

      await app.evaluate(async ({ BrowserWindow }) => {
        const win = BrowserWindow.getAllWindows()[0];
        await win!.webContents.executeJavaScript(`window.kepler.commands.invoke("eden:open")`);
      });

      const edenWin = await app.waitForEvent("window", { timeout: 10_000 });
      await edenWin.waitForLoadState("domcontentloaded");
      edenWin.on("console", (msg) => {
        const text = msg.text();
        if (
          text.includes("eden TaskRef") ||
          text.includes("commitAndCreateNew") ||
          text.includes("AFTER insert")
        ) {
          console.log("[browser]", text);
        }
      });
      await edenWin.waitForTimeout(2000);

      // Create a note via API
      const noteId = `task-enter-${Date.now()}`;
      await edenWin.evaluate(async (id) => {
        const result = await window.api.saveEntry({
          id,
          title: "Task Enter test",
          content_json: JSON.stringify({ type: "doc", content: [{ type: "paragraph" }] }),
          created_at: Date.now(),
          updated_at: Date.now(),
          folder_id: null,
          type_id: "note_obj",
          header_layout: "default",
          header_props_json: "{}",
          schema_version: 1,
        } as any);
        if (!result?.ok) throw new Error(`saveEntry failed: ${JSON.stringify(result)}`);
      }, noteId);

      // Navigate to it via eden store
      await edenWin
        .evaluate(async (id) => {
          const mod = await import("/src/store/eden.ts");
          const store = mod.useEdenStore();
          await store.refreshEntries();
          await store.navigateTo(id);
        }, noteId)
        .catch(() => {});
      await edenWin.waitForTimeout(1000);

      // Insert TaskRef via slash command UI:
      // Type "/задача" in editor, press Enter to select first slash option
      const pm = edenWin.locator(".ProseMirror").first();
      await pm.click();
      await edenWin.keyboard.type("/задача");
      await edenWin.waitForTimeout(500);
      await edenWin.keyboard.press("Enter");
      await edenWin.waitForTimeout(800);

      // Should have 1 task-ref-node + maybe leftover paragraph
      let taskCount = await edenWin.locator(".task-ref-node").count();
      expect(taskCount).toBeGreaterThanOrEqual(1);

      // Focus on the input of the first task and fill title (fill дублирует
      // type events, гарантированно синкает v-model перед Enter).
      const firstTaskInput = edenWin.locator(".task-ref-title-input").first();
      await firstTaskInput.click();
      await edenWin.waitForTimeout(200);
      await firstTaskInput.fill("first task");
      await edenWin.waitForTimeout(300);

      // Sanity: title input should have value
      const titleBeforeEnter = await firstTaskInput.inputValue();
      console.log("[e2e] title before Enter:", titleBeforeEnter);

      // Doc structure + PM doc JSON BEFORE Enter
      const beforeData = await edenWin.evaluate(() => {
        const pm = document.querySelector(".ProseMirror");
        const dom = Array.from(pm?.children ?? []).map((el) => ({
          t: (el as HTMLElement).tagName,
          cls: (el as HTMLElement).className,
        }));
        // Try to get PM doc JSON via global editor reference
        const editor = (window as any).__edenEditor || null;
        const docJson = editor?.state?.doc?.toJSON?.() ?? null;
        return { dom, docJson };
      });
      console.log("[e2e] BEFORE Enter DOM:", JSON.stringify(beforeData.dom));

      // activeElement check
      const activeEl = await edenWin.evaluate(() => {
        const el = document.activeElement as HTMLElement | null;
        return { tag: el?.tagName, cls: el?.className };
      });
      console.log("[e2e] activeElement before Enter:", JSON.stringify(activeEl));

      // Press Enter ON the input element specifically (not just generic keyboard).
      await firstTaskInput.press("Enter");
      await edenWin.waitForTimeout(1500); // give createTask + loadTask time

      // After Enter: exactly 2 task-ref-nodes (original + new), and NO
      // extra empty paragraph inserted by buggy code path.
      taskCount = await edenWin.locator(".task-ref-node").count();
      expect(taskCount, `expected exactly 2 tasks after Enter, got ${taskCount}`).toBe(2);

      // First task should have "first task" title; second should be empty
      const firstTitle = await edenWin.locator(".task-ref-title-input").nth(0).inputValue();
      expect(firstTitle).toBe("first task");

      // Log block structure for diagnosis
      const blockStructure = await edenWin.evaluate(() => {
        const pm = document.querySelector(".ProseMirror");
        if (!pm) return [];
        return Array.from(pm.children).map((el) => {
          const t = (el as HTMLElement).tagName;
          const cls = (el as HTMLElement).className;
          const ti = (el as HTMLElement).getAttribute("data-task-id");
          const text = (el as HTMLElement).textContent?.slice(0, 50);
          return { t, cls, ti, text };
        });
      });
      console.log("[e2e] block structure:", JSON.stringify(blockStructure, null, 2));
      const blockCount = blockStructure.length;
      // Ровно 2: [orig taskRef, new taskRef]. Без trailing paragraph —
      // StarterKit.trailingNode отключён, и наш replaceWith убирает
      // оставшийся от slash-команды empty paragraph.
      expect(blockCount).toBe(2);
    } finally {
      await app.close();
    }
  });
});
