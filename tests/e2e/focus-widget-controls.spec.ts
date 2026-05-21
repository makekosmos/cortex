// Focus widget — inline controls (Pause / Skip / Stop) для pomodoro и
// stopwatch режимов.
//
// Файлы:
//   - shell/src/views/FocusWidgetView.vue — UI (.ctl-btn[aria-label=...])
//   - shell/electron/focus-widget.ts — `kepler:focus-widget:pomodoro:{pause,
//     resume,skip,stop}` + `kepler:focus-widget:stopwatch:stop`
//   - FocusState расширен `isPaused: boolean`
//
// В headless mode widget BrowserWindow создаётся (show:false) — Playwright
// видит его через `app.windows()` и может click'ать DOM.

import { test, expect } from "@playwright/test";
import { freshDataDir, launchKeplerWithDataDir } from "./helpers/launch";
import {
  findFocusWidgetPage,
  getFocusWidgetState,
  getPomodoroState,
  gracefulQuit,
  setFocusWidgetState,
  startPomodoroViaArk,
} from "./helpers/horologion";
import {
  pomodoroStateFileExists,
  waitForPomodoroStateFileAbsent,
} from "./helpers/pomodoro-state-file";

async function openWidget(
  app: Awaited<ReturnType<typeof launchKeplerWithDataDir>>,
): Promise<Awaited<ReturnType<typeof findFocusWidgetPage>>> {
  // Триггерим setState({active:true}) → widget BrowserWindow lazy-create.
  await setFocusWidgetState(app, {
    active: true,
    remainingSec: 1500,
    label: "Тестовая сессия",
    mode: "work",
    blockingActive: false,
    isPaused: false,
    phaseEndsAtMs: Date.now() + 1_500_000,
  });
  const widget = await findFocusWidgetPage(app, 5_000);
  if (widget) {
    await widget.waitForLoadState("domcontentloaded");
    // Ждём пока Vue смонтирует кнопки.
    await widget.waitForSelector(".ctl-btn", { timeout: 5_000 });
  }
  return widget;
}

test("focus widget: показывает 3 inline кнопки когда active=true (pomodoro mode)", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-buttons-visible");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    const widget = await openWidget(app);
    expect(widget, "focus widget Page не найден").not.toBeNull();

    const buttons = widget!.locator(".controls .ctl-btn");
    await expect(buttons).toHaveCount(3);
    // Pause / Skip / Stop — все aria-label есть.
    await expect(
      widget!.locator('.ctl-btn[aria-label="Пауза"]'),
    ).toBeVisible();
    await expect(
      widget!.locator('.ctl-btn[aria-label="Пропустить фазу"]'),
    ).toBeVisible();
    await expect(
      widget!.locator('.ctl-btn[aria-label="Остановить"]'),
    ).toBeVisible();
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Pause кнопка триггерит backend pomodoro.pause + state.isPaused=true", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-pause");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app, { workMin: 25 });
    // Sync widget state с running pomodoro.
    const widget = await openWidget(app);
    expect(widget).not.toBeNull();

    await widget!.locator('.ctl-btn[aria-label="Пауза"]').click();
    await new Promise((r) => setTimeout(r, 800));

    const pomo = await getPomodoroState(app);
    expect(pomo).not.toBeNull();
    expect(pomo!.isPaused).toBe(true);
    expect(pomo!.phase).toBe("work");
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Skip кнопка двигает phase work → shortBreak", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-skip");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app, { workMin: 25 });
    const widget = await openWidget(app);
    expect(widget).not.toBeNull();

    await widget!.locator('.ctl-btn[aria-label="Пропустить фазу"]').click();
    await new Promise((r) => setTimeout(r, 800));

    const pomo = await getPomodoroState(app);
    expect(pomo).not.toBeNull();
    expect(pomo!.phase).toBe("shortBreak");
    expect(pomo!.completedPomodoros).toBe(1);
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Stop кнопка останавливает pomodoro и удаляет state-файл", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-stop");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app, { workMin: 25 });
    // Дать backend'у пописать persist.
    await new Promise((r) => setTimeout(r, 500));
    expect(pomodoroStateFileExists(dataDir)).toBe(true);

    const widget = await openWidget(app);
    expect(widget).not.toBeNull();
    await widget!.locator('.ctl-btn[aria-label="Остановить"]').click();
    await new Promise((r) => setTimeout(r, 1000));

    const pomo = await getPomodoroState(app);
    expect(pomo).not.toBeNull();
    expect(pomo!.phase).toBe("idle");
    expect(pomo!.isRunning).toBe(false);
    await waitForPomodoroStateFileAbsent(dataDir, 3_000);
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: isPaused прокидывается в state через get-state IPC", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-is-paused-state");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await setFocusWidgetState(app, {
      active: true,
      remainingSec: 1500,
      label: "Тест",
      mode: "work",
      blockingActive: false,
      isPaused: true,
      phaseEndsAtMs: null,
    });
    const state = await getFocusWidgetState(app);
    expect(state).not.toBeNull();
    expect(state!.active).toBe(true);
    // isPaused — newly added field в FocusState.
    expect((state as unknown as { isPaused: boolean }).isPaused).toBe(true);
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Stopwatch mode скрывает Skip кнопку", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-stopwatch-no-skip");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await setFocusWidgetState(app, {
      active: true,
      remainingSec: 0,
      label: "Секундомер",
      mode: "stopwatch",
      blockingActive: false,
      isPaused: false,
      phaseEndsAtMs: null,
    });
    const widget = await findFocusWidgetPage(app, 5_000);
    expect(widget).not.toBeNull();
    await widget!.waitForLoadState("domcontentloaded");
    await widget!.waitForSelector(".ctl-btn", { timeout: 5_000 });

    // Skip отсутствует, Pause + Stop есть.
    await expect(widget!.locator(".controls .ctl-btn")).toHaveCount(2);
    await expect(
      widget!.locator('.ctl-btn[aria-label="Пропустить фазу"]'),
    ).toHaveCount(0);
    await expect(
      widget!.locator('.ctl-btn[aria-label="Пауза"]'),
    ).toBeVisible();
    await expect(
      widget!.locator('.ctl-btn[aria-label="Остановить"]'),
    ).toBeVisible();
  } finally {
    await gracefulQuit(app);
  }
});
