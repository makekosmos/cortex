import { ipcMain, shell } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { existsSync, readFileSync } from "node:fs";
import type { BackendStatus } from "../shared/ipc-types";

interface MainShellIpcOptions {
  getBackendLockPath(): string;
  isBackendRunning(): boolean;
  awaitArkReady(timeoutMs?: number): Promise<ArkClient>;
  getArkClient(): ArkClient | null;
  hideLauncher(): void;
  setLauncherExpanded(expanded: boolean): void;
}

function readBackendStatus(
  options: Pick<MainShellIpcOptions, "getBackendLockPath" | "isBackendRunning">,
): BackendStatus {
  const backendLockPath = options.getBackendLockPath();
  if (!backendLockPath || !existsSync(backendLockPath)) {
    return { running: false, lockFilePath: backendLockPath };
  }
  try {
    const lock = JSON.parse(readFileSync(backendLockPath, "utf8")) as {
      pid: number;
      ws_port: number;
    };
    return {
      running: options.isBackendRunning(),
      pid: lock.pid,
      wsPort: lock.ws_port,
      lockFilePath: backendLockPath,
    };
  } catch {
    return { running: false, lockFilePath: backendLockPath };
  }
}

export function registerMainShellIpc(options: MainShellIpcOptions): void {
  const { awaitArkReady, getArkClient, hideLauncher, setLauncherExpanded } = options;

  ipcMain.handle("kepler:backend:status", () => readBackendStatus(options));

  if (process.env.KOSMOS_TEST_MODE === "1") {
    ipcMain.handle("kepler:__test:waitForReady", async (_e, timeoutMs?: number): Promise<void> => {
      const ms = typeof timeoutMs === "number" && timeoutMs > 0 ? timeoutMs : 15000;
      await awaitArkReady(ms);
    });
    ipcMain.handle("kepler:__test:getStats", async () => {
      let commands: string[] = [];
      const arkClient = getArkClient();
      if (arkClient) {
        try {
          const list = await arkClient.commands.list();
          if (Array.isArray(list)) commands = list.map((c) => c.id);
        } catch {
          // Backend can be reconnecting; tests only need a stable empty fallback.
        }
      }
      return {
        arkConnected: arkClient !== null,
        commands,
        commandsRegistered: commands.length,
      };
    });
  }

  ipcMain.handle("kepler:shell:openExternal", (_e, url: string) => shell.openExternal(url));
  ipcMain.handle("kepler:window:hide", () => hideLauncher());
  ipcMain.handle("kepler:window:setExpanded", (_e, expanded: boolean) =>
    setLauncherExpanded(!!expanded),
  );
}
