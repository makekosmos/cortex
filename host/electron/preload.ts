import { contextBridge, ipcRenderer } from "electron";

const identityArg = process.argv.find((value) => value.startsWith("--kosmos-app="));
const arkAllowed = process.argv.includes("--kosmos-ark=1");
let identity: unknown;
try {
  identity = identityArg ? JSON.parse(identityArg.slice("--kosmos-app=".length)) : undefined;
} catch {
  identity = undefined;
}

const api: Record<string, unknown> = {
  identity,
  window: {
    minimize: () => ipcRenderer.send("host:window", "minimize"),
    close: () => ipcRenderer.send("host:window", "close"),
  },
  launcher: {
    request: (operation: string, params: Record<string, unknown> = {}) =>
      ipcRenderer.invoke("host:launcher-request", { operation, params }),
  },
};

if (arkAllowed) {
  api.ark = {
    request: (operation: string, params: Record<string, unknown> = {}) =>
      ipcRenderer.invoke("host:ark-request", { operation, params }),
    subscribe: (callback: (event: unknown) => void) => {
      const listener = (_event: Electron.IpcRendererEvent, value: unknown) => callback(value);
      ipcRenderer.on("host:ark-event", listener);
      ipcRenderer.send("host:ark-subscribe");
      return () => ipcRenderer.removeListener("host:ark-event", listener);
    },
  };
}

contextBridge.exposeInMainWorld("kosmosApp", api);
