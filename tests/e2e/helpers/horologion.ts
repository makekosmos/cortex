// Shared helpers для e2e тестов Horologion.
//
// Паттерн открытия окна Horologion повторяется в 5+ spec'ах:
// warmup launcher → poll commands.invoke("horologion:open") → wait window.
// Здесь — единый `openHorologion(app)`.
//
// Также сюда выносится seedTask (создание task_obj через ARK напрямую из
// launcher webContents — у extension preload нет прямого upsert_object,
// но launcher preload exposes window.kepler.ark.request).

import { expect, type ElectronApplication, type Page } from "@playwright/test";

export interface OpenHorologionResult {
  /** Renderer window открытого Horologion extension'а. */
  horo: Page;
  /** Главное launcher window — для прямых ARK ops / focusWidget IPC. */
  launcher: Page;
}

export interface OpenOptions {
  /** Дополнительный idle delay после load (default 1500ms — backend WS warmup). */
  postLoadWaitMs?: number;
  /** Базовый warmup перед invoke (default 2000ms). */
  warmupMs?: number;
}

/**
 * Открывает Horologion через command bus с warmup'ом и polling'ом.
 * Возвращает renderer pages launcher + horologion.
 *
 * Идемпотентность не гарантируется — вызывать один раз на test.
 */
export async function openHorologion(
  app: ElectronApplication,
  opts: OpenOptions = {},
): Promise<OpenHorologionResult> {
  const warmupMs = opts.warmupMs ?? 2000;
  const postLoadWaitMs = opts.postLoadWaitMs ?? 1500;

  await new Promise((r) => setTimeout(r, warmupMs));

  const launcher = await app.firstWindow();
  await launcher.waitForLoadState("domcontentloaded");

  await app.evaluate(async ({ BrowserWindow }, commandId) => {
    const wins = BrowserWindow.getAllWindows();
    const l = wins[0];
    if (!l) throw new Error("no launcher window");
    await l.webContents.executeJavaScript(
      `window.kepler?.commands?.invoke?.(${JSON.stringify(commandId)})`,
    );
  }, "horologion:open");

  const horo = await app.waitForEvent("window", { timeout: 10_000 });
  await horo.waitForLoadState("domcontentloaded");
  await horo.waitForTimeout(postLoadWaitMs);

  return { horo, launcher };
}

export interface SeedTaskInput {
  id: string;
  title: string;
}

/**
 * Создаёт `task_obj` через ARK напрямую (launcher preload's
 * `window.kepler.ark.request`). Используется для setup'а stopwatch task
 * tracking тестов — Delphi extension в test'е может быть и не open'нут.
 *
 * Возвращает id созданной задачи (== input.id).
 */
export async function seedTask(
  app: ElectronApplication,
  input: SeedTaskInput,
): Promise<string> {
  const now = new Date().toISOString();
  const record = {
    id: input.id,
    typeId: "task_obj",
    title: input.title,
    contentJson: {},
    propsJson: {
      // Минимальный набор полей под task_obj — Delphi реальный shape
      // богаче, но для seed'а достаточно identity + title.
      done: false,
    },
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
  };

  const result = await app.evaluate(async ({ BrowserWindow }, payload) => {
    const launcher = BrowserWindow.getAllWindows()[0];
    if (!launcher) throw new Error("no launcher window");
    return (await launcher.webContents.executeJavaScript(
      `(async () => {
        try {
          const nowIso = new Date().toISOString();
          await window.kepler.ark.request("upsert_object_type", {
            object_type: {
              id: "task_obj",
              name: "Задача",
              schemaJson: "{}",
              uiSchemaJson: "{}",
              systemLocked: false,
              createdAt: nowIso,
              updatedAt: nowIso,
            },
          });
          await window.kepler.ark.request("upsert_object", { object: ${JSON.stringify(payload)} });
          return "ok";
        } catch (e) {
          return "throw:" + (e && e.message ? e.message : String(e));
        }
      })()`,
    )) as string;
  }, record);

  if (result !== "ok") {
    throw new Error(`seedTask failed: ${result}`);
  }
  return input.id;
}

export interface SeedPomodoroDraftInput {
  title: string;
  tasks: Array<{ id: string; title: string }>;
}

/**
 * Засеивает `pomodoroDraft` в Horologion renderer через реальный UI flow:
 *   1. Очищает input.
 *   2. Печатает `title`.
 *   3. Для каждой задачи: вводит `@<title>` → ждёт menu → Enter.
 *
 * Требуется чтобы task_obj'ы были предварительно созданы (`seedTask`).
 * Возвращает после того как все chip'ы появились в `.pdi__chip`.
 */
