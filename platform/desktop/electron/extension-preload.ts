// Shared preload для Vue extension'ов внутри Kepler shell.
//
// Exposes `window.kepler` namespace в extension renderer:
//   - kepler.ark.request(operation, params?)        — RPC к ARK
//   - kepler.ark.subscribe(event, handler) → off()  — события (commands_changed,...)
//   - kepler.window.{close,minimize,maximize}()     — управление окном
//   - kepler.meta.id()                              — id текущего extension'а
//   - kepler.host.invoke(action, payload?)          — host-level действия
//   - kepler.userData.{readJson,writeJson,readFile,writeFile,readBinary,writeBinary,deleteFile,path}()
//     — persistent user data extension'а в <APPDATA>/Kosmos/extensions-data/<id>/.
//       Эта папка не трогается install/uninstall'ом — settings и кеш переживают
//       реинсталл кода extension'а.
//
// Контракт ARK совпадает с публичным SidecarRequest API: вызывающая сторона
// формирует объект { operation: string, ...params } который main proxy
// прокидывает через ArkClient. Это даёт extension'у тот же набор операций,
// что и Kepler renderer'у — без дублирования типов в preload.

import { contextBridge, ipcRenderer } from "electron";

function platformMarker(): "mac" | "windows" | "linux" {
  if (process.platform === "darwin") return "mac";
  if (process.platform === "win32") return "windows";
  return "linux";
}

// Платформенный маркер на <html> для extension-окон (Eden/Delphi/...).
// Inline + DOMContentLoaded — см. preload.ts (chunk-split ломает sandbox,
// preload запускается до создания documentElement).
{
  const platform = platformMarker();
  const mark = () => {
    if (document.documentElement) document.documentElement.dataset.platform = platform;
  };
  mark();
  document.addEventListener("DOMContentLoaded", mark, { once: true });
}

type Unsubscribe = () => void;

