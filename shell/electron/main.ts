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
  crashReporter,
  dialog,
  globalShortcut,
  ipcMain,
  shell,
  Tray,
  Menu,
  nativeImage,
  nativeTheme,
  screen,
} from "electron";
import { resolveInstance, applyInstanceToApp, verifyUserDataMatches } from "./instance";

// КРИТИЧНО: applyInstanceToApp ДОЛЖЕН выполниться до requestSingleInstanceLock
// и до любого app.getPath('userData') / app.getName() — Electron кэширует эти
// значения и singleInstanceLock scope'ится по userData. Импорты ниже могут
// потянуть модули, которые читают app.getPath('userData') на module top-level
// (settings-window.ts, autoupdater-host.ts), — поэтому здесь, не в bootstrap().
const KEPLER_INSTANCE = resolveInstance();
applyInstanceToApp(KEPLER_INSTANCE);
import { spawn, type ChildProcess } from "node:child_process";
import path from "node:path";
import {
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { fileURLToPath } from "node:url";
import { ArkClient, ensureKeplerRunning } from "@kosmos/ark";
import { keplerDataDir } from "./data-dir";
import { keplerLog } from "./logging";
// Side-effect import: регистрирует kepler:diagnostics:* IPC handlers.
import "./diagnostics";
import type { BackendStatus, CommandRecord, SearchResult } from "../shared/ipc-types";
import { COMMANDS, findCommand } from "./commands";
import {
  findDeclaredCommand,
  isExtensionRunning,
  loadDeclaredCommands,
  openExtension,
  setExtensionArkBridge,
} from "./extension-host";
import { listInstalledUserExtensions } from "./extension-installer";
import { openDashboardWindow } from "./dashboard-window";
// Side-effect import — регистрирует IPC handlers для окна настроек
// (kepler:settings:*). Окно создаётся лениво из openSettings().
import {
  openSettings,
  getStoredHotkey,
  setHotkeyReregisterCallback,
  isUsageTrackerEnabled,
  isFocusServiceAutoInstallDeclined,
  setFocusServiceAutoInstallDeclined,
} from "./settings-window";
import { registerMarketplaceIpc, startPeriodicCatalogCheck } from "./extension-marketplace";
// Side-effect: регистрирует kepler:focus-widget:* IPC handlers.
import "./focus-widget";
import { getServiceStatus, runServiceCliElevated, pingService } from "./focus-service";
import { findKextInArgv, openInstallExtensionWindow } from "./install-extension-window";
import {
  check as checkForUpdates,
  getState as getUpdateState,
  install as installUpdate,
  setupAutoUpdater,
} from "./autoupdater-host";
import { setupPomodoroNotifier, teardownPomodoroNotifier } from "./pomodoro-notifier";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const WINDOW_WIDTH = 720;
const WINDOW_HEIGHT = 460;
const WINDOW_STATE_FILENAME = "kepler-shell-window-state.json";

const isDev = !!process.env.VITE_DEV_SERVER_URL;

// Mica — modern Win11 22H2+ backdrop (static texture от desktop wallpaper,
// дешевле acrylic'а в DWM). Env override `KEPLER_BG_MATERIAL=acrylic|mica|none`
// для experiment'ов / fallback. Non-Win11 systems: setBackgroundMaterial no-op'ит,
// окно остаётся opaque (acceptable graceful fallback).
type LauncherBgMaterial = "acrylic" | "mica" | "none";
function resolveLauncherBgMaterial(): LauncherBgMaterial {
  const v = (process.env.KEPLER_BG_MATERIAL ?? "").toLowerCase();
  if (v === "mica" || v === "none" || v === "acrylic") return v;
  return "mica";
}

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let backendProc: ChildProcess | null = null;
let backendLockPath = "";
let isQuiting = false;
let arkClient: ArkClient | null = null;

// --- Backend supervisor (hardening proof loop #5) ---------------------------
// Backend crash → exponential backoff respawn (1s → 5s → 30s → 1min → 2min).
// После 5 неудачных попыток подряд — error dialog, manual restart required.
// Counter сбрасывается если backend проработал > 5 минут (значит crash был
// transient, не permanent).
const BACKEND_RESPAWN_DELAYS_MS = [1000, 5000, 30_000, 60_000, 120_000];
const BACKEND_SUCCESSFUL_RUN_MS = 5 * 60 * 1000;
const BACKEND_MAX_CRASH_STREAK = BACKEND_RESPAWN_DELAYS_MS.length;
let backendCrashStreak = 0;
let backendStartedAt = 0;
let backendRespawnTimer: NodeJS.Timeout | null = null;
let backendCrashDialogShown = false;
// Promise который резолвится когда arkClient готов принимать request'ы.
// Renderer может стрелять kepler:ark:request как только подняло окно —
// до того как initArkClient прошёл handshake. Handler ниже await'ит ready
// (с таймаутом), вместо моментального throw "ArkClient not ready".
let arkClientReady: Promise<ArkClient> | null = null;
let arkClientReadyResolve: ((c: ArkClient) => void) | null = null;
let arkClientReadyReject: ((e: Error) => void) | null = null;
let windowStateSaveTimer: NodeJS.Timeout | null = null;

// --- single instance ---------------------------------------------------------

if (!app.requestSingleInstanceLock()) {
  app.quit();
  process.exit(0);
}

app.on("second-instance", (_event, argv) => {
  // Если второй instance запустился с .kext в argv (file association /
  // повторный запуск через CLI) — форвардим в running instance, открываем
  // install dialog и НЕ показываем launcher.
  const kext = findKextInArgv(argv);
  if (kext) {
    openInstallExtensionWindow(kext);
    return;
  }
  // Если второй instance — autorun (Windows зачем-то выстрелил Run-entry
  // повторно при уже запущенном Kepler), не дёргаем launcher: пользователь
  // не нажимал хоткей.
  if (argv.includes("--autostart")) {
    return;
  }
  showLauncher();
});

// --- Electron crash reporter ------------------------------------------------
// Должен быть инициализирован ДО app.whenReady. uploadToServer: false —
// minidump'ы остаются локально в app.getPath("crashDumps") (по умолчанию
// %APPDATA%/<productName>/Crashpad/). Submit вручную из Settings UI.
crashReporter.start({
  // productName выбирает поддиректорию для minidump'ов:
  //   %APPDATA%/<productName>/Crashpad/. Per-slot — не смешиваем dev и prod
  //   crash'и.
  productName: KEPLER_INSTANCE.productName,
  companyName: "Kosmos",
  uploadToServer: false,
  submitURL: "",
  compress: false,
});

// --- backend spawn -----------------------------------------------------------

function resolveBackendExe(): string {
  // Override через env — нужен для e2e тестов (Playwright spawns Electron
  // напрямую, в этом случае process.resourcesPath указывает на electron's
  // own resources, а не на нашу dist).
  const fromEnv = process.env.KEPLER_BACKEND_EXE;
  if (fromEnv && existsSync(fromEnv)) return fromEnv;

  // Try dev paths first regardless of isDev — Playwright тесты не выставляют
  // VITE_DEV_SERVER_URL, но cargo build выкладывает binary в target/{debug,release}/
  // как при dev так и при first-time test run.
  const devDebug = path.resolve(__dirname, "../../target/debug/kepler-backend.exe");
  if (existsSync(devDebug)) return devDebug;
  const devRelease = path.resolve(__dirname, "../../target/release/kepler-backend.exe");
  if (existsSync(devRelease)) return devRelease;

  // production: рядом с упакованным приложением (extraResources)
  return path.join(process.resourcesPath ?? __dirname, "kepler-backend.exe");
}

// --- Phase 7: boot self-check ------------------------------------------------
// Проверяет invariant'ы slot isolation + наличие backend exe ДО spawnBackend
// и любых window'ов. Failure path — error dialog + app.exit(1). Fail-loud,
// без retry / recovery: если invariant broken, лучше упасть на старте чем
// писать в чужой userData / висеть без backend'а.
function runBootSelfCheck(): void {
  // 1. userData совпадает с тем что resolveInstance насчитал. Защита от
  //    ситуации когда что-то ниже по импортной цепочке успело вызвать
  //    getPath ДО applyInstanceToApp — Electron кэширует первое значение,
  //    и slot isolation тихо ломается. verifyUserDataMatches инкапсулирует
  //    единственный whitelisted вызов getPath('userData') (в instance.ts).
  const verify = verifyUserDataMatches(KEPLER_INSTANCE);
  if (!verify.ok) {
    const msg =
      `Kepler boot self-check failed: userData mismatch.\n` +
      `Expected: ${verify.expected}\n` +
      `Actual:   ${verify.actual}\n` +
      `Slot:     ${KEPLER_INSTANCE.slot}\n\n` +
      `Это означает что applyInstanceToApp не успел отработать до первого ` +
      `чтения userData path. Запустите Kepler заново; если повторяется — ` +
      `см. shell/electron/instance.ts.`;
    keplerLog.error("boot", "userData mismatch", {
      expected: verify.expected,
      actual: verify.actual,
      slot: KEPLER_INSTANCE.slot,
    });
    dialog.showErrorBox("Kepler — ошибка запуска", msg);
    app.exit(1);
    return;
  }

  // 2. Backend exe существует. Без него ничего не работает; show dialog
  //    и выходим, иначе пользователь увидит вечно "загрузка" в launcher.
  const backendExe = resolveBackendExe();
  if (!existsSync(backendExe)) {
    const msg =
      `kepler-backend.exe не найден по ожидаемому пути:\n${backendExe}\n\n` +
      `Возможно установка повреждена. Переустановите Kepler.`;
    keplerLog.error("boot", "backend exe missing", { backendExe });
    dialog.showErrorBox("Kepler — ошибка запуска", msg);
    app.exit(1);
    return;
  }

  // 3. test slot не должен резолвиться в обычном запуске (без
  //    KOSMOS_TEST_MODE=1). Если кто-то случайно прокинул KOSMOS_DATA_DIR
  //    в production env — это ошибка конфигурации.
  if (KEPLER_INSTANCE.kind === "test" && process.env.KOSMOS_TEST_MODE !== "1") {
    const msg =
      `Kepler запущен в test slot (${KEPLER_INSTANCE.slot}) без KOSMOS_TEST_MODE=1.\n` +
      `Это обычно означает что KOSMOS_DATA_DIR / KEPLER_INSTANCE прокинут случайно.\n` +
      `Очистите env и запустите снова.`;
    keplerLog.error("boot", "test slot without KOSMOS_TEST_MODE", {
      slot: KEPLER_INSTANCE.slot,
    });
    dialog.showErrorBox("Kepler — ошибка запуска", msg);
    app.exit(1);
    return;
  }

  keplerLog.info("boot", "self-check passed", {
    slot: KEPLER_INSTANCE.slot,
    kind: KEPLER_INSTANCE.kind,
    userDataDir: KEPLER_INSTANCE.userDataDir,
    dataDir: KEPLER_INSTANCE.dataDir,
    backendExe,
  });
}

function spawnBackend() {
  const exe = resolveBackendExe();
  if (!existsSync(exe)) {
    keplerLog.error("backend", "kepler-backend.exe not found", { exe });
    return;
  }
  const dataDir = keplerDataDir();
  backendLockPath = path.join(dataDir, "kepler.lock.json");
  keplerLog.info("backend", "spawning backend", { exe, dataDir });
  const trackerEnabled = isUsageTrackerEnabled();
  backendProc = spawn(exe, [], {
    detached: false,
    stdio: ["ignore", "pipe", "pipe"],
    // Forwarder KOSMOS_DATA_DIR в backend — обязательно для dev/test
    // изоляции. Production: env пустой, backend defaults к %APPDATA%/Kosmos.
    // KEPLER_USAGE_TRACKER — toggle из Settings; "0" выключает фоновый
    // трекинг активных окон. Изменения применяются после рестарта Kepler.
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      // KEPLER_INSTANCE прокидывается backend'у чтобы он мог tag'нуть
      // crash log'и и (в перспективе) device id под slot.
      KEPLER_INSTANCE: KEPLER_INSTANCE.slot,
      KEPLER_USAGE_TRACKER: trackerEnabled ? "1" : "0",
      // RUST_BACKTRACE=1 → crash_reporter::install получает полный backtrace
      // в `<data_dir>/crashes/panic-*.log`. Production cost ~50KB на panic,
      // приемлемо для responsible shipping.
      RUST_BACKTRACE: "1",
    },
  });
  backendProc.stdout?.on("data", (b) => process.stderr.write(`[kepler-backend] ${b.toString()}`));
  backendProc.stderr?.on("data", (b) => process.stderr.write(`[kepler-backend] ${b.toString()}`));
  backendStartedAt = Date.now();
  backendProc.on("exit", (code) => {
    const ranForMs = Date.now() - backendStartedAt;
    console.error(
      `[kepler-shell] backend exited code=${code} after ${(ranForMs / 1000).toFixed(1)}s`,
    );
    backendProc = null;
    // Без cleanup ArkClient остаётся "connected" к мёртвому WS, все
    // последующие invokeOperation либо висят на reconnect, либо падают.
    // Reset кладёт его в исходное состояние; новый initArkClient() будет
    // вызван при следующем spawnBackend (через supervisor ниже).
    void resetArkClient(`backend exited code=${code}`);
    // Supervisor decision.
    if (isQuiting) return; // shutdown — no respawn
    if (code === 0) return; // clean exit (manual stop / restart IPC) — no respawn
    scheduleBackendRespawn(ranForMs);
  });
}

