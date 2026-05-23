// Regression: focus widget остаётся видим после окончания work-фазы, когда
// auto_start_break=false. Раньше main process derive (focus-widget.ts) и
// renderer push (usePomodoroSession.ts) считали `widgetActive` через
// `isRunning && phase !== "idle"`. Backend на `finish_phase()` сбрасывает
// `isRunning=false` (фаза переключилась на ShortBreak/LongBreak и ждёт
// ручного Skip/Resume), → widget с `active=false` уходил в hide(). Симптом:
// «pomodoro запущен, виджет показывается, через какое-то время пропадает».
//
// Фикс — widget видим пока phase !== "idle" (то есть пока pomodoro session
// существует). Скрывается только на stop().
//
// Сценарий:
//   1. Запустить pomodoro через ARK (workMin=25, auto_start_break=false default).
//   2. Проверить widget.active === true.
//   3. Вызвать ARK op pomodoro.skip — backend эмитит Finished + PhaseChanged,
//      phase становится ShortBreak с isRunning=false.
//   4. Проверить widget.active всё ещё true, widget BrowserWindow существует.
//   5. Вызвать pomodoro.stop — теперь widget должен скрыться.

import { test, expect } from "@playwright/test";
import { launchKepler } from "./helpers/launch";
import {
  getFocusWidgetState,
  getPomodoroState,
  startPomodoroViaArk,
  gracefulQuit,
} from "./helpers/horologion";
import { waitForBackendReady } from "./helpers/wait";

test("focus widget остаётся видим между фазами (work finished, break ждёт ручного старта)", async () => {
  const app = await launchKepler({ slug: "horologion-focus-widget-between-phases" });

  try {
    const launcher = await app.firstWindow();
    await launcher.waitForLoadState("domcontentloaded");
    await waitForBackendReady(launcher);

    // 1. Старт pomodoro через ARK (минуя UI Horologion'а).
    const started = await startPomodoroViaArk(app, {
      workMin: 25,
      shortBreakMin: 5,
      title: "Regression",
    });
    expect(started?.phase).toBe("work");
    expect(started?.isRunning).toBe(true);

    // Дать main process'у обработать backend event + setupFocusWidgetBackendSync hydrate.
    await new Promise((r) => setTimeout(r, 1_500));

    // 2. Widget active.
    const beforeSkip = await getFocusWidgetState(app);
    expect(beforeSkip, "widget state не получен").not.toBeNull();
    expect(beforeSkip!.active, "widget должен быть видим во время work-фазы").toBe(true);

    // 3. Skip work-фазу — backend перейдёт в ShortBreak с isRunning=false
    //    (auto_start_break=false по дефолту).
    await launcher.evaluate(async () => {
      await window.kepler?.ark?.request("pomodoro.skip");
    });

    // Дождаться broadcast'а Finished + PhaseChanged.
    await new Promise((r) => setTimeout(r, 1_000));

    const stateAfterSkip = await getPomodoroState(app);
    expect(stateAfterSkip?.phase).toBe("shortBreak");
    expect(
      stateAfterSkip?.isRunning,
      "backend по контракту переходит в ShortBreak с isRunning=false когда auto_start_break=false",
    ).toBe(false);

    // 4. AC: widget остаётся видим в межфазном простое.
    const afterSkip = await getFocusWidgetState(app);
    expect(afterSkip, "widget state не получен после skip").not.toBeNull();
    expect(
      afterSkip!.active,
      `BUG: widget исчезает между фазами. phase=${stateAfterSkip?.phase}, isRunning=${stateAfterSkip?.isRunning}, widget.active=${afterSkip!.active}`,
    ).toBe(true);

    // Widget BrowserWindow всё ещё существует.
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
    expect(widgetExists, "focus widget BrowserWindow не должен быть destroyed").toBe(true);

    // 5. После stop — widget уходит (phase=idle).
    await launcher.evaluate(async () => {
      await window.kepler?.ark?.request("pomodoro.stop");
    });
    await new Promise((r) => setTimeout(r, 800));

    const stoppedState = await getPomodoroState(app);
    expect(stoppedState?.phase).toBe("idle");

    const afterStop = await getFocusWidgetState(app);
    expect(afterStop?.active, "после stop pomodoro widget должен скрываться (active=false)").toBe(
      false,
    );
  } finally {
    await gracefulQuit(app);
  }
});
