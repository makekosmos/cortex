import { BrowserWindow, clipboard, ipcMain } from "electron";
import type { ArkClient } from "@kosmos/ark";
import {
  check as checkForUpdates,
  getState as getUpdateState,
  install as installUpdate,
} from "./autoupdater-host";
import { getServiceStatus, pingService, runServiceCliElevated } from "./focus-service";
import {
  isFocusServiceAutoInstallDeclined,
  setFocusServiceAutoInstallDeclined,
} from "./settings-window";

interface MainDataIpcSettingsOptions {
  getArkClient(): ArkClient | null;
  broadcastSettingsSyncUpdated(): void;
}

export function registerMainDataSettingsIpc(options: MainDataIpcSettingsOptions): void {
  const { broadcastSettingsSyncUpdated, getArkClient } = options;

  ipcMain.handle("kepler:settings:update:check", () => checkForUpdates());
  ipcMain.handle("kepler:settings:update:install", () => {
    installUpdate();
    return true;
  });
  ipcMain.handle("kepler:settings:update:state", () => getUpdateState());

  ipcMain.handle("kepler:settings:sync:snapshot", async () => {
    let raw: unknown = null;
    const arkClient = getArkClient();
    if (arkClient) {
      try {
        raw = await arkClient.invokeOperation<unknown>({
          operation: "get_sync_snapshot",
        });
      } catch {
        // Backend не поддерживает операцию (старая версия / не-Windows платформа).
      }
    }
    return normalizeSyncSnapshot(raw);
  });

  ipcMain.handle("kepler:settings:sync:get-pairing-code", async () => {
    const arkClient = getArkClient();
    if (!arkClient) return null;
    try {
      const code = await arkClient.getOwnIrohTicket();
      return typeof code === "string" && code.length > 0 ? code : null;
    } catch {
      return null;
    }
  });

  ipcMain.handle("kepler:settings:sync:disconnect", async (_e, deviceId: string) => {
    if (typeof deviceId !== "string" || deviceId.trim().length === 0) {
      throw new Error("deviceId must be a non-empty string");
    }
    const arkClient = getArkClient();
    if (arkClient) {
      await arkClient.invokeOperation({
        operation: "disconnect_peer",
        device_id: deviceId.trim(),
      });
    }
    broadcastSettingsSyncUpdated();
  });

  ipcMain.handle("kepler:settings:sync:connect-with-pairing-code", async (_e, code: string) => {
    if (typeof code !== "string" || code.trim().length < 8) {
      throw new Error("Некорректный код синхронизации");
    }
    const arkClient = getArkClient();
    if (!arkClient) throw new Error("ARK unavailable");
    await arkClient.invokeOperation({
      operation: "connect_with_pairing_code",
      pairing_code: code.trim(),
    });
    broadcastSettingsSyncUpdated();
  });

  ipcMain.handle("kepler:settings:sync:copy-pairing-code", async (_e, code: string) => {
    if (typeof code === "string" && code.trim()) clipboard.writeText(code.trim());
  });

  ipcMain.handle("kepler:focus-service:status", () => getServiceStatus());
  ipcMain.handle("kepler:focus-service:ping", () => pingService());
  ipcMain.handle("kepler:focus-service:install", async () => {
    const result = await runServiceCliElevated("install");
    if (result.ok) {
      setFocusServiceAutoInstallDeclined(false);
      broadcastFocusServiceStatusChanged();
    }
    return result;
  });
  ipcMain.handle("kepler:focus-service:uninstall", async () => {
    const result = await runServiceCliElevated("uninstall");
    if (result.ok) broadcastFocusServiceStatusChanged();
    return result;
  });
  ipcMain.handle("kepler:focus-service:start", () => runServiceCliElevated("start"));
  ipcMain.handle("kepler:focus-service:stop", () => runServiceCliElevated("stop"));
  ipcMain.handle("kepler:focus-service:auto-install-declined:get", () =>
    isFocusServiceAutoInstallDeclined(),
  );
  ipcMain.handle("kepler:focus-service:auto-install-declined:set", (_e, value: boolean) => {
    setFocusServiceAutoInstallDeclined(!!value);
  });
}

function normalizeSyncSnapshot(raw: unknown) {
  const obj = raw && typeof raw === "object" ? (raw as Record<string, unknown>) : {};
  const peersRaw = Array.isArray(obj.peers) ? obj.peers : [];
  const peers = peersRaw.map((peer) => {
    const p = peer && typeof peer === "object" ? (peer as Record<string, unknown>) : {};
    return {
      deviceId: String(p.deviceId ?? p.device_id ?? ""),
      deviceName: String(p.deviceName ?? p.device_name ?? "Неизвестное устройство"),
      lastSeen:
        typeof p.lastSeen === "string"
          ? p.lastSeen
          : typeof p.last_seen === "string"
            ? p.last_seen
            : null,
      status: p.status === "online" ? "online" : "offline",
      deviceKind: "unknown",
    };
  });
  const ldRaw = obj.localDevice ?? obj.local_device;
  const localDevice =
    ldRaw && typeof ldRaw === "object"
      ? {
          deviceId: String(
            (ldRaw as Record<string, unknown>).deviceId ??
              (ldRaw as Record<string, unknown>).device_id ??
              "",
          ),
          deviceName: String(
            (ldRaw as Record<string, unknown>).deviceName ??
              (ldRaw as Record<string, unknown>).device_name ??
              "",
          ),
        }
      : null;
  return {
    running: obj.running === true,
    transport:
      obj.transport === "iroh" || obj.transport === "relay" || obj.transport === "lan"
        ? obj.transport
        : "unknown",
    pairingAvailable: obj.pairingAvailable === true || obj.pairing_available === true,
    ownPairingCodeAvailable:
      obj.ownPairingCodeAvailable === true || obj.own_pairing_code_available === true,
    peers,
    localDevice,
  };
}

function broadcastFocusServiceStatusChanged(): void {
  for (const win of BrowserWindow.getAllWindows()) {
    if (!win.isDestroyed()) {
      try {
        win.webContents.send("kepler:focus-service:status-changed");
      } catch {
        /* ignore */
      }
    }
  }
}
