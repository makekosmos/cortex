import { app, BrowserWindow, dialog, globalShortcut, ipcMain } from "electron";
import { fileURLToPath } from "node:url";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import path from "node:path";
import {
  EngineClient,
  hasArkGrant,
  hasLaunchReadPermission,
  isInstalledEnabledApp,
  isV2Launch,
  launchRenewalDelayMs,
  hasLauncherGrant,
  redactedCrashMetadata,
  SAFE_ID,
  type AppLaunch,
  type SidecarEvent,
  type JsonRecord,
  isJsonRecord,
  isJsonString,
} from "./host-api";
import { HostLifecycle } from "./lifecycle";
import { LaunchOwnership, type OwnedLaunch } from "./launch-ownership";
import { kosmosAppIcon, kosmosAppName, kosmosAppShortcutIcon } from "./kosmos-app-branding";
import { reconcileShortcuts } from "./shortcuts";

const requested = (argv: string[]) => {
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    const id = argument.startsWith("--open-app=")
      ? argument.slice("--open-app=".length)
      : argument === "--open-app"
        ? argv[index + 1]
        : undefined;
    if (id !== undefined) return SAFE_ID.test(id) ? id : undefined;
  }
  return undefined;
};
const requestedDevelopmentUrl = (argv: string[]): string | undefined => {
  if (process.env.KOSMOS_DEV_MODE !== "1") return undefined;
  const value = argv
    .find((argument) => argument.startsWith("--dev-url="))
    ?.slice("--dev-url=".length);
  try {
    if (!value) return undefined;
    const url = new URL(value);
    return url?.protocol === "http:" &&
      ["127.0.0.1", "localhost"].includes(url.hostname) &&
      url.port
      ? url.toString()
      : undefined;
  } catch {
    return undefined;
  }
};
const isOperationRequest = (value: JsonRecord): value is JsonRecord & { operation: string } =>
  isJsonString(value.operation);
const hasOpenApp = (argv: string[]) =>
  argv.some((argument) => argument === "--open-app" || argument.startsWith("--open-app="));
const windows = new Map<string, BrowserWindow>();
const manifests = new Map<string, AppLaunch>();
const eventSubscribers = new Set<number>();
const renewalTimers = new Map<string, ReturnType<typeof setTimeout>>();
const ownership = new LaunchOwnership();
const headless = process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1";
const engine = new EngineClient(app.getPath("appData"), process.env.KOSMOS_DATA_DIR);
let shutdownDraining = false;
const requestExit = (): void => {
  console.warn("[host-lifecycle] exit requested");
  app.quit();
};
// HostLifecycle's default mirrors the Engine's five-minute warm window; an
// explicit Engine setting of 0 still overrides it before the first close.
const lifecycle = new HostLifecycle(requestExit);

function clearLaunchRenewal(launchId: string): void {
  const timer = renewalTimers.get(launchId);
  if (timer) clearTimeout(timer);
  renewalTimers.delete(launchId);
}

function scheduleLaunchRenewal(manifest: AppLaunch, retry = false): void {
  clearLaunchRenewal(manifest.launch_id);
  if (!isV2Launch(manifest) || !manifest.broker_token) return;
  const delay = launchRenewalDelayMs(manifest.expires_at, Date.now(), retry);
  if (delay === null) {
    console.warn("desktop-host launch renewal expired", {
      app_id: manifest.id.slice(0, 128),
      outcome: "expired",
    });
    const win = windows.get(manifest.id);
    if (win && !win.isDestroyed()) win.close();
    else void revokeLaunch(manifest.id, manifest.launch_id);
    return;
  }
  const timer = setTimeout(async () => {
    renewalTimers.delete(manifest.launch_id);
    const result = await engine.renewLaunch(manifest.launch_id, manifest.broker_token!);
    if (manifests.get(manifest.id)?.launch_id !== manifest.launch_id) return;
    if (
      result.ok &&
      result.data.launch_id === manifest.launch_id &&
      Number.isFinite(result.data.ttl_seconds) &&
      isJsonString(result.data.expires_at)
    ) {
      manifest.ttl_seconds = result.data.ttl_seconds;
      manifest.expires_at = result.data.expires_at;
      scheduleLaunchRenewal(manifest);
    } else {
      console.warn("desktop-host launch renewal failed", {
        app_id: manifest.id.slice(0, 128),
        outcome: "rejected",
      });
      scheduleLaunchRenewal(manifest, true);
    }
  }, delay);
  renewalTimers.set(manifest.launch_id, timer);
}

const reportFailure = (message: string, exit = false): void => {
  if (!headless && app.isReady()) dialog.showErrorBox("Не удалось открыть приложение", message);
  if (exit) app.quit();
};

