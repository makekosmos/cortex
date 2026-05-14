// Shared preload для Vue extension'ов внутри Kepler shell.
//
// Exposes `window.kepler` namespace в extension renderer:
//   - kepler.ark.request(operation, params?)        — RPC к ARK
//   - kepler.ark.subscribe(event, handler) → off()  — события (commands_changed,...)
//   - kepler.window.{close,minimize,maximize}()     — управление окном
//   - kepler.meta.id()                              — id текущего extension'а
//   - kepler.host.invoke(action, payload?)          — host-level действия
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
};

export type KeplerExtensionApi = typeof api;

contextBridge.exposeInMainWorld("kepler", api);
