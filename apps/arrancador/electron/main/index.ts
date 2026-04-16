import { mkdir } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { app, type BrowserWindow, type Tray } from "electron";
import { initializeBackend } from "./backend";
import { createAppTray } from "./tray";
import {
  createMainWindow,
  hideMainWindow,
  resolveWindowIcon,
  showMainWindow,
} from "./windows";
import { resolveSharedUserDataPath } from "./helpers/user-data";

const APP_NAME = "arrancador";
const DISPLAY_NAME = "Arrancador";
const APP_ID = "com.arrancador.app";
const ROOT_DIR = fileURLToPath(new URL(".", import.meta.url));

let mainWindow: BrowserWindow | null = null;
let tray: Tray | null = null;
let quitting = false;
let windowCreationPromise: Promise<BrowserWindow> | null = null;

function getRendererUrl(): string | null {
  const url =
    process.env.ELECTRON_RENDERER_URL ??
    process.env.VITE_DEV_SERVER_URL ??
    process.env.ELECTRON_VITE_DEV_SERVER_URL ??
    null;

  return url && url.trim().length > 0 ? url : null;
}

function getPreloadPath(): string {
  return path.join(ROOT_DIR, "../preload/index.js");
}

async function startWindow(): Promise<BrowserWindow> {
  if (mainWindow && !mainWindow.isDestroyed()) {
    return mainWindow;
  }

  if (windowCreationPromise) {
    return windowCreationPromise;
  }

  windowCreationPromise = (async () => {
    mainWindow = null;

    const icon = await resolveWindowIcon({
      resourcesPath: process.resourcesPath,
    });

    const window = createMainWindow({
      appPath: app.getAppPath(),
      preloadPath: getPreloadPath(),
      rendererUrl: getRendererUrl(),
      icon,
      title: DISPLAY_NAME,
      width: 1200,
      height: 800,
      minWidth: 1200,
      minHeight: 800,
    });

    window.on("close", (event) => {
      if (quitting) {
        return;
      }

      event.preventDefault();
      hideMainWindow(window);
    });

    window.on("minimize" as never, (event) => {
      if (quitting) {
        return;
      }

      event.preventDefault();
      hideMainWindow(window);
    });

    mainWindow = window;
    return window;
  })();

  try {
    return await windowCreationPromise;
  } finally {
    windowCreationPromise = null;
  }
}

async function shutdownApp() {
  if (quitting) {
    return;
  }

  quitting = true;
  tray?.destroy();
  tray = null;

  if (mainWindow && !mainWindow.isDestroyed()) {
    mainWindow.removeAllListeners("close");
    mainWindow.removeAllListeners("minimize");
    mainWindow.destroy();
  }
  mainWindow = null;

  app.exit(0);
}

export async function runApp(): Promise<void> {
  app.setAppUserModelId(APP_ID);
  app.setName(DISPLAY_NAME);
  app.setPath("userData", resolveSharedUserDataPath(APP_NAME));

  if (!app.requestSingleInstanceLock()) {
    app.quit();
    return;
  }

  app.on("second-instance", () => {
    if (mainWindow && !mainWindow.isDestroyed()) {
      showMainWindow(mainWindow);
      return;
    }

    mainWindow = null;
    void startWindow().then((window) => {
      showMainWindow(window);
    });
  });

  app.on("activate", () => {
    if (mainWindow && !mainWindow.isDestroyed()) {
      showMainWindow(mainWindow);
      return;
    }

    mainWindow = null;
    void startWindow();
  });

  app.on("before-quit", (event) => {
    if (quitting) {
      return;
    }

    event.preventDefault();
    void shutdownApp();
  });

  await app.whenReady();
  await mkdir(resolveSharedUserDataPath(APP_NAME), { recursive: true });
  await initializeBackend();

  await startWindow();

  tray = createAppTray({
    icon: await resolveWindowIcon({
      resourcesPath: process.resourcesPath,
    }),
    onToggle: () => {
      if (!mainWindow || mainWindow.isDestroyed()) {
        mainWindow = null;
        void startWindow();
        return;
      }

      if (mainWindow.isVisible()) {
        hideMainWindow(mainWindow);
      } else {
        showMainWindow(mainWindow);
      }
    },
    onShow: () => {
      if (!mainWindow || mainWindow.isDestroyed()) {
        mainWindow = null;
        void startWindow().then((window) => {
          showMainWindow(window);
        });
        return;
      }

      showMainWindow(mainWindow);
    },
    onHide: () => {
      if (mainWindow && !mainWindow.isDestroyed()) {
        hideMainWindow(mainWindow);
      }
    },
    onQuit: () => {
      void shutdownApp();
    },
  });
}
