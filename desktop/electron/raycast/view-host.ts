import { BrowserWindow, ipcMain, screen, clipboard, dialog } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { extensionUserDataDir } from "../extension-host";
import { isHeadless, isHeadlessOrTest } from "../extension-manifest";
import { runRaycastViewCommand, type RaycastCommandLaunchProps } from "./command-runner";
import { handleRaycastAction } from "./view-actions";
import { normalizeRaycastNode, type RaycastViewCallbackRegistry } from "./view-model";
import type { ExtensionSource } from "../extension-permissions";
import type {
  RaycastActionRequest,
  RaycastActionResult,
  RaycastFeedbackEvent,
  RaycastFilePickerRequest,
  RaycastFilePickerResult,
  RaycastSnapshot,
} from "../../shared/raycast-ipc";
import type { LaunchCommandOptions } from "@raycast/api";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const RAYCAST_HOST_WIDTH = 760;
const RAYCAST_HOST_HEIGHT = 560;
const RAYCAST_HOST_MIN_WIDTH = 620;
const RAYCAST_HOST_MIN_HEIGHT = 420;

const sessions = new Map<string, RaycastSnapshot>();
const callbacks = new Map<
  string,
  Map<string, (payload?: Record<string, unknown>) => unknown | Promise<unknown>>
>();
const launchers = new Map<string, (options: LaunchCommandOptions) => Promise<void>>();
const windows = new Map<string, BrowserWindow>();

function sendFeedback(
  win: BrowserWindow | null,
  sessionId: string | null,
  event: RaycastFeedbackEvent,
): void {
  if (!win || win.isDestroyed() || !sessionId) return;
  win.webContents.send("kepler:raycast:feedback", { sessionId, event });
}

function sessionWindowFromSender(
  sessionId: string,
  sender: Electron.WebContents,
): BrowserWindow | null {
  const sessionWindow = windows.get(sessionId);
  if (!sessionWindow || BrowserWindow.fromWebContents(sender) !== sessionWindow) return null;
  return sessionWindow;
}

function loadRaycastHost(
  win: BrowserWindow,
  sessionId: string,
  route: "raycast-host" | "command-host" = "raycast-host",
): void {
  const devUrl = process.env.VITE_DEV_SERVER_URL;
  const hash = `${route}?session=${encodeURIComponent(sessionId)}`;
  if (devUrl) {
    void win.loadURL(`${devUrl}#${hash}`);
  } else {
    void win.loadFile(path.join(__dirname, "../dist/index.html"), {
      hash,
    });
  }
}

