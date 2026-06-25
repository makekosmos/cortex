// Делphi tasks visibility (AC2 + AC5).
//
// Сценарий:
//   1. Launch Kepler с пустой test DB.
//   2. Через launcher's window.kepler.ark.request инжектим один task_obj
//      в ARK (project_id=null, не today). Это эквивалент юзеровской
//      ситуации с «Деструктурирующим присваиванием».
//   3. Открываем Делphi extension.
//   4. Проверяем что задача появилась — она должна попасть в Inbox
//      (predicate: !projectId && !isSomeday && isActive).
//   5. Нет missing field errors.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { waitForBackendReady } from "./helpers/wait";

test("delphi: inbox shows task created via ARK", async () => {
  const app = await launchKepler({ slug: "delphi-tasks-visible" });
  const consoleErrors: string[] = [];
  app.on("window", (w) => {
    w.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    w.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));
  });

  try {
    const launcher = await app.firstWindow();
    await launcher.waitForLoadState("domcontentloaded");
    await waitForBackendReady(launcher);

    // Seed task через launcher's ark bridge.
    const TASK_TITLE = "Test inbox задача";
    const TASK_ID = `task-${Date.now()}-test`;
    const seedResult = await app.evaluate(
      async ({ BrowserWindow }, { id, title }) => {
        const wins = BrowserWindow.getAllWindows();
        const launcher = wins[0];
        if (!launcher) return { ok: false, error: "no launcher" };
        // upsert_object expects { object: { id, typeId, title, ... } }.
        try {
          const result = await launcher.webContents.executeJavaScript(`
            (async () => {
              // FK requires object_types row to exist first.
              await window.kepler.ark.request("upsert_object_type", {
                object_type: {
                  id: "task_obj",
                  name: "Задача",
                  schemaJson: "{}",
                  uiSchemaJson: "{}",
                  systemLocked: false,
                  createdAt: new Date().toISOString(),
                  updatedAt: new Date().toISOString()
                }
              });
              const obj = {
                id: ${JSON.stringify(id)},
                typeId: "task_obj",
                title: ${JSON.stringify(title)},
                contentJson: {},
                propsJson: {
                  is_today: false,
                  is_evening: false,
                  is_someday: false,
                  is_completed: false,
                  is_cancelled: false,
                  is_trashed: false,
                  project_id: null,
                  area_id: null,
                  heading_id: null,
                  scheduled_date: null,
                  priority: 0,
                  tag_ids: [],
                  checklist_items: [],
                  source_app: "test",
                  model_version: 1
                },
                createdAt: new Date().toISOString(),
                updatedAt: new Date().toISOString(),
                deletedAt: null
              };
              return await window.kepler.ark.request("upsert_object", { object: obj });
            })()
          `);
          return { ok: true, result };
        } catch (e) {
          return { ok: false, error: String(e) };
        }
      },
      { id: TASK_ID, title: TASK_TITLE },
    );
    if (!seedResult.ok) {
      throw new Error(`seed failed: ${seedResult.error}`);
    }

    // Open Делphi.
    await app.evaluate(async ({ BrowserWindow }, commandId) => {
      const wins = BrowserWindow.getAllWindows();
      const launcher = wins[0];
      if (!launcher) throw new Error("no launcher");
      await launcher.webContents.executeJavaScript(
        `window.kepler?.commands?.invoke?.(${JSON.stringify(commandId)})`,
      );
    }, "delphi:open");

    const delphiWindow = await app.waitForEvent("window", { timeout: 10_000 });
    await delphiWindow.waitForLoadState("domcontentloaded");

    // delphi:open deep-link'ает в /today; seed-задача не isToday → она в
    // Inbox (/). Переходим явно, иначе assertion на body упрётся в "На сегодня
    // задач нет".
    await delphiWindow.getByText("Входящие", { exact: true }).first().click();
    await expect
      .poll(
        async () => {
          const text = await delphiWindow.locator("body").textContent();
          return text?.includes(TASK_TITLE) === true;
        },
        { timeout: 10_000 },
      )
      .toBe(true);

    const bodyText = (await delphiWindow.locator("body").textContent()) ?? "";

    if (!bodyText.includes(TASK_TITLE)) {
      throw new Error(
        `Делphi не показал задачу «${TASK_TITLE}». Body (300 chars): ${bodyText.slice(0, 300)}`,
      );
    }
    expect(bodyText).toContain(TASK_TITLE);

    const missingFieldErrors = consoleErrors.filter((e) => e.includes("missing field"));
    if (missingFieldErrors.length > 0) {
      throw new Error(
        `${missingFieldErrors.length} missing field errors:\n${missingFieldErrors.join("\n")}`,
      );
    }
  } finally {
    await app.close();
  }
});
