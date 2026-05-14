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
  nativeTheme,
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
import type { BackendStatus, CommandRecord, SearchResult } from "../shared/ipc-types";
import { COMMANDS, findCommand } from "./commands";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WINDOW_WIDTH = 720;
const WINDOW_HEIGHT = 460;
const WINDOW_STATE_FILENAME = "kepler-shell-window-state.json";

const isDev = !!process.env.VITE_DEV_SERVER_URL;

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let backendProc: ChildProcess | null = null;
let backendLockPath = "";
let isQuiting = false;
let arkClient: ArkClient | null = null;
let windowStateSaveTimer: NodeJS.Timeout | null = null;

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
    height: WINDOW_HEIGHT,
    x: pos.x,
    y: pos.y,
    show: false,
    frame: false,
    // Acrylic / mica игнорируется при transparent:true. На Win11 22H2+ окно
    // автоматически получает rounded corners. Acrylic intense чем mica —
    // лучше визуально для launcher'а (как PowerToys Run / Raycast).
    transparent: false,
    resizable: false,
    movable: true,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: true,
    alwaysOnTop: true,
    backgroundMaterial: "acrylic",
    backgroundColor: "#00000000",
    roundedCorners: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  // Явный вызов после create — иногда constructor option backgroundMaterial
  // не применяется на frameless+alwaysOnTop комбинации; setBackgroundMaterial
  // прямо дёргает DwmSetWindowAttribute. Безопасно: no-op на non-Win11.
  try {
    mainWindow.setBackgroundMaterial("acrylic");
  } catch (e) {
    console.error("[kepler-shell] setBackgroundMaterial failed:", e);
  }

  // В prod hide вместо close при потере фокуса; в dev — оставляем открытым
  // чтобы переключаться в DevTools / IDE без потери launcher'а.
  if (!isDev) {
    mainWindow.on("blur", () => hideLauncher());
  }
  mainWindow.on("close", (e) => {
    if (!isQuiting) {
      e.preventDefault();
      hideLauncher();
    }
  });
  mainWindow.on("moved", () => scheduleWindowStateSave());

  if (isDev && process.env.VITE_DEV_SERVER_URL) {
    void mainWindow.loadURL(process.env.VITE_DEV_SERVER_URL);
    // detached DevTools — отдельное окно, не блокирует launcher.
    mainWindow.webContents.openDevTools({ mode: "detach" });
  } else {
    void mainWindow.loadFile(path.join(__dirname, "../dist/index.html"));
  }
}

function showLauncher() {
  if (!mainWindow) createLauncher();
  if (!mainWindow) return;
  const saved = loadWindowState();
  const pos = saved ?? defaultLauncherPosition();
  mainWindow.setBounds({
    x: pos.x,
    y: pos.y,
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT,
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

// Окно теперь fixed-size (WINDOW_HEIGHT) — никакой compact/expanded логики.
// Renderer показывает список объектов всегда; на typing просто фильтрует.
// IPC обработчик оставлен для backward-compat с preload bridge, но noop.
function setLauncherExpanded(_expanded: boolean) {
  // no-op
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
    // Subscribe на commands_changed → пушим renderer'у сигнал перефетчить
    // список (он сам вызовет kepler:commands:list). Сам список не шлём —
    // renderer должен пройти через тот же merge-pipeline (static + dynamic).
    client.commands.onChanged(() => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send("kepler:commands:updated");
      }
    });
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

function staticCommands(): CommandRecord[] {
  return COMMANDS.map((c) => ({
    id: c.id,
    title: c.title,
    subtitle: c.subtitle,
    category: c.category,
  }));
}

ipcMain.handle("kepler:commands:list", async (): Promise<CommandRecord[]> => {
  const statics = staticCommands();
  if (!arkClient) return statics;
  try {
    const dynamic = await arkClient.commands.list();
    const byId = new Map<string, CommandRecord>();
    for (const c of statics) byId.set(c.id, c);
    for (const c of dynamic) {
      // Static open-commands имеют приоритет (их id типа "eden:open" не должны
      // переопределяться апкой). Если апка регистрирует уникальный id —
      // добавляем; иначе static побеждает.
      if (!byId.has(c.id)) {
        byId.set(c.id, {
          id: c.id,
          title: c.title,
          subtitle: c.subtitle,
          category: c.category,
        });
      }
    }
    return Array.from(byId.values());
  } catch (e) {
    console.error("[kepler-shell] commands.list (dynamic) failed:", e);
    return statics;
  }
});

ipcMain.handle("kepler:commands:invoke", async (_e, id: string): Promise<void> => {
  const cmd = findCommand(id);
  if (cmd) {
    try {
      await cmd.exec();
    } catch (e) {
      console.error(`[kepler-shell] command ${id} failed:`, e);
    }
  } else if (arkClient) {
    // Dynamic command — backend broadcast'нёт command_invoked, апка handle'нёт.
    try {
      await arkClient.commands.invoke(id);
    } catch (e) {
      console.error(`[kepler-shell] dynamic command ${id} invoke failed:`, e);
    }
  } else {
    console.warn(`[kepler-shell] unknown command (no arkClient): ${id}`);
  }
  // Спрятать launcher после успешного / неуспешного invoke — стандартное
  // поведение Spotlight/Raycast: command выполнен → окно уходит.
  hideLauncher();
});

ipcMain.handle(
  "kepler:objects:listRecent",
  async (_e, limit?: number): Promise<SearchResult[]> => {
    if (!arkClient) return [];
    const cap = typeof limit === "number" && limit > 0 ? Math.min(limit, 500) : 200;
    try {
      const records = await arkClient.objects.list();
      const sorted = records
        .filter((r) => !r.deletedAt)
        .sort((a, b) => (a.updatedAt < b.updatedAt ? 1 : -1))
        .slice(0, cap);
      return sorted.map((r) => ({
        id: r.id,
        title: r.title && r.title.length > 0 ? r.title : r.id,
        type_id: r.typeId,
      }));
    } catch (e) {
      console.error("[kepler-shell] objects.list failed:", e);
      return [];
    }
  },
);

// --- lifecycle ---------------------------------------------------------------

app.whenReady().then(async () => {
  // Принудительно темная тема — чтобы acrylic backgroundMaterial использовал
  // dark variant независимо от Windows system theme (иначе на light theme
  // launcher просвечивает белым).
  nativeTheme.themeSource = "dark";

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
  // F12 toggle DevTools (dev mode только) — глобальный hotkey удобнее чем
  // accelerator menu, т.к. меню у frameless окна нет.
  if (isDev) {
    globalShortcut.register("F12", () => {
      mainWindow?.webContents.toggleDevTools();
    });
  }

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

