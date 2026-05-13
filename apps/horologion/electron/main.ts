import {
  app,
  BrowserWindow,
  ipcMain,
  Menu,
  nativeImage,
  Tray,
  type IpcMainInvokeEvent,
} from "electron";
import path from "node:path";
import fs from "node:fs";
import { fileURLToPath } from "node:url";
import {
  ArkClient,
  buildPersonalSelectedSpace,
  getArkDbPathForSelectedSpace,
  readSharedSelectedSpace,
  writeSharedSelectedSpace,
  type ArkObjectRecord,
  type JsonValue,
} from "@kepler/ark";
import type {
  CreateTimeEntryInput,
  StartTimerInput,
  UpdateTimeEntryInput,
  TimeEntry,
  Tag,
  DelphiTask,
  ArkStatus,
} from "../shared/ipc-types";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const isDev = !!process.env.VITE_DEV_SERVER_URL;

function resolveSidecarPath(): string {
  // Explicit override (test/e2e) wins
  if (process.env.ARK_SIDECAR_PATH && process.env.ARK_SIDECAR_PATH.trim().length > 0) {
    return process.env.ARK_SIDECAR_PATH.trim();
  }
  const exe = process.platform === "win32" ? "ark-core-rpc.exe" : "ark-core-rpc";
  if (isDev) {
    return path.resolve(__dirname, `../../../packages/ark-core/rust/target/debug/${exe}`);
  }
  // built (test mode): release target из cargo
  const releaseFromBuild = path.resolve(__dirname, `../../../packages/ark-core/rust/target/release/${exe}`);
  // packaged: рядом с приложением через resourcesPath
  const fromResources = process.resourcesPath
    ? path.resolve(process.resourcesPath, exe)
    : null;
  // Предпочитаем packaged, fallback на cargo release
  if (fromResources) {
    try {
      // require('fs') — но во избежание лишнего top-level я держу sync только тут
      const { existsSync } = require("node:fs") as typeof import("node:fs");
      if (existsSync(fromResources)) return fromResources;
    } catch {
      // ignore
    }
  }
  return releaseFromBuild;
}

function ensureSelectedSpace(): { spaceId: string; dbPath: string } {
  // Test override: ARK_DB_PATH направляет на изолированную БД (`.e2e`, `.tmp`, OS temp).
  // См. docs-site/concepts/test-isolation.md.
  const override = process.env.ARK_DB_PATH;
  if (override && override.trim().length > 0) {
    return { spaceId: "test-space", dbPath: override.trim() };
  }

  const appDataPath = app.getPath("appData");
  let selection = readSharedSelectedSpace(appDataPath);
  if (!selection) {
    selection = buildPersonalSelectedSpace(appDataPath, "horologion");
    writeSharedSelectedSpace(appDataPath, selection);
  }
  const dbPath = getArkDbPathForSelectedSpace(appDataPath, selection);
  return { spaceId: selection.spaceId, dbPath };
}

let arkClient: ArkClient | null = null;
let arkClientPromise: Promise<ArkClient> | null = null;
let arkStatus: ArkStatus = { status: "connecting" };

async function getArk(): Promise<ArkClient> {
  if (arkClient) return arkClient;
  if (arkClientPromise) return arkClientPromise;
  arkClientPromise = (async () => {
    const { spaceId, dbPath } = ensureSelectedSpace();
    arkStatus = { status: "connecting", dbPath };
    try {
      const client = new ArkClient({
        spaceId,
        deviceId: `horologion-${app.getPath("userData").slice(-12)}`,
        deviceName: "Horologion",
        dbPath,
        sidecarPath: resolveSidecarPath(),
      });
      await client.start();
      arkClient = client;
      arkStatus = { status: "connected", dbPath };
      return client;
    } catch (e) {
      arkStatus = {
        status: "error",
        message: e instanceof Error ? e.message : String(e),
        dbPath,
      };
      arkClientPromise = null; // allow retry on next call
      throw e;
    }
  })();
  return arkClientPromise;
}

function asObject(value: JsonValue | undefined): Record<string, JsonValue> {
  if (value && typeof value === "object" && !Array.isArray(value)) {
    return value as Record<string, JsonValue>;
  }
  return {};
}

function objectToTimeEntry(obj: ArkObjectRecord): TimeEntry {
  const props = asObject(obj.propsJson);
  return {
    id: obj.id,
    title: obj.title ?? "",
    startedAt: String(props.startedAt ?? obj.createdAt),
    endedAt: (props.endedAt as string | null | undefined) ?? null,
    source: (props.source as TimeEntry["source"]) ?? "manual",
    billable: Boolean(props.billable ?? false),
    tagIds: [],   // TODO: resolve через object_links (linkType='tagged')
    taskId: (props.taskId as string | null | undefined) ?? null,
    taskTitle: (props.taskTitle as string | null | undefined) ?? null,
  };
}