function scheduleBackendRespawn(lastRanForMs: number): void {
  if (backendRespawnTimer) {
    clearTimeout(backendRespawnTimer);
    backendRespawnTimer = null;
  }
  // Если последний запуск backend проработал > 5min — это transient crash,
  // не цепочка failures. Сбрасываем counter.
  if (lastRanForMs > BACKEND_SUCCESSFUL_RUN_MS) {
    backendCrashStreak = 0;
    backendCrashDialogShown = false;
  }
  if (backendCrashStreak >= BACKEND_MAX_CRASH_STREAK) {
    showBackendCrashDialog();
    return;
  }
  const delay = BACKEND_RESPAWN_DELAYS_MS[backendCrashStreak] ?? 120_000;
  backendCrashStreak += 1;
  console.error(
    `[kepler-shell] supervisor: respawn attempt ${backendCrashStreak}/${BACKEND_MAX_CRASH_STREAK} in ${delay / 1000}s`,
  );
  backendRespawnTimer = setTimeout(() => {
    backendRespawnTimer = null;
    if (isQuiting) return;
    keplerLog.warn("supervisor", "respawning backend", { streak: backendCrashStreak });
    spawnBackend();
    // Дёргаем ARK reinit — старый promise сброшен в resetArkClient,
    // initArkClient создаст новый.
    void initArkClient();
  }, delay);
}

