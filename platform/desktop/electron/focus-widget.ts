// Focus widget — Spotify-mini-player-style плавающий always-on-top окно,
// показывается во время активной pomodoro сессии. Слева MM:SS countdown,
// справа task label.
//
// Architecture:
//   - Window lazy-create при первом setState с active=true.
//   - State хранится в main process (singleton focusState). Renderer
//     получает через initial `getState()` IPC + push events `state:update`.
//   - Position персистится в kepler-shell-settings.json (focusWidgetBounds).
//   - Listens на backend pomodoro state and renderer focus-session updates.
//   - На active=false (pomodoro stopped) → hide(), но window не destroy
//     чтобы reopen был мгновенным.

import { BrowserWindow, ipcMain, screen, app, Menu } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { assertExtensionSenderHostPermissionIfExtension } from "./extension-host";
import { awaitArkReady } from "./main";
import path from "node:path";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WIDGET_WIDTH = 280;
const WIDGET_HEIGHT = 52;
const STATE_FILENAME = "kepler-focus-widget-state.json";

export interface FocusState {
  active: boolean;
  remainingSec: number;
  totalSec: number;
  label: string;
  mode: "work" | "break" | "stopwatch";
  /** Применён ли активный блоклист (focus mode blocking). Управляет 🛡️ индикатором в widget. */
  blockingActive: boolean;
  /** Pomodoro session на паузе. Виджет показывает Play вместо Pause. Для
      stopwatch / idle всегда false. */
  isPaused: boolean;
  /**
   * Wallclock (Unix ms) когда текущая фаза должна закончиться. null = idle/paused
   * (нет автономного тика). Когда задан и active=true, main process сам
   * пересчитывает remainingSec каждую секунду — поэтому виджет продолжает
   * тикать даже если renderer скрыт / закрыт и Chromium throttle'ит
   * его таймеры. На каждый setState от renderer'а перезаписываем — он
   * authoritative.
   */
  phaseEndsAtMs: number | null;
}

interface PersistedBounds {
  x: number;
  y: number;
}

const DEFAULT_STATE: FocusState = {
  active: false,
  remainingSec: 0,
  totalSec: 0,
  label: "",
  mode: "work",
  blockingActive: false,
  isPaused: false,
  phaseEndsAtMs: null,
};

let widgetWindow: BrowserWindow | null = null;
let currentState: FocusState = { ...DEFAULT_STATE };
let saveTimer: ReturnType<typeof setTimeout> | null = null;
let tickTimer: ReturnType<typeof setInterval> | null = null;

// --- Position persistence ---------------------------------------------------

function statePath(): string {
  return path.join(keplerDataDir(), STATE_FILENAME);
}

function readPersistedBounds(): PersistedBounds | null {
  try {
    const p = statePath();
    if (!existsSync(p)) return null;
    const data = JSON.parse(readFileSync(p, "utf8")) as Partial<PersistedBounds>;
    if (typeof data.x !== "number" || typeof data.y !== "number") return null;
    return { x: data.x, y: data.y };
  } catch {
    return null;
  }
}

function writePersistedBoundsNow(b: PersistedBounds): void {
  try {
    const dir = path.dirname(statePath());
    if (!existsSync(dir)) mkdirSync(dir, { recursive: true });
    const target = statePath();
    const tmp = target + ".tmp";
    writeFileSync(tmp, JSON.stringify(b, null, 2), "utf8");
    renameSync(tmp, target);
  } catch (e) {
    console.error("[focus-widget] save bounds failed:", e);
  }
}

function schedulePersist(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    if (!widgetWindow || widgetWindow.isDestroyed()) return;
    const [x, y] = widgetWindow.getPosition();
    writePersistedBoundsNow({ x, y });
  }, 400);
}

function isOnSomeDisplay(x: number, y: number): boolean {
  const displays = screen.getAllDisplays();
  for (const d of displays) {
    const w = d.workArea;
    if (x >= w.x && x < w.x + w.width && y >= w.y && y < w.y + w.height) return true;
  }
  return false;
}

function defaultPosition(): PersistedBounds {
  const primary = screen.getPrimaryDisplay().workArea;
  return defaultPositionForWorkArea(primary);
}

