import { BrowserWindow, screen } from "electron";
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  checkApiCompat,
  ensureUserDataDir,
  extensionUserDataDir,
  isHeadless,
  resolveExtensionLocation,
  resolveExtensionSource,
  resolvePreloadForManifest,
  type ExtensionManifest,
} from "./extension-manifest";
import { macWindowChrome } from "./mac-window";
import {
  applyWindowMaterial,
  backgroundMaterialOption,
  resolveWindowMaterial,
  type KosmosWindowMaterial,
} from "./window-effects";
import {
  settingsWindowBounds,
  windowProfileTraits,
  type ExtensionWindowProfile,
} from "./extension-window-profile";
import { openIncompatibilityWindow } from "./extension-native-runner";
import type { ExtensionSource } from "./extension-permissions";

interface ExtensionWindowEntry {
  win: BrowserWindow;
  id: string;
  windowKey: string;
  keepAliveInBackground: boolean;
  initialRoute?: string;
}

interface ExtensionRendererContext {
  id: string;
  windowKey: string;
  source: ExtensionSource;
  manifestPermissions?: readonly string[];
}

interface OpenExtensionBrowserWindowOptions {
  id: string;
  isAppQuitting(): boolean;
  route?: string;
  windowKey: string;
  profile: ExtensionWindowProfile;
  manifest: ExtensionManifest;
  extensionWindows: Map<string, ExtensionWindowEntry>;
  webContentsToExtensionContext: Map<number, ExtensionRendererContext>;
  clearWindowIpcState(webContentsId: number): void;
  focusExistingWindow(win: BrowserWindow): void;
}

