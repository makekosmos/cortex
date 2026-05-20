// Settings window для Kepler — отдельный BrowserWindow, грузит тот же
// renderer-bundle с hash `#settings`, src/main.ts по hash рендерит
// SettingsView вместо LauncherView. Single Vue codebase, два окна.
//
// IPC handlers регистрируются eagerly на module-import (см. `import
// "./settings-window"` в main.ts). Открывается через
// `window.kepler.settings.open()` из renderer'а либо прямым вызовом
// `openSettings()` из main process (tray menu).

import { app, BrowserWindow, ipcMain, screen } from "electron";
import { existsSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { resolveInstance } from "./instance";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// ---------------------------------------------------------------------------
// Persisted settings file (`<userData>/kepler-shell-settings.json`)
// ---------------------------------------------------------------------------

interface KeplerShellSettings {
  developerMode?: boolean;
  hotkey?: string;
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

export const DEFAULT_HOTKEY_PROD = "Alt+Space";
export const DEFAULT_HOTKEY_DEV = "Alt+`";
// Slot-aware default. prod = Alt+Space; dev = Alt+` (не конфликтует с
// installed Kepler); dev-<x> / test-<x> = пусто (hotkey запрещён, см.
// instance.ts → instance.hotkey). main.ts читает instance.hotkey напрямую
// для решения «регистрировать или нет».
export const DEFAULT_HOTKEY = resolveInstance().hotkey ?? DEFAULT_HOTKEY_PROD;

export function getStoredHotkey(): string {
  return readSettings().hotkey ?? DEFAULT_HOTKEY;
}

export function setStoredHotkey(value: string): void {
  writeSettings({ hotkey: value });
}

function settingsFilePath(): string {
  return path.join(app.getPath("userData"), "kepler-shell-settings.json");
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

const SETTINGS_WIDTH = 880;
const SETTINGS_HEIGHT = 560;

let settingsWindow: BrowserWindow | null = null;

export function openSettings(): void {
  if (settingsWindow && !settingsWindow.isDestroyed()) {
    settingsWindow.focus();
    return;
  }
  const display = screen.getPrimaryDisplay().workAreaSize;
  settingsWindow = new BrowserWindow({
    width: SETTINGS_WIDTH,
    height: SETTINGS_HEIGHT,
    x: Math.round((display.width - SETTINGS_WIDTH) / 2),
    y: Math.round((display.height - SETTINGS_HEIGHT) / 2),
    show: true,
    frame: false,
    resizable: true,
    minimizable: false,
    maximizable: false,
    fullscreenable: false,
    skipTaskbar: false,
    alwaysOnTop: false,
    backgroundColor: "#00000000",
    backgroundMaterial: "mica",
    roundedCorners: true,
    title: "Kepler — Настройки",
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
    },
  });

  try {
    settingsWindow.setBackgroundMaterial("mica");
  } catch (e) {
    console.error("[kepler-shell] settings setBackgroundMaterial failed:", e);
  }

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

export function isAutostartEnabled(): boolean {
  return app.getLoginItemSettings().openAtLogin;
}

export function setAutostartEnabled(enabled: boolean): void {
  // Только prod slot может писать в HKCU Run. Из dev process.execPath это
  // electron.exe из node_modules — прописывать его в autorun бессмысленно
  // и грязно (мусор в реестре). Из dev-<x> / test — silently no-op.
  if (!resolveInstance().autorunEnabled) {
    console.warn(
      `[kepler-shell] autostart toggle ignored for slot ${resolveInstance().slot} (only prod)`,
    );
    return;
  }
  app.setLoginItemSettings({
    openAtLogin: enabled,
    path: process.execPath,
  });
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

ipcMain.handle(
  "kepler:settings:autostart:set",
  (_e, enabled: boolean) => {
    setAutostartEnabled(!!enabled);
  },
);

ipcMain.handle("kepler:settings:version", () => app.getVersion());

ipcMain.handle("kepler:settings:hotkey", () => getStoredHotkey());

ipcMain.handle(
  "kepler:settings:hotkey:set",
  (_e, value: string): { ok: boolean; error?: string } => {
    const normalized = String(value || "").trim();
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
export function setHotkeyReregisterCallback(
  cb: (accelerator: string) => boolean,
): void {
  reregisterHotkeyCallback = cb;
}

ipcMain.handle(
  "kepler:settings:developer-mode:get",
  () => process.env.KEPLER_DEV === "1" || !!readSettings().developerMode,
);

ipcMain.handle(
  "kepler:settings:developer-mode:set",
  (_e, enabled: boolean) => {
    writeSettings({ developerMode: !!enabled });
  },
);

ipcMain.handle("kepler:settings:usage-tracker:get", () => isUsageTrackerEnabled());

ipcMain.handle(
  "kepler:settings:usage-tracker:set",
  (_e, enabled: boolean) => {
    writeSettings({ usageTrackerEnabled: !!enabled });
  },
);