function defaultPositionForWorkArea(workArea: Electron.Rectangle): PersistedBounds {
  const bottomOffset = 50;
  return {
    x: Math.round(workArea.x + (workArea.width - WIDGET_WIDTH) / 2),
    y: Math.max(workArea.y + 24, workArea.y + workArea.height - WIDGET_HEIGHT - bottomOffset),
  };
}

function resetWidgetPosition(): void {
  const win = ensureWindow();
  const [x, y] = win.getPosition();
  const display = screen.getDisplayNearestPoint({ x, y });
  const pos = defaultPositionForWorkArea(display.workArea);
  win.setPosition(pos.x, pos.y, false);
  writePersistedBoundsNow(pos);
}

// --- Window lifecycle -------------------------------------------------------

function createWidgetWindow(): BrowserWindow {
  const persisted = readPersistedBounds();
  const pos =
    persisted && isOnSomeDisplay(persisted.x, persisted.y) ? persisted : defaultPosition();

  const win = new BrowserWindow({
    width: WIDGET_WIDTH,
    height: WIDGET_HEIGHT,
    x: pos.x,
    y: pos.y,
    show: false,
    frame: false,
    resizable: false,
    movable: true,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    transparent: true,
    backgroundColor: "#00000000",
    roundedCorners: true,
    // focusable: true (default). Раньше было false («не воровать фокус
    // когда показывается»), но на Win32 non-focusable окно не получает
    // WM_NCLBUTTONDOWN для драга → `-webkit-app-region: drag` молча не
    // работал. Show-без-кражи-фокуса всё равно обеспечивается `showInactive()`
    // — окно появляется, фокус остаётся на текущем приложении пользователя.
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: false, // тикает каждую секунду — нельзя throttle'ить
    },
  });

  // Удерживаем поверх even над fullscreen apps (best-effort).
  win.setAlwaysOnTop(true, "screen-saver", 1);

  // Load с hash для FocusWidgetView dispatch в src/main.ts.
  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void win.loadURL(`${devUrl}#focus-widget`);
  } else {
    void win.loadFile(path.join(__dirname, "..", "dist", "index.html"), {
      hash: "focus-widget",
    });
  }

  win.on("move", schedulePersist);

  win.on("closed", () => {
    widgetWindow = null;
  });

  return win;
}

function ensureWindow(): BrowserWindow {
  if (!widgetWindow || widgetWindow.isDestroyed()) {
    widgetWindow = createWidgetWindow();
  }
  return widgetWindow;
}

function showWidget(): void {
  const win = ensureWindow();
  // Headless / test mode: окно живёт логически (state, tick, IPC), но не
  // показывается визуально. Playwright читает state через main process.
  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") {
    return;
  }
  if (!win.isVisible()) win.showInactive();
}

function hideWidget(): void {
  if (widgetWindow && !widgetWindow.isDestroyed() && widgetWindow.isVisible()) {
    widgetWindow.hide();
  }
}

function broadcastState(): void {
  if (widgetWindow && !widgetWindow.isDestroyed()) {
    try {
      widgetWindow.webContents.send("kepler:focus-widget:state", currentState);
    } catch {
      /* ignore */
    }
  }
}

// --- Autonomous tick --------------------------------------------------------
//
// Renderer пушит state на каждой смене целой секунды. Когда
// окно скрыто или закрыто, Chromium агрессивно throttle'ит
// его setInterval (вплоть до полной остановки), и pushFocusWidgetState
// перестаёт приходить → виджет «замерзает». Чтобы это пережить, main
// process сам пересчитывает remainingSec из wallclock anchor'а
// (phaseEndsAtMs) и broadcast'ит в виджет каждую секунду.
//
// Renderer всё ещё source of truth: каждый его setState перезатирает
// phaseEndsAtMs / remainingSec / label / mode. Tick только заполняет
// промежутки, когда renderer молчит.

function recomputeRemainingFromAnchor(): boolean {
  if (currentState.phaseEndsAtMs == null) return false;
  const next = Math.max(0, Math.ceil((currentState.phaseEndsAtMs - Date.now()) / 1000));
  if (currentState.remainingSec === next) return false;
  currentState.remainingSec = next;
  return true;
}

