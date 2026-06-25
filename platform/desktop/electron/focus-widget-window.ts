import { BrowserWindow, screen } from "electron";
import path from "node:path";
import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { keplerDataDir } from "./data-dir";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  resolveWindowMaterial,
} from "./window-effects";

const WIDGET_WIDTH = 280;
const WIDGET_HEIGHT = 52;

interface PersistedBounds {
  x: number;
  y: number;
}

const STATE_FILENAME = "kepler-focus-widget-state.json";
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
let saveTimer: ReturnType<typeof setTimeout> | null = null;

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

function isOnSomeDisplay(x: number, y: number): boolean {
  const displays = screen.getAllDisplays();
  for (const d of displays) {
    const w = d.workArea;
    if (x >= w.x && x < w.x + w.width && y >= w.y && y < w.y + w.height) return true;
  }
  return false;
}

function defaultPositionForWorkArea(workArea: Electron.Rectangle): PersistedBounds {
  const bottomOffset = 50;
  return {
    x: Math.round(workArea.x + (workArea.width - WIDGET_WIDTH) / 2),
    y: Math.max(workArea.y + 24, workArea.y + workArea.height - WIDGET_HEIGHT - bottomOffset),
  };
}

function defaultPosition(): PersistedBounds {
  return defaultPositionForWorkArea(screen.getPrimaryDisplay().workArea);
}

export function resetFocusWidgetPosition(win: BrowserWindow): void {
  const [x, y] = win.getPosition();
  const display = screen.getDisplayNearestPoint({ x, y });
  const pos = defaultPositionForWorkArea(display.workArea);
  win.setPosition(pos.x, pos.y, false);
  writePersistedBoundsNow(pos);
}

export function scheduleFocusWidgetBoundsPersist(getWindow: () => BrowserWindow | null): void {
  if (saveTimer) clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    saveTimer = null;
    const win = getWindow();
    if (!win || win.isDestroyed()) return;
    const [x, y] = win.getPosition();
    writePersistedBoundsNow({ x, y });
  }, 400);
}

export function createFocusWidgetWindow(opts: {
  onMove: () => void;
  onClosed: () => void;
}): BrowserWindow {
  const persisted = readPersistedBounds();
  const pos =
    persisted && isOnSomeDisplay(persisted.x, persisted.y) ? persisted : defaultPosition();
  const backgroundMaterial = resolveWindowMaterial("none");

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
    ...backgroundMaterialOption(backgroundMaterial),
    roundedCorners: true,
    // focusable: true (default). Раньше было false ("не воровать фокус
    // когда показывается"), но на Win32 non-focusable окно не получает
    // WM_NCLBUTTONDOWN для драга -> `-webkit-app-region: drag` молча не
    // работал. Show-без-кражи-фокуса всё равно обеспечивается `showInactive()`
    // — окно появляется, фокус остаётся на текущем приложении пользователя.
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: false, // тикает каждую секунду — нельзя throttle'ить
    },
  });

  win.setAlwaysOnTop(true, "screen-saver", 1);
  applyWindowMaterial(win, backgroundMaterial, "focus-widget");

  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void win.loadURL(`${devUrl}#focus-widget`);
  } else {
    void win.loadFile(path.join(__dirname, "..", "dist", "index.html"), {
      hash: "focus-widget",
    });
  }

  win.on("move", opts.onMove);
  win.on("closed", opts.onClosed);
  return win;
}
