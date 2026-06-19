// Settings window для Kepler — отдельный BrowserWindow, грузит тот же
// renderer-bundle с hash `#settings`, src/main.ts по hash рендерит
// SettingsView вместо LauncherView. Single Vue codebase, два окна.
//
// IPC handlers регистрируются eagerly на module-import (см. `import
// "./settings-window"` в main.ts). Открывается через
// `window.kepler.settings.open()` из renderer'а либо прямым вызовом
// `openSettings()` из main process (tray menu).

import { app, BrowserWindow, ipcMain, screen } from "electron";
import {
  existsSync,
  lstatSync,
  readFileSync,
  readdirSync,
  renameSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import path from "node:path";

const execFileAsync = promisify(execFile);
import { fileURLToPath } from "node:url";
import { resolveInstance } from "./instance";
import { keplerDataDir } from "./data-dir";
import { macWindowChrome } from "./mac-window";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  resolveWindowMaterial,
} from "./window-effects";
import type { StorageSummary, StorageSummaryItem } from "../shared/ipc-types";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ---------------------------------------------------------------------------
// Persisted settings file (`<userData>/kepler-shell-settings.json`)
// ---------------------------------------------------------------------------

interface KeplerShellSettings {
  developerMode?: boolean;
  hotkey?: string;
  showTrayIcon?: boolean;
  /** Трекать активные приложения (usage-tracker модуль в kepler-backend).
      Default = true (трекинг включён). Toggle off → backend стартует с
      `KEPLER_USAGE_TRACKER=0` и не пишет данные. Изменение применяется
      после рестарта Kepler. */
  usageTrackerEnabled?: boolean;
  /** Юзер отклонил auto-install kepler-focus-svc (UAC cancel в этой сессии
      или раньше). Когда true — runHelper не спросит UAC автоматически,
      пользователь явно нажимает «Установить» в Settings → Фокус.
      Сбрасывается через Settings UI. */
  focusServiceAutoInstallDeclined?: boolean;
  /** Сколько минут хранить позицию в лаунчере (query / selection / scroll)
      между открытиями. 0 = всегда ресетим. Default 5. */
  launcherStateTtlMinutes?: number;
}

export function isFocusServiceAutoInstallDeclined(): boolean {
  return readSettings().focusServiceAutoInstallDeclined === true;
}

export function setFocusServiceAutoInstallDeclined(value: boolean): void {
  writeSettings({ focusServiceAutoInstallDeclined: value });
}

export function isUsageTrackerEnabled(): boolean {
  return readSettings().usageTrackerEnabled !== false;
}

export function isTrayIconEnabled(): boolean {
  return readSettings().showTrayIcon !== false;
}

export function setTrayIconEnabled(enabled: boolean): void {
  writeSettings({ showTrayIcon: !!enabled });
}

export const DEFAULT_HOTKEY_PROD = process.platform === "darwin" ? "Command+Space" : "Alt+Space";
export const DEFAULT_HOTKEY_DEV = "Alt+`";
// Slot-aware default. prod = platform-native launcher hotkey; dev = Alt+`
// (не конфликтует с installed Kepler); dev-<x> / test-<x> = пусто (hotkey
// запрещён, см. instance.ts → instance.hotkey). main.ts читает instance.hotkey
// напрямую для решения «регистрировать или нет».
export const DEFAULT_HOTKEY = resolveInstance().hotkey ?? DEFAULT_HOTKEY_PROD;

const MODIFIER_ALIASES: Record<string, string> = {
  cmd: "Command",
  command: "Command",
  meta: process.platform === "darwin" ? "Command" : "Super",
  super: process.platform === "darwin" ? "Command" : "Super",
  win: "Super",
  windows: "Super",
  option: "Alt",
  alt: "Alt",
  ctrl: "Control",
  control: "Control",
  shift: "Shift",
};

const MODIFIER_ORDER = ["Command", "Control", "Alt", "Shift", "Super"];

function normalizeHotkeyPart(part: string): string {
  const trimmed = part.trim();
  if (!trimmed) return "";
  const alias = MODIFIER_ALIASES[trimmed.toLowerCase()];
  if (alias) return alias;
  if (trimmed.length === 1) return trimmed.toUpperCase();
  if (trimmed.toLowerCase() === "space") return "Space";
  if (trimmed.toLowerCase() === "escape") return "Escape";
  return trimmed;
}