function ensureTickTimer(): void {
  if (tickTimer != null) return;
  tickTimer = setInterval(() => {
    if (!currentState.active || currentState.phaseEndsAtMs == null) {
      clearTickTimer();
      return;
    }
    if (recomputeRemainingFromAnchor()) {
      broadcastState();
    }
  }, 1000);
}

function clearTickTimer(): void {
  if (tickTimer != null) {
    clearInterval(tickTimer);
    tickTimer = null;
  }
}

function isDefaultFocusLabel(label: string | undefined): boolean {
  const normalized = (label ?? "").trim();
  return normalized === "" || normalized === "Фокус" || normalized === "Перерыв";
}

// --- Public API -------------------------------------------------------------

export function setFocusState(next: Partial<FocusState>): void {
  const previousLabel = currentState.label;
  const shouldKeepCurrentLabel =
    currentState.active &&
    next.active !== false &&
    next.mode === currentState.mode &&
    isDefaultFocusLabel(next.label) &&
    !isDefaultFocusLabel(currentState.label);

  currentState = { ...currentState, ...next };
  if (shouldKeepCurrentLabel) {
    // См. postmortems.md § 2026-05-28: backend ticks may carry only generic UI context.
    currentState.label = previousLabel;
  }
  // active=true → ensure widget shown.
  // active=false → hide (но не destroy, чтобы reopen был быстрым).
  if (currentState.active) {
    showWidget();
  } else {
    hideWidget();
  }
  // Start/stop autonomous tick. Без него, когда окно скрыто,
  // setInterval в renderer'е throttle'ится Chromium'ом → MM:SS замерзает.
  if (currentState.active && currentState.phaseEndsAtMs != null) {
    // Сразу пересчитаем — renderer мог прислать stale remainingSec
    // (он считает на 30fps, мы хотим целую секунду по wallclock).
    recomputeRemainingFromAnchor();
    ensureTickTimer();
  } else {
    clearTickTimer();
  }
  broadcastState();
}

export function getFocusState(): FocusState {
  return { ...currentState };
}

// --- IPC --------------------------------------------------------------------

ipcMain.handle("kepler:focus-widget:set-state", (e, patch: Partial<FocusState>) => {
  assertExtensionSenderHostPermissionIfExtension(e.sender, "focus.control");
  if (!patch || typeof patch !== "object") return;
  setFocusState(patch);
});

ipcMain.handle("kepler:focus-widget:get-state", () => getFocusState());

ipcMain.handle("kepler:focus-widget:hide", () => {
  hideWidget();
});

ipcMain.handle("kepler:focus-widget:open-focus-session", async () => {
  const { openFocusSessionShell } = await import("./focus-session");
  openFocusSessionShell();
});

ipcMain.handle("kepler:focus-widget:show-menu", () => {
  const win = widgetWindow;
  if (!win || win.isDestroyed()) return;
  const menu = Menu.buildFromTemplate([
    {
      label: "Редактировать",
      click: () => {
        void import("./focus-session").then(({ openFocusSessionShell }) => {
          openFocusSessionShell();
        });
      },
    },
    {
      label: "Пропустить сессию",
      click: () => {
        void invokePomodoro("skip");
      },
    },
    {
      label: "Сбросить позицию",
      click: () => {
        resetWidgetPosition();
      },
    },
    { type: "separator" },
    {
      label: "Скрыть виджет",
      click: () => {
        hideWidget();
      },
    },
  ]);
  menu.popup({ window: win });
});

// --- Inline controls (Pause / Resume / Skip / Stop) -------------------------
//
// IPC от FocusWidgetView'ы. Все pomodoro операции идут через kepler-backend
// (PomodoroHost): он source of truth для session lifecycle. После успешной
// op backend сам шлёт `phase_changed` event, а focus-session renderer
// запушит свежий patch в виджет. Поэтому
// здесь дополнительно setFocusState() не дёргаем.
//
// Stopwatch stop — отдельный путь: pomodoro session не задействована, надо
// закрыть `time_entry_obj` с source=manual напрямую через ARK upsert.