export async function openRaycastViewCommand(options: {
  extensionId: string;
  extensionName: string;
  commandName: string;
  commandTitle: string;
  commandMode?: "view" | "menu-bar";
  extensionDir: string;
  source: ExtensionSource;
  launch?: RaycastCommandLaunchProps;
  system?: {
    open(target: string): Promise<void>;
    showInFinder(path: string): Promise<void>;
    trash(path: string): Promise<void>;
  };
  launchCommand?: (options: LaunchCommandOptions) => Promise<void>;
}): Promise<void> {
  const sessionCallbacks = new Map<
    string,
    (payload?: Record<string, unknown>) => unknown | Promise<unknown>
  >();
  let nextCallbackId = 0;
  const sessionId = `${options.extensionId}:${options.commandName}:${Date.now()}`;
  let sessionWindow: BrowserWindow | null = null;
  const navigationStack: RaycastSnapshot["root"][] = [];
  const callbackRegistry: RaycastViewCallbackRegistry = {
    register(callback) {
      const id = `action:${nextCallbackId++}`;
      sessionCallbacks.set(id, callback);
      return id;
    },
  };

  function publishSnapshotUpdate(): void {
    const snapshot = sessions.get(sessionId);
    const root = navigationStack[navigationStack.length - 1];
    if (!snapshot || !root) return;
    snapshot.root = root;
    sessions.set(sessionId, snapshot);
    if (!sessionWindow || sessionWindow.isDestroyed()) return;
    sessionWindow.webContents.send("kepler:raycast:snapshot-updated", {
      sessionId,
      snapshot,
    });
  }

  function pushNavigationTarget(target: unknown): void {
    const node = normalizeRaycastNode(target, callbackRegistry);
    if (!node) return;
    navigationStack.push(node);
    publishSnapshotUpdate();
  }

  const root = await runRaycastViewCommand({
    extensionId: options.extensionId,
    commandName: options.commandName,
    extensionDir: options.extensionDir,
    userDataDir: extensionUserDataDir(options.extensionId),
    source: options.source,
    commandMode: options.commandMode,
    launch: options.launch,
    clipboard,
    system: options.system,
    launchCommand: options.launchCommand,
    navigation: {
      push: pushNavigationTarget,
      pop() {
        if (navigationStack.length <= 1) return;
        navigationStack.pop();
        publishSnapshotUpdate();
      },
      popToRoot() {
        if (navigationStack.length <= 1) return;
        navigationStack.splice(1);
        publishSnapshotUpdate();
      },
    },
    feedback: (event) => sendFeedback(sessionWindow, sessionId, event),
    confirmAlert: async (alert) => {
      const options: Electron.MessageBoxOptions = {
        type: "question",
        buttons: [alert.primaryAction?.title ?? "OK", alert.dismissAction?.title ?? "Отмена"],
        defaultId: 0,
        cancelId: 1,
        title: alert.title,
        message: alert.title,
        detail: alert.message,
      };
      const messageBox = sessionWindow
        ? await dialog.showMessageBox(sessionWindow, options)
        : await dialog.showMessageBox(options);
      return messageBox.response === 0;
    },
    callbacks: callbackRegistry,
  });

  navigationStack.push(root);
  const snapshot: RaycastSnapshot = {
    sessionId,
    extensionId: options.extensionId,
    extensionName: options.extensionName,
    commandName: options.commandName,
    commandTitle: options.commandTitle,
    root,
    createdAt: new Date().toISOString(),
  };
  sessions.set(sessionId, snapshot);
  callbacks.set(sessionId, sessionCallbacks);
  if (options.launchCommand) launchers.set(sessionId, options.launchCommand);

  const display = screen.getPrimaryDisplay().workAreaSize;
  const win = new BrowserWindow({
    width: RAYCAST_HOST_WIDTH,
    height: RAYCAST_HOST_HEIGHT,
    minWidth: RAYCAST_HOST_MIN_WIDTH,
    minHeight: RAYCAST_HOST_MIN_HEIGHT,
    x: Math.round((display.width - RAYCAST_HOST_WIDTH) / 2),
    y: Math.round((display.height - RAYCAST_HOST_HEIGHT) / 2),
    show: !isHeadlessOrTest(),
    skipTaskbar: isHeadless(),
    title: `${options.extensionName} — ${options.commandTitle}`,
    backgroundColor: "#00000000",
    frame: true,
    titleBarStyle: "hidden",
    titleBarOverlay: {
      color: "#00000000",
      symbolColor: "#FFFFFF",
      height: 36,
    },
    roundedCorners: true,
    webPreferences: {
      preload: path.join(__dirname, "preload.mjs"),
      contextIsolation: true,
      nodeIntegration: false,
      backgroundThrottling: true,
    },
  });
  sessionWindow = win;

  try {
    win.setBackgroundMaterial("acrylic");
  } catch {
    /* best effort */
  }

  windows.set(sessionId, win);
  win.on("closed", () => {
    windows.delete(sessionId);
    sessions.delete(sessionId);
    callbacks.delete(sessionId);
    launchers.delete(sessionId);
  });
  win.webContents.on("before-input-event", (event, input) => {
    if (input.key === "F12" && !input.alt && !input.control && !input.shift && !input.meta) {
      event.preventDefault();
      try {
        win.webContents.toggleDevTools();
      } catch {
        /* webContents destroyed mid-flight */
      }
    }
  });

  loadRaycastHost(win, sessionId);
}

function getRaycastSnapshot(sessionId: string): RaycastSnapshot | null {
  return sessions.get(sessionId) ?? null;
}

ipcMain.handle("kepler:raycast:snapshot", (_event, sessionId: string): RaycastSnapshot | null => {
  if (typeof sessionId !== "string") return null;
  return getRaycastSnapshot(sessionId);
});

ipcMain.handle(
  "kepler:raycast:pick-files",
  async (
    event,
    sessionId: string,
    request: RaycastFilePickerRequest,
  ): Promise<RaycastFilePickerResult> => {
    if (typeof sessionId !== "string" || !request || typeof request !== "object") {
      return { ok: false, paths: [], error: "invalid_request" };
    }

    const sessionWindow = sessionWindowFromSender(sessionId, event.sender);
    if (!sessionWindow) return { ok: false, paths: [], error: "session_not_found" };

    const canChooseDirectories = request.canChooseDirectories === true;
    const canChooseFiles = request.canChooseFiles !== false || !canChooseDirectories;
    const properties: Electron.OpenDialogOptions["properties"] = [];
    if (canChooseFiles) properties.push("openFile");
    if (canChooseDirectories) properties.push("openDirectory");
    if (request.allowMultipleSelection === true) properties.push("multiSelections");
    if (request.showHiddenFiles === true) properties.push("showHiddenFiles");

    const result = await dialog.showOpenDialog(sessionWindow, { properties });
    if (result.canceled) return { ok: true, paths: [] };
    return { ok: true, paths: result.filePaths };
  },
);

ipcMain.handle(
  "kepler:raycast:action",
  async (event, sessionId: string, action: RaycastActionRequest): Promise<RaycastActionResult> => {
    if (typeof sessionId !== "string" || !action || typeof action !== "object") {
      return { ok: false, error: "invalid_request" };
    }

    const sessionWindow = sessionWindowFromSender(sessionId, event.sender);
    if (!sessionWindow) return { ok: false, error: "session_not_found" };

    const callbackId = action.props.__callbackId;
    if (typeof callbackId === "string") {
      const callback = callbacks.get(sessionId)?.get(callbackId);
      if (!callback) return { ok: false, error: "callback_not_found" };
    }

    return handleRaycastAction({
      action,
      callback:
        typeof callbackId === "string" ? callbacks.get(sessionId)?.get(callbackId) : undefined,
      launcher: launchers.get(sessionId),
    });
  },
);