export function normalizeHotkeyAccelerator(value: string): string {
  const parts = String(value || "")
    .split("+")
    .map(normalizeHotkeyPart)
    .filter(Boolean);
  if (parts.length === 0) return "";

  const modifiers = new Set<string>();
  let main = "";
  for (const part of parts) {
    if (MODIFIER_ORDER.includes(part)) {
      modifiers.add(part);
    } else if (!main) {
      main = part;
    }
  }
  if (!main) return "";
  return [...MODIFIER_ORDER.filter((part) => modifiers.has(part)), main].join("+");
}

export function getStoredHotkey(): string {
  const raw = readSettings().hotkey ?? DEFAULT_HOTKEY;
  const normalized = normalizeHotkeyAccelerator(raw);
  if (normalized && normalized !== raw) {
    writeSettings({ hotkey: normalized });
  }
  return normalized || DEFAULT_HOTKEY;
}

export function setStoredHotkey(value: string): void {
  const normalized = normalizeHotkeyAccelerator(value);
  writeSettings({ hotkey: normalized || DEFAULT_HOTKEY });
}

function settingsFilePath(): string {
  return path.join(keplerDataDir(), "kepler-shell-settings.json");
}

function readSettings(): KeplerShellSettings {
  try {
    const file = settingsFilePath();
    if (!existsSync(file)) return {};
    return JSON.parse(readFileSync(file, "utf8")) as KeplerShellSettings;
  } catch {
    return {};
  }
}

function writeSettings(patch: Partial<KeplerShellSettings>): void {
  const current = readSettings();
  const next: KeplerShellSettings = { ...current, ...patch };
  const target = settingsFilePath();
  const tmp = target + ".tmp";
  try {
    writeFileSync(tmp, JSON.stringify(next, null, 2), "utf8");
    renameSync(tmp, target);
  } catch (e) {
    console.error("[kepler-shell] settings write failed:", e);
  }
}

function safeSizeOfPath(target: string): number {
  try {
    if (!existsSync(target)) return 0;
    const stat = lstatSync(target);
    if (stat.isSymbolicLink()) return 0;
    if (stat.isFile()) return stat.size;
    if (!stat.isDirectory()) return 0;
    let total = 0;
    for (const entry of readdirSync(target, { withFileTypes: true })) {
      const child = path.join(target, entry.name);
      try {
        if (entry.isSymbolicLink()) continue;
        if (entry.isDirectory()) {
          total += safeSizeOfPath(child);
        } else if (entry.isFile()) {
          total += statSync(child).size;
        }
      } catch {
        // Best-effort summary: skip files that are locked or disappear mid-scan.
      }
    }
    return total;
  } catch {
    return 0;
  }
}

function safeSizeOfFiles(dir: string, names: string[]): number {
  return names.reduce((sum, name) => sum + safeSizeOfPath(path.join(dir, name)), 0);
}

function isPathInside(parent: string, child: string): boolean {
  const rel = path.relative(parent, child);
  return Boolean(rel) && !rel.startsWith("..") && !path.isAbsolute(rel);
}

function storageSummaryItem(
  id: string,
  label: string,
  target: string,
  description?: string,
): StorageSummaryItem {
  return {
    id,
    label,
    path: target,
    bytes: safeSizeOfPath(target),
    exists: existsSync(target),
    description,
  };
}