export async function openExtensionBrowserWindow({
  id,
  isAppQuitting,
  route,
  windowKey,
  profile,
  manifest,
  extensionWindows,
  webContentsToExtensionContext,
  clearWindowIpcState,
  focusExistingWindow,
}: OpenExtensionBrowserWindowOptions): Promise<void> {
  const traits = windowProfileTraits(profile);
  const existing = extensionWindows.get(windowKey);
  if (existing && !existing.win.isDestroyed()) {
    if (!isHeadless()) {
      focusExistingWindow(existing.win);
    }
    if (route) {
      try {
        existing.win.webContents.send("kepler:extension:navigation", route);
      } catch {
        /* webContents could be torn down between isDestroyed-check и send */
      }
    }
    return;
  }

  const incompat = checkApiCompat(manifest);
  if (incompat) {
    console.error(`[kepler-shell] ${incompat}`);
    openIncompatibilityWindow(manifest, incompat);
    return;
  }
  const location = resolveExtensionLocation(id);
  if (!location) {
    console.warn(`[kepler-shell] extension dir disappeared: ${id}`);
    return;
  }
  const extensionDir = location.dir;
  const source = await resolveExtensionSource(id, manifest, extensionDir);
  if (!source) return;

  const display = screen.getPrimaryDisplay().workAreaSize;
  const settingsBounds = profile === "settings" ? settingsWindowBounds(display) : null;
  const defaultWidth = settingsBounds?.width ?? manifest.width ?? 1200;
  const defaultHeight = settingsBounds?.height ?? manifest.height ?? 800;
  const preload = resolvePreloadForManifest(manifest, extensionDir);

  const stateFile = path.join(extensionUserDataDir(id), "window-state.json");
  let savedState: {
    width?: number;
    height?: number;
    x?: number;
    y?: number;
    isMaximized?: boolean;
  } = {};
  if (traits.persistWindowState && existsSync(stateFile)) {
    try {
      savedState = JSON.parse(readFileSync(stateFile, "utf8")) as typeof savedState;
    } catch (error) {
      console.warn(`[kepler-shell] extension '${id}' window-state.json invalid, ignoring:`, error);
    }
  }

  const displays = screen.getAllDisplays();
  const isVisibleOnAnyDisplay = (x: number, y: number, w: number, h: number): boolean =>
    displays.some((display) => {
      const workArea = display.workArea;
      return (
        x + w > workArea.x &&
        x < workArea.x + workArea.width &&
        y + h > workArea.y &&
        y < workArea.y + workArea.height
      );
    });

  let width = defaultWidth;
  let height = defaultHeight;
  let initialX: number | undefined = Math.round((display.width - defaultWidth) / 2);
  let initialY: number | undefined = Math.round((display.height - defaultHeight) / 2);

  if (
    typeof savedState.width === "number" &&
    typeof savedState.height === "number" &&
    typeof savedState.x === "number" &&
    typeof savedState.y === "number" &&
    isVisibleOnAnyDisplay(savedState.x, savedState.y, savedState.width, savedState.height)
  ) {
    width = savedState.width;
    height = savedState.height;
    initialX = savedState.x;
    initialY = savedState.y;
  }

  const headless = isHeadless();
  const manifestFallback: KosmosWindowMaterial =
    manifest.windowEffect === "acrylic" || manifest.windowEffect === "mica"
      ? manifest.windowEffect
      : "none";
  const backgroundMaterial = traits.forceAcrylic
    ? resolveWindowMaterial("acrylic")
    : resolveWindowMaterial(manifestFallback);
  const wantsBackdrop = backgroundMaterial === "acrylic" || backgroundMaterial === "mica";

  const win = new BrowserWindow({
    width,
    height,
    minWidth: settingsBounds?.minWidth ?? manifest.minWidth ?? 800,
    minHeight: settingsBounds?.minHeight ?? manifest.minHeight ?? 600,
    maximizable: traits.maximizable,
    fullscreenable: traits.fullscreenable,
    x: initialX,
    y: initialY,
    show: !headless,
    skipTaskbar: headless,
    title: manifest.name,
    backgroundColor: wantsBackdrop ? "#00000000" : "#1a1a1a",
    ...backgroundMaterialOption(backgroundMaterial),
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#00000000",
      symbolColor: "#f5f5f5",
      height: 40,
    },
    ...macWindowChrome({ trafficLightY: 14 }),
    webPreferences: {
      preload,
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: manifest.backgroundExecution !== true,
    },
  });

  applyWindowMaterial(win, backgroundMaterial, `extension '${id}'`);

  if (traits.persistWindowState && savedState.isMaximized) {
    win.once("ready-to-show", () => {
      if (!win.isDestroyed()) win.maximize();
    });
  }

  const saveWindowState = (): void => {
    if (!traits.persistWindowState) return;
    try {
      if (win.isDestroyed()) return;
      const bounds = win.getNormalBounds();
      const state = {
        width: bounds.width,
        height: bounds.height,
        x: bounds.x,
        y: bounds.y,
        isMaximized: win.isMaximized(),
        savedAt: new Date().toISOString(),
      };
      const dir = ensureUserDataDir(id);
      writeFileSync(path.join(dir, "window-state.json"), JSON.stringify(state, null, 2), "utf8");
    } catch (error) {
      console.warn(`[kepler-shell] extension '${id}' save window-state failed:`, error);
    }
  };

  let saveTimer: ReturnType<typeof setTimeout> | null = null;
  const scheduleSave = (): void => {
    if (saveTimer) clearTimeout(saveTimer);
    saveTimer = setTimeout(saveWindowState, 500);
  };

  win.on("resized", scheduleSave);
  win.on("moved", scheduleSave);
  const broadcastMaximizedState = (): void => {
    if (win.isDestroyed()) return;
    try {
      win.webContents.send("kepler:extension:window:maximized-changed", win.isMaximized());
    } catch {
      // renderer may not be ready yet, ignore
    }
  };
  win.on("maximize", () => {
    saveWindowState();
    broadcastMaximizedState();
  });
  win.on("unmaximize", () => {
    saveWindowState();
    broadcastMaximizedState();
  });
  win.on("close", (event) => {
    if (saveTimer) {
      clearTimeout(saveTimer);
      saveTimer = null;
    }
    saveWindowState();
    if (manifest.keepAliveInBackground && !isAppQuitting() && !headless && !win.isDestroyed()) {
      event.preventDefault();
      win.hide();
    }
  });

  const wcId = win.webContents.id;
  webContentsToExtensionContext.set(wcId, {
    id,
    windowKey,
    source: location.source,
    manifestPermissions: manifest.permissions,
  });
  win.on("closed", () => {
    clearWindowIpcState(wcId);
    webContentsToExtensionContext.delete(wcId);
    extensionWindows.delete(windowKey);
  });
  extensionWindows.set(windowKey, {
    win,
    id,
    windowKey,
    keepAliveInBackground: manifest.keepAliveInBackground === true,
    initialRoute: route,
  });

  if (route) {
    win.webContents.once("did-finish-load", () => {
      try {
        win.webContents.send("kepler:extension:navigation", route);
      } catch {
        /* окно могло быть закрыто во время загрузки */
      }
    });
  }

  win.webContents.on("before-input-event", (event, input) => {
    if (input.key === "F12" && !input.alt && !input.control && !input.shift && !input.meta) {
      event.preventDefault();
      try {
        win.webContents.toggleDevTools();
      } catch {
        /* webContents destroyed mid-flight, ignore */
      }
    }
  });

  if (source.kind === "dev-server" && source.url) {
    console.log(`[kepler-shell] extension '${id}' dev mode → ${source.url}`);
    void win.loadURL(route?.startsWith("#") ? `${source.url}${route}` : source.url);
    win.webContents.openDevTools({ mode: "detach" });
  } else if (source.file) {
    void win.loadFile(source.file, route?.startsWith("#") ? { hash: route.slice(1) } : undefined);
  }
}