interface ArkObjectLike {
  id: string;
  typeId?: string;
  type_id?: string;
  title?: string | null;
  contentJson?: unknown;
  content_json?: unknown;
  propsJson?: Record<string, unknown>;
  props_json?: Record<string, unknown>;
  createdAt?: string;
  created_at?: string;
  updatedAt?: string;
  updated_at?: string;
  deletedAt?: string | null;
  deleted_at?: string | null;
}

async function invokePomodoro(op: "pause" | "resume" | "skip" | "stop"): Promise<void> {
  try {
    const client = await awaitArkReady();
    const state = await client.invokeOperation<PomodoroEventState>({
      operation: `pomodoro.${op}`,
    });
    if (state && typeof state === "object") {
      // См. postmortems.md § 2026-05-30: pause/resume return state but do not emit
      // pomodoro events, so the widget must apply the operation response directly.
      setFocusState(deriveFocusStateFromBackend(state));
    }
  } catch (e) {
    console.error(`[focus-widget] pomodoro.${op} failed:`, e);
  }
}

async function stopManualStopwatch(): Promise<void> {
  try {
    const client = await awaitArkReady();
    // SQL-уровневый фильтр endedAt IS NULL + source='manual' через json_extract
    // в ARK (`list_running_time_entries`). До 2026-05-21 здесь был
    // list_objects_by_type + client-side фильтр/сортировка — на больших
    // историях это тянуло всю time_entry_obj таблицу через WS.
    const running = (await client.invokeOperation({
      operation: "list_running_time_entries",
      source: "manual",
    } as { operation: string; [k: string]: unknown })) as ArkObjectLike[];
    if (!Array.isArray(running)) return;
    const nowIso = new Date().toISOString();
    // Backend уже отсортировал startedAt DESC и отфильтровал endedAt/deleted_at.
    // Сохраняем только startedAt guard — orphan entries без started тоже не
    // нужны (хотя по идее их нет, потому что endedAt IS NULL && startedAt пуст
    // — это broken state, но defensive).
    const target = running.find((o) => {
      const props = (o.propsJson ?? o.props_json ?? {}) as Record<string, unknown>;
      const started = props.startedAt;
      return typeof started === "string" && started.length > 0;
    });
    if (!target) {
      console.warn("[focus-widget] stopwatch stop: no running manual time_entry");
      return;
    }
    const props = {
      ...((target.propsJson ?? target.props_json ?? {}) as Record<string, unknown>),
      endedAt: nowIso,
    };
    const record = {
      id: target.id,
      typeId: target.typeId ?? target.type_id ?? "time_entry_obj",
      title: target.title ?? "",
      contentJson: target.contentJson ?? target.content_json ?? {},
      propsJson: props,
      createdAt: target.createdAt ?? target.created_at ?? nowIso,
      updatedAt: nowIso,
      deletedAt: target.deletedAt ?? target.deleted_at ?? null,
    };
    await client.invokeOperation({
      operation: "upsert_object",
      object: record,
    } as { operation: string; [k: string]: unknown });
  } catch (e) {
    console.error("[focus-widget] stopwatch stop failed:", e);
  }
}

ipcMain.handle("kepler:focus-widget:pomodoro:pause", async () => {
  await invokePomodoro("pause");
});
ipcMain.handle("kepler:focus-widget:pomodoro:resume", async () => {
  await invokePomodoro("resume");
});
ipcMain.handle("kepler:focus-widget:pomodoro:skip", async () => {
  await invokePomodoro("skip");
});
ipcMain.handle("kepler:focus-widget:pomodoro:stop", async () => {
  await invokePomodoro("stop");
});
ipcMain.handle("kepler:focus-widget:stopwatch:stop", async () => {
  await stopManualStopwatch();
});

// --- Backend pomodoro events subscription ----------------------------------
//
// До 2026-05-22 focus widget состояние обновлялось ИСКЛЮЧИТЕЛЬНО через
// `kepler:focus-widget:set-state` push из renderer'а. Если юзер стартанул
// помодоро через launcher команду, renderer может быть не открыт → widget
// всё равно должен появиться.
//
// Решение: main process подписывается напрямую на backend pomodoro events
// (как `pomodoro-notifier`), деривит focus state и зовёт `setFocusState`.
// Renderer push остаётся source of truth когда focus-session открыт (его state
// содержит draft title и live blockingActive из focusBlocklistId
// settings, которые backend не знает).
//
// Idempotent: повторные set с тем же контентом перезатирают, дешёво.

