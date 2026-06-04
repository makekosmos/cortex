// Focus widget — inline controls (Pause / Done / More) для pomodoro и
// stopwatch режимов.
//
// Файлы:
//   - shell/src/views/FocusWidgetView.vue — UI (btn + IconButton + aria-labels)
//   - shell/electron/focus-widget.ts — `kepler:focus-widget:pomodoro:{pause,
//     resume,skip,stop}` + native context menu для «Ещё»
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
  waitForPomodoroStateFile,
  waitForPomodoroStateFileAbsent,
} from "./helpers/pomodoro-state-file";

async function openWidget(
  app: Awaited<ReturnType<typeof launchKeplerWithDataDir>>,
): Promise<Awaited<ReturnType<typeof findFocusWidgetPage>>> {
  // Триггерим setState({active:true}) → widget BrowserWindow lazy-create.
  await setFocusWidgetState(app, {
    active: true,
    remainingSec: 1500,
    totalSec: 1500,
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
    await widget.waitForSelector('[aria-label="Пауза"], [aria-label="Продолжить"]', {
      state: "attached",
      timeout: 5_000,
    });
  }
  return widget;
}

test("focus widget: controls появляются только на hover/focus (pomodoro mode)", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-buttons-visible");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    const widget = await openWidget(app);
    expect(widget, "focus widget Page не найден").not.toBeNull();

    const actions = widget!.locator(".actions");
    await expect(actions).toHaveCSS("opacity", "0");

    await expect(widget!.locator(".content")).toHaveCSS("-webkit-app-region", "no-drag");
    await expect(widget!.locator(".drag-handle")).toHaveCSS("-webkit-app-region", "drag");
    // Pause / Done / More — все aria-label есть.
    await widget!.locator(".widget").hover();
    await expect(actions).toHaveCSS("opacity", "1");
    await expect(
      widget!.locator('.btn[aria-label="Пауза"], .btn[aria-label="Продолжить"]'),
    ).toBeVisible();
    await expect(widget!.locator('.btn[aria-label="Выполнено"]')).toBeVisible();
    await expect(widget!.getByRole("button", { name: "Ещё" })).toBeVisible();
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

    await widget!.locator(".widget").hover();
    await widget!.locator('.btn[aria-label="Пауза"]').click();
    await new Promise((r) => setTimeout(r, 800));

    const pomo = await getPomodoroState(app);
    expect(pomo).not.toBeNull();
    expect(pomo!.isPaused).toBe(true);
    expect(pomo!.phase).toBe("work");

    // Regression: 2026-05-30. pause/resume не эмитят backend event, поэтому
    // виджет обязан применить state из RPC response сразу после клика.
    await expect(widget!.locator('.btn[aria-label="Продолжить"]')).toBeVisible();
    const widgetState = await getFocusWidgetState(app);
    expect(widgetState).not.toBeNull();
    expect((widgetState as unknown as { isPaused: boolean }).isPaused).toBe(true);
    expect(widgetState!.phaseEndsAtMs).toBeNull();

    await widget!.locator('.btn[aria-label="Продолжить"]').click();
    await expect(widget!.locator('.btn[aria-label="Пауза"]')).toBeVisible();
    const resumed = await getPomodoroState(app);
    expect(resumed).not.toBeNull();
    expect(resumed!.isPaused).toBe(false);
    expect(resumed!.isRunning).toBe(true);
    const resumedWidgetState = await getFocusWidgetState(app);
    expect(resumedWidgetState).not.toBeNull();
    expect((resumedWidgetState as unknown as { isPaused: boolean }).isPaused).toBe(false);
    expect(resumedWidgetState!.phaseEndsAtMs).not.toBeNull();
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Skip через dropdown меню двигает phase work → shortBreak", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-skip");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app, { workMin: 25 });
    const widget = await openWidget(app);
    expect(widget).not.toBeNull();

    // Skip теперь в native context menu (недоступен из Playwright DOM).
    // Вызываем напрямую через IPC для проверки backend функциональности.
    await widget!.evaluate(() =>
      (window as unknown as Record<string, unknown>).kepler
        ? (
            window as unknown as {
              kepler: { focusWidget: { pomodoro: { skip: () => Promise<void> } } };
            }
          ).kepler.focusWidget.pomodoro.skip()
        : Promise.resolve(),
    );
    await new Promise((r) => setTimeout(r, 800));

    const pomo = await getPomodoroState(app);
    expect(pomo).not.toBeNull();
    expect(pomo!.phase).toBe("shortBreak");
    expect(pomo!.completedPomodoros).toBe(1);
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Done кнопка останавливает pomodoro и удаляет state-файл", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-stop");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await startPomodoroViaArk(app, { workMin: 25 });
    await waitForPomodoroStateFile(dataDir, 5_000);
    expect(pomodoroStateFileExists(dataDir)).toBe(true);

    const widget = await openWidget(app);
    expect(widget).not.toBeNull();
    await widget!.locator(".widget").hover();
    await widget!.locator('.btn[aria-label="Выполнено"]').click();
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
      totalSec: 1500,
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

test("focus widget: backend generic label не перетирает конкретное название", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-label-stability");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await setFocusWidgetState(app, {
      active: true,
      remainingSec: 1500,
      totalSec: 1500,
      label: "Собрать релиз",
      mode: "work",
      blockingActive: false,
      isPaused: false,
      phaseEndsAtMs: Date.now() + 1_500_000,
    });
    await setFocusWidgetState(app, {
      active: true,
      remainingSec: 1499,
      totalSec: 1500,
      label: "Фокус",
      mode: "work",
      blockingActive: false,
      isPaused: false,
      phaseEndsAtMs: Date.now() + 1_499_000,
    });

    const state = await getFocusWidgetState(app);
    expect(state).not.toBeNull();
    // Regression: 2026-05-28. Main-process backend tick не должен сбрасывать renderer label.
    expect(state!.label).toBe("Собрать релиз");
    expect(state!.remainingSec).toBeLessThanOrEqual(1499);
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Stopwatch mode показывает 3 кнопки (Done + More в dropdown)", async () => {
  test.setTimeout(60_000);
  const dataDir = freshDataDir("focus-widget-stopwatch-no-skip");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await new Promise((r) => setTimeout(r, 2500));
    await setFocusWidgetState(app, {
      active: true,
      remainingSec: 0,
      totalSec: 0,
      label: "Секундомер",
      mode: "stopwatch",
      blockingActive: false,
      isPaused: false,
      phaseEndsAtMs: null,
    });
    const widget = await findFocusWidgetPage(app, 5_000);
    expect(widget).not.toBeNull();
    await widget!.waitForLoadState("domcontentloaded");
    await widget!.waitForSelector('[aria-label="Пауза"]', { state: "attached", timeout: 5_000 });

    // В stopwatch всегда 3 кнопки: Pause / Done / More.
    await widget!.locator(".widget").hover();
    await expect(widget!.locator(".controls button")).toHaveCount(3);
    await expect(widget!.locator('.btn[aria-label="Пауза"]')).toBeVisible();
    await expect(widget!.locator('.btn[aria-label="Выполнено"]')).toBeVisible();
    await expect(widget!.locator('.controls button[aria-label="Ещё"]')).toBeVisible();
  } finally {
    await gracefulQuit(app);
  }
});
