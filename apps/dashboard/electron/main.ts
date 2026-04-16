import { app, BrowserWindow, dialog, ipcMain } from "electron";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { DashboardLoadOptions } from "../shared/analytics.ts";
import { loadDashboardSnapshot, resolveDefaultArkDbPath } from "./services/analytics.ts";

const __dirname = path.dirname(fileURLToPath(import.meta.url));

process.env.APP_ROOT = path.join(__dirname, "..");

const VITE_DEV_SERVER_URL = process.env.VITE_DEV_SERVER_URL;
const ELECTRON_RENDERER_URL = process.env.ELECTRON_RENDERER_URL;
const RENDERER_DIST = path.join(process.env.APP_ROOT, "dist");

let win: BrowserWindow | null = null;

type DashboardPreferences = {
  dbPath: string | null;
};

function preferencesPath() {
  return path.join(app.getPath("userData"), "dashboard-preferences.json");
}

function readPreferences(): DashboardPreferences {
  try {
    const raw = fs.readFileSync(preferencesPath(), "utf8");
    const parsed = JSON.parse(raw) as Partial<DashboardPreferences>;
    return {
      dbPath: typeof parsed.dbPath === "string" ? parsed.dbPath : null,
    };
  } catch {
    return { dbPath: null };
  }
}

function writePreferences(next: DashboardPreferences) {
  fs.mkdirSync(path.dirname(preferencesPath()), { recursive: true });
  fs.writeFileSync(preferencesPath(), JSON.stringify(next, null, 2), "utf8");
}

function currentDbPath() {
  return readPreferences().dbPath;
}

function createWindow() {
  const isMac = process.platform === "darwin";
  const isWindows = process.platform === "win32";

  win = new BrowserWindow({
    width: 1420,
    height: 920,
    minWidth: 1100,
    minHeight: 720,
    backgroundColor: "#0f0c08",
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
          titleBarOverlay: {
            color: "#00000000",
            symbolColor: "#f4efe7",
            height: 32,
          },
        }
      : {}),
    show: false,
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
    },
  });

  win.once("ready-to-show", () => {
    win?.show();
  });

  if (ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL) {
    win.loadURL(ELECTRON_RENDERER_URL ?? VITE_DEV_SERVER_URL!);
  } else {
    win.loadFile(path.join(RENDERER_DIST, "index.html"));
  }
}

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") {
    app.quit();
    win = null;
  }
});

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow();
  }
});

app.whenReady().then(() => {
  ipcMain.handle("dashboard:get-snapshot", async (_event, options?: DashboardLoadOptions) => {
    const explicitDbPath =
      typeof options?.dbPath === "string" ? options.dbPath : undefined;
    return loadDashboardSnapshot(
      {
        ...options,
        dbPath: explicitDbPath,
      },
      explicitDbPath ? null : currentDbPath(),
    );
  });

  ipcMain.handle("dashboard:choose-database", async () => {
    if (!win) {
      return loadDashboardSnapshot({}, currentDbPath());
    }

    const result = await dialog.showOpenDialog(win, {
      title: "Выберите Ark DB",
      filters: [
        { name: "SQLite", extensions: ["db", "sqlite", "sqlite3"] },
        { name: "All files", extensions: ["*"] },
      ],
      properties: ["openFile"],
    });

    if (result.canceled || result.filePaths.length === 0) {
      return loadDashboardSnapshot({}, currentDbPath());
    }

    writePreferences({ dbPath: result.filePaths[0] });
    return loadDashboardSnapshot({}, result.filePaths[0]);
  });

  ipcMain.handle("dashboard:reset-database", async () => {
    writePreferences({ dbPath: null });
    return loadDashboardSnapshot({}, null);
  });

  ipcMain.handle("dashboard:get-default-db-path", () => resolveDefaultArkDbPath());
  ipcMain.handle("dashboard:get-platform", () => process.platform);

  ipcMain.on("window:minimize", () => win?.minimize());
  ipcMain.on("window:maximize", () => {
    if (!win) {
      return;
    }
    if (win.isMaximized()) {
      win.unmaximize();
    } else {
      win.maximize();
    }
  });
  ipcMain.on("window:close", () => win?.close());

  createWindow();
});