function buildStorageSummary(): StorageSummary {
  const dataDir = keplerDataDir();
  const userDataDir = app.getPath("userData");
  const dataDirBytes = safeSizeOfPath(dataDir);
  const userDataBytes = safeSizeOfPath(userDataDir);
  const arkBytes = safeSizeOfFiles(dataDir, ["ark.db", "ark.db-wal", "ark.db-shm"]);
  const indexBytes = safeSizeOfFiles(dataDir, [
    "app-index.db",
    "app-index.db-wal",
    "app-index.db-shm",
    "file-index.db",
    "file-index.db-wal",
    "file-index.db-shm",
  ]);

  const items: StorageSummaryItem[] = [
    {
      id: "ark-db",
      label: "База ARK",
      path: path.join(dataDir, "ark.db"),
      bytes: arkBytes,
      exists: existsSync(path.join(dataDir, "ark.db")),
      description: "Основная база объектов и синхронизации.",
    },
    {
      id: "indexes",
      label: "Индексы поиска",
      path: dataDir,
      bytes: indexBytes,
      exists:
        existsSync(path.join(dataDir, "app-index.db")) ||
        existsSync(path.join(dataDir, "file-index.db")),
      description: "Индексы приложений и файлов, их можно пересоздать.",
    },
    storageSummaryItem(
      "dictation-models",
      "Локальные модели диктации",
      path.join(dataDir, "models", "dictation"),
      "Скачанные Whisper-модели.",
    ),
    storageSummaryItem(
      "dictation-tools",
      "Локальные инструменты диктации",
      path.join(dataDir, "tools", "dictation"),
      "whisper.cpp и вспомогательные файлы.",
    ),
    storageSummaryItem("extensions", "Установленные расширения", path.join(dataDir, "extensions")),
    storageSummaryItem(
      "extensions-data",
      "Данные расширений",
      path.join(dataDir, "extensions-data"),
    ),
    storageSummaryItem("logs", "Логи", path.join(dataDir, "logs")),
    storageSummaryItem("crashes", "Краши", path.join(dataDir, "crashes")),
  ];

  const knownDataBytes = items.reduce((sum, item) => sum + item.bytes, 0);
  items.push({
    id: "other-data",
    label: "Прочие данные Kosmos",
    path: dataDir,
    bytes: Math.max(0, dataDirBytes - knownDataBytes),
    exists: existsSync(dataDir),
  });
  items.push({
    id: "electron-user-data",
    label: "Состояние окна и Chromium",
    path: userDataDir,
    bytes: userDataBytes,
    exists: existsSync(userDataDir),
    description: "Electron userData: кэши, Local Storage, состояние UI.",
  });

  const totalBytes = isPathInside(dataDir, userDataDir)
    ? dataDirBytes
    : dataDirBytes + userDataBytes;

  return {
    dataDir,
    userDataDir,
    totalBytes,
    items,
  };
}

const SETTINGS_WIDTH = 880;
const SETTINGS_HEIGHT = 560;

let settingsWindow: BrowserWindow | null = null;

function isHeadlessOrTest(): boolean {
  return process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
}

export function openSettings(): void {
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    if (isHeadlessOrTest()) return;
    if (settingsWindow.isMinimized()) settingsWindow.restore();
    if (!settingsWindow.isVisible()) settingsWindow.show();
    settingsWindow.focus();
    return;
  }
  const display = screen.getPrimaryDisplay().workAreaSize;
  const backgroundMaterial = resolveWindowMaterial("acrylic");
  settingsWindow = new BrowserWindow({
    width: SETTINGS_WIDTH,
    height: SETTINGS_HEIGHT,
    minWidth: 800,
    minHeight: 560,
    x: Math.round((display.width - SETTINGS_WIDTH) / 2),
    y: Math.round((display.height - SETTINGS_HEIGHT) / 2),
    show: !isHeadlessOrTest(),
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#00000000",
      symbolColor: "#FFFFFF",
      height: 36,
    },
    // macOS: vibrancy + центрированные traffic lights (на Windows — no-op).
    ...macWindowChrome({ trafficLightY: 12 }),
    resizable: true,
    minimizable: true,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: process.env.KOSMOS_HEADLESS === "1",
    alwaysOnTop: false,
    backgroundColor: "#00000000",
    ...backgroundMaterialOption(backgroundMaterial),
    roundedCorners: true,
    title: "Kosmos — Настройки",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
    },
  });

  try {
    applyWindowMaterial(settingsWindow, backgroundMaterial, "settings");
  } catch {}

  if (process.env.VITE_DEV_SERVER_URL) {
    void settingsWindow.loadURL(`${process.env.VITE_DEV_SERVER_URL}#settings`);
  } else {
    void settingsWindow.loadFile(path.join(__dirname, "../dist/index.html"), {
      hash: "settings",
    });
  }

  settingsWindow.on("closed", () => {
    settingsWindow = null;
  });
}

// `args` для autorun-entry. Маркер используется main.ts чтобы понять что
// запуск был из Windows autorun (HKCU\...\Run) и не показывать launcher
// автоматически (только tray). Имя `--autostart` уже зарегистрировано в
// существующих установках — менять нельзя, иначе у старых юзеров маркер
// потеряется до следующего toggle.
const AUTOSTART_ARGS: string[] = ["--autostart"];
const AUTOSTART_NAME = "Kosmos";
const LEGACY_AUTOSTART_NAMES = ["com.kazui.kepler", "Kepler", "KeplerKosmos", "KosmosKepler"];

type WindowsLaunchItem = {
  name?: string;
  path?: string;
  args?: string[];
  enabled?: boolean;
};

function normalizeWinExecutablePath(value: string): string {
  return path.normalize(value).toLowerCase();
}