async function listTimeEntries(): Promise<TimeEntry[]> {
  const ark = await getArk();
  const list = await ark.objects.listByType("time_entry_obj");
  return list
    .filter((o) => !o.deletedAt)
    .map(objectToTimeEntry)
    .sort((a, b) => b.startedAt.localeCompare(a.startedAt));
}

async function listRunningTimeEntries(): Promise<TimeEntry[]> {
  const all = await listTimeEntries();
  return all.filter((e) => !e.endedAt);
}

async function startTimer(input: StartTimerInput): Promise<TimeEntry> {
  const ark = await getArk();
  const now = new Date().toISOString();
  const id = `te-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const record: ArkObjectRecord = {
    id,
    typeId: "time_entry_obj",
    title: input.title,
    contentJson: {},
    propsJson: {
      startedAt: now,
      endedAt: null,
      source: "manual",
      billable: Boolean(input.billable),
      taskId: input.taskId ?? null,
      taskTitle: input.taskTitle ?? null,
    },
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
  };
  try {
    await ark.objects.upsert(record);
  } catch (e) {
    console.error("[horologion] upsert failed:", e);
    throw e;
  }
  return objectToTimeEntry(record);
}

async function stopTimer(id: string): Promise<TimeEntry> {
  const ark = await getArk();
  const existing = await ark.objects.get(id);
  if (!existing) throw new Error(`time_entry ${id} not found`);
  const now = new Date().toISOString();
  const props: Record<string, JsonValue> = { ...asObject(existing.propsJson), endedAt: now };
  const record: ArkObjectRecord = {
    ...existing,
    propsJson: props,
    updatedAt: now,
  };
  await ark.objects.upsert(record);
  return objectToTimeEntry(record);
}

async function updateTimeEntry(input: UpdateTimeEntryInput): Promise<TimeEntry> {
  const ark = await getArk();
  const existing = await ark.objects.get(input.id);
  if (!existing) throw new Error(`time_entry ${input.id} not found`);
  const now = new Date().toISOString();
  const props = asObject(existing.propsJson);
  if (input.startedAt !== undefined) props.startedAt = input.startedAt;
  if (input.endedAt !== undefined) props.endedAt = input.endedAt;
  if (input.billable !== undefined) props.billable = input.billable;
  if (input.taskId !== undefined) props.taskId = input.taskId;
  if (input.taskTitle !== undefined) props.taskTitle = input.taskTitle;
  const record: ArkObjectRecord = {
    ...existing,
    title: input.title !== undefined ? input.title : existing.title,
    propsJson: props,
    updatedAt: now,
  };
  await ark.objects.upsert(record);
  return objectToTimeEntry(record);
}

async function createTimeEntry(input: CreateTimeEntryInput): Promise<TimeEntry> {
  const ark = await getArk();
  const id = `te-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
  const now = new Date().toISOString();
  const record: ArkObjectRecord = {
    id,
    typeId: "time_entry_obj",
    title: input.title,
    contentJson: {},
    propsJson: {
      startedAt: input.startedAt,
      endedAt: input.endedAt,
      source: "manual",
      billable: Boolean(input.billable),
      taskId: input.taskId ?? null,
      taskTitle: input.taskTitle ?? null,
    },
    createdAt: input.startedAt,
    updatedAt: now,
    deletedAt: null,
  };
  await ark.objects.upsert(record);
  return objectToTimeEntry(record);
}

async function deleteTimeEntry(id: string): Promise<void> {
  const ark = await getArk();
  await ark.objects.delete(id);
}

async function listTags(): Promise<Tag[]> {
  const ark = await getArk();
  const list = await ark.objects.listByType("tag_obj");
  return list
    .filter((o) => !o.deletedAt)
    .map((o) => {
      const props = asObject(o.propsJson);
      return {
        id: o.id,
        name: o.title ?? "",
        color: (props.color as string | null | undefined) ?? null,
      };
    });
}

async function listDelphiTasks(): Promise<DelphiTask[]> {
  const ark = await getArk();
  const list = await ark.objects.listByType("task_obj");
  return list
    .filter((o) => !o.deletedAt)
    .map((o) => {
      const props = asObject(o.propsJson);
      return {
        id: o.id,
        title: o.title ?? "",
        status: (props.status as string | null | undefined) ?? null,
      };
    });
}

