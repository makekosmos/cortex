import { ipcRenderer } from "electron";
import type { KeplerApi } from "../shared/ipc-types";

type KeplerSettingsBridge = Pick<
  KeplerApi,
  "settings" | "edenSettings" | "focusOverlay" | "crashes" | "diagnostics" | "postUpdate"
>;

export function createKeplerSettingsBridge(): KeplerSettingsBridge {
  return {
    settings: {
      open: () => ipcRenderer.invoke("kepler:settings:open"),
      close: () => ipcRenderer.invoke("kepler:settings:close"),
      autostart: {
        get: () => ipcRenderer.invoke("kepler:settings:autostart:get"),
        allowed: () => ipcRenderer.invoke("kepler:settings:autostart:allowed"),
        set: (enabled) => ipcRenderer.invoke("kepler:settings:autostart:set", enabled),
      },
      trayIcon: {
        get: () => ipcRenderer.invoke("kepler:settings:tray-icon:get"),
        set: (enabled) => ipcRenderer.invoke("kepler:settings:tray-icon:set", enabled),
      },
      sync: {
        snapshot: () => ipcRenderer.invoke("kepler:settings:sync:snapshot"),
        getPairingCode: () => ipcRenderer.invoke("kepler:settings:sync:get-pairing-code"),
        disconnectPeer: (deviceId) =>
          ipcRenderer.invoke("kepler:settings:sync:disconnect", deviceId),
        connectWithPairingCode: (code) =>
          ipcRenderer.invoke("kepler:settings:sync:connect-with-pairing-code", code),
        copyPairingCode: (code) =>
          ipcRenderer.invoke("kepler:settings:sync:copy-pairing-code", code),
        onUpdated: (listener) => {
          const handler = () => listener();
          ipcRenderer.on("kepler:settings:sync:updated", handler);
          return () => ipcRenderer.removeListener("kepler:settings:sync:updated", handler);
        },
      },
      developerMode: {
        get: () => ipcRenderer.invoke("kepler:settings:developer-mode:get"),
        set: (enabled) => ipcRenderer.invoke("kepler:settings:developer-mode:set", enabled),
      },
      usageTracker: {
        get: () => ipcRenderer.invoke("kepler:settings:usage-tracker:get"),
        set: (enabled) => ipcRenderer.invoke("kepler:settings:usage-tracker:set", enabled),
      },
      launcherStateTtl: {
        get: () => ipcRenderer.invoke("kepler:settings:launcher-state-ttl:get"),
        set: (minutes: number) =>
          ipcRenderer.invoke("kepler:settings:launcher-state-ttl:set", minutes),
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
          const handler = (_e: unknown, state: unknown) => listener(state as never);
          ipcRenderer.on("kepler:settings:update:state", handler);
          return () => ipcRenderer.removeListener("kepler:settings:update:state", handler);
        },
      },
    },
    edenSettings: {
      open: () => ipcRenderer.invoke("kepler:eden-settings:open"),
      close: () => ipcRenderer.invoke("kepler:eden-settings:close"),
    },
    focusOverlay: {
      ready: () => ipcRenderer.send("kepler:focus-overlay:ready"),
      onShow: (listener) => {
        const handler = (_e: Electron.IpcRendererEvent, feedback: unknown) =>
          listener(feedback as Parameters<typeof listener>[0]);
        ipcRenderer.on("kepler:focus-overlay:show", handler);
        return () => ipcRenderer.removeListener("kepler:focus-overlay:show", handler);
      },
      setInteractive: (interactive: boolean) =>
        ipcRenderer.invoke("kepler:focus-overlay:set-interactive", interactive),
      showBlocked: (app) => ipcRenderer.invoke("kepler:focus-overlay:show-blocked", app),
      done: () => ipcRenderer.send("kepler:focus-overlay:done"),
    },
    crashes: {
      list: () => ipcRenderer.invoke("kepler:crashes:list"),
      openFolder: () => ipcRenderer.invoke("kepler:crashes:openFolder"),
      clear: () => ipcRenderer.invoke("kepler:crashes:clear"),
    },
    diagnostics: {
      bundle: () => ipcRenderer.invoke("kepler:diagnostics:bundle"),
      bundleSave: () => ipcRenderer.invoke("kepler:diagnostics:bundle-save"),
      openLogsFolder: () => ipcRenderer.invoke("kepler:diagnostics:open-logs-folder"),
      metrics: () => ipcRenderer.invoke("kepler:diagnostics:metrics"),
      traceStart: () => ipcRenderer.invoke("kepler:diagnostics:trace-start"),
      traceStop: (outPath?: string) => ipcRenderer.invoke("kepler:diagnostics:trace-stop", outPath),
      windowMoveBenchmark: (input) =>
        ipcRenderer.invoke("kepler:diagnostics:window-move-benchmark", input),
    },
    postUpdate: {
      onShown: (listener) => {
        const handler = (_e: unknown, payload: unknown) => listener(payload as { version: string });
        ipcRenderer.on("kepler:post-update", handler);
        return () => ipcRenderer.removeListener("kepler:post-update", handler);
      },
    },
  };
}