function sameArgs(actual: string[] | undefined, expected: string[]): boolean {
  const args = actual ?? [];
  return args.length === expected.length && args.every((arg, i) => arg === expected[i]);
}

function launchItemMatchesAutostart(item: WindowsLaunchItem): boolean {
  if (!item.path || item.enabled === false) return false;
  return (
    normalizeWinExecutablePath(item.path) === normalizeWinExecutablePath(process.execPath) &&
    sameArgs(item.args, AUTOSTART_ARGS)
  );
}

function legacyAutostartPathCandidates(): string[] {
  const dir = path.dirname(process.execPath);
  const paths = [path.join(dir, "Kepler.exe")];
  const localAppData = process.env.LOCALAPPDATA;
  if (localAppData) {
    paths.push(path.resolve(localAppData, "Programs", "Kepler", "Kepler.exe"));
  }
  return Array.from(new Set(paths));
}

function isLegacyAutostartEnabled(): boolean {
  if (process.platform !== "win32") return false;
  return legacyAutostartPathCandidates().some((legacyPath) => {
    try {
      return app.getLoginItemSettings({
        path: legacyPath,
        args: AUTOSTART_ARGS,
      }).openAtLogin;
    } catch {
      return false;
    }
  });
}

async function removeLegacyAutostartEntries(): Promise<void> {
  if (process.platform !== "win32") return;
  for (const legacyPath of legacyAutostartPathCandidates()) {
    for (const name of LEGACY_AUTOSTART_NAMES) {
      try {
        app.setLoginItemSettings({
          openAtLogin: false,
          name,
          path: legacyPath,
          args: AUTOSTART_ARGS,
        });
      } catch {
        /* best-effort cleanup */
      }
    }
  }

  try {
    await execFileAsync(
      "reg.exe",
      ["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "Kepler", "/f"],
      { windowsHide: true },
    );
  } catch {
    /* value absent or registry unavailable */
  }
}

export function isAutostartEnabled(): boolean {
  // На Windows getLoginItemSettings() без явных { path, args } сравнивает
  // запись в HKCU с `process.execPath` БЕЗ args. Запись же выставлена с
  // `args: ["--autostart"]` (строка в реестре: `"<exe>" --autostart`).
  // Из-за этого Electron видит несовпадение и возвращает openAtLogin=false,
  // даже если запись физически в реестре есть. Передаём те же path/args
  // что и при set — тогда сравнение симметрично.
  const settings = app.getLoginItemSettings({
    path: process.execPath,
    args: AUTOSTART_ARGS,
  });
  return (
    settings.openAtLogin ||
    (settings.launchItems ?? []).some((item) => launchItemMatchesAutostart(item)) ||
    isLegacyAutostartEnabled()
  );
}

export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  // Только prod slot может писать в HKCU Run. Из dev process.execPath это
  // electron.exe из node_modules — прописывать его в autorun бессмысленно
  // и грязно (мусор в реестре). Из dev-<x> / test — silently no-op.
  if (!resolveInstance().autorunEnabled) {
    console.warn(
      `[kepler-shell] autostart toggle ignored for slot ${resolveInstance().slot} (only prod)`,
    );
    return;
  }
  // args: ['--autostart'] — два эффекта:
  //   1) Передача args форсит Electron записать в HKCU\...\Run строку вида
  //      `"C:\...\Kosmos.exe" --autostart` (с кавычками вокруг path). Без
  //      args Electron на некоторых версиях кладёт path без кавычек; если в
  //      пути есть пробелы (system-wide install в "Program Files"), Windows
  //      shell не парсит и autorun не срабатывается. Per-user install в
  //      %LOCALAPPDATA%\Programs\Kepler\ пробелов не имеет, но защита
  //      универсальная.
  //   2) Маркер для main.ts: если argv содержит `--autostart`, мы знаем
  //      что запуск был из HKCU\...\Run и launcher window должен остаться
  //      скрытым (только tray). При manual launch (ярлык / exe / после
  //      update) этого маркера нет → launcher показывается сразу.
  app.setLoginItemSettings({
    openAtLogin: enabled,
    name: AUTOSTART_NAME,
    path: process.execPath,
    args: AUTOSTART_ARGS,
  });
  await removeLegacyAutostartEntries();
  // Сразу читаем обратно — если запись в HKCU не прошла, openAtLogin будет
  // false и UI покажет ошибку. Логируем для диагностики реальных установок.
  // Передаём { path, args } — без них verify фейлится из-за args mismatch
  // (см. комментарий в isAutostartEnabled).
  try {
    const verify = app.getLoginItemSettings({
      path: process.execPath,
      args: AUTOSTART_ARGS,
    });
    const launchItemVerified = (verify.launchItems ?? []).some((item) =>
      launchItemMatchesAutostart(item),
    );
    console.log(
      `[kepler-shell] autostart set → enabled=${enabled}, verified openAtLogin=${verify.openAtLogin}, launchItem=${launchItemVerified}, execPath=${process.execPath}`,
    );
  } catch (e) {
    console.warn("[kepler-shell] autostart verify failed:", e);
  }
}

