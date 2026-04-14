import { app, BrowserWindow, ipcMain, dialog } from "electron";

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

import {
  hevyLoginViaBrowser,
  hevyFetchAccount,
  hevyGetWorkoutCount,
  hevyFetchWorkouts,
  hevyFetchAllWorkouts,
} from "./hevy";

import { convertHevyWorkoutsToEntries } from "./hevySync";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

process.env.APP_ROOT = path.join(__dirname, "..");

export const VITE_DEV_SERVER_URL = process.env["VITE_DEV_SERVER_URL"];

export const ELECTRON_RENDERER_URL = process.env["ELECTRON_RENDERER_URL"];

export const MAIN_DIST = path.join(process.env.APP_ROOT, "dist-electron");

export const RENDERER_DIST = path.join(process.env.APP_ROOT, "dist");

process.env.VITE_PUBLIC = VITE_DEV_SERVER_URL
  ? path.join(process.env.APP_ROOT, "public")
  : RENDERER_DIST;

let win: BrowserWindow | null;

function isDevRenderer() {
  return Boolean(ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL);
}

function shouldShowWindowInBackground() {
  return (
    process.env.EDEN_BACKGROUND_LAUNCH === "1" || process.env.PLAYWRIGHT === "1"
  );
}

function createWindow() {
  const isMac = process.platform === "darwin";

  const isWindows = process.platform === "win32";

  const overlayHeight = 28;

  const titleBarOverlay = isWindows
    ? { color: "#232323", symbolColor: "#a3a3a3", height: overlayHeight }
    : undefined;

  win = new BrowserWindow({
    width: 1100,

    height: 750,

    minWidth: 800,

    minHeight: 600,

    frame: !isWindows,

    ...(isMac
      ? {
          titleBarStyle: "hiddenInset" as const,

          trafficLightPosition: { x: 18, y: 18 },
        }
      : {}),

    ...(isWindows
      ? {
          titleBarStyle: "hidden" as const,

          titleBarOverlay,
        }
      : {}),

    backgroundColor: "#171717",

    icon: path.join(process.env.VITE_PUBLIC, "electron-vite.svg"),

    show: false,

    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
    },
  });

  win.once("ready-to-show", () => {
    if (!win) {
      return;
    }

    if (shouldShowWindowInBackground() || isDevRenderer()) {
      win.showInactive();

      return;
    }

    win.show();
  });

  if (ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL) {
    win.loadURL(ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL!);
  } else {
    win.loadFile(path.join(RENDERER_DIST, "index.html"));
  }
}

app.on("window-all-closed", () => {
  shutdownHeart();

  if (process.platform !== "darwin") {
    app.quit();

    win = null;
  }
});

app.on("before-quit", () => {
  shutdownHeart();
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

  void purgeExpiredTrash().catch(() => {});

  setInterval(
    () => {
      void purgeExpiredTrash().catch(() => {});
    },

    24 * 60 * 60 * 1000,
  );
});
