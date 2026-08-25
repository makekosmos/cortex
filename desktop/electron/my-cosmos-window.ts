// Мой космос (граф window) — отдельный BrowserWindow, грузит тот же
// renderer-bundle с hash `#/my-cosmos`, src/main.ts по hash рендерит
// MyCosmosView вместо LauncherView.
//
// Граф визуализирует объекты ARK через WebGL (cosmos.gl, force-граф в стиле
// Obsidian). Концептуально похож на Dashboard (браузер объектов), но как
// интерактивный граф, а не таблица.
//
// Открывается через `openMyCosmosWindow()` (вызов из command palette или из
// внутренней static command `kosmos:my-cosmos`).

import { BrowserWindow, screen } from "electron";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";
import { macWindowChrome } from "./mac-window";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const MYCOSMOS_DEFAULT_WIDTH = 1280;
const MYCOSMOS_DEFAULT_HEIGHT = 860;
const MYCOSMOS_MIN_WIDTH = 800;
const MYCOSMOS_MIN_HEIGHT = 600;
const STATE_FILENAME = "kepler-my-cosmos-window-state.json";

let myCosmosWin: BrowserWindow | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function isHeadlessOrTest(): boolean {
  return process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
}

interface MyCosmosWindowState {
  width: number;
  height: number;
  x: number;
  y: number;
  isMaximized: boolean;
}

function stateFilePath(): string {
  return path.join(keplerDataDir(), STATE_FILENAME);
}

function readState(): Partial<MyCosmosWindowState> | null {
  const p = stateFilePath();
  if (!existsSync(p)) return null;
  try {
    // SAFETY: The persisted state is written by writeStateNow with this shape.
    return JSON.parse(readFileSync(p, "utf8")) as Partial<MyCosmosWindowState>;
  } catch {
    return null;
  }
}

function writeStateNow(): void {
  if (!myCosmosWin || myCosmosWin.isDestroyed()) return;
  try {
    const bounds = myCosmosWin.getNormalBounds();
    const state: MyCosmosWindowState = {
      width: bounds.width,
      height: bounds.height,
      x: bounds.x,
      y: bounds.y,
      isMaximized: myCosmosWin.isMaximized(),
    };
    const p = stateFilePath();
    mkdirSync(path.dirname(p), { recursive: true });
    writeFileSync(p, JSON.stringify(state, null, 2), "utf8");
  } catch (e) {
    console.error("[kepler-shell] my-cosmos saveWindowState failed:", e);
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
    return x + w > wa.x && x < wa.x + wa.width && y + h > wa.y && y < wa.y + wa.height;
  });
}

export function openMyCosmosWindow(): void {
  if (myCosmosWin && !myCosmosWin.isDestroyed()) {
    if (isHeadlessOrTest()) return;
    myCosmosWin.show();
    myCosmosWin.focus();
    return;
  }
  const display = screen.getPrimaryDisplay().workAreaSize;

  let width = MYCOSMOS_DEFAULT_WIDTH;
  let height = MYCOSMOS_DEFAULT_HEIGHT;
  let x = Math.round((display.width - width) / 2);
  let y = Math.round((display.height - height) / 2);
  let restoreMaximize = false;

  const saved = readState();
  if (
    saved &&
    isNumber(saved.width) &&
    isNumber(saved.height) &&
    isNumber(saved.x) &&
    isNumber(saved.y) &&
    isOnSomeDisplay(saved.x, saved.y, saved.width, saved.height)
  ) {
    width = saved.width;
    height = saved.height;
    x = saved.x;
    y = saved.y;
    restoreMaximize = !!saved.isMaximized;
  }

  myCosmosWin = new BrowserWindow({
    width,
    height,
    minWidth: MYCOSMOS_MIN_WIDTH,
    minHeight: MYCOSMOS_MIN_HEIGHT,
    x,
    y,
    show: !isHeadlessOrTest(),
    title: "Мой космос",
    backgroundColor: "#0d0d0d",
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#00000000",
      symbolColor: "#FFFFFF",
      height: 36,
    },
    // macOS: центрированные traffic lights (на Windows — no-op).
    ...macWindowChrome({ trafficLightY: 12 }),
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
    },
  });

  if (restoreMaximize) {
    myCosmosWin.once("ready-to-show", () => {
      if (myCosmosWin && !myCosmosWin.isDestroyed()) myCosmosWin.maximize();
    });
  }

  // F12 — DevTools toggle (без модификаторов).
  myCosmosWin.webContents.on("before-input-event", (e, input) => {
    if (input.key === "F12" && !input.alt && !input.control && !input.shift && !input.meta) {
      e.preventDefault();
      try {
        myCosmosWin?.webContents.toggleDevTools();
      } catch {
        /* webContents destroyed mid-flight */
      }
    }
  });

  myCosmosWin.on("resized", scheduleSave);
  myCosmosWin.on("moved", scheduleSave);
  myCosmosWin.on("maximize", writeStateNow);
  myCosmosWin.on("unmaximize", writeStateNow);
  myCosmosWin.on("close", () => {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    writeStateNow();
  });
  myCosmosWin.on("closed", () => {
    myCosmosWin = null;
  });

  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void myCosmosWin.loadURL(`${devUrl}#/my-cosmos`);
  } else {
    void myCosmosWin.loadFile(path.join(__dirname, "../dist/index.html"), {
      hash: "/my-cosmos",
    });
  }
}

function isNumber(value: number | undefined): value is number {
  return typeof value === "number";
}