export function isAutostartAllowed(): boolean {
  return resolveInstance().autorunEnabled;
}

// --- IPC handlers (eagerly registered on import) -----------------------------

ipcMain.handle("kepler:settings:open", () => {
  openSettings();
});

ipcMain.handle("kepler:settings:close", () => {
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    settingsWindow.close();
  }
});

ipcMain.handle("kepler:settings:autostart:get", () => isAutostartEnabled());

ipcMain.handle("kepler:settings:autostart:allowed", () => isAutostartAllowed());

ipcMain.handle("kepler:settings:autostart:set", async (_e, enabled: boolean) => {
  await setAutostartEnabled(!!enabled);
});

ipcMain.handle("kepler:settings:tray-icon:get", () => isTrayIconEnabled());

ipcMain.handle("kepler:settings:tray-icon:set", (_e, enabled: boolean) => {
  const value = !!enabled;
  setTrayIconEnabled(value);
  setTrayVisibilityCallback?.(value);
});

ipcMain.handle("kepler:settings:version", () => app.getVersion());

ipcMain.handle("kepler:settings:storage-summary", () => buildStorageSummary());

ipcMain.handle("kepler:settings:hotkey", () => getStoredHotkey());

ipcMain.handle(
  "kepler:settings:hotkey:set",
  (_e, value: string): { ok: boolean; error?: string } => {
    const normalized = normalizeHotkeyAccelerator(String(value || "").trim());
    if (!normalized) return { ok: false, error: "empty" };
    try {
      const success = reregisterHotkeyCallback?.(normalized) ?? false;
      if (!success) return { ok: false, error: "register-failed" };
      setStoredHotkey(normalized);
      return { ok: true };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  },
);

ipcMain.handle("kepler:settings:hotkey:reset", () => {
  reregisterHotkeyCallback?.(DEFAULT_HOTKEY);
  setStoredHotkey(DEFAULT_HOTKEY);
  return DEFAULT_HOTKEY;
});

// main.ts регистрирует callback, который умеет переcнять globalShortcut.
let reregisterHotkeyCallback: ((accelerator: string) => boolean) | null = null;
export function setHotkeyReregisterCallback(cb: (accelerator: string) => boolean): void {
  reregisterHotkeyCallback = cb;
}

let setTrayVisibilityCallback: ((enabled: boolean) => void) | null = null;
export function setTrayVisibilityController(cb: (enabled: boolean) => void): void {
  setTrayVisibilityCallback = cb;
}

ipcMain.handle(
  "kepler:settings:developer-mode:get",
  () => process.env.KEPLER_DEV === "1" || !!readSettings().developerMode,
);

ipcMain.handle("kepler:settings:developer-mode:set", (_e, enabled: boolean) => {
  writeSettings({ developerMode: !!enabled });
});

ipcMain.handle("kepler:settings:usage-tracker:get", () => isUsageTrackerEnabled());

ipcMain.handle("kepler:settings:usage-tracker:set", (_e, enabled: boolean) => {
  writeSettings({ usageTrackerEnabled: !!enabled });
});

const DEFAULT_LAUNCHER_STATE_TTL_MIN = 5;
export function getLauncherStateTtlMinutes(): number {
  const v = readSettings().launcherStateTtlMinutes;
  if (typeof v !== "number" || Number.isNaN(v) || v < 0) {
    return DEFAULT_LAUNCHER_STATE_TTL_MIN;
  }
  return Math.floor(v);
}

ipcMain.handle("kepler:settings:launcher-state-ttl:get", () => getLauncherStateTtlMinutes());

ipcMain.handle("kepler:settings:launcher-state-ttl:set", (_e, minutes: number) => {
  const n = Number(minutes);
  if (Number.isFinite(n) && n >= 0) {
    writeSettings({ launcherStateTtlMinutes: Math.floor(n) });
  }
});