const api = {
  ark: {
    request: <T = unknown>(operation: string, params?: Record<string, unknown>): Promise<T> =>
      ipcRenderer.invoke("kepler:extension:ark:request", operation, params) as Promise<T>,
    subscribe: (event: string, handler: (payload: unknown) => void): Unsubscribe => {
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
    id: (): Promise<string | null> => ipcRenderer.invoke("kepler:extension:meta:id"),
  },
  dialogs: {
    pickDirectory: (): Promise<string | null> =>
      ipcRenderer.invoke("kepler:extension:dialogs:pick-directory") as Promise<string | null>,
  },
  window: {
    close: (): Promise<void> => ipcRenderer.invoke("kepler:extension:window:close"),
    minimize: (): Promise<void> => ipcRenderer.invoke("kepler:extension:window:minimize"),
    maximize: (): Promise<void> => ipcRenderer.invoke("kepler:extension:window:maximize"),
    isMaximized: (): Promise<boolean> =>
      ipcRenderer.invoke("kepler:extension:window:is-maximized") as Promise<boolean>,
    zoomGet: (): Promise<number> =>
      ipcRenderer.invoke("kepler:extension:window:zoom-get") as Promise<number>,
    zoomSet: (factor: number): Promise<number> =>
      ipcRenderer.invoke("kepler:extension:window:zoom-set", factor) as Promise<number>,
    /** Toggle floating-widget mode: always-on-top + top-right corner.
        Повторный вызов возвращает окно в исходное положение. */
    toggleDockCorner: (): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:toggle-dock-corner"),
    /** Snapshot текущего docked-состояния (для initial hydrate). */
    isDocked: (): Promise<boolean> =>
      ipcRenderer.invoke("kepler:extension:window:is-docked") as Promise<boolean>,
    /** Подписка на изменение docked-состояния (broadcast при
        toggleDockCorner). Returns unsubscribe. */
    onDockedChange: (handler: (isDocked: boolean) => void): Unsubscribe => {
      const wrapped = (_e: unknown, value: unknown) => {
        if (typeof value === "boolean") handler(value);
      };
      ipcRenderer.on("kepler:extension:window:docked-changed", wrapped);
      return () => ipcRenderer.removeListener("kepler:extension:window:docked-changed", wrapped);
    },
    /** Включить/выключить maximize. На Windows `false` блокирует native
        double-click-on-titlebar-maximize — даёт нашему dblclick handler
        отработать без флика. */
    setMaximizable: (value: boolean): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:set-maximizable", value),
    setTitlebarSymbolColor: (symbolColor: string): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:set-titlebar-symbol-color", symbolColor),
    beginManualDrag: (point: { screenX: number; screenY: number }): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:begin-manual-drag", point),
    moveManualDrag: (point: { screenX: number; screenY: number }): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:move-manual-drag", point),
    endManualDrag: (): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:end-manual-drag"),
    setTitlebarHoverTracking: (enabled: boolean, height: number): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:window:set-titlebar-hover-tracking", enabled, height),
    onTitlebarHoverChange: (handler: (hovered: boolean) => void): Unsubscribe => {
      const wrapped = (_e: unknown, value: unknown) => {
        if (typeof value === "boolean") handler(value);
      };
      ipcRenderer.on("kepler:extension:window:titlebar-hover-changed", wrapped);
      return () =>
        ipcRenderer.removeListener("kepler:extension:window:titlebar-hover-changed", wrapped);
    },
    onMaximizedChange: (handler: (isMaximized: boolean) => void): Unsubscribe => {
      const wrapped = (_e: unknown, value: unknown) => {
        if (typeof value === "boolean") handler(value);
      };
      ipcRenderer.on("kepler:extension:window:maximized-changed", wrapped);
      return () => ipcRenderer.removeListener("kepler:extension:window:maximized-changed", wrapped);
    },
  },
  host: {
    invoke: (action: string, payload?: unknown): Promise<boolean> =>
      ipcRenderer.invoke("kepler:extension:invoke-host", action, payload),
  },
  // Eden settings window controls. Живут на shared extension preload (а не
  // только на главном preload.ts), потому что Eden рендерится в extension-окне
  // с этим preload'ом — без этого `window.kepler.edenSettings` undefined и
  // кнопка настроек / закрытие настроек молча не работают.
  edenSettings: {
    open: (): Promise<void> => ipcRenderer.invoke("kepler:eden-settings:open") as Promise<void>,
    close: (): Promise<void> => ipcRenderer.invoke("kepler:eden-settings:close") as Promise<void>,
  },
  markdownFiles: {
    open: (): Promise<{ path: string; name: string; content: string } | null> =>
      ipcRenderer.invoke("kepler:extension:markdownFiles:open") as Promise<{
        path: string;
        name: string;
        content: string;
      } | null>,
    save: (suggestedName: string, content: string): Promise<{ path: string } | null> =>
      ipcRenderer.invoke("kepler:extension:markdownFiles:save", suggestedName, content) as Promise<{
        path: string;
      } | null>,
    openVault: (): Promise<unknown | null> =>
      ipcRenderer.invoke("kepler:extension:markdownFiles:openVault") as Promise<unknown | null>,
    exportVault: (
      files: Array<
        | { relativePath: string; content: string; sourcePath?: never }
        | { relativePath: string; sourcePath: string; content?: never }
      >,
    ): Promise<{ outputDir: string; exportedCount: number } | null> =>
      ipcRenderer.invoke("kepler:extension:markdownFiles:exportVault", files) as Promise<{
        outputDir: string;
        exportedCount: number;
      } | null>,
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
  arrancador: {
    // Arrancador-specific operations. Тонкие обёртки над ark.request — backend
    // регистрирует операции под namespace `arrancador.*`. Если backend ещё не
    // реализован (Phase A/B/C subagent'ы), вызовы вернут «Unknown operation»
    // — это OK для UI smoke до merge.
    scan: <T = unknown>(): Promise<T> =>
      ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.scan", {}) as Promise<T>,
    addManual: <T = unknown>(params: {
      name: string;
      exePath: string;
      savePaths?: string[];
    }): Promise<T> =>
      ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.add_manual", {
        name: params.name,
        exe_path: params.exePath,
        save_paths: params.savePaths ?? [],
      }) as Promise<T>,
    launch: <T = unknown>(gameId: string): Promise<T> =>
      ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.launch", {
        game_id: gameId,
      }) as Promise<T>,
    rawg: {
      search: <T = unknown>(query: string): Promise<T> =>
        ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.rawg.search", {
          query,
        }) as Promise<T>,
      apply: <T = unknown>(gameId: string, rawgId: number): Promise<T> =>
        ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.rawg.apply", {
          game_id: gameId,
          rawg_id: rawgId,
        }) as Promise<T>,
    },
    sqoba: {
      backup: <T = unknown>(gameId: string): Promise<T> =>
        ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.sqoba.backup", {
          game_id: gameId,
        }) as Promise<T>,
      list: <T = unknown>(gameId: string): Promise<T> =>
        ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.sqoba.list", {
          game_id: gameId,
        }) as Promise<T>,
      restore: <T = unknown>(gameId: string, backupId: string): Promise<T> =>
        ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.sqoba.restore", {
          game_id: gameId,
          backup_id: backupId,
        }) as Promise<T>,
    },
    config: {
      getRawgKey: <T = unknown>(): Promise<T> =>
        ipcRenderer.invoke(
          "kepler:extension:ark:request",
          "arrancador.config.get_rawg_key",
          {},
        ) as Promise<T>,
      setRawgKey: <T = unknown>(key: string): Promise<T> =>
        ipcRenderer.invoke("kepler:extension:ark:request", "arrancador.config.set_rawg_key", {
          key,
        }) as Promise<T>,
    },
  },
  focusWidget: {
    /** Push partial state update to host. Active=true → widget show, false → hide.
        Шлите `{active:true, remainingSec, label, mode}` каждую секунду пока
        pomodoro идёт, `{active:false}` при stop. */
    setState: (patch: {
      active?: boolean;
      remainingSec?: number;
      totalSec?: number;
      label?: string;
      mode?: "work" | "break" | "stopwatch";
      blockingActive?: boolean;
      /** Wallclock (Unix ms) когда фаза закончится; main process использует
          для автономного тика когда renderer throttle'ится. null = pause/idle. */
      phaseEndsAtMs?: number | null;
    }): Promise<void> =>
      ipcRenderer.invoke("kepler:focus-widget:set-state", patch) as Promise<void>,
  },
  backend: {
    onReady: (listener: () => void): Unsubscribe => {
      const handler = () => listener();
      ipcRenderer.on("kepler:backend:ready", handler);
      return () => ipcRenderer.removeListener("kepler:backend:ready", handler);
    },
    onDisconnected: (listener: () => void): Unsubscribe => {
      const handler = () => listener();
      ipcRenderer.on("kepler:backend:disconnected", handler);
      return () => ipcRenderer.removeListener("kepler:backend:disconnected", handler);
    },
  },
  userData: {
    readJson: <T = unknown>(name: string): Promise<T | null> =>
      ipcRenderer.invoke("kepler:extension:userData:readJson", name) as Promise<T | null>,
    writeJson: <T = unknown>(name: string, value: T): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:userData:writeJson", name, value) as Promise<void>,
    readFile: (name: string): Promise<string | null> =>
      ipcRenderer.invoke("kepler:extension:userData:readFile", name) as Promise<string | null>,
    writeFile: (name: string, content: string): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:userData:writeFile", name, content) as Promise<void>,
    readBinary: (name: string): Promise<string | null> =>
      ipcRenderer.invoke("kepler:extension:userData:readBinary", name) as Promise<string | null>,
    writeBinary: (name: string, base64: string): Promise<void> =>
      ipcRenderer.invoke("kepler:extension:userData:writeBinary", name, base64) as Promise<void>,
    deleteFile: (name: string): Promise<boolean> =>
      ipcRenderer.invoke("kepler:extension:userData:deleteFile", name) as Promise<boolean>,
    path: (): Promise<string> =>
      ipcRenderer.invoke("kepler:extension:userData:path") as Promise<string>,
  },
};

export type KeplerExtensionApi = typeof api;

// Test rig — exposed только в test mode. Mirror platform/desktop/electron/preload.ts.
if (process.env.KOSMOS_TEST_MODE === "1") {
  (
    api as KeplerExtensionApi & {
      __test?: {
        waitForReady(timeoutMs?: number): Promise<void>;
        getStats(): Promise<{
          arkConnected: boolean;
          commands: string[];
          commandsRegistered: number;
        }>;
      };
    }
  ).__test = {
    waitForReady: (timeoutMs?: number) =>
      ipcRenderer.invoke("kepler:__test:waitForReady", timeoutMs) as Promise<void>,
    getStats: () =>
      ipcRenderer.invoke("kepler:__test:getStats") as Promise<{
        arkConnected: boolean;
        commands: string[];
        commandsRegistered: number;
      }>,
  };
}

contextBridge.exposeInMainWorld("kepler", api);
