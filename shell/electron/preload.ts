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
  },
  window: {
    hide: () => ipcRenderer.invoke("kepler:window:hide"),
    onShow: (listener) => {
      const handler = () => listener();
      ipcRenderer.on("kepler:window:show", handler);
      return () => ipcRenderer.removeListener("kepler:window:show", handler);
    },
    setExpanded: (expanded) =>
      ipcRenderer.invoke("kepler:window:setExpanded", expanded),
  },
  search: {
    query: (text) => ipcRenderer.invoke("kepler:search:query", text),
  },
  objects: {
    listRecent: (limit) =>
      ipcRenderer.invoke("kepler:objects:listRecent", limit),
  },
  ark: {
    request: (operation, params) =>
      ipcRenderer.invoke("kepler:ark:request", operation, params),
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
    installDo: (sourcePath) =>
      ipcRenderer.invoke("kepler:extension:install:do", sourcePath),
    installedList: () =>
      ipcRenderer.invoke("kepler:extension:installed:list"),
    revert: (id, timestamp) =>
      ipcRenderer.invoke("kepler:extension:revert", id, timestamp),
    backupsList: (id) =>
      ipcRenderer.invoke("kepler:extension:backups:list", id),
    uninstall: (id) => ipcRenderer.invoke("kepler:extension:uninstall", id),
    catalogFetch: (force) =>
      ipcRenderer.invoke("kepler:extension:catalog:fetch", force),
    installFromUrl: (url, expectedSha256) =>
      ipcRenderer.invoke("kepler:extension:install:fromUrl", url, expectedSha256),
  },
  settings: {
    open: () => ipcRenderer.invoke("kepler:settings:open"),
    close: () => ipcRenderer.invoke("kepler:settings:close"),
    autostart: {
      get: () => ipcRenderer.invoke("kepler:settings:autostart:get"),
      set: (enabled) =>
        ipcRenderer.invoke("kepler:settings:autostart:set", enabled),
    },
    developerMode: {
      get: () => ipcRenderer.invoke("kepler:settings:developer-mode:get"),
      set: (enabled) =>
        ipcRenderer.invoke("kepler:settings:developer-mode:set", enabled),
    },
    version: () => ipcRenderer.invoke("kepler:settings:version"),
    hotkey: () => ipcRenderer.invoke("kepler:settings:hotkey"),
  },
};

contextBridge.exposeInMainWorld("kepler", api);
