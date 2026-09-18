import { contextBridge, ipcRenderer } from "electron";
import type { OpenAppRequest, OpenAppRequestResult } from "./app-navigation";
import type {
  UserDataDeleteResult,
  UserDataReadResult,
  UserDataStatResult,
  UserDataWriteResult,
} from "./extension-user-data-ipc";

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
export type AuxWindowRequest = Readonly<{
  key: string;
  route?: string;
  width?: number;
  height?: number;
  minWidth?: number;
  minHeight?: number;
  alwaysOnTop?: boolean;
}>;
export type AuxWindowOpenResult = { ok: true } | { ok: false; message: string };

type ExposedApi = {
  identity: ApiValue | undefined;
  window: {
    minimize(): void;
    close(): void;
    open(request: AuxWindowRequest): Promise<AuxWindowOpenResult>;
    setAlwaysOnTop(flag: boolean): Promise<boolean>;
    isAlwaysOnTop(): Promise<boolean>;
  };
  dialogs: {
    pickDirectoryGrant(): Promise<{
      persistentGrantId: string;
      label: string;
    } | null>;
  };
  userData: {
    read(key: string): Promise<UserDataReadResult>;
    write(key: string, bytes: Uint8Array): Promise<UserDataWriteResult>;
    delete(key: string): Promise<UserDataDeleteResult>;
    stat(key: string): Promise<UserDataStatResult>;
    readJson<T = unknown>(name: string): Promise<T | null>;
    writeJson<T = unknown>(name: string, value: T): Promise<void>;
    deleteFile(name: string): Promise<boolean>;
    path(): Promise<string>;
  };
  launcher: {
    request(operation: string, params?: ApiParams): Promise<ApiValue>;
  };
  apps: {
    open(request: OpenAppRequest): Promise<OpenAppRequestResult>;
  };
  navigation: {
    onNavigate(handler: (route: string) => void): () => void;
  };
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
    // SAFETY: main validates the request shape, binds the window to the
    // sender's app id and launch, and caps secondary window count.
    open: (request: AuxWindowRequest) =>
      ipcRenderer.invoke("host:window:open", request) as Promise<AuxWindowOpenResult>,
    // SAFETY: main returns the actual always-on-top flag after applying it.
    setAlwaysOnTop: (flag: boolean) =>
      ipcRenderer.invoke("host:window:always-on-top", flag) as Promise<boolean>,
    // SAFETY: main returns the window's current always-on-top flag.
    isAlwaysOnTop: () => ipcRenderer.invoke("host:window:always-on-top") as Promise<boolean>,
  },
  dialogs: {
    // SAFETY: main owns this IPC handler and validates the exact opaque response shape.
    pickDirectoryGrant: () =>
      ipcRenderer.invoke("host:dialogs:pick-directory-grant") as Promise<{
        persistentGrantId: string;
        label: string;
      } | null>,
  },
  userData: {
    read: (key) => ipcRenderer.invoke("host:user-data:binary", { operation: "read", key }),
    write: (key, bytes) =>
      ipcRenderer.invoke("host:user-data:binary", { operation: "write", key, bytes }),
    delete: (key) => ipcRenderer.invoke("host:user-data:binary", { operation: "delete", key }),
    stat: (key) => ipcRenderer.invoke("host:user-data:binary", { operation: "stat", key }),
    readJson: (name) => ipcRenderer.invoke("host:user-data", { operation: "readJson", name }),
    writeJson: (name, value) =>
      ipcRenderer.invoke("host:user-data", {
        operation: "writeJson",
        name,
        value,
      }),
    deleteFile: (name) => ipcRenderer.invoke("host:user-data", { operation: "deleteFile", name }),
    path: () => ipcRenderer.invoke("host:user-data", { operation: "path" }),
  },
  launcher: {
    request: (operation: string, params: ApiParams = {}) =>
      ipcRenderer.invoke("host:launcher-request", { operation, params }),
  },
  apps: {
    open: (request: OpenAppRequest) =>
      // SAFETY: the main process validates the renderer request and result shape.
      ipcRenderer.invoke("host:app-open", request) as Promise<OpenAppRequestResult>,
  },
  navigation: {
    onNavigate: (handler: (route: string) => void) => {
      const listener = (_event: Electron.IpcRendererEvent, route: string) => handler(route);
      ipcRenderer.on("kepler:extension:navigation", listener);
      return () => ipcRenderer.removeListener("kepler:extension:navigation", listener);
    },
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
contextBridge.exposeInMainWorld("kepler", api);
