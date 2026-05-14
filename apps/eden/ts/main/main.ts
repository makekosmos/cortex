import electron from "electron";
import type {
  BrowserWindow as BrowserWindowType,
  Event as ElectronEvent,
  Input,
  MenuItemConstructorOptions,
} from "electron";

import { execFile } from "node:child_process";

import fs from "node:fs";

import { fileURLToPath } from "node:url";

import path from "node:path";

import {
  initStore,
  saveEntry,
  loadEntry,
  listEntries,
  getVaultPath,
  getRecentVaultPaths,
  setVaultPath,
  createFolder,
  listFolders,
  listNoteTypes,
  moveEntryToFolder,
  moveFolderToFolder,
  deleteEntry,
  deleteFolder,
  getCodeToolsSettings,
  updateCodeToolsSettings,
  saveNoteType,
  deleteNoteType,
  searchEntries,
  getSidebarConfig,
  updateSidebarConfig,
  exportMarkdownVault,
  listTrashEntries,
  restoreEntry,
  permanentDeleteEntry,
  purgeExpiredTrash,
  getVaultStorageInfo,
  getHevyAuthToken,
  getHevyUsername,
  setHevyAuth,
  clearHevyAuth,
  type SidebarConfigPatch,
} from "./store";

import type { Entry } from "./store";

import { formatCode, lintCode } from "./codeTools";

import { shutdownHeart } from "./heart";
import { shutdownArk } from "./ark";

import {
  hevyLoginViaBrowser,
  hevyFetchAccount,
  hevyGetWorkoutCount,
  hevyFetchWorkouts,
  hevyFetchAllWorkouts,
} from "./hevy";

import { convertHevyWorkoutsToEntries } from "./hevySync";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const { app, BrowserWindow, ipcMain, dialog, Menu } = electron;

process.env.APP_ROOT = path.join(__dirname, "..");

const testAppDataPath = process.env.KOSMOS_TEST_APPDATA?.trim()
  ? path.resolve(process.env.KOSMOS_TEST_APPDATA)
  : null;
const testUserDataPath = process.env.KOSMOS_TEST_USER_DATA?.trim()
  ? path.resolve(process.env.KOSMOS_TEST_USER_DATA)
  : null;

if (testAppDataPath) {
  fs.mkdirSync(testAppDataPath, { recursive: true });
  app.setPath("appData", testAppDataPath);
}

if (testUserDataPath) {
  fs.mkdirSync(testUserDataPath, { recursive: true });
  app.setPath("userData", testUserDataPath);
}

export const VITE_DEV_SERVER_URL = process.env["VITE_DEV_SERVER_URL"];

export const ELECTRON_RENDERER_URL = process.env["ELECTRON_RENDERER_URL"];

export const MAIN_DIST = path.join(process.env.APP_ROOT, "dist-electron");

export const RENDERER_DIST = path.join(process.env.APP_ROOT, "dist");

process.env.VITE_PUBLIC = VITE_DEV_SERVER_URL
  ? path.join(process.env.APP_ROOT, "public")
  : RENDERER_DIST;

let win: BrowserWindowType | null;
const WINDOWS_TITLEBAR_SYMBOL_COLOR = "#e5e7eb";

