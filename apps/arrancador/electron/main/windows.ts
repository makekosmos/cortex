import fs from "node:fs";
import path from "node:path";
import {
  BrowserWindow,
  type NativeImage,
  nativeImage,
  nativeTheme,
  shell,
} from "electron";

export interface MainWindowOptions {
  appPath: string;
  preloadPath: string;
  rendererUrl: string | null;
  icon: NativeImage;
  title: string;
  width: number;
  height: number;
  minWidth: number;
  minHeight: number;
}

export interface IconResolutionOptions {
  resourcesPath: string;
}

function getWindowsTitlebarSymbolColor() {
  return nativeTheme.shouldUseDarkColors ? "#e5e7eb" : "#111827";
}

function createFallbackIcon(): NativeImage {
  const svg = `
<svg xmlns="http://www.w3.org/2000/svg" width="128" height="128" viewBox="0 0 128 128">
  <defs>
    <linearGradient id="bg" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#1f2937"/>
      <stop offset="100%" stop-color="#0f172a"/>
    </linearGradient>
    <linearGradient id="accent" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0%" stop-color="#f59e0b"/>
      <stop offset="100%" stop-color="#f97316"/>
    </linearGradient>
  </defs>
  <rect x="8" y="8" width="112" height="112" rx="28" fill="url(#bg)"/>
  <rect x="30" y="34" width="68" height="12" rx="6" fill="url(#accent)"/>
  <rect x="30" y="56" width="52" height="12" rx="6" fill="#cbd5e1"/>
  <rect x="30" y="78" width="38" height="12" rx="6" fill="#94a3b8"/>
</svg>`.trim();

  return nativeImage.createFromDataURL(
    `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`,
  );
}

function tryCreateIconFromFile(filePath: string): NativeImage | null {
  if (!fs.existsSync(filePath)) {
    return null;
  }

  const image = nativeImage.createFromPath(filePath);
  return image.isEmpty() ? null : image;
}

export async function resolveWindowIcon(
  options: IconResolutionOptions,
): Promise<NativeImage> {
  const candidates = [
    path.join(options.resourcesPath, "icon.png"),
    path.join(options.resourcesPath, "32x32.png"),
  ];

  for (const candidate of candidates) {
    const image = tryCreateIconFromFile(candidate);
    if (image) {
      return image;
    }
  }

  return createFallbackIcon();
}

export function createMainWindow(options: MainWindowOptions): BrowserWindow {
  const isDev = Boolean(options.rendererUrl);
  const isMac = process.platform === "darwin";
  const isWindows = process.platform === "win32";
  const window = new BrowserWindow({
    width: options.width,
    height: options.height,
    minWidth: options.minWidth,
    minHeight: options.minHeight,
    title: options.title,
    show: isDev,
    frame: !isWindows,
    autoHideMenuBar: true,
    backgroundColor: "#0f131a",
    icon: options.icon,
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
            symbolColor: getWindowsTitlebarSymbolColor(),
          },
        }
      : {}),
    webPreferences: {
      preload: options.preloadPath,
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: false,
    },
  });

  if (isWindows) {
    const syncOverlay = () => {
      if (window.isDestroyed()) {
        return;
      }

      window.setTitleBarOverlay({
        color: "#00000000",
        symbolColor: getWindowsTitlebarSymbolColor(),
      });
    };

    window.webContents.on("did-finish-load", syncOverlay);
    window.webContents.on("zoom-changed", syncOverlay);
    nativeTheme.on("updated", syncOverlay);
    syncOverlay();

    window.once("closed", () => {
      nativeTheme.off("updated", syncOverlay);
    });
  }

  window.webContents.setWindowOpenHandler(({ url }) => {
    if (/^https?:\/\//i.test(url)) {
      void shell.openExternal(url);
    }
    return { action: "deny" };
  });

  const showWindow = (focus = false) => {
    if (window.isDestroyed()) {
      return;
    }

    if (!window.isVisible()) {
      window.show();
    }

    if (focus) {
      window.focus();
    }
  };

  window.once("ready-to-show", () => {
    showWindow(false);
  });

  window.webContents.on(
    "preload-error",
    (_event, preloadPath, error) => {
      console.error(
        "Preload failed:",
        JSON.stringify({
          preloadPath,
          message: error.message,
          stack: error.stack,
        }),
      );
    },
  );

  window.webContents.on(
    "did-fail-load",
    (_event, errorCode, errorDescription, validatedURL) => {
      console.error(
        "Renderer failed to load:",
        JSON.stringify({ errorCode, errorDescription, validatedURL }),
      );
      showWindow(false);
    },
  );

  window.webContents.on("did-finish-load", () => {
    showWindow(false);
  });

  window.webContents.on("render-process-gone", (_event, details) => {
    console.error("Renderer process exited:", details);
  });

  const showFallbackTimer = setTimeout(() => {
    showWindow(false);
  }, isDev ? 10_000 : 2_000);

  window.once("closed", () => {
    clearTimeout(showFallbackTimer);
  });

  if (options.rendererUrl) {
    void window.loadURL(options.rendererUrl);
  } else {
    void window.loadFile(path.join(options.appPath, "out", "renderer", "index.html"));
  }

  return window;
}

export function showMainWindow(window: BrowserWindow): void {
  if (window.isDestroyed()) {
    return;
  }

  if (window.isMinimized()) {
    window.restore();
  }

  window.show();
  window.focus();
}

export function hideMainWindow(window: BrowserWindow): void {
  if (!window.isDestroyed()) {
    window.hide();
  }
}
