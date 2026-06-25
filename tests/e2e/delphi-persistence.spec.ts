// Делphi persistence (репродукция бага юзера).
//
// Сценарий:
//   1. Launch Kepler с пустой test DB (никакого pre-seeded task_obj).
//   2. Открываем Делphi extension через command bus.
//   3. Эмулируем user action «создал задачу через FAB / quick-entry» —
//      вызываем `addTodo` через store напрямую внутри delphi renderer'а
//      (это ровно тот path, который выполняется при click'е по FAB).
//   4. Закрываем Делphi window.
//   5. Снова открываем Делphi через command bus.
//   6. Verify: задача отображается + она реально persisted в ARK
//      (list_objects_by_type вернёт её).
//
// Pre-fix (без ensureTaskObjectTypeRegistered) ARK upsert_object падает с
// FK constraint, потому что `task_obj` тип не существует в свежей DB
// (builtin types в core/ark/crates/ark-core не включают task_obj). Задача остаётся
// в Pinia store (in-memory) → видна до закрытия → исчезает после reopen.
//
// Post-fix shim лениво регистрирует object_type перед первым upsert.

import { test, expect } from "@playwright/test";
import type { Page } from "@playwright/test";
import { launchKepler } from "./helpers/launch";

const TASK_TITLE = "Тест персистентности через FAB";

async function openDelphi(app: Awaited<ReturnType<typeof launchKepler>>): Promise<Page> {
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
  // Дать Vue mount + activateSpace + ARK load завершиться.
  await delphiWindow.waitForTimeout(3000);
  return delphiWindow;
}

test("delphi: задача создана через addTodo сохраняется после reopen", async () => {
  const app = await launchKepler({ slug: "delphi-persistence" });
  const consoleErrors: string[] = [];
  app.on("window", (w) => {
    w.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    w.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));
  });

  try {
    await new Promise((r) => setTimeout(r, 2500));

    // ----- step 1: open Делphi, создаём задачу через electronAPI shim -----
    let delphi = await openDelphi(app);

    const taskId = `task-persist-${Date.now()}`;
    // Эмулируем то, что делает store.addTodo: создаёт TodoItem + вызывает
    // electronAPI.invoke("ark:upsertDelphiTask", todo). Это тот же channel,
    // который вызывается из FAB / quick-entry в реальном UI.
    const upsertResult = await delphi.evaluate(
      async ({ id, title }) => {
        const now = new Date().toISOString();
        const todo = {
          id,
          title,
          notes: null,
          priority: 0,
          scheduledDate: null,
          deadline: null,
          reminderDate: null,
          isToday: false,
          isEvening: false,
          isSomeday: false,
          isCompleted: false,
          completedAt: null,
          isCancelled: false,
          cancelledAt: null,
          isTrashed: false,
          sortOrder: 0,
          createdAt: now,
          headingId: null,
          projectId: null,
          areaId: null,
          tagIds: [],
          checklistItems: [],
          recurrenceRule: null,
          billable: false,
          price: null,
        };
        const api = (
          window as unknown as {
            electronAPI?: { invoke: (c: string, ...a: unknown[]) => Promise<unknown> };
          }
        ).electronAPI;
        if (!api) return { ok: false, reason: "no electronAPI shim" };
        const result = await api.invoke("ark:upsertDelphiTask", todo);
        return { ok: true, result };
      },
      { id: taskId, title: TASK_TITLE },
    );
    expect(upsertResult.ok, `upsert вернул: ${JSON.stringify(upsertResult)}`).toBe(true);
    // Shim должен реально записать в ARK (true), а не silently no-op'нуть (false).
    expect(upsertResult.result, "shim вернул false — task НЕ записан в ARK").toBe(true);

    // Verify задача persisted в ARK через launcher's bridge.
    const arkCheck = await app.evaluate(async ({ BrowserWindow }) => {
      const wins = BrowserWindow.getAllWindows();
      const launcher = wins[0];
      if (!launcher) return { ok: false, count: 0 };
      const list = await launcher.webContents.executeJavaScript(`
        window.kepler.ark.request("list_objects_by_type", { type_id: "task_obj" })
      `);
      return { ok: true, list };
    });
    const arkList = (arkCheck.list ?? []) as Array<{ id: string; title: string }>;
    const matchingArk = arkList.find((o) => o.id === taskId);
    expect(
      matchingArk,
      `ARK не содержит task ${taskId}. ARK list: ${JSON.stringify(arkList)}`,
    ).toBeDefined();
    expect(matchingArk?.title).toBe(TASK_TITLE);

    // ----- step 2: закрываем Делphi window -----
    await delphi.evaluate(() => window.close());
    await new Promise((r) => setTimeout(r, 1000));

    // ----- step 3: reopen Делphi -----
    delphi = await openDelphi(app);
    // Дать ARK load завершиться (activateSpace + ark:listDelphiTasks).
    await delphi.waitForTimeout(2500);

    // delphi:open deep-link'ает в /today; созданная задача isToday=false →
    // показывается в Inbox (/). Переходим явно.
    await delphi.getByText("Входящие", { exact: true }).first().click();
    await delphi.waitForTimeout(800);

    const bodyText = (await delphi.locator("body").textContent()) ?? "";
    if (!bodyText.includes(TASK_TITLE)) {
      throw new Error(
        `После reopen Делphi НЕ показывает задачу «${TASK_TITLE}». ` +
          `Body (300 chars): ${bodyText.slice(0, 300)}`,
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
    await app.evaluate(({ app: a }) => a.quit());
    await Promise.race([
      new Promise<void>((resolve) => app.process().once("exit", () => resolve())),
      new Promise<void>((_, rej) =>
        setTimeout(() => rej(new Error("process exit timeout 10s")), 10_000),
      ),
    ]);
  }
});
