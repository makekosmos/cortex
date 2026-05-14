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
};

contextBridge.exposeInMainWorld("kepler", api);