function registerIpc(): void {
  ipcMain.handle("horologion:time-entries:list", () => listTimeEntries());
  ipcMain.handle("horologion:time-entries:list-running", () => listRunningTimeEntries());
  ipcMain.handle(
    "horologion:time-entries:start",
    (_event: IpcMainInvokeEvent, input: StartTimerInput) => startTimer(input),
  );
  ipcMain.handle("horologion:time-entries:stop", (_event: IpcMainInvokeEvent, id: string) =>
    stopTimer(id),
  );
  ipcMain.handle(
    "horologion:time-entries:update",
    (_event: IpcMainInvokeEvent, input: UpdateTimeEntryInput) => updateTimeEntry(input),
  );
  ipcMain.handle(
    "horologion:time-entries:create",
    (_event: IpcMainInvokeEvent, input: CreateTimeEntryInput) => createTimeEntry(input),
  );
  ipcMain.handle("horologion:time-entries:delete", (_event: IpcMainInvokeEvent, id: string) =>
    deleteTimeEntry(id),
  );
  ipcMain.handle("horologion:tags:list", () => listTags());
  ipcMain.handle("horologion:tasks:list", () => listDelphiTasks());
  ipcMain.handle("horologion:ark:status", (): ArkStatus => arkStatus);
}

// Прогреваем ArkClient при старте окна, чтобы статус становился `connected`
// независимо от user-инициированных операций.
app.whenReady().then(() => {
  void getArk().catch(() => {
    // arkStatus уже выставлен в "error" внутри getArk
  });
});

// Когда окно «закрыто» — мы его прячем, приложение продолжает жить в трее
// (таймер pomodoro тикает, ARK-запись пишет на остановке). Реальный выход —
// только через tray-меню «Выйти» (выставляет allowQuit = true).
let allowQuit = false;
let tray: Tray | null = null;
let mainWindow: BrowserWindow | null = null;

function resolveIconPath(): string {
  // В dev иконка лежит рядом с исходниками (apps/horologion/build/icon.png).
  // В packaged build она копируется через `extraResources` в resources/icon.png.
  return isDev
    ? path.resolve(__dirname, "../build/icon.png")
    : path.join(process.resourcesPath ?? "", "icon.png");
}

function createTray(win: BrowserWindow): void {
  if (tray) return;
  const iconPath = resolveIconPath();
  const trayIcon = nativeImage.createFromPath(iconPath).resize({ width: 16, height: 16 });
  tray = new Tray(trayIcon);
  tray.setToolTip("Horologion");
  const menu = Menu.buildFromTemplate([
    {
      label: "Открыть Horologion",
      click: () => {
        win.show();
        win.focus();
      },
    },
    { type: "separator" },
    {
      label: "Выйти",
      click: () => {
        allowQuit = true;
        app.quit();
      },
    },
  ]);
  tray.setContextMenu(menu);
  tray.on("click", () => {
    if (win.isVisible()) win.hide();
    else {
      win.show();
      win.focus();
    }
  });
}

interface PersistedWindowState {
  x?: number;
  y?: number;
  width: number;
  height: number;
  isMaximized?: boolean;
}

function windowStateFile(): string {
  return path.join(app.getPath("userData"), "window-state.json");
}

