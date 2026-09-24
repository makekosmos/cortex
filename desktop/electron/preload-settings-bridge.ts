import { ipcRenderer } from "electron";
import type { KeplerApi } from "../shared/ipc-types";
import type { FocusOverlayFeedback } from "../shared/ipc-api-shell-services";
import type { UpdateState } from "../shared/ipc-types";

type KeplerSettingsBridge = Pick<KeplerApi, "settings" | "focusOverlay" | "diagnostics">;

export function createKeplerSettingsBridge(): KeplerSettingsBridge {
  return {
    settings: {
      open: () => ipcRenderer.invoke("kepler:settings:open"),
      close: () => ipcRenderer.invoke("kepler:settings:close"),
      trayIcon: {
        get: () => ipcRenderer.invoke("kepler:settings:tray-icon:get"),
        set: (enabled) => ipcRenderer.invoke("kepler:settings:tray-icon:set", enabled),
      },
      version: () => ipcRenderer.invoke("kepler:settings:version"),
      storageSummary: () => ipcRenderer.invoke("kepler:settings:storage-summary"),
      hotkey: () => ipcRenderer.invoke("kepler:settings:hotkey"),
      hotkeySet: (value) => ipcRenderer.invoke("kepler:settings:hotkey:set", value),
      hotkeyReset: () => ipcRenderer.invoke("kepler:settings:hotkey:reset"),
      update: {
        check: () => ipcRenderer.invoke("kepler:settings:update:check"),
        install: () => ipcRenderer.invoke("kepler:settings:update:install"),
        state: () => ipcRenderer.invoke("kepler:settings:update:state"),
        onStateChanged: (listener) => {
          const handler = (_e: Electron.IpcRendererEvent, state: UpdateState) => listener(state);
          ipcRenderer.on("kepler:settings:update:state", handler);
          return () => ipcRenderer.removeListener("kepler:settings:update:state", handler);
        },
      },
    },
    focusOverlay: {
      ready: () => ipcRenderer.send("kepler:focus-overlay:ready"),
      onShow: (listener) => {
        const handler = (_e: Electron.IpcRendererEvent, feedback: FocusOverlayFeedback) =>
          listener(feedback);
        ipcRenderer.on("kepler:focus-overlay:show", handler);
        return () => ipcRenderer.removeListener("kepler:focus-overlay:show", handler);
      },
      setInteractive: (interactive: boolean) =>
        ipcRenderer.invoke("kepler:focus-overlay:set-interactive", interactive),
      done: () => ipcRenderer.send("kepler:focus-overlay:done"),
    },
    diagnostics: {
      bundle: () => ipcRenderer.invoke("kepler:diagnostics:bundle"),
      metrics: () => ipcRenderer.invoke("kepler:diagnostics:metrics"),
      traceStart: () => ipcRenderer.invoke("kepler:diagnostics:trace-start"),
      traceStop: (outPath?: string) => ipcRenderer.invoke("kepler:diagnostics:trace-stop", outPath),
      windowMoveBenchmark: (input) =>
        ipcRenderer.invoke("kepler:diagnostics:window-move-benchmark", input),
    },
  };
}