const fanoutArkEvent = (event: SidecarEvent): void => {
  for (const [id, win] of windows) {
    const manifest = manifests.get(id);
    if (
      !manifest ||
      !eventSubscribers.has(win.webContents.id) ||
      !hasLaunchReadPermission(manifest, event)
    )
      continue;
    win.webContents.send("host:ark-event", event);
  }
};

async function resolveLiveLaunchManifest(id: string, launch: AppLaunch): Promise<void> {
  const resolved = await engine.resolveApp(id, launch.version);
  if (!resolved.ok || resolved.data.id !== launch.id || resolved.data.version !== launch.version) {
    console.warn("desktop-host live manifest resolve rejected", { app_id: id });
  }
}

function revokeLaunch(id: string, launchId: string): Promise<void> {
  return engine.revokeApp(launchId).then(
    (result) => {
      if (!result.ok)
        console.warn("desktop-host launch revoke failed", {
          app_id: id.slice(0, 128),
          outcome: "rejected",
        });
    },
    () =>
      console.warn("desktop-host launch revoke failed", {
        app_id: id.slice(0, 128),
        outcome: "rejected",
      }),
  );
}

function releaseLaunch(
  id: string,
  win: BrowserWindow,
  claim: OwnedLaunch,
  webContentsId: number,
): void {
  const launchId = ownership.take(claim);
  if (!launchId) return;
  clearLaunchRenewal(launchId);
  eventSubscribers.delete(webContentsId);
  if (windows.get(id) === win) windows.delete(id);
  if (manifests.get(id)?.launch_id === launchId) manifests.delete(id);
  lifecycle.closed();
  void revokeLaunch(id, launchId);
}

function cleanupReplacedLaunch(replaced: OwnedLaunch): void {
  clearLaunchRenewal(replaced.launchId);
  eventSubscribers.delete(replaced.webContentsId);
  if (windows.get(replaced.id) === replaced.owner) windows.delete(replaced.id);
  if (manifests.get(replaced.id)?.launch_id === replaced.launchId) manifests.delete(replaced.id);
  lifecycle.closed();
  void revokeLaunch(replaced.id, replaced.launchId);
}

async function openApp(
  id: string,
  initial = false,
  developmentUrl = requestedDevelopmentUrl(process.argv),
): Promise<void> {
  const existing = windows.get(id);
  if (existing && !existing.isDestroyed()) {
    if (!headless) {
      existing.show();
      existing.focus();
    }
    return;
  }
  const result = await engine.launchApp(id);
  if (!result.ok) {
    console.warn("desktop-host app launch rejected", { app_id: id });
    reportFailure(result.message, initial);
    return;
  }
  const manifest = result.data;
  const name = kosmosAppName(manifest.id, manifest.name);
  const win = new BrowserWindow({
    show: !headless,
    title: name,
    icon: kosmosAppIcon(process.resourcesPath, manifest.id),
    autoHideMenuBar: true,
    width: 1100,
    height: 760,
    // Package apps keep their renderer surface transparent so the shared
    // Kosmos dark canvas is supplied by the native Host window.
    backgroundColor: "#1d1d1f",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      preload: fileURLToPath(new URL("./preload.mjs", import.meta.url)),
      additionalArguments: [
        `--kosmos-app=${JSON.stringify({ id: manifest.id, version: manifest.version, name })}`,
        `--kosmos-ark=${isV2Launch(manifest) || hasArkGrant(manifest.permissions) ? "1" : "0"}`,
      ],
    },
  });
  // Package HTML may still carry its legacy document title (Eden/Delphi).
  // Keep the native window, taskbar, and Alt+Tab name canonical.
  win.on("page-title-updated", (event) => {
    event.preventDefault();
    win.setTitle(name);
  });
  const webContentsId = win.webContents.id;
  const { current: claim, replaced } = ownership.claim(id, win, webContentsId, manifest.launch_id);
  lifecycle.opened();
  if (replaced) cleanupReplacedLaunch(replaced);
  manifests.set(id, manifest);
  windows.set(id, win);
  scheduleLaunchRenewal(manifest);
  const launchOrigin = (() => {
    try {
      return new URL(developmentUrl ?? manifest.launch_url).origin;
    } catch {
      return "";
    }
  })();
  const leavesLaunchOrigin = (url: string): boolean => {
    if (!isV2Launch(manifest)) return false;
    try {
      return new URL(url).origin !== launchOrigin;
    } catch {
      return true;
    }
  };
  const revokeOnNavigation = (url: string): void => {
    if (!leavesLaunchOrigin(url)) return;
    releaseLaunch(id, win, claim, webContentsId);
    if (!win.isDestroyed()) win.close();
  };
  win.webContents.on("will-navigate", (event, url) => {
    if (!leavesLaunchOrigin(url)) return;
    event.preventDefault();
    revokeOnNavigation(url);
  });
  win.webContents.on("did-start-navigation", (_event, url, _isInPlace, isMainFrame) => {
    if (isMainFrame) revokeOnNavigation(url);
  });
  win.on("closed", () => {
    releaseLaunch(id, win, claim, webContentsId);
    console.warn("[host-lifecycle] window closed", {
      app_id: id,
      remaining: windows.size,
    });
  });
  win.webContents.on("render-process-gone", (_event, details) => {
    releaseLaunch(id, win, claim, webContentsId);
    console.warn(
      "desktop-host renderer stopped",
      redactedCrashMetadata(id, manifest.version, details),
    );
  });
  win.webContents.on("did-fail-load", () => {
    releaseLaunch(id, win, claim, webContentsId);
  });
  try {
    await win.loadURL(developmentUrl ?? manifest.launch_url);
    void resolveLiveLaunchManifest(id, manifest);
  } catch {
    // Closing a window while navigation is pending is normal lifecycle, not a launch failure.
    if (win.isDestroyed()) return;
    console.warn("desktop-host app resource failed", { app_id: id });
    reportFailure("Ресурс приложения недоступен.", initial);
    win.close();
  }
}