function setupApplicationMenu() {
  const template: MenuItemConstructorOptions[] = [];

  if (process.platform === "darwin") {
    template.push({
      label: app.name,
      submenu: [
        { role: "about" },
        { type: "separator" },
        { role: "services" },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { role: "quit" },
      ],
    });
  } else {
    template.push({
      label: "File",
      submenu: [{ role: "quit" }],
    });
  }

  template.push({
    label: "View",
    submenu: [
      { role: "reload" },
      { role: "forceReload" },
      { role: "toggleDevTools" },
      { type: "separator" },
      { role: "resetZoom" },
      { role: "zoomIn" },
      { role: "zoomOut" },
      { type: "separator" },
      { role: "togglefullscreen" },
    ],
  });

  template.push({
    label: "Window",
    submenu:
      process.platform === "darwin"
        ? [
          { role: "minimize" },
          { role: "close" },
          { type: "separator" },
          { role: "front" },
          { role: "window" },
        ]
        : [{ role: "minimize" }, { role: "close" }],
  });

  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

function isDevRenderer() {
  return Boolean(ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL);
}

function shouldShowWindowInBackground() {
  return (
    process.env.EDEN_BACKGROUND_LAUNCH === "1" || process.env.PLAYWRIGHT === "1"
  );
}

function applyWindowsTitlebarOverlay(window: BrowserWindowType) {
  if (process.platform !== "win32" || window.isDestroyed()) {
    return;
  }

  window.setTitleBarOverlay({
    color: "#00000000",
    symbolColor: WINDOWS_TITLEBAR_SYMBOL_COLOR,
  });
}

function createWindow() {
  const launchInBackground = shouldShowWindowInBackground();
  const isMac = process.platform === "darwin";

  const isWindows = process.platform === "win32";

  setupApplicationMenu();

  win = new BrowserWindow({
    width: 1100,

    height: 750,

    minWidth: 800,

    minHeight: 600,

    frame: !isWindows,

    ...(isMac
      ? {
        titleBarStyle: "hiddenInset" as const,
      }
      : {}),

    ...(isWindows
      ? {
        titleBarStyle: "hidden" as const,
        titleBarOverlay: {
          color: "#00000000",
          symbolColor: WINDOWS_TITLEBAR_SYMBOL_COLOR,
        },
      }
      : {}),

    backgroundColor: "#171717",

    icon: path.join(process.env.VITE_PUBLIC, "electron-vite.svg"),

    show: false,

    webPreferences: {
      backgroundThrottling: !launchInBackground,
      preload: path.join(__dirname, "preload.mjs"),
    },
  });

  win.once("ready-to-show", () => {
    if (!win) {
      return;
    }

    if (launchInBackground) {
      return;
    }

    if (isDevRenderer()) {
      win.showInactive();

      return;
    }

    win.show();
  });

  if (isWindows) {
    const syncOverlay = () => {
      if (win) {
        applyWindowsTitlebarOverlay(win);
      }
    };

    win.webContents.on("did-finish-load", syncOverlay);
    syncOverlay();
  }

  if (ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL) {
    win.loadURL(ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL!);
  } else {
    win.loadFile(path.join(RENDERER_DIST, "index.html"));
  }

  win.webContents.on("before-input-event", (event: ElectronEvent, input: Input) => {
    const isToggleDevTools =
      input.key === "F12" ||
      ((input.control || input.meta) &&
        input.shift &&
        input.key.toLowerCase() === "i");

    if (!isToggleDevTools) {
      return;
    }

    event.preventDefault();
    win?.webContents.toggleDevTools();
  });
}

app.on("window-all-closed", () => {
  shutdownHeart();
  shutdownArk();

  if (process.platform !== "darwin") {
    app.quit();

    win = null;
  }
});

app.on("before-quit", () => {
  shutdownHeart();
  shutdownArk();
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});

app.whenReady().then(async () => {
  await initStore();

  ipcMain.handle("save-entry", async (_event, entry: Entry) => {
    return saveEntry(entry);
  });

  ipcMain.handle("load-entry", async (_event, id) => {
    return loadEntry(id);
  });

  ipcMain.handle("list-entries", async () => {
    return listEntries();
  });

  ipcMain.handle("create-folder", async (_event, id, name, parentId) => {
    return createFolder(id, name, parentId ?? null);
  });

  ipcMain.handle("list-folders", async () => {
    return listFolders();
  });

  ipcMain.handle("list-note-types", async () => {
    return listNoteTypes();
  });

  ipcMain.handle("save-note-type", async (_event, noteType) => {
    return saveNoteType(noteType);
  });

  ipcMain.handle("delete-note-type", async (_event, noteTypeId: string) => {
    return deleteNoteType(noteTypeId);
  });

  ipcMain.handle("move-entry-to-folder", async (_event, entryId, folderId) => {
    await moveEntryToFolder(entryId, folderId);

    return true;
  });

  ipcMain.handle(
    "move-folder-to-folder",
    async (_event, folderId, parentId) => {
      return moveFolderToFolder(folderId, parentId ?? null);
    },
  );

  ipcMain.handle("delete-entry", async (_event, entryId: string) => {
    return deleteEntry(entryId);
  });

  ipcMain.handle("delete-folder", async (_event, folderId: string) => {
    return deleteFolder(folderId);
  });

  ipcMain.handle("get-vault-path", () => {
    return getVaultPath();
  });

  ipcMain.handle("get-recent-vault-paths", () => {
    return getRecentVaultPaths();
  });

  ipcMain.handle("select-folder", async () => {
    if (!win) return null;

    const result = await dialog.showOpenDialog(win, {
      properties: ["openDirectory", "createDirectory"],
    });

    if (!result.canceled && result.filePaths.length > 0) {
      return result.filePaths[0];
    }

    return null;
  });

  ipcMain.handle("set-vault-path", async (_event, path) => {
    await setVaultPath(path);

    return true;
  });

  ipcMain.handle("export-markdown-vault", async () => {
    if (!win) return null;

    const result = await dialog.showOpenDialog(win, {
      properties: ["openDirectory", "createDirectory"],

      title: "Выберите папку для экспорта Markdown",
    });

    if (result.canceled || result.filePaths.length === 0) {
      return null;
    }

    return exportMarkdownVault(result.filePaths[0]);
  });

  ipcMain.handle("search-entries", async (_event, query: string) => {
    if (!query.trim()) return [];

    try {
      return await searchEntries(query);
    } catch (e) {
      console.error("Search error:", e);

      return [];
    }
  });

  ipcMain.handle("get-code-tools-settings", () => {
    return getCodeToolsSettings();
  });

  ipcMain.handle("update-code-tools-settings", (_event, settings) => {
    return updateCodeToolsSettings(settings);
  });

  ipcMain.handle(
    "lint-code-block",
    async (_event, language: string, code: string) => {
      return await lintCode(language, code, getCodeToolsSettings());
    },
  );

  ipcMain.handle(
    "format-code-block",
    async (_event, language: string, code: string) => {
      return await formatCode(language, code, getCodeToolsSettings());
    },
  );

  ipcMain.handle("get-sidebar-config", () => {
    return getSidebarConfig();
  });

  ipcMain.handle(
    "update-sidebar-config",
    (_event, config: SidebarConfigPatch) => {
      return updateSidebarConfig(config);
    },
  );

  ipcMain.handle("zoom-get", () => {
    return win?.webContents.getZoomFactor() ?? 1;
  });

  ipcMain.handle("zoom-set", (_event, factor: number) => {
    if (win) {
      const clamped = Math.max(0.5, Math.min(2.0, factor));

      win.webContents.setZoomFactor(clamped);

      return clamped;
    }

    return 1;
  });

  ipcMain.on("window-min", () => win?.minimize());

  ipcMain.on("window-max", () => {
    if (win?.isMaximized()) {
      win.unmaximize();
    } else {
      win?.maximize();
    }
  });

  ipcMain.on("window-close", () => win?.close());

  ipcMain.handle("get-platform", () => process.platform);

  ipcMain.handle("hevy-login", async () => {
    const result = await hevyLoginViaBrowser();

    if (result.ok) {
      setHevyAuth(result.authToken, result.username);
    }

    return result;
  });

  ipcMain.handle("hevy-logout", () => {
    clearHevyAuth();

    return { ok: true };
  });

  ipcMain.handle("hevy-get-auth-status", () => {
    const token = getHevyAuthToken();

    return {
      loggedIn: token !== null,

      username: getHevyUsername(),
    };
  });

  ipcMain.handle("hevy-get-account", async () => {
    const token = getHevyAuthToken();

    if (!token) return { ok: false, error: "Not logged in" };

    try {
      const account = await hevyFetchAccount(token);

      return { ok: true, data: account };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  });

  ipcMain.handle("hevy-get-workout-count", async () => {
    const token = getHevyAuthToken();

    if (!token) return { ok: false, error: "Not logged in" };

    try {
      const count = await hevyGetWorkoutCount(token);

      return { ok: true, count };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  });

  ipcMain.handle("hevy-fetch-workouts", async (_event, offset?: number) => {
    const token = getHevyAuthToken();

    const username = getHevyUsername();

    if (!token || !username) return { ok: false, error: "Not logged in" };

    try {
      const workouts = await hevyFetchWorkouts(token, username, offset ?? 0);

      return { ok: true, data: workouts };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  });

  ipcMain.handle("hevy-fetch-all-workouts", async () => {
    const token = getHevyAuthToken();

    const username = getHevyUsername();

    if (!token || !username) return { ok: false, error: "Not logged in" };

    try {
      const workouts = await hevyFetchAllWorkouts(token, username);

      return { ok: true, data: workouts };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  });

  ipcMain.handle("hevy-sync-workouts", async () => {
    const token = getHevyAuthToken();

    const username = getHevyUsername();

    if (!token || !username) return { ok: false, error: "Not logged in" };

    try {
      const rawWorkouts = await hevyFetchAllWorkouts(token, username);

      const existingEntries = await listEntries();

      const { workoutEntries, exerciseEntries, stats } =
        convertHevyWorkoutsToEntries(
          rawWorkouts,

          existingEntries,
        );

      for (const entry of workoutEntries) {
        await saveEntry(entry);
      }

      for (const entry of exerciseEntries) {
        await saveEntry(entry);
      }

      return { ok: true, stats };
    } catch (e) {
      return { ok: false, error: (e as Error).message };
    }
  });

  ipcMain.handle("list-trash-entries", async () => {
    return listTrashEntries();
  });

  ipcMain.handle("restore-entry", async (_event, entryId: string) => {
    return restoreEntry(entryId);
  });

  ipcMain.handle("permanent-delete-entry", async (_event, entryId: string) => {
    return permanentDeleteEntry(entryId);
  });

  ipcMain.handle("purge-expired-trash", async () => {
    return purgeExpiredTrash();
  });

  ipcMain.handle("get-vault-storage-info", async () => {
    return getVaultStorageInfo();
  });

  ipcMain.handle("get-disk-free-space", async () => {
    const vaultPath = getVaultPath();

    if (!vaultPath) return 0;

    try {
      if (process.platform === "win32") {
        // Use PowerShell (wmic is deprecated) with execFile to avoid shell injection

        const drive = vaultPath.substring(0, 2).replace(/[^A-Za-z:]/g, "");

        return new Promise<number>((resolve) => {
          execFile(
            "powershell",

            ["-NoProfile", "-Command", `(Get-PSDrive ${drive[0]}).Free`],

            (err, stdout) => {
              if (err) {
                resolve(0);

                return;
              }

              const value = parseInt(stdout.trim(), 10);

              resolve(Number.isFinite(value) ? value : 0);
            },
          );
        });
      }

      // macOS / Linux: use Node's fs.statfs (available since Node 18.15)

      return new Promise<number>((resolve) => {
        fs.statfs(vaultPath, (err, stats) => {
          if (err) {
            resolve(0);

            return;
          }

          resolve(stats.bavail * stats.bsize);
        });
      });
    } catch {
      return 0;
    }
  });

  createWindow();

  // Purge expired trash on startup and daily

  void purgeExpiredTrash().catch(() => { });

  setInterval(
    () => {
      void purgeExpiredTrash().catch(() => { });
    },

    24 * 60 * 60 * 1000,
  );
});
