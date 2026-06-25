import { ipcRenderer } from "electron";
import type { KeplerApi } from "../shared/ipc-types";
import { createKeplerSettingsBridge } from "./preload-settings-bridge";

type KeplerTestApi = {
  __test?: {
    waitForReady(timeoutMs?: number): Promise<void>;
    getStats(): Promise<{
      arkConnected: boolean;
      commands: string[];
      commandsRegistered: number;
    }>;
  };
};

type KeplerPreloadApi = KeplerApi & KeplerTestApi;

export function installKeplerPlatformMarker(): void {
  const platform = platformMarker();
  const mark = () => {
    if (document.documentElement) document.documentElement.dataset.platform = platform;
  };
  mark();
  document.addEventListener("DOMContentLoaded", mark, { once: true });
}

export function createKeplerPreloadApi(): KeplerPreloadApi {
  const api: KeplerPreloadApi = {
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
    shell: {
      openExternal: (url: string) => ipcRenderer.invoke("kepler:shell:openExternal", url),
    },
    window: {
      hide: () => ipcRenderer.invoke("kepler:window:hide"),
      onShow: (listener) => {
        const handler = () => listener();
        ipcRenderer.on("kepler:window:show", handler);
        return () => ipcRenderer.removeListener("kepler:window:show", handler);
      },
      onHide: (listener) => {
        const handler = () => listener();
        ipcRenderer.on("kepler:window:hide", handler);
        return () => ipcRenderer.removeListener("kepler:window:hide", handler);
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
      onEvent: (listener) => {
        const handler = (_e: Electron.IpcRendererEvent, event: unknown) =>
          listener(event as Record<string, unknown>);
        ipcRenderer.on("kepler:ark:event", handler);
        return () => ipcRenderer.removeListener("kepler:ark:event", handler);
      },
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
    clipboardHistory: {
      list: () => ipcRenderer.invoke("kepler:clipboard-history:list"),
      copy: (id) => ipcRenderer.invoke("kepler:clipboard-history:copy", id),
      open: (id) => ipcRenderer.invoke("kepler:clipboard-history:open", id),
      togglePin: (id) => ipcRenderer.invoke("kepler:clipboard-history:toggle-pin", id),
      delete: (id) => ipcRenderer.invoke("kepler:clipboard-history:delete", id),
      clear: () => ipcRenderer.invoke("kepler:clipboard-history:clear"),
      clearAll: () => ipcRenderer.invoke("kepler:clipboard-history:clear-all"),
      settings: () => ipcRenderer.invoke("kepler:clipboard-history:settings"),
      updateSettings: (patch) =>
        ipcRenderer.invoke("kepler:clipboard-history:settings:update", patch),
      stats: () => ipcRenderer.invoke("kepler:clipboard-history:stats"),
      hide: () => ipcRenderer.invoke("kepler:clipboard-history:hide"),
      onOpenShell: (listener) => {
        const handler = () => listener();
        ipcRenderer.on("kepler:clipboard-history:open-shell", handler);
        return () => ipcRenderer.removeListener("kepler:clipboard-history:open-shell", handler);
      },
      onUpdated: (listener) => {
        const handler = () => listener();
        ipcRenderer.on("kepler:clipboard-history:updated", handler);
        return () => ipcRenderer.removeListener("kepler:clipboard-history:updated", handler);
      },
    },
    focusSession: {
      open: () => ipcRenderer.invoke("kepler:focus-session:open"),
      snapshot: () => ipcRenderer.invoke("kepler:focus-session:snapshot"),
      listTasks: () => ipcRenderer.invoke("kepler:focus-session:list-tasks"),
      listBlocklists: () => ipcRenderer.invoke("kepler:focus-session:list-blocklists"),
      start: (input) => ipcRenderer.invoke("kepler:focus-session:start", input),
      pause: () => ipcRenderer.invoke("kepler:focus-session:pause"),
      resume: () => ipcRenderer.invoke("kepler:focus-session:resume"),
      skip: () => ipcRenderer.invoke("kepler:focus-session:skip"),
      stop: () => ipcRenderer.invoke("kepler:focus-session:stop"),
      complete: () => ipcRenderer.invoke("kepler:focus-session:complete"),
      onOpenShell: (listener) => {
        const handler = () => listener();
        ipcRenderer.on("kepler:focus-session:open-shell", handler);
        return () => ipcRenderer.removeListener("kepler:focus-session:open-shell", handler);
      },
      onUpdated: (listener) => {
        const handler = () => listener();
        ipcRenderer.on("kepler:focus-session:updated", handler);
        return () => ipcRenderer.removeListener("kepler:focus-session:updated", handler);
      },
      onAppBlocked: (listener) => {
        const handler = (_e: Electron.IpcRendererEvent, app: unknown) =>
          listener(app as { id: string; title: string; icon?: string | null });
        ipcRenderer.on("kepler:focus:app-blocked", handler);
        return () => ipcRenderer.removeListener("kepler:focus:app-blocked", handler);
      },
      snoozeApp: (appId: string) => ipcRenderer.invoke("kepler:focus-session:snooze-app", appId),
    },
    raycast: {
      snapshot: (sessionId) => ipcRenderer.invoke("kepler:raycast:snapshot", sessionId),
      action: (sessionId, action) => ipcRenderer.invoke("kepler:raycast:action", sessionId, action),
      pickFiles: (sessionId, request) =>
        ipcRenderer.invoke("kepler:raycast:pick-files", sessionId, request),
      onSnapshotUpdated: (sessionId, listener) => {
        const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
          const update = payload as { sessionId?: unknown; snapshot?: unknown };
          if (update.sessionId === sessionId) listener(update.snapshot as never);
        };
        ipcRenderer.on("kepler:raycast:snapshot-updated", handler);
        return () => ipcRenderer.removeListener("kepler:raycast:snapshot-updated", handler);
      },
      onFeedback: (sessionId, listener) => {
        const handler = (_event: Electron.IpcRendererEvent, payload: unknown) => {
          const feedback = payload as { sessionId?: unknown; event?: unknown };
          if (feedback.sessionId === sessionId) listener(feedback.event as never);
        };
        ipcRenderer.on("kepler:raycast:feedback", handler);
        return () => ipcRenderer.removeListener("kepler:raycast:feedback", handler);
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
    fileSearch: {
      settingsGet: () => ipcRenderer.invoke("kepler:file-search:settings:get"),
      settingsSet: (patch) => ipcRenderer.invoke("kepler:file-search:settings:set", patch),
      diagnostics: () => ipcRenderer.invoke("kepler:file-search:diagnostics"),
      estimateRoot: (path) => ipcRenderer.invoke("kepler:file-search:estimate-root", path),
      scopeAdd: (path) => ipcRenderer.invoke("kepler:file-search:scope:add", path),
      scopeRemove: (path) => ipcRenderer.invoke("kepler:file-search:scope:remove", path),
      ignoreAdd: (pattern) => ipcRenderer.invoke("kepler:file-search:ignore:add", pattern),
      ignoreRemove: (pattern) => ipcRenderer.invoke("kepler:file-search:ignore:remove", pattern),
      rescan: () => ipcRenderer.invoke("kepler:file-search:rescan"),
      clearCache: () => ipcRenderer.invoke("kepler:file-search:clear-cache"),
      pickScope: () => ipcRenderer.invoke("kepler:file-search:pickScope"),
    },
    focusWidget: {
      setState: (patch) => ipcRenderer.invoke("kepler:focus-widget:set-state", patch),
      getState: () => ipcRenderer.invoke("kepler:focus-widget:get-state"),
      hide: () => ipcRenderer.invoke("kepler:focus-widget:hide"),
      openFocusSession: () => ipcRenderer.invoke("kepler:focus-widget:open-focus-session"),
      pomodoro: {
        pause: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:pause"),
        resume: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:resume"),
        skip: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:skip"),
        stop: () => ipcRenderer.invoke("kepler:focus-widget:pomodoro:stop"),
      },
      stopwatch: {
        stop: () => ipcRenderer.invoke("kepler:focus-widget:stopwatch:stop"),
      },
      showMenu: () => ipcRenderer.invoke("kepler:focus-widget:show-menu"),
      onState: (handler) => {
        const wrapper = (_e: Electron.IpcRendererEvent, state: unknown) =>
          handler(
            state as {
              active: boolean;
              remainingSec: number;
              totalSec: number;
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
    dictation: {
      toggle: () => ipcRenderer.invoke("kepler:dictation:toggle"),
      cancel: () => ipcRenderer.invoke("kepler:dictation:cancel"),
      pillFinished: () => ipcRenderer.invoke("kepler:dictation:pill-finished"),
      onCommand: (cb) => {
        const wrapper = (_e: Electron.IpcRendererEvent, cmd: unknown) =>
          cb(cmd as { kind: "start" | "stop" | "cancel" });
        ipcRenderer.on("kepler:dictation:command", wrapper);
        return () => ipcRenderer.removeListener("kepler:dictation:command", wrapper);
      },
      onCaptureEvent: (cb) => {
        const wrapper = (_e: Electron.IpcRendererEvent, payload: unknown) =>
          cb(payload as Record<string, unknown>);
        ipcRenderer.on("kepler:dictation:capture", wrapper);
        return () => ipcRenderer.removeListener("kepler:dictation:capture", wrapper);
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
    ...createKeplerSettingsBridge(),
  };

  if (process.env.KOSMOS_TEST_MODE === "1") {
    api.__test = {
      waitForReady: (timeoutMs?: number) =>
        ipcRenderer.invoke("kepler:__test:waitForReady", timeoutMs),
      getStats: () =>
        ipcRenderer.invoke("kepler:__test:getStats") as Promise<{
          arkConnected: boolean;
          commands: string[];
          commandsRegistered: number;
        }>,
    };
  }

  return api;
}

function platformMarker(): "mac" | "windows" | "linux" {
  if (process.platform === "darwin") return "mac";
  if (process.platform === "win32") return "windows";
  return "linux";
}
