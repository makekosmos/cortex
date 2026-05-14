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
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  writeFileSync,
} from "node:fs";
import { fileURLToPath } from "node:url";
import {
  ArkClient,
  buildPersonalSelectedSpace,
  ensureKeplerRunning,
  getArkDbPathForSelectedSpace,
  readSharedSelectedSpace,
  writeSharedSelectedSpace,
} from "@kosmos/ark";
import type { BackendStatus, SearchResult } from "../shared/ipc-types";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WINDOW_WIDTH = 720;
const WINDOW_HEIGHT_COMPACT = 76;
const WINDOW_HEIGHT_EXPANDED = 460;
const WINDOW_STATE_FILENAME = "kepler-shell-window-state.json";

const isDev = !!process.env.VITE_DEV_SERVER_URL;

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let backendProc: ChildProcess | null = null;
let backendLockPath = "";
let isQuiting = false;
let arkClient: ArkClient | null = null;
let windowStateSaveTimer: NodeJS.Timeout | null = null;
let isExpanded = false;

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

// --- window state persistence -----------------------------------------------

interface WindowState {
  x: number;
  y: number;
}

function windowStatePath(): string {
  return path.join(app.getPath("appData"), "Kosmos", WINDOW_STATE_FILENAME);
}

function loadWindowState(): WindowState | null {
  const p = windowStatePath();
  if (!existsSync(p)) return null;
  try {
    const raw = JSON.parse(readFileSync(p, "utf8")) as Partial<WindowState>;
    if (typeof raw.x === "number" && typeof raw.y === "number") {
      return { x: raw.x, y: raw.y };
    }
  } catch {
    /* corrupt file — игнор, используем default */
  }
  return null;
}

function saveWindowStateNow() {
  if (!mainWindow || mainWindow.isDestroyed()) return;
  const bounds = mainWindow.getBounds();
  const state: WindowState = { x: bounds.x, y: bounds.y };
  const targetPath = windowStatePath();
  try {
    mkdirSync(path.dirname(targetPath), { recursive: true });
    // Атомарная запись через tmp + rename, чтобы прерванный shutdown не оставил пустой JSON.
    const tmp = `${targetPath}.tmp`;
    writeFileSync(tmp, JSON.stringify(state, null, 2), "utf8");
    renameSync(tmp, targetPath);
  } catch (e) {
    console.error("[kepler-shell] saveWindowState failed:", e);
  }
}

function scheduleWindowStateSave() {
  if (windowStateSaveTimer) clearTimeout(windowStateSaveTimer);
  windowStateSaveTimer = setTimeout(() => {
    windowStateSaveTimer = null;
    saveWindowStateNow();
  }, 500);
}

function defaultLauncherPosition(): WindowState {
  const display = screen.getPrimaryDisplay().workAreaSize;
  return {
    x: Math.round((display.width - WINDOW_WIDTH) / 2),
    y: Math.round(display.height * 0.25),
  };
}

// --- launcher window ---------------------------------------------------------

