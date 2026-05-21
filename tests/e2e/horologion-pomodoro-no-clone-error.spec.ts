// Regression: `pomodoro.start` IPC call'ом не должен валиться на
// «An object could not be cloned» когда `config.tasks` приходит из
// Vue reactive `pomodoroDraft` (Proxy-обёрнутые элементы).
//
// Фикс — в `extensions/horologion/src/lib/usePomodoroSession.ts::start`:
// `tasksPlain` строится явным map'ом в `{ id: String, title: String }` →
// IPC structured clone больше не падает.
//
// Сценарий:
//   1. Открыть Horologion (Pomodoro mode по умолчанию).
//   2. Создать `task_obj` через ARK напрямую.
//   3. Через @-mention засеять draft.tasks одной задачей.
//   4. Кликнуть «Начать сессию».
//   5. Проверить: state перешёл в phase=work + isRunning, в console.error
//      нет «could not be cloned» / «pomodoroSession] start failed».
//   6. Stop + assert clean teardown.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import {
  openHorologion,
  seedTask,
  seedPomodoroDraft,
  gracefulQuit,
} from "./helpers/horologion";

test("horologion pomodoro: tasks из reactive proxy не валят start с clone error", async () => {
  const app = await launchKepler({
    slug: "horologion-pomodoro-no-clone-error",
  });
  const consoleErrors: string[] = [];
  app.on("window", (w) => {
    w.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    w.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));
  });

  try {
    // Seed task ДО открытия Horologion — чтобы при первом
    // `tasks.list()` он уже был в кэше.
    const taskId = "t-clone-error-1";
    const { horo } = await openHorologion(app);
    await seedTask(app, { id: taskId, title: "Кодинг" });

    // Force tasks refresh — Horologion кэширует первый список.
    await horo.evaluate(async () => {
      const w = window as unknown as {
        horologion?: { tasks?: { list?: () => Promise<unknown[]> } };
      };
      try {
        await w.horologion?.tasks?.list?.();
      } catch {}
    });

    // Драфт: title + 1 задача через @-mention.
    await seedPomodoroDraft(horo, {
      title: "работа",
      tasks: [{ id: taskId, title: "Кодинг" }],
    });

    // Проверяем chip появился.
    await expect(
      horo.locator(`.pdi__chip-label:has-text("Кодинг")`),
    ).toBeVisible({ timeout: 3_000 });

    // Старт — это submit формы → onSubmit → p.start(ctx) → pomodoro.start IPC.
    const startBtn = horo.getByRole("button", { name: /Начать сессию/ });
    await expect(startBtn).toBeVisible({ timeout: 3_000 });
    await startBtn.click();
    await horo.waitForTimeout(2_000);

    // AC: после start phase=work + status «Идёт…».
    await expect(horo.locator(".pomo__phase")).toHaveText("Фокус", {
      timeout: 5_000,
    });
    await expect(horo.locator(".pomo__status")).toHaveText("Идёт…", {
      timeout: 3_000,
    });

    // AC: НИКАКИХ console.error «could not be cloned».
    const cloneErrors = consoleErrors.filter((e) =>
      e.toLowerCase().includes("could not be cloned"),
    );
    if (cloneErrors.length > 0) {
      throw new Error(
        `BUG: ${cloneErrors.length} clone error(s):\n${cloneErrors.join("\n")}`,
      );
    }
    // AC: и `start failed` тоже отсутствует.
    const startFailures = consoleErrors.filter((e) =>
      e.includes("[pomodoroSession] start failed"),
    );
    if (startFailures.length > 0) {
      throw new Error(
        `BUG: pomodoro start failed:\n${startFailures.join("\n")}`,
      );
    }

    // Stop.
    const stopBtn = horo.getByRole("button", { name: /^Стоп/ });
    if (await stopBtn.isVisible().catch(() => false)) {
      await stopBtn.click();
      await horo.waitForTimeout(600);
    }
  } finally {
    await gracefulQuit(app);
  }
});
