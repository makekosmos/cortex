import { BrowserWindow, ipcMain, screen, clipboard, dialog, shell } from "electron";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { extensionUserDataDir } from "../extension-host";
import { runRaycastViewCommand, type RaycastCommandLaunchProps } from "./command-runner";
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

function actionStringProp(props: Record<string, unknown>, ...keys: string[]): string | null {
  for (const key of keys) {
    const value = props[key];
    if (typeof value === "string" && value.trim().length > 0) return value;
  }
  return null;
}

function actionPathTargets(props: Record<string, unknown>): string[] {
  const paths = props.paths;
  if (Array.isArray(paths)) {
    return paths.filter(
      (item): item is string => typeof item === "string" && item.trim().length > 0,
    );
  }
  const single = actionStringProp(props, "path", "target");
  return single ? [single] : [];
}

function sessionWindowFromSender(
  sessionId: string,
  sender: Electron.WebContents,
): BrowserWindow | null {
  const sessionWindow = windows.get(sessionId);
  if (!sessionWindow || BrowserWindow.fromWebContents(sender) !== sessionWindow) return null;
  return sessionWindow;
}

function isHeadlessOrTest(): boolean {
  return process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
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

function loadLauncherRoot(win: BrowserWindow): void {
  const devUrl = process.env.VITE_DEV_SERVER_URL;
  if (devUrl) {
    void win.loadURL(devUrl);
  } else {
    void win.loadFile(path.join(__dirname, "../dist/index.html"));
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
    skipTaskbar: process.env.KOSMOS_HEADLESS === "1",
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

export async function openRaycastElementView(options: {
  extensionId: string;
  extensionName: string;
  commandName: string;
  commandTitle: string;
  root: unknown;
  hostWindow?: BrowserWindow | null;
}): Promise<void> {
  const sessionCallbacks = new Map<
    string,
    (payload?: Record<string, unknown>) => unknown | Promise<unknown>
  >();
  let nextCallbackId = 0;
  const sessionId = `${options.extensionId}:${options.commandName}:${Date.now()}`;
  const callbackRegistry: RaycastViewCallbackRegistry = {
    register(callback) {
      const id = `action:${nextCallbackId++}`;
      sessionCallbacks.set(id, callback);
      return id;
    },
  };
  const root = normalizeRaycastNode(options.root, callbackRegistry);
  if (!root) {
    throw new Error(
      `[kepler-shell] built-in Raycast command returned no UI: ${options.commandName}`,
    );
  }

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

  const hosted = options.hostWindow && !options.hostWindow.isDestroyed();
  const display = screen.getPrimaryDisplay().workAreaSize;
  const win =
    options.hostWindow && !options.hostWindow.isDestroyed()
      ? options.hostWindow
      : new BrowserWindow({
          width: RAYCAST_HOST_WIDTH,
          height: RAYCAST_HOST_HEIGHT,
          minWidth: RAYCAST_HOST_MIN_WIDTH,
          minHeight: RAYCAST_HOST_MIN_HEIGHT,
          x: Math.round((display.width - RAYCAST_HOST_WIDTH) / 2),
          y: Math.round((display.height - RAYCAST_HOST_HEIGHT) / 2),
          show: !isHeadlessOrTest(),
          skipTaskbar: process.env.KOSMOS_HEADLESS === "1",
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

  if (hosted) {
    const bounds = win.getBounds();
    win.setBounds({
      x: Math.round(bounds.x + (bounds.width - RAYCAST_HOST_WIDTH) / 2),
      y: Math.round(bounds.y + (bounds.height - RAYCAST_HOST_HEIGHT) / 2),
      width: RAYCAST_HOST_WIDTH,
      height: RAYCAST_HOST_HEIGHT,
    });
    win.setTitle(`${options.extensionName} — ${options.commandTitle}`);
  }

  try {
    win.setBackgroundMaterial("acrylic");
  } catch {
    /* best effort */
  }

  windows.set(sessionId, win);
  const cleanup = () => {
    windows.delete(sessionId);
    sessions.delete(sessionId);
    callbacks.delete(sessionId);
  };
  win.once("closed", cleanup);
  if (hosted) {
    win.once("hide", () => {
      cleanup();
      loadLauncherRoot(win);
    });
  }
  if (!hosted) {
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
  }

  loadRaycastHost(win, sessionId, "command-host");
}

export function getRaycastSnapshot(sessionId: string): RaycastSnapshot | null {
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
      try {
        await callback(action.payload);
        return { ok: true };
      } catch (err) {
        return { ok: false, error: err instanceof Error ? err.message : String(err) };
      }
    }

    if (action.type === "Action.CopyToClipboard") {
      const content = action.props.content;
      if (typeof content !== "string") return { ok: false, error: "missing_content" };
      clipboard.writeText(content);
      return { ok: true };
    }

    if (action.type === "Action.Paste") {
      const content = action.props.content;
      if (typeof content !== "string") return { ok: false, error: "missing_content" };
      clipboard.writeText(content);
      return { ok: true };
    }

    if (action.type === "Action.OpenInBrowser") {
      const url = action.props.url;
      if (typeof url !== "string") return { ok: false, error: "missing_url" };
      await shell.openExternal(url);
      return { ok: true };
    }

    if (action.type === "Action.Open") {
      const target = action.props.target;
      if (typeof target !== "string") return { ok: false, error: "missing_target" };
      if (/^https?:\/\//i.test(target)) {
        await shell.openExternal(target);
        return { ok: true };
      }

      const error = await shell.openPath(target);
      return error ? { ok: false, error } : { ok: true };
    }

    if (action.type === "Action.ShowInFinder") {
      const target = actionStringProp(action.props, "path", "target");
      if (!target) return { ok: false, error: "missing_path" };
      shell.showItemInFolder(target);
      return { ok: true };
    }

    if (action.type === "Action.Trash") {
      const targets = actionPathTargets(action.props);
      if (targets.length === 0) return { ok: false, error: "missing_path" };
      for (const target of targets) await shell.trashItem(target);
      return { ok: true };
    }

    if (action.type === "Action.LaunchCommand") {
      const name = action.props.name;
      if (typeof name !== "string") return { ok: false, error: "missing_name" };
      const launcher = launchers.get(sessionId);
      if (!launcher) return { ok: false, error: "launcher_not_configured" };
      const extensionName =
        typeof action.props.extensionName === "string" ? action.props.extensionName : undefined;
      const fallbackText =
        typeof action.props.fallbackText === "string" ? action.props.fallbackText : undefined;
      const type =
        typeof action.props.type === "string"
          ? (action.props.type as LaunchCommandOptions["type"])
          : undefined;
      const args =
        action.props.arguments && typeof action.props.arguments === "object"
          ? (action.props.arguments as Record<string, unknown>)
          : undefined;
      await launcher({
        name,
        extensionName,
        type,
        arguments: args,
        context: action.props.context,
        fallbackText,
      });
      return { ok: true };
    }

    return { ok: false, error: "unsupported_action" };
  },
);
