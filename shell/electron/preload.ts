// Preload script: контекст-bridge между renderer'ом и main process.
//
// Exposes `window.kepler` namespace по контракту из shared/ipc-types.ts.
// Renderer не имеет прямого доступа к Node/Electron API — только через эти
// invoke handlers и pub/sub каналы.

import { contextBridge, ipcRenderer } from "electron";
import type { KeplerApi } from "../shared/ipc-types";

const api: KeplerApi = {
  backend: {
    status: () => ipcRenderer.invoke("kepler:backend:status"),
    restart: () => ipcRenderer.invoke("kepler:backend:restart"),
    onReady: (listener) => {
      const handler = () => listener();
      ipcRenderer.on("kepler:backend:ready", handler);
      return () => ipcRenderer.removeListener("kepler:backend:ready", handler);
    },
    onDisconnected: (listener) => {
      const handler = () => listener();
      ipcRenderer.on("kepler:backend:disconnected", handler);
      return () => ipcRenderer.removeListener("kepler:backend:disconnected", handler);
    },
  },
  window: {
    hide: () => ipcRenderer.invoke("kepler:window:hide"),
    onShow: (listener) => {
      const handler = () => listener();
      ipcRenderer.on("kepler:window:show", handler);
      return () => ipcRenderer.removeListener("kepler:window:show", handler);
    },
    setExpanded: (expanded) => ipcRenderer.invoke("kepler:window:setExpanded", expanded),
  },
  search: {
    query: (text) => ipcRenderer.invoke("kepler:search:query", text),
  },
  objects: {
    listRecent: (limit) => ipcRenderer.invoke("kepler:objects:listRecent", limit),
  },
  ark: {
    request: (operation, params) => ipcRenderer.invoke("kepler:ark:request", operation, params),
  },
  commands: {
    list: () => ipcRenderer.invoke("kepler:commands:list"),
    invoke: (id) => ipcRenderer.invoke("kepler:commands:invoke", id),
    onUpdated: (listener) => {
      const handler = () => listener();
      ipcRenderer.on("kepler:commands:updated", handler);
      return () => ipcRenderer.removeListener("kepler:commands:updated", handler);
    },
  },
  extension: {
    installPreview: (sourcePath) =>
      ipcRenderer.invoke("kepler:extension:install:preview", sourcePath),
    installDo: (sourcePath) => ipcRenderer.invoke("kepler:extension:install:do", sourcePath),
    installedList: () => ipcRenderer.invoke("kepler:extension:installed:list"),
    revert: (id, timestamp) => ipcRenderer.invoke("kepler:extension:revert", id, timestamp),
    backupsList: (id) => ipcRenderer.invoke("kepler:extension:backups:list", id),
    uninstall: (id) => ipcRenderer.invoke("kepler:extension:uninstall", id),
    catalogFetch: (force) => ipcRenderer.invoke("kepler:extension:catalog:fetch", force),
    installFromUrl: (url, expectedSha256) =>
      ipcRenderer.invoke("kepler:extension:install:fromUrl", url, expectedSha256),
  },
  export: {
    list: () => ipcRenderer.invoke("kepler:export:list"),
    run: (args) => ipcRenderer.invoke("kepler:export:run", args),
    pickDir: () => ipcRenderer.invoke("kepler:export:pickDir"),
  },
  focusWidget: {
    setState: (patch) => ipcRenderer.invoke("kepler:focus-widget:set-state", patch),
    getState: () => ipcRenderer.invoke("kepler:focus-widget:get-state"),
    hide: () => ipcRenderer.invoke("kepler:focus-widget:hide"),
    openHorologion: () => ipcRenderer.invoke("kepler:focus-widget:open-horologion"),
    pomodoro: {
      pause: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:pause"),
      resume: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:resume"),
      skip: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:skip"),
      stop: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:stop"),
    },
    stopwatch: {
      stop: () => ipcRenderer.invoke("kepler:focus-widget:stopwatch:stop"),
    },
    onState: (handler) => {
      const wrapper = (_e: Electron.IpcRendererEvent, state: unknown) =>
        handler(
          state as {
            active: boolean;
            remainingSec: number;
            label: string;
            mode: "work" | "break" | "stopwatch";
            blockingActive: boolean;
            isPaused: boolean;
          },
        );
      ipcRenderer.on("kepler:focus-widget:state", wrapper);
      return () => ipcRenderer.removeListener("kepler:focus-widget:state", wrapper);
    },
  },
  focusService: {
    status: () => ipcRenderer.invoke("kepler:focus-service:status"),
    ping: () => ipcRenderer.invoke("kepler:focus-service:ping"),
    install: () => ipcRenderer.invoke("kepler:focus-service:install"),
    uninstall: () => ipcRenderer.invoke("kepler:focus-service:uninstall"),
    start: () => ipcRenderer.invoke("kepler:focus-service:start"),
    stop: () => ipcRenderer.invoke("kepler:focus-service:stop"),
    autoInstallDeclined: {
      get: () => ipcRenderer.invoke("kepler:focus-service:auto-install-declined:get"),
      set: (value) => ipcRenderer.invoke("kepler:focus-service:auto-install-declined:set", value),
    },
    onStatusChanged: (cb) => {
      const wrapper = () => cb();
      ipcRenderer.on("kepler:focus-service:status-changed", wrapper);
      return () => ipcRenderer.removeListener("kepler:focus-service:status-changed", wrapper);
    },
  },
  settings: {
    open: () => ipcRenderer.invoke("kepler:settings:open"),
    close: () => ipcRenderer.invoke("kepler:settings:close"),
    autostart: {
      get: () => ipcRenderer.invoke("kepler:settings:autostart:get"),
      allowed: () => ipcRenderer.invoke("kepler:settings:autostart:allowed"),
      set: (enabled) => ipcRenderer.invoke("kepler:settings:autostart:set", enabled),
    },
    trayIcon: {
      get: () => ipcRenderer.invoke("kepler:settings:tray-icon:get"),
      set: (enabled) => ipcRenderer.invoke("kepler:settings:tray-icon:set", enabled),
    },
    developerMode: {
      get: () => ipcRenderer.invoke("kepler:settings:developer-mode:get"),
      set: (enabled) => ipcRenderer.invoke("kepler:settings:developer-mode:set", enabled),
    },
    usageTracker: {
      get: () => ipcRenderer.invoke("kepler:settings:usage-tracker:get"),
      set: (enabled) => ipcRenderer.invoke("kepler:settings:usage-tracker:set", enabled),
    },
    launcherStateTtl: {
      get: () => ipcRenderer.invoke("kepler:settings:launcher-state-ttl:get"),
      set: (minutes: number) =>
        ipcRenderer.invoke("kepler:settings:launcher-state-ttl:set", minutes),
    },
    version: () => ipcRenderer.invoke("kepler:settings:version"),
    hotkey: () => ipcRenderer.invoke("kepler:settings:hotkey"),
    hotkeySet: (value) => ipcRenderer.invoke("kepler:settings:hotkey:set", value),
    hotkeyReset: () => ipcRenderer.invoke("kepler:settings:hotkey:reset"),
    update: {
      check: () => ipcRenderer.invoke("kepler:settings:update:check"),
      install: () => ipcRenderer.invoke("kepler:settings:update:install"),
      state: () => ipcRenderer.invoke("kepler:settings:update:state"),
      onStateChanged: (listener) => {
        const handler = (_e: unknown, state: unknown) => listener(state as never);
        ipcRenderer.on("kepler:settings:update:state", handler);
        return () => ipcRenderer.removeListener("kepler:settings:update:state", handler);
      },
    },
  },
  crashes: {
    list: () => ipcRenderer.invoke("kepler:crashes:list"),
    openFolder: () => ipcRenderer.invoke("kepler:crashes:openFolder"),
    clear: () => ipcRenderer.invoke("kepler:crashes:clear"),
  },
  diagnostics: {
    bundle: () => ipcRenderer.invoke("kepler:diagnostics:bundle"),
    bundleSave: () => ipcRenderer.invoke("kepler:diagnostics:bundle-save"),
    openLogsFolder: () => ipcRenderer.invoke("kepler:diagnostics:open-logs-folder"),
  },
  postUpdate: {
    onShown: (listener) => {
      const handler = (_e: unknown, payload: unknown) => listener(payload as { version: string });
      ipcRenderer.on("kepler:post-update", handler);
      return () => ipcRenderer.removeListener("kepler:post-update", handler);
    },
  },
};

// Test rig — exposed только в test mode. Production preload не вешает
// __test, поэтому `window.kepler.__test` === undefined у настоящих
// пользователей. См. spec: .agent/tasks/2026-05-21-bug-detection-phase1-determinism/.
if (process.env.KOSMOS_TEST_MODE === "1") {
  api.__test = {
    waitForReady: (timeoutMs?: number) =>
      ipcRenderer.invoke("kepler:__test:waitForReady", timeoutMs),
    getStats: () => ipcRenderer.invoke("kepler:__test:getStats"),
  };
}

contextBridge.exposeInMainWorld("kepler", api);
