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

test.describe("Eden TaskRef Enter", () => {
  test("Enter в title с текстом создаёт ровно одну новую task", async () => {
    const app = await launchKepler({ slug: "eden-task-enter" });
    try {
      const launcher = await app.firstWindow();
      await launcher.waitForLoadState("domcontentloaded");
      await launcher.waitForTimeout(2500);

      await app.evaluate(async ({ BrowserWindow }) => {
        const win = BrowserWindow.getAllWindows()[0];
        await win!.webContents.executeJavaScript(
          `window.kepler.commands.invoke("eden:open")`,
        );
      });

      const edenWin = await app.waitForEvent("window", { timeout: 10_000 });
      await edenWin.waitForLoadState("domcontentloaded");
      await edenWin.waitForTimeout(2000);

      // Create a note via API
      const noteId = `task-enter-${Date.now()}`;
      await edenWin.evaluate(async (id) => {
        const result = await window.api.saveEntry({
          id,
          title: "Task Enter test",
          content_json: JSON.stringify({ type: "doc", content: [{ type: "paragraph" }] }),
          type_id: "system-type-note",
          folder_id: null,
          header_layout: "default",
          header_props_json: "{}",
          schema_version: 1,
          updated_at: Date.now(),
        } as any);
        if (!result?.ok) throw new Error(`saveEntry failed`);
      }, noteId);

      // Navigate to it via eden store
      await edenWin.evaluate(async (id) => {
        const mod = await import("/src/store/eden.ts");
        const store = mod.useEdenStore();
        await store.refreshEntries();
        await store.navigateTo(id);
      }, noteId).catch(() => {});
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

      // Focus on the input of the first task and type title
      const firstTaskInput = edenWin.locator(".task-ref-title-input").first();
      await firstTaskInput.focus();
      await edenWin.keyboard.type("first task");
      await edenWin.waitForTimeout(300);

      // Press Enter ONCE (no typematic / repeat)
      await edenWin.keyboard.press("Enter");
      await edenWin.waitForTimeout(1500); // give createTask + loadTask time

      // After Enter: exactly 2 task-ref-nodes (original + new), and NO
      // extra empty paragraph inserted by buggy code path.
      taskCount = await edenWin.locator(".task-ref-node").count();
      expect(taskCount, `expected exactly 2 tasks after Enter, got ${taskCount}`).toBe(2);

      // First task should have "first task" title; second should be empty
      const firstTitle = await edenWin.locator(".task-ref-title-input").nth(0).inputValue();
      expect(firstTitle).toBe("first task");

      // Verify no random empty paragraph between tasks (besides taskRef nodes)
      const blockCount = await edenWin.evaluate(() => {
        const pm = document.querySelector(".ProseMirror");
        return pm?.children.length ?? 0;
      });
      // Expect: 2 task-ref-nodes (+ maybe the original empty paragraph from
      // setContent before slash). Allow up to 3 total.
      expect(blockCount).toBeLessThanOrEqual(3);
    } finally {
      await app.close();
    }
  });
});
