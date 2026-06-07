// Regression: stopwatch start с задачей через @-mention сохраняет
// `taskId` + `taskTitle` в `time_entry_obj.propsJson`, и ListView
// рендерит chip `@<taskTitle>` через `.row__task` accent-span.
//
// Фикс — `incubator/horologion/src/views/ListView.vue::titleParts()`:
// если `g.taskTitle` есть, но `@<taskTitle>` нет inline в title, всё равно
// добавляем accent prefix.
//
// Сценарий:
//   1. Открыть Horologion → переключиться на «Секундомер».
//   2. Seed task_obj в ARK.
//   3. Засеять `pomodoroDraft` (title + tasks через @-mention).
//   4. Кликнуть «Начать сессию» → таймер запустился.
//   5. ARK direct read: `list_objects_by_type time_entry_obj` →
//      есть running entry с propsJson.taskId === seeded id.
//   6. DOM check: в ListView есть .row__task с текстом seeded title.
//   7. Stop + cleanup.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { openHorologion, seedTask, seedPomodoroDraft, gracefulQuit } from "./helpers/horologion";

interface ArkObjectRecord {
  id: string;
  typeId: string;
  title: string | null;
  contentJson: unknown;
  propsJson: Record<string, unknown> | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
}

test("horologion stopwatch: task привязка через @-mention сохраняется в propsJson и виден chip", async () => {
  const app = await launchKepler({ slug: "horologion-stopwatch-task-link" });
  const consoleErrors: string[] = [];
  app.on("window", (w) => {
    w.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    w.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));
  });

  try {
    const { horo } = await openHorologion(app);
    const taskId = "t-sw-link-1";
    const taskTitle = "testtask";
    await seedTask(app, { id: taskId, title: taskTitle });

    // Force refresh tasks cache.
    await horo.evaluate(async () => {
      const w = window as unknown as {
        horologion?: { tasks?: { list?: () => Promise<unknown[]> } };
      };
      try {
        await w.horologion?.tasks?.list?.();
      } catch {}
    });

    // Driver: засеваем draft пока ещё в pomodoro mode (UI шарит draft).
    await seedPomodoroDraft(horo, {
      title: "работа",
      tasks: [{ id: taskId, title: taskTitle }],
    });

    // Переключаемся на «Секундомер».
    const swTab = horo.getByRole("tab", { name: "Секундомер" });
    await swTab.click();
    // Ждём пока pomo__primary detach'нется (transition).
    await horo.waitForSelector(".pomo__primary", {
      state: "detached",
      timeout: 3_000,
    });

    // Старт секундомера.
    const startBtn = horo.locator(".sw__primary");
    await expect(startBtn).toBeVisible({ timeout: 3_000 });
    await expect(startBtn).toHaveText(/Начать сессию/);
    await startBtn.click();

    // Ждём первый тик + ARK persist round-trip.
    await horo.waitForTimeout(1_500);

    // AC1: timer тикает.
    const timer = horo.locator(".sw__time");
    const text = (await timer.textContent()) ?? "";
    expect(text).not.toBe("00:00:00");

    // AC2: ARK direct read — running entry имеет taskId.
    const arkProbe = (await app.evaluate(async ({ BrowserWindow }) => {
      const launcher = BrowserWindow.getAllWindows()[0];
      if (!launcher) return null;
      return await launcher.webContents.executeJavaScript(
        `(async () => {
          const list = await window.kepler.ark.request("list_objects_by_type", { type_id: "time_entry_obj" });
          return list;
        })()`,
      );
    })) as ArkObjectRecord[] | null;
    expect(arkProbe).not.toBeNull();

    const running = (arkProbe ?? []).filter((o) => {
      const props = (o.propsJson ?? {}) as Record<string, unknown>;
      return !props.endedAt && !o.deletedAt;
    });
    expect(running.length, "должна быть ровно 1 running entry").toBeGreaterThanOrEqual(1);

    const entry = running[0]!;
    const props = (entry.propsJson ?? {}) as Record<string, unknown>;
    expect(
      props.taskId,
      `BUG: taskId не записан в propsJson (got ${JSON.stringify(props.taskId)})`,
    ).toBe(taskId);
    expect(props.taskTitle).toBe(taskTitle);

    // AC3: DOM ListView показывает accent-span `.row__task` с текстом.
    const taskAccent = horo.locator(".row__task", { hasText: taskTitle });
    await expect(taskAccent).toBeVisible({ timeout: 3_000 });

    // Stop.
    await horo.locator(".sw__primary").click();
    await horo.waitForTimeout(600);

    // No clone / IPC errors.
    const fatal = consoleErrors.filter(
      (e) => e.toLowerCase().includes("could not be cloned") || e.includes("missing field"),
    );
    if (fatal.length > 0) {
      throw new Error(`BUG: console errors:\n${fatal.join("\n")}`);
    }
  } finally {
    await gracefulQuit(app);
  }
});
