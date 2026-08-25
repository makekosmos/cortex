import { contextBridge, ipcRenderer } from "electron";

const identityArg = process.argv.find((value) => value.startsWith("--kosmos-app="));
const arkAllowed = process.argv.includes("--kosmos-ark=1");
type ApiValue =
  | string
  | number
  | boolean
  | null
  | readonly ApiValue[]
  | { readonly [key: string]: ApiValue };
type ApiParams = Readonly<{ [key: string]: ApiValue }>;
type ExposedApi = {
  identity: ApiValue | undefined;
  window: { minimize(): void; close(): void };
  launcher: { request(operation: string, params?: ApiParams): Promise<ApiValue> };
  ark?: {
    request(operation: string, params?: ApiParams): Promise<ApiValue>;
    subscribe(callback: (event: ApiValue) => void): () => void;
  };
};
let identity: ApiValue | undefined;
try {
  identity = identityArg ? JSON.parse(identityArg.slice("--kosmos-app=".length)) : undefined;
} catch {
  identity = undefined;
}

const api: ExposedApi = {
  identity,
  window: {
    minimize: () => ipcRenderer.send("host:window", "minimize"),
    close: () => ipcRenderer.send("host:window", "close"),
  },
  launcher: {
    request: (operation: string, params: ApiParams = {}) =>
      ipcRenderer.invoke("host:launcher-request", { operation, params }),
  },
};

if (arkAllowed) {
  api.ark = {
    request: (operation: string, params: ApiParams = {}) =>
      ipcRenderer.invoke("host:ark-request", { operation, params }),
    subscribe: (callback: (event: ApiValue) => void) => {
      const listener = (_event: Electron.IpcRendererEvent, value: ApiValue) => callback(value);
      ipcRenderer.on("host:ark-event", listener);
      ipcRenderer.send("host:ark-subscribe");
      return () => ipcRenderer.removeListener("host:ark-event", listener);
    },
  };
}

contextBridge.exposeInMainWorld("kosmosApp", api);
