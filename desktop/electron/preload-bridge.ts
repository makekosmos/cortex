import { ipcRenderer } from "electron";
import type { KeplerApi } from "../shared/ipc-types";
import { createKeplerSettingsBridge } from "./preload-settings-bridge";
import type { JsonRecord } from "./json-types";

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

type ArkEventPayload = JsonRecord;
type DictationCommand = { kind: "start" | "stop" | "cancel" };
type DictationCapturePayload = JsonRecord;

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
    search: {
      query: (text) => ipcRenderer.invoke("kepler:search:query", text),
    },
    objects: {
      listRecent: (limit) => ipcRenderer.invoke("kepler:objects:listRecent", limit),
    },
    ark: {
      request: (operation, params) => ipcRenderer.invoke("kepler:ark:request", operation, params),
      onEvent: (listener) => {
        const handler = (_e: Electron.IpcRendererEvent, event: ArkEventPayload) => {
          // SAFETY: The main process emits structured Ark events on this channel.
          listener(event);
        };
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
    export: {
      list: () => ipcRenderer.invoke("kepler:export:list"),
      run: (args) => ipcRenderer.invoke("kepler:export:run", args),
      pickDir: () => ipcRenderer.invoke("kepler:export:pickDir"),
    },
    fileIndex: {
      pickRoot: () => ipcRenderer.invoke("kepler:file-index:pick-root"),
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
    dictation: {
      toggle: () => ipcRenderer.invoke("kepler:dictation:toggle"),
      cancel: () => ipcRenderer.invoke("kepler:dictation:cancel"),
      pillFinished: () => ipcRenderer.invoke("kepler:dictation:pill-finished"),
      onCommand: (cb) => {
        const wrapper = (_e: Electron.IpcRendererEvent, cmd: DictationCommand) => {
          // SAFETY: The dictation IPC channel emits one of the documented commands.
          cb(cmd);
        };
        ipcRenderer.on("kepler:dictation:command", wrapper);
        return () => ipcRenderer.removeListener("kepler:dictation:command", wrapper);
      },
      onCaptureEvent: (cb) => {
        const wrapper = (_e: Electron.IpcRendererEvent, payload: DictationCapturePayload) => {
          // SAFETY: The dictation capture channel emits structured event payloads.
          cb(payload);
        };
        ipcRenderer.on("kepler:dictation:capture", wrapper);
        return () => ipcRenderer.removeListener("kepler:dictation:capture", wrapper);
      },
    },
    ...createKeplerSettingsBridge(),
  };

  if (process.env.KOSMOS_TEST_MODE === "1") {
    api.__test = {
      waitForReady: (timeoutMs?: number) =>
        ipcRenderer.invoke("kepler:__test:waitForReady", timeoutMs),
      getStats: () =>
        // SAFETY: The test IPC handler returns the documented stats snapshot.
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
