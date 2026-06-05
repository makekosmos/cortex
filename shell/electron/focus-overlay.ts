import { BrowserWindow, ipcMain, screen } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Окно создаётся сразу на весь экран и показывается ОДИН раз (прозрачное,
// click-through). На каждую блокировку мы НЕ делаем show/hide — иначе ОС
// проигрывает системную анимацию открытия окна («расширение»). Появление и
// исчезновение градиента/плашки — чисто CSS внутри renderer'а.
let overlayWin: BrowserWindow | null = null;
let isReady = false;
let shownOnce = false;
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
  ensureShown();
  if (pendingApp) {
    const app = pendingApp;
    pendingApp = null;
    overlayWin?.webContents.send("kepler:focus-overlay:show", app);
  }
}

// Показ окна один раз: прозрачное, click-through, поверх всего. Дальше окно
// больше не прячется и не ресайзится.
function ensureShown(): void {
  if (!overlayWin || overlayWin.isDestroyed() || shownOnce) return;
  overlayWin.setAlwaysOnTop(true, "screen-saver", 1);
  overlayWin.setIgnoreMouseEvents(true, { forward: true });
  overlayWin.showInactive();
  shownOnce = true;
}

function getOrCreateOverlay(): BrowserWindow | null {
  if (isHeadless()) return null;
  if (overlayWin && !overlayWin.isDestroyed()) return overlayWin;

  isReady = false;
  shownOnce = false;
  const { bounds } = screen.getPrimaryDisplay();

  overlayWin = new BrowserWindow({
    x: bounds.x,
    y: bounds.y,
    width: bounds.width,
    height: bounds.height,
    backgroundColor: "#00000000",
    transparent: true,
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
      backgroundThrottling: false,
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

  // Fallback: если ready не пришёл за 3с — показываем окно всё равно.
  readyFallbackTimer = setTimeout(() => markReady(), 3000);

  overlayWin.on("closed", () => {
    overlayWin = null;
    isReady = false;
    shownOnce = false;
    if (readyFallbackTimer) {
      clearTimeout(readyFallbackTimer);
      readyFallbackTimer = null;
    }
  });

  return overlayWin;
}

export function createFocusBlockOverlay(): void {
  getOrCreateOverlay();
}

export function showFocusBlockOverlay(app: {
  id: string;
  title: string;
  icon?: string | null;
}): void {
  const win = getOrCreateOverlay();
  if (!win) return;
  if (isReady) {
    ensureShown();
    win.webContents.send("kepler:focus-overlay:show", app);
  } else {
    pendingApp = app;
  }
}