export async function seedPomodoroDraft(
  horo: Page,
  draft: SeedPomodoroDraftInput,
): Promise<void> {
  const input = horo.locator(".pdi__input");
  await expect(input).toBeVisible({ timeout: 5_000 });
  await input.click();
  // Очищаем существующие chip'ы — Backspace на пустом input убирает
  // последний chip (см. PomodoroDraftInput onKeyDown).
  for (let i = 0; i < 10; i++) {
    const chipCount = await horo.locator(".pdi__chip").count();
    if (chipCount === 0) break;
    await input.fill("");
    await horo.keyboard.press("Backspace");
    await horo.waitForTimeout(80);
  }
  await input.fill("");

  // Title (без @ — мы их добавим отдельно для каждой задачи).
  if (draft.title) {
    await input.type(draft.title);
  }

  // @-mention каждый task. После pickTask query удаляется из input'а,
  // chip добавляется к tasks.
  for (const task of draft.tasks) {
    await horo.waitForTimeout(60);
    await input.type(" @");
    // Печатаем достаточно символов чтобы task попал в menu top.
    // MentionMenu фильтрует по startsWith / contains — берём первые ~10 chars.
    const query = task.title.slice(0, 12);
    await input.type(query);
    // Ждём пока menu откроется и task попадёт в filtered list.
    const menuItem = horo.locator(`.mention-menu__item:has-text(${JSON.stringify(task.title)})`);
    await expect(menuItem.first()).toBeVisible({ timeout: 3_000 });
    await horo.keyboard.press("Enter");
    // Chip должен материализоваться.
    await expect(
      horo.locator(`.pdi__chip-label:has-text(${JSON.stringify(task.title)})`).first(),
    ).toBeVisible({ timeout: 3_000 });
  }
}

/**
 * Прочитать focus widget state через main process IPC.
 * Достаточно для assert'ов про autonomous tick.
 */
export interface FocusWidgetStateSnapshot {
  active: boolean;
  remainingSec: number;
  label: string;
  mode: string;
  blockingActive: boolean;
  phaseEndsAtMs: number | null;
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

/** Закрыть конкретное BrowserWindow по pid renderer'a / по индексу. */
export async function closeWindowByTitle(
  app: ElectronApplication,
  titleSubstring: string,
): Promise<boolean> {
  return (await app.evaluate(({ BrowserWindow }, needle) => {
    const wins = BrowserWindow.getAllWindows();
    for (const w of wins) {
      const t = w.getTitle();
      if (t && t.toLowerCase().includes(needle.toLowerCase())) {
        w.close();
        return true;
      }
    }
    return false;
  }, titleSubstring)) as boolean;
}

/** Сравнить два snapshot'а focus widget state — diff по remainingSec. */
export function remainingSecDiff(
  before: FocusWidgetStateSnapshot | null,
  after: FocusWidgetStateSnapshot | null,
): number | null {
  if (!before || !after) return null;
  return before.remainingSec - after.remainingSec;
}

/** Graceful app quit с timeout. */
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

/** Минимальная typing helper'а для test name on русском. */
export const t = expect;

/**
 * Найти Playwright Page для focus widget BrowserWindow. Widget создаётся lazy
 * после первого `setFocusState({active:true})`. В headless mode окно живёт без
 * визуального показа — `app.windows()` всё равно возвращает его страницу.
 */
export async function findFocusWidgetPage(
  app: ElectronApplication,
  timeoutMs = 5_000,
): Promise<Page | null> {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    for (const p of app.windows()) {
      try {
        const url = p.url();
        if (url.includes("focus-widget") || url.includes("#focus-widget")) {
          return p;
        }
      } catch {
        /* page might be transitioning */
      }
    }
    await new Promise((r) => setTimeout(r, 150));
  }
  return null;
}

/**
 * Прямое получение state pomodoro session через ARK операцию
 * `pomodoro.get_state` (роутится в backend → PomodoroHost).
 */
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

/**
 * Запуск pomodoro session через ARK op (минуя UI). Удобно когда тест
 * не интересует open flow Horologion. Возвращает state из ответа start.
 */
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

/**
 * Записать patch в focus widget state через main IPC. Эмулирует Horologion
 * push без необходимости открывать его окно.
 */
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

/**
 * Триггерит main-side `kepler:pomodoro:notify-now` IPC handler. В Electron
 * `ipcMain._invokeHandlers` — internal Map с handler'ами,
 * зарегистрированными через `ipcMain.handle(...)`. Доступ через
 * `app.evaluate` (main world). Возвращает true/false handler'а (false в
 * headless by design).
 *
 * Возвращает `null` если handler не зарегистрирован (notifier не setup'нут).
 */
export async function triggerNotifyNow(
  app: ElectronApplication,
  args: { title?: string; body?: string } = {},
): Promise<boolean | null> {
  return (await app.evaluate(async ({ ipcMain }, payload) => {
    // Internal Electron API — Map handler'ов. Стабилен на всех 25+ версиях.
    const ipcAny = ipcMain as unknown as {
      _invokeHandlers?: Map<
        string,
        (event: unknown, ...args: unknown[]) => unknown
      >;
    };
    const handlers = ipcAny._invokeHandlers;
    if (!handlers) return null;
    const h = handlers.get("kepler:pomodoro:notify-now");
    if (!h) return null;
    try {
      const fakeEvent = {};
      const r = await h(fakeEvent, payload);
      return r as boolean;
    } catch {
      return null;
    }
  }, args)) as boolean | null;
}

