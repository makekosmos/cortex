// Shared preload для Vue extension'ов внутри Kepler shell.
//
// Exposes `window.kepler` namespace в extension renderer:
//   - kepler.ark.request(operation, params?)        — RPC к ARK
//   - kepler.ark.subscribe(event, handler) → off()  — события (commands_changed,...)
//   - kepler.window.{close,minimize,maximize}()     — управление окном
//   - kepler.meta.id()                              — id текущего extension'а
//   - kepler.host.invoke(action, payload?)          — host-level действия
//   - kepler.userData.{readJson,writeJson,readFile,writeFile,path}()
//     — persistent user data extension'а в <APPDATA>/Kosmos/extensions-data/<id>/.
//       Эта папка не трогается install/uninstall'ом — settings и кеш переживают
//       реинсталл кода extension'а.
//
// Контракт ARK совпадает с публичным SidecarRequest API: вызывающая сторона
// формирует объект { operation: string, ...params } который main proxy
// прокидывает через ArkClient. Это даёт extension'у тот же набор операций,
// что и Kepler renderer'у — без дублирования типов в preload.

import { contextBridge, ipcRenderer } from "electron";

type Unsubscribe = () => void;

const api = {
  ark: {
    request: <T = unknown>(
      operation: string,
      params?: Record<string, unknown>,
    ): Promise<T> =>
      ipcRenderer.invoke(
        "kepler:extension:ark:request",
        operation,
        params,
      ) as Promise<T>,
    subscribe: (
      event: string,
      handler: (payload: unknown) => void,
    ): Unsubscribe => {
      const channel = `kepler:extension:ark:event:${event}`;
      const wrapped = (_e: unknown, payload: unknown) => handler(payload);
      ipcRenderer.on(channel, wrapped);
      void ipcRenderer.invoke("kepler:extension:ark:subscribe", event);
      return () => {
        ipcRenderer.removeListener(channel, wrapped);
        void ipcRenderer.invoke("kepler:extension:ark:unsubscribe", event);
      };
    },
  },
  meta: {
    id: (): Promise<string | null> =>
      ipcRenderer.invoke("kepler:extension:meta:id"),
  },
  window: {
    close: (): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:close"),
    minimize: (): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:minimize"),
    maximize: (): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:maximize"),
  },
  host: {
    invoke: (action: string, payload?: unknown): Promise<boolean> =>
      ipcRenderer.invoke("kepler:extension:invoke-host", action, payload),
  },
  navigation: {
    /** Subscribe to navigation events (`router.push(route)`). Initial route
        приходит сразу после `did-finish-load`, для уже открытого окна — при
        повторном `openExtension(id, route)` из лаунчера. */
    onNavigate: (handler: (route: string) => void): Unsubscribe => {
      const wrapped = (_e: unknown, route: unknown) => {
        if (typeof route === "string") handler(route);
      };
      ipcRenderer.on("kepler:extension:navigation", wrapped);
      return () => ipcRenderer.removeListener("kepler:extension:navigation", wrapped);
    },
    /** Synchronous read of initial route stashed by host before page load.
        Returns null если route не задан (обычное открытие). */
    initialRoute: (): Promise<string | null> =>
      ipcRenderer.invoke("kepler:extension:navigation:initial") as Promise<string | null>,
  },
  userData: {
    readJson: <T = unknown>(name: string): Promise<T | null> =>
      ipcRenderer.invoke("kepler:extension:userData:readJson", name) as Promise<
        T | null
      >,
    writeJson: <T = unknown>(name: string, value: T): Promise<void> =>
      ipcRenderer.invoke(
        "kepler:extension:userData:writeJson",
        name,
        value,
      ) as Promise<void>,
    readFile: (name: string): Promise<string | null> =>
      ipcRenderer.invoke("kepler:extension:userData:readFile", name) as Promise<
        string | null
      >,
    writeFile: (name: string, content: string): Promise<void> =>
      ipcRenderer.invoke(
        "kepler:extension:userData:writeFile",
        name,
        content,
      ) as Promise<void>,
    path: (): Promise<string> =>
      ipcRenderer.invoke("kepler:extension:userData:path") as Promise<string>,
  },
};

export type KeplerExtensionApi = typeof api;

contextBridge.exposeInMainWorld("kepler", api);
