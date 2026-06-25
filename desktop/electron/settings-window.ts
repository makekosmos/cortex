// Settings window для Kepler — отдельный BrowserWindow, грузит тот же
// renderer-bundle с hash `#settings`, src/main.ts по hash рендерит
// SettingsView вместо LauncherView. Single Vue codebase, два окна.
//
// IPC handlers регистрируются eagerly на module-import (см. `import
// "./settings-window"` в main.ts). Открывается через
// `window.kepler.settings.open()` из renderer'а либо прямым вызовом
// `openSettings()` из main process (tray menu).

import { app, BrowserWindow, ipcMain, screen } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { macWindowChrome } from "./mac-window";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  resolveWindowMaterial,
} from "./window-effects";
import { buildStorageSummary } from "./settings-storage-summary";
import {
  isAutostartAllowed,
  isAutostartEnabled,
  setAutostartEnabled,
} from "./settings-autostart-controller";
export {
  DEFAULT_HOTKEY,
  getStoredHotkey,
  isFocusServiceAutoInstallDeclined,
  isTrayIconEnabled,
  isUsageTrackerEnabled,
  normalizeHotkeyAccelerator,
  setFocusServiceAutoInstallDeclined,
} from "./settings-store";
import {
  DEFAULT_HOTKEY,
  getLauncherStateTtlMinutes,
  getStoredHotkey,
  isTrayIconEnabled,
  isUsageTrackerEnabled,
  normalizeHotkeyAccelerator,
  readSettings,
  setStoredHotkey,
  setTrayIconEnabled,
  writeSettings,
} from "./settings-store";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const SETTINGS_WIDTH = 880;
const SETTINGS_HEIGHT = 560;

let settingsWindow: BrowserWindow | null = null;

function isHeadlessOrTest(): boolean {
  return envFlag("KOSMOS_HEADLESS") || envFlag("KOSMOS_TEST_MODE");
}

function envFlag(name: string): boolean {
  return process.env[name] === "1";
}

function isHeadless(): boolean {
  return envFlag("KOSMOS_HEADLESS");
}

function devServerUrl(): string | undefined {
  return process.env.VITE_DEV_SERVER_URL;
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
    skipTaskbar: isHeadless(),
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

  const devUrl = devServerUrl();
  if (devUrl) {
    void settingsWindow.loadURL(`${devUrl}#settings`);
  } else {
    void settingsWindow.loadFile(path.join(__dirname, "../dist/index.html"), {
      hash: "settings",
    });
  }

  settingsWindow.on("closed", () => {
    settingsWindow = null;
  });
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

ipcMain.handle("kepler:settings:launcher-state-ttl:get", () => getLauncherStateTtlMinutes());

ipcMain.handle("kepler:settings:launcher-state-ttl:set", (_e, minutes: number) => {
  const n = Number(minutes);
  if (Number.isFinite(n) && n >= 0) {
    writeSettings({ launcherStateTtlMinutes: Math.floor(n) });
  }
});
