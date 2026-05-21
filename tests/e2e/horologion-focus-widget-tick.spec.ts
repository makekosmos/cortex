// Regression: focus widget тикает каждую секунду из main process даже
// когда Horologion renderer окно скрыто/закрыто. Раньше тик жил в
// renderer'е → Chromium throttling замораживал MM:SS, когда окно
// уходило из foreground.
//
// Фикс — `shell/electron/focus-widget.ts::ensureTickTimer`: setInterval
// в main process пересчитывает `remainingSec` из `phaseEndsAtMs`
// wallclock anchor'а и broadcast'ит виджету.
//
// Сценарий:
//   1. Открыть Horologion, запустить pomodoro (без задач — minimal config).
//   2. Получить focus widget state через `kepler:focus-widget:get-state`.
//      Должно быть active=true, remainingSec ~ workMin*60, phaseEndsAtMs != null.
//   3. Скрыть Horologion окно (window.hide() main-side).
//   4. Подождать 3.5s.
//   5. Прочитать state снова → remainingSec должно уменьшиться минимум на 2
//      (без main tick'а оно бы зависло, потому что renderer push'ит).
//   6. Stop pomodoro + cleanup.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import {
  openHorologion,
  getFocusWidgetState,
  gracefulQuit,
} from "./helpers/horologion";

test("horologion focus widget: main-process tick идёт даже когда Horologion окно скрыто", async () => {
  const app = await launchKepler({ slug: "horologion-focus-widget-tick" });
  const consoleErrors: string[] = [];
  app.on("window", (w) => {
    w.on("console", (msg) => {
      if (msg.type() === "error") consoleErrors.push(msg.text());
    });
    w.on("pageerror", (e) => consoleErrors.push(`pageerror: ${e.message}`));
  });

  try {
    const { horo } = await openHorologion(app);

    // Старт pomodoro — Pomodoro mode default, минимальный draft без задач.
    const startBtn = horo.getByRole("button", { name: /Начать сессию/ });
    await expect(startBtn).toBeVisible({ timeout: 3_000 });
    await startBtn.click();

    // Ждём пока backend handshake'нет + first state push + widget create.
    await horo.waitForTimeout(2_000);

    // AC1: focus widget state active + phaseEndsAtMs выставлен.
    const initialState = await getFocusWidgetState(app);
    expect(initialState, "focus widget state не получен из main").not.toBeNull();
    expect(initialState!.active).toBe(true);
    expect(initialState!.phaseEndsAtMs).not.toBeNull();
    expect(initialState!.remainingSec).toBeGreaterThan(0);

    // AC2: убедимся что widget BrowserWindow существует (lazy-created).
    const widgetExists = await app.evaluate(({ BrowserWindow }) => {
      return BrowserWindow.getAllWindows().some((w) => {
        try {
          const url = w.webContents.getURL();
          return url.includes("focus-widget") || url.includes("#focus-widget");
        } catch {
          return false;
        }
      });
    });
    expect(widgetExists, "focus widget BrowserWindow должен быть создан").toBe(true);

    // Скрываем Horologion окно из main — renderer setInterval начинает
    // throttle'иться Chromium'ом. Закрыть нельзя: при close
    // `usePomodoroSession` lifetime обрывается → state мог бы reset'нуться.
    // Hide достаточно чтобы спровоцировать throttle.
    await app.evaluate(({ BrowserWindow }) => {
      const wins = BrowserWindow.getAllWindows();
      for (const w of wins) {
        try {
          const url = w.webContents.getURL();
          if (
            (url.includes("horologion") || url.includes("extensions")) &&
            !url.includes("focus-widget")
          ) {
            w.hide();
            return true;
          }
        } catch {}
      }
      return false;
    });

    // Подождать 3.5s — окно скрыто, renderer throttled.
    // В headless mode skipTaskbar+show:false и так не было visible, но
    // hide() ставит явно невидимое состояние → Chromium policy throttle.
    await new Promise((r) => setTimeout(r, 3500));

    // AC3: state.remainingSec уменьшилось как минимум на 2.
    const afterState = await getFocusWidgetState(app);
    expect(afterState, "focus widget state не получен после hide").not.toBeNull();
    expect(afterState!.active).toBe(true);

    const diff = initialState!.remainingSec - afterState!.remainingSec;
    expect(
      diff,
      `BUG: focus widget tick зависает когда Horologion скрыт; ` +
        `initial.remainingSec=${initialState!.remainingSec}, ` +
        `after.remainingSec=${afterState!.remainingSec}, diff=${diff}`,
    ).toBeGreaterThanOrEqual(2);

    // AC4: phaseEndsAtMs не сбросился в null (это бы убило tick).
    expect(afterState!.phaseEndsAtMs).not.toBeNull();
  } finally {
    await gracefulQuit(app);
  }
});
