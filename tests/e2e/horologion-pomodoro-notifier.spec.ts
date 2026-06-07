// Main-process Pomodoro notifier — отправка native OS toast'ов при смене
// фазы independently от того, открыто ли окно Horologion.
//
// Файлы:
//   - platform/desktop/electron/pomodoro-notifier.ts — subscribe на pomodoro_phase_changed
//   - platform/desktop/electron/system-notifications.ts::notify — headless guard
//   - IPC `kepler:pomodoro:notify-now` — тестовый триггер
//
// В headless / test mode `notify()` возвращает false и не материализует
// toast — это design: окна не должны мелькать на экране пользователя.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import { gracefulQuit, startPomodoroViaArk, triggerNotifyNow } from "./helpers/horologion";

test("pomodoro notifier: notify-now IPC возвращает false в headless mode", async () => {
  test.setTimeout(45_000);
  const app = await launchKepler({ slug: "pomodoro-notifier-headless-guard" });
  try {
    // Backend boot — notifier setup'ится после ArkClient ready.
    await new Promise((r) => setTimeout(r, 3000));

    const result = await triggerNotifyNow(app, {
      title: "Тест",
      body: "Проверка headless guard",
    });
    expect(
      result,
      "notify-now IPC handler не зарегистрирован — notifier не setup'нут",
    ).not.toBeNull();
    // KOSMOS_HEADLESS=1 + KOSMOS_TEST_MODE=1 — toast не показывается.
    expect(result).toBe(false);
  } finally {
    await gracefulQuit(app);
  }
});

test("pomodoro notifier: phase_changed event приходит на skip, notifier не падает", async () => {
  test.setTimeout(45_000);
  const app = await launchKepler({ slug: "pomodoro-notifier-phase-changed" });
  const mainConsoleErrors: string[] = [];
  // Collect main-process stderr — notifier должен молча работать.
  const proc = app.process();
  proc.stderr?.on("data", (chunk: Buffer) => {
    const s = chunk.toString();
    // Только реальные errors notifier'а — стандартные warning'и backend'а
    // фильтруем.
    if (s.includes("[pomodoro-notifier]") && s.toLowerCase().includes("error")) {
      mainConsoleErrors.push(s);
    }
  });

  try {
    await new Promise((r) => setTimeout(r, 3000));

    // Start work pomodoro через ARK.
    const startState = await startPomodoroViaArk(app, { workMin: 25 });
    expect(startState).not.toBeNull();
    expect(startState!.phase).toBe("work");

    // Skip work → должен переключиться на shortBreak. На границе backend
    // emit'нет phase_changed (work → shortBreak), notifier поймает,
    // позовёт notify() — в headless mode возврат false но без exception.
    await app.evaluate(async ({ BrowserWindow }) => {
      const launcher = BrowserWindow.getAllWindows()[0];
      if (!launcher) return;
      await launcher.webContents.executeJavaScript(
        `window.kepler.ark.request("pomodoro.skip", null)`,
      );
    });

    await new Promise((r) => setTimeout(r, 1500));

    // Проверяем что state переключился — значит phase_changed event родился.
    const stateAfter = await app.evaluate(async ({ BrowserWindow }) => {
      const launcher = BrowserWindow.getAllWindows()[0];
      if (!launcher) return null;
      return launcher.webContents.executeJavaScript(
        `window.kepler.ark.request("pomodoro.get_state", null)`,
      );
    });
    expect(stateAfter).toMatchObject({ phase: "shortBreak" });

    // notify-now ещё работает — notifier alive после skip cycle.
    const r2 = await triggerNotifyNow(app, {
      title: "После skip",
      body: "notifier alive",
    });
    expect(r2).not.toBeNull();

    expect(mainConsoleErrors, `notifier бросил error'ы:\n${mainConsoleErrors.join("\n")}`).toEqual(
      [],
    );
  } finally {
    await gracefulQuit(app);
  }
});