function createLauncher() {
  const saved = loadWindowState();
  const pos = saved ?? defaultLauncherPosition();

  mainWindow = new BrowserWindow({
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT_COMPACT,
    x: pos.x,
    y: pos.y,
    show: false,
    frame: false,
    transparent: true,
    resizable: false,
    movable: true,
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
  mainWindow.on("moved", () => scheduleWindowStateSave());

  if (isDev && process.env.VITE_DEV_SERVER_URL) {
    void mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    void mainWindow.loadFile(path.join(__dirname, "../dist/index.html"));
  }
}

function showLauncher() {
  if (!mainWindow) createLauncher();
  if (!mainWindow) return;
  const saved = loadWindowState();
  const pos = saved ?? defaultLauncherPosition();
  isExpanded = false;
  mainWindow.setBounds({
    x: pos.x,
    y: pos.y,
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

function setLauncherExpanded(expanded: boolean) {
  if (!mainWindow || mainWindow.isDestroyed()) return;
  if (isExpanded === expanded) return;
  isExpanded = expanded;
  const bounds = mainWindow.getBounds();
  mainWindow.setBounds({
    x: bounds.x,
    y: bounds.y,
    width: WINDOW_WIDTH,
    height: expanded ? WINDOW_HEIGHT_EXPANDED : WINDOW_HEIGHT_COMPACT,
  });
}

// --- tray --------------------------------------------------------------------

function resolveTrayIconPath(): string | null {
  // dev: dist-electron/main.js → ../build/icon.png
  // prod electron-builder asar: resources/app.asar/dist-electron/main.js → ../../build/icon.png
  const candidates = [
    path.resolve(__dirname, "../build/icon.png"),
    path.resolve(__dirname, "../../build/icon.png"),
  ];
  for (const c of candidates) {
    if (existsSync(c)) return c;
  }
  return null;
}

function createTray() {
  const iconPath = resolveTrayIconPath();
  const icon = iconPath
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

// --- ArkClient (WS to kepler-backend) ---------------------------------------

function ensureSelectedSpace(): { spaceId: string; dbPath: string } {
  const appDataPath = app.getPath("appData");
  let selection = readSharedSelectedSpace(appDataPath);
  if (!selection) {
    selection = buildPersonalSelectedSpace(appDataPath, "kepler-shell");
    writeSharedSelectedSpace(appDataPath, selection);
  }
  const dbPath = getArkDbPathForSelectedSpace(appDataPath, selection);
  return { spaceId: selection.spaceId, dbPath };
}

async function initArkClient(): Promise<void> {
  try {
    const { spaceId } = ensureSelectedSpace();
    // kepler-shell сам спавнит kepler-backend выше (spawnBackend), здесь
    // только ждём lock-файл и коннектимся через WS. autoLaunch=false — повторно
    // не запускаем.
    const state = await ensureKeplerRunning({
      appDataPath: app.getPath("appData"),
      waitMs: 10000,
      autoLaunch: false,
    });
    if (state.kind !== "connected") {
      console.error(
        `[kepler-shell] kepler-backend ${state.kind}: search will return empty`,
      );
      return;
    }
    const deviceId = `kepler-shell-${app.getPath("userData").slice(-12)}`;
    const client = new ArkClient({
      spaceId,
      deviceId,
      deviceName: "Kepler Shell",
      keplerLock: state.lock,
    });
    await client.start();
    arkClient = client;
    console.error(
      `[kepler-shell] ArkClient connected to kepler-backend (pid ${state.lock.pid}, ws_port ${state.lock.ws_port})`,
    );
  } catch (e) {
    console.error("[kepler-shell] ArkClient init failed:", e);
  }
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

ipcMain.handle(
  "kepler:window:setExpanded",
  (_e, expanded: boolean) => setLauncherExpanded(!!expanded),
);

ipcMain.handle(
  "kepler:search:query",
  async (_e, text: string): Promise<SearchResult[]> => {
    if (!arkClient || !text.trim()) return [];
    try {
      const hits = await arkClient.objects.search(text);
      if (hits.length === 0) return [];
      // hits — массив { file, line, text, entryId } от db::search_objects.
      // Резолвим title/typeId через get_objects_by_ids одной батч-операцией.
      const ids = Array.from(new Set(hits.map((h) => h.entryId))).slice(0, 8);
      const records = await arkClient.objects.getMany(ids);
      const recordById = new Map(records.map((r) => [r.id, r]));
      const out: SearchResult[] = [];
      const seen = new Set<string>();
      for (const h of hits) {
        if (seen.has(h.entryId)) continue;
        seen.add(h.entryId);
        const rec = recordById.get(h.entryId);
        if (!rec) continue;
        out.push({
          id: rec.id,
          title: rec.title && rec.title.length > 0 ? rec.title : rec.id,
          type_id: rec.typeId,
          snippet: h.text,
        });
        if (out.length >= 8) break;
      }
      return out;
    } catch (e) {
      console.error("[kepler-shell] search failed:", e);
      return [];
    }
  },
);

// --- lifecycle ---------------------------------------------------------------

app.whenReady().then(async () => {
  spawnBackend();
  createLauncher();
  createTray();

  void initArkClient();

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
  if (windowStateSaveTimer) {
    clearTimeout(windowStateSaveTimer);
    windowStateSaveTimer = null;
    saveWindowStateNow();
  }
  if (backendProc && !backendProc.killed) {
    backendProc.kill();
  }
});

