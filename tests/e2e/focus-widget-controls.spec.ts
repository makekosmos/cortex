// Focus widget — inline controls (Pause / Done / More) для pomodoro и
// stopwatch режимов.
//
// Файлы:
//   - platform/desktop/src/views/FocusWidgetView.vue — UI (btn + IconButton + aria-labels)
//   - platform/desktop/electron/focus-widget.ts — `kepler:focus-widget:pomodoro:{pause,
//     resume,skip,stop}` + native context menu для «Ещё»
//   - FocusState расширен `isPaused: boolean`
//
// В headless mode widget BrowserWindow создаётся (show:false) — Playwright
// видит его через `app.windows()` и может click'ать DOM.

import { test, expect } from "@playwright/test";
import { freshDataDir, launchKeplerWithDataDir } from "./helpers/launch";
import {
  type FocusWidgetStateSnapshot,
  type PomodoroStateSnapshot,
  findFocusWidgetPage,
  getFocusWidgetState,
  getPomodoroState,
  gracefulQuit,
  setFocusWidgetState,
  startPomodoroViaArk,
} from "./helpers/focus-widget";
import {
  pomodoroStateFileExists,
  waitForPomodoroStateFile,
  waitForPomodoroStateFileAbsent,
} from "./helpers/pomodoro-state-file";

type KeplerE2eApp = Awaited<ReturnType<typeof launchKeplerWithDataDir>>;

async function waitForRuntimeReady(app: KeplerE2eApp): Promise<void> {
  await expect
    .poll(
      async () =>
        app.evaluate(async ({ BrowserWindow }) => {
          const launcher = BrowserWindow.getAllWindows()[0];
          if (!launcher) return false;
          return launcher.webContents.executeJavaScript(`
            Boolean(
              window.kepler?.focusWidget?.setState &&
              window.kepler?.focusWidget?.getState &&
              window.kepler?.ark?.request
            )
          `);
        }),
      { timeout: 15_000, message: "launcher APIs are ready" },
    )
    .toBe(true);
  await expect
    .poll(
      async () => {
        const state = await getFocusWidgetState(app);
        return Boolean(state && !state.active && state.label);
      },
      { timeout: 15_000, message: "initial focus widget hydration is complete" },
    )
    .toBe(true);
}

function requireValue<T>(value: T | null | undefined, message: string): T {
  if (value == null) throw new Error(message);
  return value;
}

async function waitForPomodoroSnapshot(
  app: KeplerE2eApp,
  expected: Partial<PomodoroStateSnapshot>,
): Promise<PomodoroStateSnapshot> {
  await expect.poll(async () => getPomodoroState(app), { timeout: 10_000 }).toMatchObject(expected);
  return requireValue(await getPomodoroState(app), "pomodoro state не найден");
}

async function waitForWidgetSnapshot(
  app: KeplerE2eApp,
  expected: Partial<FocusWidgetStateSnapshot>,
): Promise<FocusWidgetStateSnapshot> {
  await expect
    .poll(async () => getFocusWidgetState(app), { timeout: 10_000 })
    .toMatchObject(expected);
  return requireValue(await getFocusWidgetState(app), "focus widget state не найден");
}

