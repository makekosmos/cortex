// Kepler — Electron host для Kosmos ecosystem.
//
// Phase 1 baseline (scaffold):
//   1. Spawn kepler-backend.exe child (singleton, держит ARK).
//   2. Один frameless transparent BrowserWindow по центру (launcher).
//   3. Окно hidden default; globalShortcut Ctrl+Shift+K (Win/Linux) /
//      Cmd+Shift+K (macOS) toggle show/hide.
//   4. backgroundMaterial: 'mica' (Win11) — graceful fallback на flat на
//      older Windows / non-Win платформах.
//   5. app.requestSingleInstanceLock — одна копия Kepler на машину.
//   6. Tray icon с menu Open/Quit.
//
// Phase 2+ задачи (не в этом scaffold'е):
//   - Полноценное extension API в preload (window.kepler.extensions.*).
//   - WS-client к kepler-backend (FTS5 search, quick-create).
//   - Window state persistence.
//   - Auto-update mechanism.

import {
  app,
  BrowserWindow,
  globalShortcut,
  ipcMain,
  Tray,
  Menu,
  nativeImage,
  screen,
} from "electron";
import { spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import { existsSync, readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { BackendStatus, SearchResult } from "../shared/ipc-types";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WINDOW_WIDTH = 720;
const WINDOW_HEIGHT_COMPACT = 76;

const isDev = !!process.env.VITE_DEV_SERVER_URL;

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let backendProc: ChildProcess | null = null;
let backendLockPath = "";
let isQuiting = false;

// --- single instance ---------------------------------------------------------

if (!app.requestSingleInstanceLock()) {
  app.quit();
  process.exit(0);
}

app.on("second-instance", () => {
  showLauncher();
});

// --- backend spawn -----------------------------------------------------------

function resolveBackendExe(): string {
  if (isDev) {
    const dev = path.resolve(
      __dirname,
      "../../../services/kepler-backend/target/debug/kepler-backend.exe",
    );
    if (existsSync(dev)) return dev;
    const devRelease = path.resolve(
      __dirname,
      "../../../services/kepler-backend/target/release/kepler-backend.exe",
    );
    if (existsSync(devRelease)) return devRelease;
  }
  // production: рядом с упакованным приложением (extraResources)
  return path.join(process.resourcesPath ?? __dirname, "kepler-backend.exe");
}

function spawnBackend() {
  const exe = resolveBackendExe();
  if (!existsSync(exe)) {
    console.error("[kepler-shell] kepler-backend.exe not found at", exe);
    return;
  }
  backendLockPath = path.join(
    app.getPath("appData"),
    "Kosmos",
    "kepler.lock.json",
  );
  console.error("[kepler-shell] spawning backend:", exe);
  backendProc = spawn(exe, [], {
    detached: false,
    stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env },
  });
  backendProc.stdout?.on("data", (b) =>
    process.stderr.write(`[kepler-backend] ${b.toString()}`),
  );
  backendProc.stderr?.on("data", (b) =>
    process.stderr.write(`[kepler-backend] ${b.toString()}`),
  );
  backendProc.on("exit", (code) => {
    console.error(`[kepler-shell] backend exited code=${code}`);
    backendProc = null;
  });
}

function readBackendStatus(): BackendStatus {
  if (!backendLockPath || !existsSync(backendLockPath)) {
    return { running: false, lockFilePath: backendLockPath };
  }
  try {
    const lock = JSON.parse(readFileSync(backendLockPath, "utf8")) as {
      pid: number;
      ws_port: number;
    };
    return {
      running: !!backendProc && !backendProc.killed,
      pid: lock.pid,
      wsPort: lock.ws_port,
      lockFilePath: backendLockPath,
    };
  } catch {
    return { running: false, lockFilePath: backendLockPath };
  }
}

// --- launcher window ---------------------------------------------------------

function createLauncher() {
  const display = screen.getPrimaryDisplay().workAreaSize;
  const x = Math.round((display.width - WINDOW_WIDTH) / 2);
  const y = Math.round(display.height * 0.25);

  mainWindow = new BrowserWindow({
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT_COMPACT,
    x,
    y,
    show: false,
    frame: false,
    transparent: true,
    resizable: false,
    movable: false,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    // Mica на Win11; на старых Windows / других OS Electron сам fallback'ит
    // на плоский background. См. Electron BrowserWindow docs.
    backgroundMaterial: "mica",
    backgroundColor: "#00000000",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  // hide вместо close при потере фокуса / Esc / окно скрыто по умолчанию
  mainWindow.on("blur", () => hideLauncher());
  mainWindow.on("close", (e) => {
    if (!isQuiting) {
      e.preventDefault();
      hideLauncher();
    }
  });

  if (isDev && process.env.VITE_DEV_SERVER_URL) {
    void mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    void mainWindow.loadFile(path.join(__dirname, "../dist/index.html"));
  }
}

function showLauncher() {
  if (!mainWindow) createLauncher();
  if (!mainWindow) return;
  // re-center при показе (на случай если monitor layout изменился)
  const display = screen.getPrimaryDisplay().workAreaSize;
  const x = Math.round((display.width - WINDOW_WIDTH) / 2);
  const y = Math.round(display.height * 0.25);
  mainWindow.setBounds({
    x,
    y,
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT_COMPACT,
  });
  mainWindow.show();
  mainWindow.focus();
  mainWindow.webContents.send("kepler:window:show");
}

function hideLauncher() {
  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.hide();
  }
}

// --- tray --------------------------------------------------------------------

function createTray() {
  // Phase 1 fallback: пустая иконка-заглушка (16x16 чёрный квадрат). Реальная
  // PNG/ICO иконка добавляется отдельно в build/ resources.
  const iconPath = path.resolve(__dirname, "../build/icon.png");
  const icon = existsSync(iconPath)
    ? nativeImage.createFromPath(iconPath)
    : nativeImage.createEmpty();
  tray = new Tray(icon);
  tray.setToolTip("Kepler");
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: "Открыть", click: () => showLauncher() },
      { type: "separator" },
      {
        label: "Выход",
        click: () => {
          isQuiting = true;
          app.quit();
        },
      },
    ]),
  );
  tray.on("click", () => showLauncher());
}