function showBackendCrashDialog(): void {
  if (backendCrashDialogShown) return;
  backendCrashDialogShown = true;
  keplerLog.error("supervisor", "backend crashed too many times in a row, giving up", {
    streak: backendCrashStreak,
  });
  const crashesDir = path.join(keplerDataDir(), "crashes");
  // dialog.showMessageBox — async, не блокирует event loop. Отдельный
  // import dialog уже есть в shell.
  void dialog
    .showMessageBox({
      type: "error",
      title: "Kepler — backend не запускается",
      message: "kepler-backend упал 5 раз подряд. Автоматический перезапуск приостановлен.",
      detail:
        "Откройте Настройки → Диагностика и посмотрите последние crash-логи.\n\n" +
        `Папка с отчётами: ${crashesDir}\n\n` +
        "После проверки попробуйте: Настройки → Перезапустить backend.",
      buttons: ["OK"],
      defaultId: 0,
    })
    .catch((e) => {
      keplerLog.error("supervisor", "dialog failed", { err: String(e) });
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
  return path.join(keplerDataDir(), WINDOW_STATE_FILENAME);
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
    keplerLog.warn("window", "saveWindowState failed", { err: String(e) });
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
    paintWhenInitiallyHidden: true,
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
    backgroundMaterial: resolveLauncherBgMaterial(),
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
    mainWindow.setBackgroundMaterial(resolveLauncherBgMaterial());
  } catch (e) {
    console.error("[kepler-shell] setBackgroundMaterial failed:", e);
  }

  // Hide launcher при потере фокуса (клик вне окна / Alt+Tab).
  // В dev пропускаем если фокус ушёл на DevTools — иначе нечем отлаживать.
  mainWindow.on("blur", () => {
    if (isDev && mainWindow?.webContents.isDevToolsFocused()) return;
    hideLauncher();
  });
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

// Instant show/hide без Windows DWM fade. Окно остаётся в нужной позиции,
// но при hide ставится opacity 0 + setIgnoreMouseEvents(true) (клики
// проходят сквозь). При show — opacity 1 + setIgnoreMouseEvents(false) +
// focus. Бounds не двигаем — это вызывало пропадание контента (Chromium
// прекращал painting когда окно полностью off-screen).
let launcherHidden = true;

function showLauncher() {
  if (!mainWindow) createLauncher();
  if (!mainWindow) return;
  const headless = process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
  const saved = loadWindowState();
  const pos = saved ?? defaultLauncherPosition();
  mainWindow.setBounds({
    x: pos.x,
    y: pos.y,
    width: WINDOW_WIDTH,
    height: WINDOW_HEIGHT,
  });
  mainWindow.setIgnoreMouseEvents(false);
  mainWindow.setOpacity(1);
  // В headless / test mode окно НИКОГДА не показывается визуально — Playwright
  // работает через webContents без paint'а. Renderer всё равно получает
  // `kepler:window:show` для focus/refresh, и `launcherHidden` обновляется.
  if (!headless) {
    if (!mainWindow.isVisible()) {
      mainWindow.showInactive();
    }
    mainWindow.focus();
    // Гарантируем что окно реально окажется на переднем плане (нужно для
    // post-update flow: процесс только что перезапустился и Windows может
    // отдать focus текущему foreground app). Снимаем флаг через 800мс —
    // постоянный always-on-top раздражает.
    if (!mainWindow.isAlwaysOnTop()) {
      const win = mainWindow;
      win.setAlwaysOnTop(true);
      setTimeout(() => {
        if (!win.isDestroyed()) win.setAlwaysOnTop(false);
      }, 800);
    }
  }
  launcherHidden = false;
  mainWindow.webContents.send("kepler:window:show");
}

function hideLauncher() {
  if (!mainWindow || mainWindow.isDestroyed()) return;
  if (launcherHidden) return;
  launcherHidden = true;
  // Раньше делали `setOpacity(0) + setIgnoreMouseEvents(true)` — это
  // визуально «прятало» окно, но Win32 EnumWindows / GetWindowList всё
  // ещё видели его как visible top-level window. Скриншот-апы (Snipping
  // Tool, ShareX) ловили пустой прямоугольник в кадр; Raycast/PowerToys
  // фильтруют по `IsWindowVisible` (= ShowWindow state) и не видели —
  // отсюда асимметрия. Настоящий `hide()` вызывает `ShowWindow(SW_HIDE)`,
  // окно уходит из enum'а для всех инструментов.
  mainWindow.hide();
}

// Окно теперь fixed-size (WINDOW_HEIGHT) — никакой compact/expanded логики.
// Renderer показывает список объектов всегда; на typing просто фильтрует.
// IPC обработчик оставлен для backward-compat с preload bridge, но noop.
function setLauncherExpanded(_expanded: boolean) {
  // no-op
}

// --- tray --------------------------------------------------------------------

function resolveTrayIconPath(): string | null {
  // production install: electron-builder копирует build/icon.png в
  // <install>/resources/icon.png через extraResources. process.resourcesPath
  // указывает на тот же `resources/` dir в production, поэтому это первый
  // кандидат и работает в installed Kepler.
  // dev: __dirname = shell/dist-electron, нужен относительный путь к
  // build/icon.png в source tree. В dev-сессии используем dev.png — визуально
  // отличает trayIcon разработческого инстанса от установленного.
  const iconName = isDev ? "dev.png" : "icon.png";
  const candidates: string[] = [];
  if (process.resourcesPath) {
    candidates.push(path.join(process.resourcesPath, iconName));
  }
  candidates.push(path.resolve(__dirname, `../build/${iconName}`));
  candidates.push(path.resolve(__dirname, `../../build/${iconName}`));
  for (const c of candidates) {
    if (existsSync(c)) return c;
  }
  return null;
}

function createTray() {
  const iconPath = resolveTrayIconPath();
  const icon = iconPath ? nativeImage.createFromPath(iconPath) : nativeImage.createEmpty();
  tray = new Tray(icon);
  tray.setToolTip(KEPLER_INSTANCE.productName);
  tray.setContextMenu(
    Menu.buildFromTemplate([
      { label: "Открыть", click: () => showLauncher() },
      { label: "Настройки", click: () => openSettings() },
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

// Global space id для всех apps: концепция spaces убрана 2026-05-15, БД одна
// на юзера. Передаём backend'у через ArkClient как фиксированный id, чтобы LAN
// sync namespace был стабильным между запусками.
const KEPLER_SPACE_ID = "kepler-default";

function ensureArkReadyPromise(): Promise<ArkClient> {
  if (arkClient) return Promise.resolve(arkClient);
  if (arkClientReady) return arkClientReady;
  arkClientReady = new Promise<ArkClient>((resolve, reject) => {
    arkClientReadyResolve = resolve;
    arkClientReadyReject = reject;
  });
  return arkClientReady;
}

// Сбрасывает state ArkClient'а и резолверы. Звать при backend exit / restart /
// init failure. После reset следующий ensureArkReadyPromise() создаст свежий
// promise — иначе старые pending awaitArkReady() висят пока не отвалятся по
// 15-секундному таймауту.
// Broadcast lifecycle ARK-клиента всем окнам. Renderer / e2e тесты слушают
// `kepler:backend:ready` вместо polling'а `kepler:backend:status`. Phase 1
// determinism — см. .agent/tasks/2026-05-21-bug-detection-phase1-determinism/.
function broadcastBackendEvent(
  event: "kepler:backend:ready" | "kepler:backend:disconnected",
): void {
  for (const w of BrowserWindow.getAllWindows()) {
    if (w.isDestroyed()) continue;
    try {
      w.webContents.send(event);
    } catch {
      // окно может быть в процессе destroy; игнор.
    }
  }
}

async function resetArkClient(reason: string): Promise<void> {
  const wasConnected = arkClient !== null;
  const err = new Error(`ArkClient reset: ${reason}`);
  arkClientReadyReject?.(err);
  arkClientReadyResolve = null;
  arkClientReadyReject = null;
  arkClientReady = null;
  const prev = arkClient;
  arkClient = null;
  setExtensionArkBridge({ request: null, subscribe: null });
  if (wasConnected) broadcastBackendEvent("kepler:backend:disconnected");
  // pomodoro-notifier держит ref на старый arkClient через onArkEvent callback —
  // отписываем до stop(), иначе при следующем setupPomodoroNotifier останется
  // double-subscribe на новый клиент.
  teardownPomodoroNotifier();
  if (prev) {
    try {
      await prev.stop();
    } catch (e) {
      keplerLog.warn("ark", "ArkClient stop failed", { err: String(e) });
    }
  }
}

export async function awaitArkReady(timeoutMs = 15000): Promise<ArkClient> {
  if (arkClient) return arkClient;
  const p = ensureArkReadyPromise();
  let timer: NodeJS.Timeout | null = null;
  try {
    return await Promise.race([
      p,
      new Promise<ArkClient>((_, rej) => {
        timer = setTimeout(() => rej(new Error("ArkClient not ready (timeout)")), timeoutMs);
      }),
    ]);
  } finally {
    if (timer) clearTimeout(timer);
  }
}

async function initArkClient(): Promise<void> {
  try {
    const spaceId = KEPLER_SPACE_ID;
    ensureArkReadyPromise(); // создаём promise до handshake'а если ещё нет
    // kepler-shell сам спавнит kepler-backend выше (spawnBackend), здесь
    // только ждём lock-файл и коннектимся через WS. autoLaunch=false — повторно
    // не запускаем.
    // Передаём dataDir явно — в shell main process env KOSMOS_DATA_DIR
    // не set (мы его выставляем только для backend child в spawnBackend).
    // Без этого resolveLockPath fallback'ил к %APPDATA%/Kosmos/ и не
    // находил lock в Kosmos-dev/ (dev mode) — отсюда «ArkClient not ready».
    const state = await ensureKeplerRunning({
      appDataPath: app.getPath("appData"),
      dataDir: keplerDataDir(),
      waitMs: 10000,
      autoLaunch: false,
    });
    if (state.kind !== "connected") {
      console.error(`[kepler-shell] kepler-backend ${state.kind}: ArkClient unavailable`);
      // Без reject pending awaitArkReady() висят 15s и потом получают
      // generic "timeout" вместо реальной причины. Сбрасываем promise чтобы
      // следующий ensureArkReadyPromise() (после backend respawn) попробовал
      // заново.
      const err = new Error(`kepler-backend ${state.kind}`);
      arkClientReadyReject?.(err);
      arkClientReadyResolve = null;
      arkClientReadyReject = null;
      arkClientReady = null;
      return;
    }
    // DeviceId стабилен внутри slot'а (не зависит от точного userData path,
    // который меняется между OS / user account'ами). Каждый slot — отдельный
    // sync identity.
    const deviceId = `kepler-shell-${KEPLER_INSTANCE.slot}`;
    const client = new ArkClient({
      spaceId,
      deviceId,
      deviceName: "Kepler Shell",
      keplerLock: state.lock,
    });
    await client.start();
    arkClient = client;
    arkClientReadyResolve?.(client);
    broadcastBackendEvent("kepler:backend:ready");
    console.error(
      `[kepler-shell] ArkClient connected to kepler-backend (pid ${state.lock.pid}, ws_port ${state.lock.ws_port})`,
    );
    // Pomodoro main-process notifier: подписывается на pomodoro_phase_changed
    // через свежий arkClient. Idempotent — при reconnect старый sub отпишется
    // первым делом. См. shell/electron/pomodoro-notifier.ts.
    try {
      setupPomodoroNotifier({ arkClient: client });
    } catch (e) {
      keplerLog.error("pomodoro-notifier", "setup failed", { err: String(e) });
    }
    // Bridge для Vue-extensions: extension-host прокидывает renderer-запросы
    // сюда через IPC. invokeOperation — public escape-hatch для generic RPC,
    // onArkEvent — generic подписка, фильтруем по event-имени.
    setExtensionArkBridge({
      request: async (req) => {
        if (!arkClient) throw new Error("ArkClient not ready");
        return arkClient.invokeOperation(req as { operation: string; [key: string]: unknown });
      },
      subscribe: (event, handler) => {
        if (!arkClient) return () => {};
        return arkClient.onArkEvent((e) => {
          if (e.event === event) handler(e);
        });
      },
    });
    // Subscribe на commands_changed → пушим renderer'у сигнал перефетчить
    // список (он сам вызовет kepler:commands:list). Сам список не шлём —
    // renderer должен пройти через тот же merge-pipeline (static + dynamic).
    client.commands.onChanged(() => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        mainWindow.webContents.send("kepler:commands:updated");
      }
    });
  } catch (e) {
    keplerLog.error("ark", "ArkClient init failed", { err: String(e) });
    arkClientReadyReject?.(e instanceof Error ? e : new Error(String(e)));
  }
}

// --- IPC handlers ------------------------------------------------------------

ipcMain.handle("kepler:backend:status", () => readBackendStatus());

// --- Test rig (gated by KOSMOS_TEST_MODE=1) ---------------------------------
// Production preload не зовёт эти каналы; контракт preload exposes их только
// в test mode. Если случайно вызвал из production — IPC throw'нет «no handler».
if (process.env.KOSMOS_TEST_MODE === "1") {
  ipcMain.handle("kepler:__test:waitForReady", async (_e, timeoutMs?: number): Promise<void> => {
    const ms = typeof timeoutMs === "number" && timeoutMs > 0 ? timeoutMs : 15000;
    await awaitArkReady(ms);
  });
  ipcMain.handle("kepler:__test:getStats", async () => {
    let commands: string[] = [];
    if (arkClient) {
      try {
        const list = await arkClient.commands.list();
        if (Array.isArray(list)) commands = list.map((c) => c.id);
      } catch {
        // backend может быть в процессе reconnect'а — пустой список.
      }
    }
    return {
      arkConnected: arkClient !== null,
      commands,
      commandsRegistered: commands.length,
    };
  });
}

// --- Diagnostics / crashes IPC (hardening #1) -------------------------------
function crashesDirPath(): string {
  return path.join(keplerDataDir(), "crashes");
}

function listCrashLogs(): Array<{ name: string; size: number; mtime: string }> {
  const dir = crashesDirPath();
  if (!existsSync(dir)) return [];
  try {
    // Lazy readdir — небольшая директория, sync OK.
    const entries = require("node:fs").readdirSync(dir) as string[];
    return entries
      .filter((f) => f.endsWith(".log") || f.endsWith(".dmp"))
      .map((f) => {
        const full = path.join(dir, f);
        const stats = require("node:fs").statSync(full);
        return {
          name: f,
          size: stats.size,
          mtime: new Date(stats.mtimeMs).toISOString(),
        };
      })
      .sort((a, b) => b.mtime.localeCompare(a.mtime));
  } catch (e) {
    console.error("[kepler-shell] listCrashLogs failed:", e);
    return [];
  }
}

ipcMain.handle("kepler:crashes:list", () => listCrashLogs());

ipcMain.handle("kepler:crashes:openFolder", async () => {
  const dir = crashesDirPath();
  try {
    mkdirSync(dir, { recursive: true });
  } catch (e) {
    console.error("[kepler-shell] crashes:openFolder mkdir failed:", e);
  }
  const result = await shell.openPath(dir);
  if (result) {
    console.error("[kepler-shell] crashes:openFolder error:", result);
  }
});

ipcMain.handle("kepler:crashes:clear", () => {
  const dir = crashesDirPath();
  if (!existsSync(dir)) return { removed: 0 };
  let removed = 0;
  try {
    const entries = require("node:fs").readdirSync(dir) as string[];
    for (const f of entries) {
      try {
        unlinkSync(path.join(dir, f));
        removed += 1;
      } catch (e) {
        console.error(`[kepler-shell] crashes:clear failed for ${f}:`, e);
      }
    }
  } catch (e) {
    console.error("[kepler-shell] crashes:clear readdir failed:", e);
  }
  return { removed };
});

ipcMain.handle("kepler:backend:restart", async () => {
  // Manual restart — это user action, не supervisor failure. Сбрасываем
  // crash streak counter чтобы dialog не показывался если backend упадёт
  // позже (даём свежий window of opportunity).
  backendCrashStreak = 0;
  backendCrashDialogShown = false;
  if (backendRespawnTimer) {
    clearTimeout(backendRespawnTimer);
    backendRespawnTimer = null;
  }
  await resetArkClient("backend restart");
  if (backendProc && !backendProc.killed) {
    backendProc.kill();
  }
  spawnBackend();
  // initArkClient() сам await'ит lock-файл; не блокируем restart handler
  // на всё время handshake'а — renderer покажет "загрузка" через ark:request.
  void initArkClient();
});

ipcMain.handle("kepler:window:hide", () => hideLauncher());

ipcMain.handle("kepler:window:setExpanded", (_e, expanded: boolean) =>
  setLauncherExpanded(!!expanded),
);

ipcMain.handle("kepler:search:query", async (_e, text: string): Promise<SearchResult[]> => {
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
    keplerLog.warn("search", "ARK search failed", { err: String(e) });
    return [];
  }
});

function staticCommands(): CommandRecord[] {
  // Filter: команды с requiresExtension показываются только если этот
  // extension реально установлен (manifest.json в %APPDATA%\Kosmos\extensions\).
  // Snapshot installed ids per-call — listInstalledUserExtensions делает
  // disk scan, дёшево (4-10 dir entries).
  let installedIds: Set<string>;
  try {
    installedIds = new Set(listInstalledUserExtensions().map((e) => e.id));
  } catch {
    installedIds = new Set();
  }

  return COMMANDS.filter((c) => !c.requiresExtension || installedIds.has(c.requiresExtension)).map(
    (c) => ({
      id: c.id,
      title: c.title,
      subtitle: c.subtitle,
      category: c.category,
      kind: c.kind,
      appName: c.appName,
      icon: c.icon?.(),
    }),
  );
}

/**
 * Resolve три источника команд в единый список (priority: internal >
 * manifest-declared > runtime-dynamic). См.
 * `docs-site/concepts/command-bus.md`.
 */
ipcMain.handle("kepler:commands:list", async (): Promise<CommandRecord[]> => {
  const byId = new Map<string, CommandRecord>();

  // 1) Kepler-internal (settings/dashboard/check-updates).
  for (const c of staticCommands()) byId.set(c.id, c);

  // 2) Manifest-declared из всех установленных + dev-tree extension'ов.
  try {
    for (const cmd of loadDeclaredCommands()) {
      if (byId.has(cmd.id)) continue; // internal priority
      byId.set(cmd.id, {
        id: cmd.id,
        title: cmd.title,
        subtitle: cmd.subtitle,
        category: cmd.category,
        kind: cmd.kind,
        appName: cmd.appName,
        icon: cmd.icon,
      });
    }
  } catch (e) {
    keplerLog.error("commands", "loadDeclaredCommands failed", { err: String(e) });
  }

  // 3) Runtime dynamic (commands.register от running extension'ов).
  //    Видны только пока соответствующий extension запущен.
  if (arkClient) {
    try {
      const dynamic = await arkClient.commands.list();
      if (Array.isArray(dynamic)) {
        for (const c of dynamic) {
          if (byId.has(c.id)) continue; // internal/manifest priority
          const d = c as CommandRecord & { kind?: "app" | "command"; appName?: string };
          byId.set(c.id, {
            id: c.id,
            title: c.title,
            subtitle: c.subtitle,
            category: c.category,
            kind: d.kind,
            appName: d.appName,
          });
        }
      } else {
        console.warn("[kepler-shell] commands.list returned non-array:", dynamic);
      }
    } catch (e) {
      keplerLog.warn("commands", "commands.list (dynamic) failed", { err: String(e) });
    }
  }

  return Array.from(byId.values());
});

/**
 * Auto-launch helper: если extension не запущен, openExtension + ждём пока
 * mount успеет зарегистрировать command listener'ы. Используется V2
 * dynamic action invoke flow (см. `kepler:commands:invoke` ниже).
 *
 * Через ARK we опрашиваем commands.list пока в нём не появится команда —
 * это надёжнее чем polling по extensionWindows (window появляется до того
 * как Vue mount + commands.register IPC отработал).
 */
async function awaitExtensionCommand(
  _extensionId: string,
  fullCommandId: string,
  timeoutMs = 5000,
): Promise<boolean> {
  if (!arkClient) return false;
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeoutMs) {
    try {
      const list = await arkClient.commands.list();
      if (Array.isArray(list) && list.some((c) => c.id === fullCommandId)) {
        return true;
      }
    } catch {
      // ARK rpc race — retry
    }
    await new Promise((r) => setTimeout(r, 100));
  }
  return false;
}

ipcMain.handle("kepler:commands:invoke", async (_e, id: string): Promise<void> => {
  // 1) Internal commands win — exec локально.
  const internal = findCommand(id);
  if (internal) {
    try {
      await internal.exec();
    } catch (e) {
      console.error(`[kepler-shell] command ${id} failed:`, e);
    }
    hideLauncher();
    return;
  }

  // 2) Manifest-declared — open или action mode.
  const declared = findDeclaredCommand(id);
  if (declared) {
    if (declared.mode === "open") {
      await openExtension(declared.extensionId, declared.route);
      hideLauncher();
      return;
    }
    // action mode: dynamic invoke через ARK. Auto-launch если extension
    // не запущен — окно открывается, ждём commands.register, dispatch'им.
    if (!isExtensionRunning(declared.extensionId)) {
      await openExtension(declared.extensionId, declared.route);
      const ready = await awaitExtensionCommand(declared.extensionId, id);
      if (!ready) {
        console.warn(
          `[kepler-shell] auto-launch для action команды ${id}: extension не зарегистрировал её в течение 5s`,
        );
        hideLauncher();
        return;
      }
    }
    if (arkClient) {
      try {
        await arkClient.commands.invoke(id);
      } catch (e) {
        console.error(`[kepler-shell] declared action invoke ${id} failed:`, e);
      }
    }
    hideLauncher();
    return;
  }

  // 3) Runtime dynamic — backend broadcasts command_invoked.
  if (arkClient) {
    // Auto-launch: если id начинается с `<extId>:` и extension установлен
    // но не running — ткнуть openExtension + ждать. Это покрывает кейс
    // когда extension через `commands.register` объявил action-команду в
    // manifest.tests, а пользователь её триггерит из launcher'а после
    // того как extension успел зарегистрировать (история launcher'а).
    const colonIdx = id.indexOf(":");
    if (colonIdx > 0) {
      const extId = id.slice(0, colonIdx);
      if (!isExtensionRunning(extId)) {
        await openExtension(extId);
        await awaitExtensionCommand(extId, id);
      }
    }
    try {
      await arkClient.commands.invoke(id);
    } catch (e) {
      console.error(`[kepler-shell] dynamic command ${id} invoke failed:`, e);
    }
  } else {
    console.warn(`[kepler-shell] unknown command (no arkClient): ${id}`);
  }
  hideLauncher();
});

ipcMain.handle(
  "kepler:ark:request",
  async (_e, operation: string, params?: Record<string, unknown>) => {
    if (typeof operation !== "string" || operation.length === 0) {
      throw new Error("kepler:ark:request: operation must be a non-empty string");
    }
    // Renderer (Dashboard / extensions) может стрелять до того как initArkClient
    // прошёл handshake — ждём ready (up to 15s) вместо моментального throw.
    const client = await awaitArkReady();
    const req: Record<string, unknown> = { operation, ...(params ?? {}) };
    return client.invokeOperation(req as { operation: string; [key: string]: unknown });
  },
);

// --- Phase 7: Universal per-type data export ---------------------------------
// Тонкая прокси на backend WS operations `export.list` / `export.run`.
// Конвертеры регистрируются в kepler-backend, shell ничего о них не знает —
// просто показывает список и запускает.

ipcMain.handle("kepler:export:list", async () => {
  const client = await awaitArkReady();
  // Backend returns `{ converters: [...] }`. Renderer ожидает плоский массив.
  const resp = (await client.invokeOperation({ operation: "export.list" })) as
    | { converters?: unknown }
    | unknown[]
    | null;
  if (Array.isArray(resp)) return resp;
  if (
    resp &&
    typeof resp === "object" &&
    Array.isArray((resp as { converters?: unknown }).converters)
  ) {
    return (resp as { converters: unknown[] }).converters;
  }
  return [];
});

ipcMain.handle(
  "kepler:export:run",
  async (_e, args: { converter_id: string; format: string; dest_dir: string }) => {
    if (!args || typeof args.converter_id !== "string") {
      throw new Error("kepler:export:run: invalid args");
    }
    const client = await awaitArkReady();
    return client.invokeOperation({
      operation: "export.run",
      converter_id: args.converter_id,
      format: args.format,
      dest_dir: args.dest_dir,
    });
  },
);

ipcMain.handle("kepler:export:pickDir", async (e): Promise<string | null> => {
  const win = BrowserWindow.fromWebContents(e.sender);
  const result = win
    ? await dialog.showOpenDialog(win, {
        title: "Выберите папку для экспорта",
        properties: ["openDirectory", "createDirectory"],
      })
    : await dialog.showOpenDialog({
        title: "Выберите папку для экспорта",
        properties: ["openDirectory", "createDirectory"],
      });
  if (result.canceled || result.filePaths.length === 0) return null;
  return result.filePaths[0];
});

ipcMain.handle("kepler:objects:listRecent", async (_e, limit?: number): Promise<SearchResult[]> => {
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
    keplerLog.warn("objects", "objects.list failed", { err: String(e) });
    return [];
  }
});

// --- autoUpdater IPC ----------------------------------------------------------
// Implementation в `./autoupdater-host.ts` — state machine + broadcast.
// Settings UI subscribes к `kepler:settings:update:state` event channel.

ipcMain.handle("kepler:settings:update:check", () => checkForUpdates());
ipcMain.handle("kepler:settings:update:install", () => {
  installUpdate();
  return true;
});
ipcMain.handle("kepler:settings:update:state", () => getUpdateState());

// --- Focus service control (Phase 2) -----------------------------------------
// Soft Windows Service для hosts file management — опционально устанавливается
// юзером через Settings → Focus. Когда running → focus-block.ts использует
// pipe path (no UAC). Не установлен → helper bin fallback path (UAC per toggle).

ipcMain.handle("kepler:focus-service:status", () => getServiceStatus());
ipcMain.handle("kepler:focus-service:ping", () => pingService());
ipcMain.handle("kepler:focus-service:install", async () => {
  const result = await runServiceCliElevated("install");
  if (result.ok) {
    setFocusServiceAutoInstallDeclined(false);
    // Broadcast чтобы Settings UI обновил статус.
    for (const win of BrowserWindow.getAllWindows()) {
      if (!win.isDestroyed()) {
        try {
          win.webContents.send("kepler:focus-service:status-changed");
        } catch {
          /* ignore */
        }
      }
    }
  }
  return result;
});
ipcMain.handle("kepler:focus-service:uninstall", async () => {
  const result = await runServiceCliElevated("uninstall");
  if (result.ok) {
    for (const win of BrowserWindow.getAllWindows()) {
      if (!win.isDestroyed()) {
        try {
          win.webContents.send("kepler:focus-service:status-changed");
        } catch {
          /* ignore */
        }
      }
    }
  }
  return result;
});
ipcMain.handle("kepler:focus-service:start", () => runServiceCliElevated("start"));
ipcMain.handle("kepler:focus-service:stop", () => runServiceCliElevated("stop"));
ipcMain.handle("kepler:focus-service:auto-install-declined:get", () =>
  isFocusServiceAutoInstallDeclined(),
);
ipcMain.handle("kepler:focus-service:auto-install-declined:set", (_e, value: boolean) => {
  setFocusServiceAutoInstallDeclined(!!value);
});

// --- lifecycle ---------------------------------------------------------------

app.whenReady().then(async () => {
  // Phase 7 boot self-check: проверяем slot isolation invariant'ы +
  // backend exe до того как что-либо стартует. Failure → exit(1).
  runBootSelfCheck();

  // Принудительно темная тема — чтобы acrylic backgroundMaterial использовал
  // dark variant независимо от Windows system theme (иначе на light theme
  // launcher просвечивает белым).
  nativeTheme.themeSource = "dark";

  // Диагностика: был ли запуск из autorun (Windows HKCU\...\Run). Маркер
  // --autostart выставляется в setAutostartEnabled() через args. Запись
  // в stdout попадает в crash log / electron log если backend crash'нется
  // на early init. Сейчас launcher по умолчанию hidden (`launcherHidden = true`),
  // поэтому при autorun пользователь видит только tray icon — это by design.
  const startedFromAutorun = process.argv.includes("--autostart");
  if (startedFromAutorun) {
    console.log(
      "[kepler-shell] launched from Windows autorun (--autostart marker present); launcher remains hidden, tray icon only",
    );
  }

  spawnBackend();
  createLauncher();
  createTray();

  void initArkClient();

  registerMarketplaceIpc();
  // autoupdater + periodic marketplace check разрешены только в prod slot'е.
  // Dev / dev-<x> / test не должны пуллить релизы и спамить GitHub.
  setupAutoUpdater({ isDev: !KEPLER_INSTANCE.autoupdaterEnabled });

  // Post-update first launch: если только что обновились через
  // quitAndInstall (autoupdater-host пишет флаг в userData/post-update.flag),
  // открываем launcher автоматически и пробрасываем событие в renderer —
  // тот покажет одноразовый banner «Kepler обновлён до vX.Y.Z».
  // Версию берём из `app.getVersion()` уже после старта (это новая версия —
  // процесс перезапущен с обновлённым кодом). Backwards-compat: старый формат
  // флага (просто timestamp число) тоже принимаем — версию всё равно
  // получаем из app.getVersion().
  try {
    const flag = path.join(keplerDataDir(), "post-update.flag");
    if (existsSync(flag)) {
      // Содержимое не используем (формат может быть как старый — number-as-string,
      // так и новый — JSON `{at: number}`). Главное — сам факт наличия флага.
      try {
        readFileSync(flag, "utf8");
      } catch {
        /* ignore — флаг всё равно удаляем */
      }
      unlinkSync(flag);
      const newVersion = app.getVersion();
      showLauncher();
      // Renderer подписывается на `kepler:post-update` через preload
      // (см. LauncherView.vue → onPostUpdateShown). Шлём после небольшой
      // задержки, чтобы renderer успел смонтироваться, если launcher
      // только что был создан в createLauncher().
      setTimeout(() => {
        if (mainWindow && !mainWindow.isDestroyed()) {
          try {
            mainWindow.webContents.send("kepler:post-update", {
              version: newVersion,
            });
          } catch {
            /* dead webContents — skip */
          }
        }
      }, 300);
    }
  } catch (e) {
    console.warn("[kepler-shell] post-update flag handling failed:", e);
  }
  // Skip periodic в test mode чтобы Playwright не делал HTTPS вызовов.
  // Skip также в dev / dev-<x> чтобы dev-инстансы не спамили GitHub.
  if (process.env.KOSMOS_TEST_MODE !== "1" && KEPLER_INSTANCE.periodicMarketplaceCheckEnabled) {
    startPeriodicCatalogCheck();
  }

  // Если процесс был запущен с .kext в argv (file association / CLI) —
  // открываем install dialog сразу после whenReady. Launcher остаётся
  // hidden (default behavior); пользователь видит только install dialog.
  const initialKext = findKextInArgv(process.argv);
  if (initialKext) {
    openInstallExtensionWindow(initialKext);
  }

  // BENCHMARK: KEPLER_BENCHMARK_OPEN_ALL=1 → автоматически открыть все
  // мигрированные extensions + Dashboard для RAM-измерения. После warmup 5s.
  if (process.env.KEPLER_BENCHMARK_OPEN_ALL === "1") {
    setTimeout(() => {
      for (const id of ["horologion", "delphi", "arrancador", "eden"]) {
        void openExtension(id).catch((e) => console.error(`bench open ${id} failed:`, e));
      }
      // Dashboard — встроенный shell view (не extension); открывается через
      // tray-команду.
      try {
        openDashboardWindow();
      } catch (e) {
        console.error("bench open dashboard failed:", e);
      }
    }, 5000);
  }

  // Anti-repeat по delta-времени между fire'ами. Windows key-repeat шлёт
  // WM_HOTKEY каждые ~33 мс пока сочетание зажато; тап-тап (с реальным
  // отпусканием пробела) даёт паузу >>=100 мс. Threshold 80 мс отрезает
  // auto-repeat но пропускает быстрые тапы (>12 Hz всё равно бывает редко).
  // Глобальный accelerator при этом всегда зарегистрирован — Windows не
  // выдаёт Alt+Space в системные меню окна.
  let lastFireAt = 0;
  const showHide = () => {
    const now = Date.now();
    const gap = now - lastFireAt;
    lastFireAt = now;
    if (gap < 80) return; // auto-repeat от удержания
    if (launcherHidden) showLauncher();
    else hideLauncher();
  };
  // Slot'ы без hotkey (dev-<x>, test-<x>) — пропускаем регистрацию вовсе.
  // Пользователь активирует launcher через tray click. Это критично для
  // multi-dev: два инстанса не могут поделить один accelerator, второй
  // молча проиграл бы Windows OS race.
  if (KEPLER_INSTANCE.hotkey !== null) {
    let currentAccelerator = getStoredHotkey();
    function tryRegister(accelerator: string): boolean {
      try {
        if (globalShortcut.isRegistered(currentAccelerator)) {
          globalShortcut.unregister(currentAccelerator);
        }
        const reg = globalShortcut.register(accelerator, showHide);
        if (reg) {
          currentAccelerator = accelerator;
          console.log(`[kepler-shell] globalShortcut ${accelerator} registered`);
          return true;
        }
        // Откатываемся на предыдущий, если новая регистрация не удалась.
        globalShortcut.register(currentAccelerator, showHide);
        return false;
      } catch (e) {
        console.error(`[kepler-shell] globalShortcut register error:`, e);
        return false;
      }
    }
    const ok = tryRegister(currentAccelerator);
    setHotkeyReregisterCallback(tryRegister);
    if (!ok) {
      console.error(`[kepler-shell] globalShortcut ${currentAccelerator} register failed`);
    }
  } else {
    console.log(`[kepler-shell] hotkey disabled for slot ${KEPLER_INSTANCE.slot} — use tray click`);
  }
  // F12 toggle DevTools (dev mode только) — глобальный hotkey удобнее чем
  // accelerator menu, т.к. меню у frameless окна нет. F12 не конфликтует
  // между параллельными dev-инстансами потому что Windows route'ит global
  // accelerator к одному фокусному окну; в multi-dev только активное окно
  // получит toggle, остальные тихо ничего не делают.
  if (isDev) {
    globalShortcut.register("F12", () => {
      mainWindow?.webContents.toggleDevTools();
    });
  }
});

app.on("window-all-closed", () => {
  // Test mode (KOSMOS_TEST_MODE=1, выставляется tests/e2e/helpers/launch.ts):
  // выходим, чтобы Playwright app.close() не висел до timeout — обычное
  // behaviour Kepler'а — tray-resident, не quit'ить, но в тестах окна
  // не закрываются (launcher hidden default'ом), и quit нужен.
  if (process.env.KOSMOS_TEST_MODE === "1") {
    app.quit();
  }
  // иначе не выходим — Kepler tray-resident; закрытие окна только hide
});

// Любой источник app.quit() (Playwright, programmatic, signals) должен
// взвести isQuiting — иначе mainWindow.close handler делает preventDefault
// (тarayresident-режим) и quit блокируется.
app.on("before-quit", () => {
  isQuiting = true;
});

app.on("will-quit", () => {
  console.error("[kepler-shell] will-quit: starting cleanup");
  globalShortcut.unregisterAll();
  setExtensionArkBridge({ request: null, subscribe: null });
  teardownPomodoroNotifier();
  if (windowStateSaveTimer) {
    clearTimeout(windowStateSaveTimer);
    windowStateSaveTimer = null;
    saveWindowStateNow();
  }
  if (backendProc && !backendProc.killed) {
    const pid = backendProc.pid;
    console.error(`[kepler-shell] will-quit: killing backend tree pid=${pid}`);
    // SIGTERM на Windows = TerminateProcess, не trim grandchild'ов. Backend
    // спавнит `ark-core-rpc` как child — без tree-kill он остаётся висеть в
    // task manager после quit, держит lock на ark.db и блокирует следующий
    // запуск Kepler ("database is locked"). taskkill /T /F убивает всё
    // дерево разом.
    if (process.platform === "win32" && pid) {
      try {
        spawn("taskkill", ["/T", "/F", "/PID", String(pid)], {
          stdio: "ignore",
          windowsHide: true,
        });
      } catch (e) {
        console.error("[kepler-shell] taskkill failed:", e);
        backendProc.kill();
      }
    } else {
      backendProc.kill();
    }
  }
  // ArkClient WebSocket keeps event loop alive — закрываем явно чтобы
  // Electron мог exit без timeout (test mode особенно чувствителен).
  if (arkClient) {
    try {
      void arkClient.stop();
    } catch (e) {
      console.error("[kepler-shell] arkClient stop error:", e);
    }
    arkClient = null;
  }
  console.error("[kepler-shell] will-quit: cleanup done");
});
