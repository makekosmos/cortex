import type { ElectronApplication, Page } from "@playwright/test";

export interface FocusWidgetStateSnapshot {
  active: boolean;
  remainingSec: number;
  totalSec: number;
  label: string;
  mode: string;
  blockingActive: boolean;
  phaseEndsAtMs: number | null;
}

export interface PomodoroStateSnapshot {
  phase: "idle" | "work" | "shortBreak" | "longBreak";
  remainingMs: number;
  totalMs: number;
  completedPomodoros: number;
  isRunning: boolean;
  isPaused: boolean;
  title: string;
  tasks: Array<{ id: string; title: string }>;
  phaseEndsAtMs: number | null;
}

export async function gracefulQuit(app: ElectronApplication): Promise<void> {
  try {
    await app.evaluate(({ app: a }) => a.quit());
  } catch {
    /* already quitting */
  }
  await Promise.race([
    new Promise<void>((resolve) => app.process().once("exit", () => resolve())),
    new Promise<void>((_, rej) =>
      setTimeout(() => rej(new Error("process exit timeout 10s")), 10_000),
    ),
  ]).catch(() => undefined);
}

export async function findFocusWidgetPage(
  app: ElectronApplication,
  timeoutMs = 5_000,
): Promise<Page | null> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    for (const page of app.windows()) {
      try {
        const url = page.url();
        if (url.includes("focus-widget") || url.includes("#focus-widget")) {
          return page;
        }
      } catch {
        /* page might be transitioning */
      }
    }
    await new Promise((resolve) => setTimeout(resolve, 150));
  }
  return null;
}

export async function getFocusWidgetState(
  app: ElectronApplication,
): Promise<FocusWidgetStateSnapshot | null> {
  return (await app.evaluate(async ({ BrowserWindow }) => {
    const launcher = BrowserWindow.getAllWindows()[0];
    if (!launcher) return null;
    const res = await launcher.webContents.executeJavaScript(
      `(async () => {
        try {
          if (!window.kepler?.focusWidget?.getState) return null;
          return await window.kepler.focusWidget.getState();
        } catch (e) { return null; }
      })()`,
    );
    return res ?? null;
  })) as FocusWidgetStateSnapshot | null;
}

export async function setFocusWidgetState(
  app: ElectronApplication,
  patch: Partial<FocusWidgetStateSnapshot & { isPaused: boolean }>,
): Promise<void> {
  await app.evaluate(async ({ BrowserWindow }, p) => {
    const launcher = BrowserWindow.getAllWindows()[0];
    if (!launcher) return;
    await launcher.webContents.executeJavaScript(
      `window.kepler.focusWidget.setState(${JSON.stringify(p)})`,
    );
  }, patch);
}

export async function getPomodoroState(
  app: ElectronApplication,
): Promise<PomodoroStateSnapshot | null> {
  return (await app.evaluate(async ({ BrowserWindow }) => {
    const launcher = BrowserWindow.getAllWindows()[0];
    if (!launcher) return null;
    return (await launcher.webContents.executeJavaScript(
      `(async () => {
        try {
          return await window.kepler.ark.request("pomodoro.get_state", null);
        } catch (e) { return null; }
      })()`,
    )) as PomodoroStateSnapshot | null;
  })) as PomodoroStateSnapshot | null;
}

export async function startPomodoroViaArk(
  app: ElectronApplication,
  config: {
    workMin?: number;
    shortBreakMin?: number;
    longBreakMin?: number;
    pomodorosUntilLongBreak?: number;
    title?: string;
    tasks?: Array<{ id: string; title: string }>;
  } = {},
): Promise<PomodoroStateSnapshot | null> {
  const cfg = {
    workMin: config.workMin ?? 25,
    shortBreakMin: config.shortBreakMin ?? 5,
    longBreakMin: config.longBreakMin ?? 15,
    pomodorosUntilLongBreak: config.pomodorosUntilLongBreak ?? 4,
    title: config.title ?? "",
    tasks: config.tasks ?? [],
  };
  return (await app.evaluate(async ({ BrowserWindow }, payload) => {
    const launcher = BrowserWindow.getAllWindows()[0];
    if (!launcher) return null;
    return (await launcher.webContents.executeJavaScript(
      `(async () => {
        try {
          return await window.kepler.ark.request("pomodoro.start", { config: ${JSON.stringify(payload)} });
        } catch (e) { return null; }
      })()`,
    )) as PomodoroStateSnapshot | null;
  }, cfg)) as PomodoroStateSnapshot | null;
}