// --- IPC handlers ------------------------------------------------------------

ipcMain.handle("kepler:backend:status", () => readBackendStatus());

ipcMain.handle("kepler:backend:restart", () => {
  if (backendProc && !backendProc.killed) {
    backendProc.kill();
  }
  spawnBackend();
});

ipcMain.handle("kepler:window:hide", () => hideLauncher());

ipcMain.handle("kepler:search:query", async (_e, _text: string): Promise<SearchResult[]> => {
  // Phase 1 stub: реальный WS-вызов к kepler-backend для search_objects
  // будет добавлен в Phase 2. Сейчас возвращаем пустой массив, чтобы UI
  // не падал.
  return [];
});

// --- lifecycle ---------------------------------------------------------------

app.whenReady().then(() => {
  spawnBackend();
  createLauncher();
  createTray();

  const accelerator =
    process.platform === "darwin" ? "Command+Shift+K" : "Control+Shift+K";
  const ok = globalShortcut.register(accelerator, () => {
    if (mainWindow?.isVisible()) {
      hideLauncher();
    } else {
      showLauncher();
    }
  });
  if (!ok) {
    console.error(`[kepler-shell] globalShortcut ${accelerator} register failed`);
  } else {
    console.error(`[kepler-shell] globalShortcut ${accelerator} registered`);
  }
});

app.on("window-all-closed", () => {
  // не выходим — Kepler tray-resident; закрытие окна только hide
});

app.on("will-quit", () => {
  globalShortcut.unregisterAll();
  if (backendProc && !backendProc.killed) {
    backendProc.kill();
  }
});

