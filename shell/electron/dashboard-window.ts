// Dashboard window для Kepler — отдельный BrowserWindow, грузит тот же
// renderer-bundle с hash `#/dashboard`, src/main.ts по hash рендерит
// DashboardView вместо LauncherView.
//
// Dashboard теперь встроен в shell (не extension), это ARK browser:
// sidebar по типам объектов + таблица содержимого. Концепция spaces убрана
// 2026-05-15 — одна БД на юзера, никакого welcome-screen'а.
//
// Открывается через `openDashboardWindow()` (вызов из tray menu или из
// внутренней static command `dashboard:open`).

import { BrowserWindow, screen } from "electron";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const DASHBOARD_DEFAULT_WIDTH = 1200;
const DASHBOARD_DEFAULT_HEIGHT = 800;
const DASHBOARD_MIN_WIDTH = 900;
const DASHBOARD_MIN_HEIGHT = 600;
const STATE_FILENAME = "kepler-dashboard-window-state.json";

let dashboardWin: BrowserWindow | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

interface DashboardWindowState {
  width: number;
  height: number;
  x: number;
  y: number;
  isMaximized: boolean;
}

function stateFilePath(): string {
  return path.join(keplerDataDir(), STATE_FILENAME);
}

function readState(): Partial<DashboardWindowState> | null {
  const p = stateFilePath();
  if (!existsSync(p)) return null;
  try {
    return JSON.parse(readFileSync(p, "utf8")) as Partial<DashboardWindowState>;
  } catch {
    return null;
  }
}

function writeStateNow(): void {
  if (!dashboardWin || dashboardWin.isDestroyed()) return;
  try {
    const bounds = dashboardWin.getNormalBounds();
    const state: DashboardWindowState = {
      width: bounds.width,
      height: bounds.height,
      x: bounds.x,
      y: bounds.y,
      isMaximized: dashboardWin.isMaximized(),
    };
    const p = stateFilePath();
    mkdirSync(path.dirname(p), { recursive: true });
    writeFileSync(p, JSON.stringify(state, null, 2), "utf8");
  } catch (e) {
    console.error("[kepler-shell] dashboard saveWindowState failed:", e);
  }
}

function scheduleSave(): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    writeStateNow();
  }, 500);
}

function isOnSomeDisplay(x: number, y: number, w: number, h: number): boolean {
  const displays = screen.getAllDisplays();
  return displays.some((d) => {
    const wa = d.workArea;
    return (
      x + w > wa.x &&
      x < wa.x + wa.width &&
      y + h > wa.y &&
      y < wa.y + wa.height
    );
  });
}

export function openDashboardWindow(): void {
  if (dashboardWin && !dashboardWin.isDestroyed()) {
    dashboardWin.show();
    dashboardWin.focus();
    return;
  }
  const display = screen.getPrimaryDisplay().workAreaSize;

  let width = DASHBOARD_DEFAULT_WIDTH;
  let height = DASHBOARD_DEFAULT_HEIGHT;
  let x = Math.round((display.width - width) / 2);
  let y = Math.round((display.height - height) / 2);
  let restoreMaximize = false;

  const saved = readState();
  if (
    saved &&
    typeof saved.width === "number" &&
    typeof saved.height === "number" &&
    typeof saved.x === "number" &&
    typeof saved.y === "number" &&
    isOnSomeDisplay(saved.x, saved.y, saved.width, saved.height)
  ) {
    width = saved.width;
    height = saved.height;
    x = saved.x;
    y = saved.y;
    restoreMaximize = !!saved.isMaximized;
  }

  dashboardWin = new BrowserWindow({
    width,
    height,
    minWidth: DASHBOARD_MIN_WIDTH,
    minHeight: DASHBOARD_MIN_HEIGHT,
    x,
    y,
    show: true,
    title: "Kosmos",
    backgroundColor: "#0d0d0d",
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#0d0d0d",
      symbolColor: "#cccccc",
      height: 36,
    },
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
    },
  });

  if (restoreMaximize) {
    dashboardWin.once("ready-to-show", () => {
      if (dashboardWin && !dashboardWin.isDestroyed()) dashboardWin.maximize();
    });
  }

  // F12 — DevTools toggle (без модификаторов).
  dashboardWin.webContents.on("before-input-event", (e, input) => {
    if (
      input.key === "F12" &&
      !input.alt &&
      !input.control &&
      !input.shift &&
      !input.meta
    ) {
      e.preventDefault();
      try {
        dashboardWin?.webContents.toggleDevTools();
      } catch {
        /* webContents destroyed mid-flight */
      }
    }
  });

  dashboardWin.on("resized", scheduleSave);
  dashboardWin.on("moved", scheduleSave);
  dashboardWin.on("maximize", writeStateNow);
  dashboardWin.on("unmaximize", writeStateNow);
  dashboardWin.on("close", () => {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    writeStateNow();
  });
  dashboardWin.on("closed", () => {
    dashboardWin = null;
  });

  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void dashboardWin.loadURL(`${devUrl}#/dashboard`);
  } else {
    void dashboardWin.loadFile(path.join(__dirname, "../dist/index.html"), {
      hash: "/dashboard",
    });
  }
}

export function isDashboardOpen(): boolean {
  return !!dashboardWin && !dashboardWin.isDestroyed();
}