function loadWindowState(): PersistedWindowState | null {
  try {
    const raw = fs.readFileSync(windowStateFile(), "utf-8");
    const parsed = JSON.parse(raw) as PersistedWindowState;
    if (typeof parsed.width !== "number" || typeof parsed.height !== "number") {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

let saveStateTimer: ReturnType<typeof setTimeout> | null = null;

function scheduleWindowStateSave(win: BrowserWindow): void {
  if (saveStateTimer) clearTimeout(saveStateTimer);
  // Debounce — пользователь может resize'ить рывками; пишем не чаще раза в 400мс.
  saveStateTimer = setTimeout(() => {
    saveWindowState(win);
    saveStateTimer = null;
  }, 400);
}

function saveWindowState(win: BrowserWindow): void {
  if (win.isDestroyed()) return;
  // На minimize не записываем — это не «настоящие» bounds, перезапись затрёт
  // последний реальный размер. (Окно может прятаться в трей через win.hide()
  // — `getBounds` на скрытом окне возвращает корректные предыдущие bounds.)
  if (win.isMinimized()) return;
  try {
    const isMaximized = win.isMaximized();
    // При maximized getBounds возвращает screen bounds — берём normalBounds
    // (внутреннее API), либо просто пропускаем, чтобы при restore был тот же
    // maximized. Electron автоматически восстановит maximized из флага.
    const b = isMaximized ? win.getNormalBounds() : win.getBounds();
    const payload: PersistedWindowState = {
      x: b.x,
      y: b.y,
      width: b.width,
      height: b.height,
      isMaximized,
    };
    fs.writeFileSync(windowStateFile(), JSON.stringify(payload));
  } catch (e) {
    console.error("[window-state] failed to save:", e);
  }
}

function createWindow(): void {
  // Иконка приложения (видна в dev mode и при packaged запуске).
  const iconPath = resolveIconPath();

  // Восстанавливаем размер/позицию из предыдущей сессии. Если файла нет
  // или он битый — fallback на дефолтные дименсии. Electron сам клампит
  // bounds внутрь доступных дисплеев (если монитор отключили — окно
  // переедет на primary).
  const saved = loadWindowState();
  const initialBounds = saved
    ? { x: saved.x, y: saved.y, width: saved.width, height: saved.height }
    : { width: 600, height: 800 };

  const win = new BrowserWindow({
    ...initialBounds,
    minWidth: 420,
    minHeight: 560,
    icon: nativeImage.createFromPath(iconPath),
    // Скрываем системный titlebar — рисуем свой через @kepler/visuals (DesktopChrome).
    // titlebarOverlay даёт нам env(titlebar-area-*) для расчёта safe-area под кнопками окна.
    titleBarStyle: "hidden",
    // titleBarOverlay и backgroundColor нужны Electron'у ДО загрузки рендерера и CSS,
    // поэтому литеральные значения; цвета подобраны под dark-токены kepler-visuals
    // (--sidebar-bg ≈ #171717, --sidebar-foreground ≈ #fafafa).
    titleBarOverlay: { color: "rgba(0,0,0,0)", symbolColor: "#fafafa", height: 44 },
    backgroundColor: "#171717",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
    },
  });

  win.webContents.on("before-input-event", (event, input) => {
    if (input.type !== "keyDown") return;
    const wc = win.webContents;
    // F12 / Ctrl+Shift+I → DevTools toggle (по умолчанию Electron не биндит F12)
    if (input.key === "F12" || (input.control && input.shift && input.key.toLowerCase() === "i")) {
      if (wc.isDevToolsOpened()) wc.closeDevTools();
      else wc.openDevTools({ mode: "detach" });
      event.preventDefault();
      return;
    }
    // Ctrl + = / + → zoom in (фикc Electron'овского default для main keyboard).
    // Ctrl + - / 0 — для надёжности тоже руками.
    if (!input.control || input.alt || input.meta) return;
    if (input.key === "=" || input.key === "+") {
      wc.setZoomLevel(Math.min(wc.getZoomLevel() + 0.5, 9));
      event.preventDefault();
    } else if (input.key === "-") {
      wc.setZoomLevel(Math.max(wc.getZoomLevel() - 0.5, -3));
      event.preventDefault();
    } else if (input.key === "0") {
      wc.setZoomLevel(0);
      event.preventDefault();
    }
  });

  // Восстанавливаем maximized-флаг сразу после ready-to-show, иначе при
  // вызове до показа Electron может проигнорировать.
  if (saved?.isMaximized) {
    win.maximize();
  }

  // Сохраняем bounds на каждое движение/ресайз (debounced) и финально на close.
  // Через `close` ловим и обычный quit (allowQuit=true), и tray-«закрытие» в трей.
  win.on("resize", () => scheduleWindowStateSave(win));
  win.on("move", () => scheduleWindowStateSave(win));
  win.on("maximize", () => scheduleWindowStateSave(win));
  win.on("unmaximize", () => scheduleWindowStateSave(win));

  // Перехватываем «закрытие» — прячем окно вместо выхода.
  win.on("close", (event) => {
    // Финальный flush — debounce-таймер мог не успеть.
    saveWindowState(win);
    if (allowQuit) return;
    event.preventDefault();
    win.hide();
  });

  mainWindow = win;
  createTray(win);

  if (process.env.VITE_DEV_SERVER_URL) {
    void win.loadURL(process.env.VITE_DEV_SERVER_URL);
  } else {
    void win.loadFile(path.join(__dirname, "../dist/index.html"));
  }
}

app.whenReady().then(() => {
  // Windows: AppUserModelID нужен для группировки в taskbar и того, чтобы
  // Windows подхватил иконку из .exe (а не дефолтную electron.exe).
  // Должен совпадать с appId в electron-builder и со ShortcutAppUserModelId
  // в MSI/NSIS shortcut'ах, иначе будет дубль в Start Menu / taskbar.
  if (process.platform === "win32") {
    app.setAppUserModelId("com.kazui.horologion");
  }

  registerIpc();
  createWindow();

  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

// При «закрытии всех окон» НЕ выходим: окно скрыто, приложение крутится в трее.
// Выход — только через tray «Выйти» (allowQuit = true → app.quit()).
app.on("window-all-closed", () => {
  // Намеренно ничего не делаем.
});

// Перед фактическим выходом — корректно закрываем ARK sidecar.
app.on("before-quit", async (event) => {
  if (!arkClient) return;
  event.preventDefault();
  try {
    await arkClient.stop();
  } catch {
    // ignore
  }
  arkClient = null;
  allowQuit = true;
  app.quit();
});

void mainWindow;