type BackendPhase = "idle" | "work" | "shortBreak" | "longBreak";

interface PomodoroEventState {
  phase?: BackendPhase;
  remainingMs?: number;
  totalMs?: number;
  isRunning?: boolean;
  isPaused?: boolean;
  phaseEndsAtMs?: number | null;
  title?: string;
  tasks?: Array<{ id?: string; title?: string }>;
}

function deriveFocusStateFromBackend(raw: PomodoroEventState): Partial<FocusState> {
  const phase = (raw.phase ?? "idle") as BackendPhase;
  const isRunning = raw.isRunning === true;
  const isPaused = raw.isPaused === true;
  const active = isRunning && !isPaused && phase !== "idle";

  // Виджет остаётся видим пока pomodoro session не idle. Это включает:
  //   - running (work/break),
  //   - paused (показываем Play),
  //   - между фазами с auto_start_*=false (Finished пришёл, isRunning=false,
  //     но phase=Work/ShortBreak/LongBreak ждёт ручного Skip/Resume).
  // Backend сбрасывает phase в Idle только на stop() — это и есть единственное
  // условие скрытия виджета.
  const widgetActive = phase !== "idle";

  const mode: FocusState["mode"] = phase === "work" ? "work" : "break";
  const remainingSec = Math.max(0, Math.ceil((raw.remainingMs ?? 0) / 1000));
  const totalSec = Math.max(0, Math.ceil((raw.totalMs ?? 0) / 1000));

  const title = (raw.title ?? "").trim();
  const firstTask = raw.tasks?.[0];
  const label = title || firstTask?.title || (mode === "work" ? "Фокус" : "Перерыв");

  return {
    active: widgetActive,
    remainingSec,
    totalSec,
    label,
    mode,
    isPaused,
    // backend не знает про focus blocklist — оставляем как есть. Renderer
    // обновит при следующем push'е если focus-session открыт.
    phaseEndsAtMs: active ? (raw.phaseEndsAtMs ?? null) : null,
  };
}

let backendEventsUnsubscribe: (() => void) | null = null;

export function setupFocusWidgetBackendSync(opts: { arkClient: ArkClient }): void {
  if (backendEventsUnsubscribe) {
    try {
      backendEventsUnsubscribe();
    } catch (e) {
      console.error("[focus-widget] previous unsubscribe failed:", e);
    }
    backendEventsUnsubscribe = null;
  }
  backendEventsUnsubscribe = opts.arkClient.onArkEvent((e) => {
    if (
      e.event !== "pomodoro_tick" &&
      e.event !== "pomodoro_phase_changed" &&
      e.event !== "pomodoro_finished"
    ) {
      return;
    }
    const raw = e as unknown as PomodoroEventState;
    const patch = deriveFocusStateFromBackend(raw);
    setFocusState(patch);
  });

  // Hydrate widget при первом subscribe — backend мог восстановить running
  // session из persisted snapshot до того как мы успели подписаться.
  void opts.arkClient
    .invokeOperation({ operation: "pomodoro.get_state" } as { operation: string })
    .then((s) => {
      if (s && typeof s === "object") {
        setFocusState(deriveFocusStateFromBackend(s as PomodoroEventState));
      }
    })
    .catch((err) => {
      console.warn("[focus-widget] initial pomodoro.get_state failed:", err);
    });

  console.log("[focus-widget] subscribed to backend pomodoro events");
}

export function teardownFocusWidgetBackendSync(): void {
  if (backendEventsUnsubscribe) {
    try {
      backendEventsUnsubscribe();
    } catch (e) {
      console.error("[focus-widget] teardown unsubscribe failed:", e);
    }
    backendEventsUnsubscribe = null;
  }
}

// Cleanup on app quit.
app.on("before-quit", () => {
  clearTickTimer();
  teardownFocusWidgetBackendSync();
  if (widgetWindow && !widgetWindow.isDestroyed()) {
    widgetWindow.destroy();
    widgetWindow = null;
  }
});
