// Focus widget — Spotify-mini-player-style плавающий always-on-top окно,
// показывается во время активной pomodoro сессии. Слева MM:SS countdown,
// справа task label.
//
// Architecture:
//   - Window lazy-create при первом setState с active=true.
//   - State хранится в main process (singleton focusState). Renderer
//     получает через initial `getState()` IPC + push events `state:update`.
//   - Position персистится в kepler-shell-settings.json (focusWidgetBounds).
//   - Listens на pomodoro state от Horologion через
//     `kepler:focus-widget:set-state` IPC (см. preload + extension-preload).
//   - На active=false (pomodoro stopped) → hide(), но window не destroy
//     чтобы reopen был мгновенным.

import {
  BrowserWindow,
  ipcMain,
  screen,
  app,
} from "electron";
import path from "node:path";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WIDGET_WIDTH = 320;
const WIDGET_HEIGHT = 52;
const STATE_FILENAME = "kepler-focus-widget-state.json";

export interface FocusState {
  active: boolean;
  remainingSec: number;
  label: string;
  mode: "work" | "break" | "stopwatch";
}

interface PersistedBounds {
  x: number;
  y: number;
}

const DEFAULT_STATE: FocusState = {
  active: false,
  remainingSec: 0,
  label: "",
  mode: "work",
};

let widgetWindow: BrowserWindow | null = null;
let currentState: FocusState = { ...DEFAULT_STATE };
let saveTimer: ReturnType<typeof setTimeout> | null = null;

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
  // Top-right corner, 24px from edges.
  return {
    x: primary.x + primary.width - WIDGET_WIDTH - 24,
    y: primary.y + 24,
  };
}

// --- Window lifecycle -------------------------------------------------------

function createWidgetWindow(): BrowserWindow {
  const persisted = readPersistedBounds();
  const pos =
    persisted && isOnSomeDisplay(persisted.x, persisted.y)
      ? persisted
      : defaultPosition();

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
    focusable: false, // не воровать фокус когда показывается
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

// --- Public API -------------------------------------------------------------

export function setFocusState(next: Partial<FocusState>): void {
  currentState = { ...currentState, ...next };
  // active=true → ensure widget shown.
  // active=false → hide (но не destroy, чтобы reopen был быстрым).
  if (currentState.active) {
    showWidget();
  } else {
    hideWidget();
  }
  broadcastState();
}

export function getFocusState(): FocusState {
  return { ...currentState };
}

// --- IPC --------------------------------------------------------------------

ipcMain.handle(
  "kepler:focus-widget:set-state",
  (_e, patch: Partial<FocusState>) => {
    if (!patch || typeof patch !== "object") return;
    setFocusState(patch);
  },
);

ipcMain.handle("kepler:focus-widget:get-state", () => getFocusState());

ipcMain.handle("kepler:focus-widget:hide", () => {
  hideWidget();
});

ipcMain.handle("kepler:focus-widget:open-horologion", async () => {
  // Lazy import — extension-host requires shell context, не хочу cycle.
  const { openExtension } = await import("./extension-host");
  openExtension("horologion");
});

// Cleanup on app quit.
app.on("before-quit", () => {
  if (widgetWindow && !widgetWindow.isDestroyed()) {
    widgetWindow.destroy();
    widgetWindow = null;
  }
});