const initialOpenAppId = requested(process.argv);
const singleInstance = app.requestSingleInstanceLock();
if (!singleInstance) {
  app.quit();
} else {
  app.on("window-all-closed", () => {});
  app.on("second-instance", async (_event, argv) => {
    const id = requested(argv);
    console.warn("[host-lifecycle] second instance", { app_id: id ?? null });
    if (id) {
      await app.whenReady();
      await openApp(id, false, requestedDevelopmentUrl(argv));
    } else if (hasOpenApp(argv)) reportFailure("Некорректный идентификатор приложения.");
  });
  app.whenReady().then(async () => {
    const timeout = await engine.getWarmTimeout();
    if (timeout.ok) lifecycle.setWarmTimeout(timeout.data);
    engine.onArkEvent(fanoutArkEvent);
    ipcMain.on("host:window", (event, action: "minimize" | "close") => {
      const win = BrowserWindow.fromWebContents(event.sender);
      if (!win) return;
      if (action === "minimize") win.minimize();
      else win.close();
    });
    ipcMain.handle("host:user-data", (_event, input: JsonRecord | undefined) => {
      const operation = input?.operation;
      const name = input?.name;
      if (operation === "path") return path.join(app.getPath("userData"), "extension-data");
      if (!isJsonString(name) || !/^[\w][\w.-]*$/.test(name))
        throw new Error("Invalid user data file name");
      const dir = path.join(app.getPath("userData"), "extension-data");
      mkdirSync(dir, { recursive: true });
      const file = path.join(dir, name);
      if (operation === "readJson")
        return existsSync(file) ? JSON.parse(readFileSync(file, "utf8")) : null;
      if (operation === "writeJson") {
        writeFileSync(file, JSON.stringify(input?.value, null, 2), "utf8");
        return undefined;
      }
      if (operation === "deleteFile") {
        if (!existsSync(file)) return false;
        rmSync(file, { force: true });
        return true;
      }
      throw new Error("Unknown user data operation");
    });
    ipcMain.on("host:ark-subscribe", (event) => {
      const win = BrowserWindow.fromWebContents(event.sender);
      if (!win) return;
      const appId = [...windows.entries()].find(([, candidate]) => candidate === win)?.[0];
      const manifest = appId ? manifests.get(appId) : undefined;
      if (manifest && hasLaunchReadPermission(manifest)) eventSubscribers.add(event.sender.id);
    });
    ipcMain.handle("host:ark-request", async (event, input: JsonRecord | undefined) => {
      const win = BrowserWindow.fromWebContents(event.sender);
      const appId = [...windows.entries()].find(([, candidate]) => candidate === win)?.[0];
      const manifest = appId ? manifests.get(appId) : undefined;
      if (!manifest || !isJsonRecord(input))
        return {
          ok: false,
          message: "Некорректный типизированный запрос ARK.",
        };
      const request = input;
      if (isOperationRequest(request)) {
        const operation = request.operation;
        const params = isJsonRecord(request.params) ? request.params : {};
        if (isV2Launch(manifest)) {
          if (!manifest.broker_token)
            return {
              ok: false,
              message: "Launch-scoped ARK authority недоступна.",
            };
          return engine.launchArkRequest(
            manifest.launch_id,
            manifest.broker_token,
            operation,
            params,
          );
        }
        const granted = manifest.permissions.some(
          (grant) =>
            (grant.capability === "ark.read" || grant.capability === "ark.write") &&
            Array.isArray(grant.scopes) &&
            grant.scopes.includes(operation),
        );
        if (!granted)
          return {
            ok: false,
            message: "Операция ARK не разрешена приложению.",
          };
        return engine.arkRequest(request.operation, params);
      }
      return { ok: false, message: "Некорректный запрос ARK." };
    });
    ipcMain.handle("host:dialogs:pick-directory-grant", async (event) => {
      const win = BrowserWindow.fromWebContents(event.sender);
      if (!win || win.isDestroyed()) return null;
      const appId = [...windows.entries()].find(([, candidate]) => candidate === win)?.[0];
      const manifest = appId ? manifests.get(appId) : undefined;
      if (!manifest || !isV2Launch(manifest) || !manifest.broker_token) return null;

      let selectedDirectory: string | undefined;
      if (process.env.KOSMOS_TEST_MODE === "1") {
        selectedDirectory = process.env.KOSMOS_TEST_SELECTED_DIRECTORY;
      } else {
        const result = await dialog.showOpenDialog(win, {
          properties: ["openDirectory"],
        });
        selectedDirectory = result.canceled ? undefined : result.filePaths[0];
      }
      if (!selectedDirectory) return null;

      const result = await engine.registerDirectoryGrant(
        manifest.launch_id,
        manifest.broker_token,
        selectedDirectory,
      );
      return result.ok ? result.data : null;
    });
    ipcMain.handle("host:launcher-request", async (event, input: JsonRecord | undefined) => {
      const win = BrowserWindow.fromWebContents(event.sender);
      const appId = win
        ? [...windows.entries()].find(([, candidate]) => candidate === win)?.[0]
        : undefined;
      const operation = isJsonString(input?.operation) ? input.operation : "";
      const params = isJsonRecord(input?.params) ? input.params : {};
      const manifest = appId ? manifests.get(appId) : undefined;
      const allowed = new Set([
        "app_index.list_all",
        "app_index.search",
        "app_index.launch",
        "file_index.search",
        "file_index.open",
        "commands.list",
        "commands.invoke",
      ]);
      if (
        !manifest ||
        !allowed.has(operation) ||
        !hasLauncherGrant(manifest.permissions, operation)
      )
        return {
          ok: false,
          message: "Операция лаунчера не разрешена приложению.",
        };
      return engine.launcherRequest(operation, params);
    });
    const id = initialOpenAppId;
    if (id) await openApp(id, true);
    else if (hasOpenApp(process.argv))
      reportFailure("Некорректный идентификатор приложения.", true);
    if (!headless && process.platform === "win32") {
      if (
        !globalShortcut.register("CommandOrControl+Shift+K", () => {
          void openApp("com.kosmos.shell");
        })
      ) {
        console.warn("desktop-host shell shortcut unavailable; Start Menu remains functional");
      }
    }
    if (process.platform === "win32") {
      const listed = await engine.arkRequest("packages.list", { kind: "app" });
      const packageList =
        listed.ok && isJsonRecord(listed.data) && Array.isArray(listed.data.packages)
          ? listed.data.packages
              .flatMap((item) => (isJsonRecord(item) ? [item] : []))
              .filter(isInstalledEnabledApp)
          : [];
      if (packageList.length > 0) {
        reconcileShortcuts(
          packageList.map((item) => ({
            id: item.id,
            name: kosmosAppName(item.id, isJsonString(item.name) ? item.name : item.id),
            enabled: true,
            revoked: false,
            iconPath:
              kosmosAppShortcutIcon(process.resourcesPath, item.id) ??
              (isJsonString(item.icon_path) ? item.icon_path : undefined),
          })),
        );
      }
    }
  });
  app.on("before-quit", (event) => {
    globalShortcut.unregister("CommandOrControl+Shift+K");
    event.preventDefault();
    if (shutdownDraining) return;
    shutdownDraining = true;
    // Clear all renderer-reachable state before bounded best-effort network cleanup.
    windows.clear();
    manifests.clear();
    eventSubscribers.clear();
    for (const timer of renewalTimers.values()) clearTimeout(timer);
    renewalTimers.clear();
    const timeout = new Promise<void>((resolve) => setTimeout(resolve, 1_000));
    void Promise.race([
      ownership.drain((launchId) => revokeLaunch("shutdown", launchId)),
      timeout,
    ]).finally(() => app.exit(0));
  });
}
