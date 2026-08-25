import { ipcMain, shell } from "electron";
import type { ArkClient } from "@kosmos/ark";

interface MainShellIpcOptions {
  awaitArkReady(timeoutMs?: number): Promise<ArkClient>;
  getArkClient(): ArkClient | null;
  hideLauncher(): void;
  setLauncherExpanded(expanded: boolean): void;
}

export function registerMainShellIpc(options: MainShellIpcOptions): void {
  const { awaitArkReady, getArkClient, hideLauncher, setLauncherExpanded } = options;

  if (process.env.KOSMOS_TEST_MODE === "1") {
    ipcMain.handle("kepler:__test:waitForReady", async (_e, timeoutMs?: number): Promise<void> => {
      const ms = isPositiveNumber(timeoutMs) ? timeoutMs : 15000;
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

function isPositiveNumber(value: number | undefined): value is number {
  return typeof value === "number" && value > 0;
}