async function openWidget(
  app: KeplerE2eApp,
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
  const dataDir = freshDataDir("focus-widget-buttons-visible");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await waitForRuntimeReady(app);
    const widget = requireValue(await openWidget(app), "focus widget Page не найден");

    const actions = widget.locator(".actions");
    await expect(actions).toHaveCSS("opacity", "0");

    await expect(widget.locator(".content")).toHaveCSS("-webkit-app-region", "no-drag");
    await expect(widget.locator(".drag-handle")).toHaveCSS("-webkit-app-region", "drag");
    // Pause / Done / More — все aria-label есть.
    await widget.locator(".widget").hover();
    await expect(actions).toHaveCSS("opacity", "1");
    await expect(
      widget.locator('.btn[aria-label="Пауза"], .btn[aria-label="Продолжить"]'),
    ).toBeVisible();
    await expect(widget.locator('.btn[aria-label="Выполнено"]')).toBeVisible();
    await expect(widget.getByRole("button", { name: "Ещё" })).toBeVisible();
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Pause кнопка триггерит backend pomodoro.pause + state.isPaused=true", async () => {
  const dataDir = freshDataDir("focus-widget-pause");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await waitForRuntimeReady(app);
    await startPomodoroViaArk(app, { workMin: 25 });
    // Sync widget state с running pomodoro.
    const widget = requireValue(await openWidget(app), "focus widget Page не найден");

    await widget.locator(".widget").hover();
    await widget.locator('.btn[aria-label="Пауза"]').click();

    await waitForPomodoroSnapshot(app, { isPaused: true, phase: "work" });

    // Regression: 2026-05-30. pause/resume не эмитят backend event, поэтому
    // виджет обязан применить state из RPC response сразу после клика.
    await expect(widget.locator('.btn[aria-label="Продолжить"]')).toBeVisible();
    await waitForWidgetSnapshot(app, { isPaused: true, phaseEndsAtMs: null });

    await widget.locator('.btn[aria-label="Продолжить"]').click();
    await expect(widget.locator('.btn[aria-label="Пауза"]')).toBeVisible();
    await waitForPomodoroSnapshot(app, { isPaused: false, isRunning: true });
    await expect
      .poll(async () => (await getFocusWidgetState(app))?.phaseEndsAtMs ?? 0, { timeout: 10_000 })
      .toBeGreaterThan(0);
    await waitForWidgetSnapshot(app, { isPaused: false });
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Skip через dropdown меню двигает phase work → shortBreak", async () => {
  const dataDir = freshDataDir("focus-widget-skip");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await waitForRuntimeReady(app);
    await startPomodoroViaArk(app, { workMin: 25 });
    const widget = requireValue(await openWidget(app), "focus widget Page не найден");

    // Skip теперь в native context menu (недоступен из Playwright DOM).
    // Вызываем напрямую через IPC для проверки backend функциональности.
    await widget.evaluate(() =>
      (window as unknown as Record<string, unknown>).kepler
        ? (
            window as unknown as {
              kepler: { focusWidget: { pomodoro: { skip: () => Promise<void> } } };
            }
          ).kepler.focusWidget.pomodoro.skip()
        : Promise.resolve(),
    );

    await waitForPomodoroSnapshot(app, { phase: "shortBreak", completedPomodoros: 1 });
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Done кнопка останавливает pomodoro и удаляет state-файл", async () => {
  const dataDir = freshDataDir("focus-widget-stop");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await waitForRuntimeReady(app);
    await startPomodoroViaArk(app, { workMin: 25 });
    await waitForPomodoroStateFile(dataDir, 5_000);
    expect(pomodoroStateFileExists(dataDir)).toBe(true);

    const widget = requireValue(await openWidget(app), "focus widget Page не найден");
    await widget.locator(".widget").hover();
    await widget.locator('.btn[aria-label="Выполнено"]').click();

    await waitForPomodoroSnapshot(app, { phase: "idle", isRunning: false });
    await waitForPomodoroStateFileAbsent(dataDir, 3_000);
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: isPaused прокидывается в state через get-state IPC", async () => {
  const dataDir = freshDataDir("focus-widget-is-paused-state");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await waitForRuntimeReady(app);
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
    // isPaused — newly added field в FocusState.
    await waitForWidgetSnapshot(app, { active: true, isPaused: true });
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: backend generic label не перетирает конкретное название", async () => {
  const dataDir = freshDataDir("focus-widget-label-stability");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await waitForRuntimeReady(app);
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

    // Regression: 2026-05-28. Main-process backend tick не должен сбрасывать renderer label.
    const state = await waitForWidgetSnapshot(app, { label: "Собрать релиз" });
    expect(state.remainingSec).toBeLessThanOrEqual(1499);
  } finally {
    await gracefulQuit(app);
  }
});

test("focus widget: Stopwatch mode показывает 3 кнопки (Done + More в dropdown)", async () => {
  const dataDir = freshDataDir("focus-widget-stopwatch-no-skip");
  const app = await launchKeplerWithDataDir(dataDir);
  try {
    await waitForRuntimeReady(app);
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
    const widget = requireValue(
      await findFocusWidgetPage(app, 5_000),
      "focus widget Page не найден",
    );
    await widget.waitForLoadState("domcontentloaded");
    await widget.waitForSelector('[aria-label="Пауза"]', { state: "attached", timeout: 5_000 });

    // В stopwatch всегда 3 кнопки: Pause / Done / More.
    await widget.locator(".widget").hover();
    await expect(widget.locator(".controls button")).toHaveCount(3);
    await expect(widget.locator('.btn[aria-label="Пауза"]')).toBeVisible();
    await expect(widget.locator('.btn[aria-label="Выполнено"]')).toBeVisible();
    await expect(widget.locator('.controls button[aria-label="Ещё"]')).toBeVisible();
  } finally {
    await gracefulQuit(app);
  }
});
