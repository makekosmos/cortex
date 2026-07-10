import { BrowserWindow, ipcMain, screen } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

let overlayWin: BrowserWindow | null = null;
let isReady = false;
let pendingApp: { id: string; title: string; icon?: string | null } | null = null;
let readyFallbackTimer: ReturnType<typeof setTimeout> | null = null;

const isHeadless = () =>
  process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";

ipcMain.on("kepler:focus-overlay:ready", () => markReady());

// Renderer переключает интерактивность окна: пока курсор над плашкой — окно
// принимает клики; в остальное время — сквозное (click-through).
ipcMain.handle("kepler:focus-overlay:set-interactive", (_e, interactive: boolean) => {
  if (!overlayWin || overlayWin.isDestroyed()) return;
  if (interactive) {
    overlayWin.setFocusable(true);
    overlayWin.setIgnoreMouseEvents(false);
  } else {
    overlayWin.setFocusable(false);
    overlayWin.setIgnoreMouseEvents(true, { forward: true });
  }
});

// Renderer сигналит когда анимация ухода завершена — скрываем окно.
// Это убирает fullscreen transparent composited surface до следующей блокировки.
ipcMain.on("kepler:focus-overlay:done", () => {
  if (!overlayWin || overlayWin.isDestroyed()) return;
  overlayWin.hide();
});

// Renderer (launcher) просит показать overlay при запуске заблокированного app.
ipcMain.handle(
  "kepler:focus-overlay:show-blocked",
  (_e, app: { id: string; title: string; icon?: string | null }) => {
    showFocusBlockOverlay(app);
  },
);

function markReady(): void {
  if (readyFallbackTimer) {
    clearTimeout(readyFallbackTimer);
    readyFallbackTimer = null;
  }
  isReady = true;
  if (pendingApp) {
    const app = pendingApp;
    pendingApp = null;
    showOverlay();
    overlayWin?.webContents.send("kepler:focus-overlay:show", app);
  }
  // Нет pending-блокировки — окно остаётся скрытым, compositor surface не занята.
}

// Показывает overlay для текущей блокировки. Вызывается каждый раз при блокировке.
function showOverlay(): void {
  if (!overlayWin || overlayWin.isDestroyed()) return;
  overlayWin.setAlwaysOnTop(true, "screen-saver", 1);
  overlayWin.setIgnoreMouseEvents(true, { forward: true });
  overlayWin.showInactive();
}

function getOrCreateOverlay(): BrowserWindow | null {
  if (isHeadless()) return null;
  if (overlayWin && !overlayWin.isDestroyed()) return overlayWin;

  isReady = false;
  const { bounds } = screen.getPrimaryDisplay();

  overlayWin = new BrowserWindow({
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    backgroundColor: "#00000000",
    transparent: true,
    // The renderer draws only the intentional edge gradient and popup. Do not
    // attach a DWM background material to this fullscreen transparent surface:
    // even "none" can expose an opaque backing across the rest of the screen.
    frame: false,
    alwaysOnTop: true,
    skipTaskbar: true,
    focusable: true,
    resizable: false,
    movable: false,
    fullscreenable: false,
    show: false,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      // Throttling включён — окно не рисуется когда скрыто/нет анимации.
      backgroundThrottling: true,
      preload: path.join(__dirname, "preload.mjs"),
    },
  });

  overlayWin.setIgnoreMouseEvents(true, { forward: true });

  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void overlayWin.loadURL(`${devUrl}#focus-block-overlay`);
  } else {
    void overlayWin.loadFile(path.join(__dirname, "..", "dist", "index.html"), {
      hash: "focus-block-overlay",
    });
  }

  // Fallback: если ready не пришёл за 3с — считаем готовым.
  readyFallbackTimer = setTimeout(() => markReady(), 3000);

  overlayWin.on("closed", () => {
    overlayWin = null;
    isReady = false;
    if (readyFallbackTimer) {
      clearTimeout(readyFallbackTimer);
      readyFallbackTimer = null;
    }
  });

  return overlayWin;
}

export function showFocusBlockOverlay(app: {
  id: string;
  title: string;
  icon?: string | null;
}): void {
  const win = getOrCreateOverlay();
  if (!win) return;
  if (isReady) {
    showOverlay();
    win.webContents.send("kepler:focus-overlay:show", app);
  } else {
    // Окно загружается — отправим сообщение как только renderer будет готов.
    pendingApp = app;
  }
}
