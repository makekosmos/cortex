import { ipcMain, shell } from "electron";
import type { ArkClient } from "@kosmos/ark";
import { isString } from "../src/shared/runtimeGuards";

interface MainShellIpcOptions {
  awaitArkReady(timeoutMs?: number): Promise<ArkClient>;
  getArkClient(): ArkClient | null;
}

// `shell.openExternal` hands the URL to the OS shell — on Windows `file:` /
// UNC paths execute the target. Only browser-safe schemes may leave the app.
const EXTERNAL_URL_SCHEMES = new Set(["https:", "http:", "mailto:"]);

function safeParseUrl(url: string): URL | null {
  try {
    return new URL(url);
  } catch {
    return null;
  }
}

export function registerMainShellIpc(options: MainShellIpcOptions): void {
  const { awaitArkReady, getArkClient } = options;

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

  ipcMain.handle("kepler:shell:openExternal", (_e, url: string) => {
    const parsed = isString(url) ? safeParseUrl(url) : null;
    if (!parsed || !EXTERNAL_URL_SCHEMES.has(parsed.protocol)) {
      throw new Error("kepler:shell:openExternal: URL scheme is not allowed");
    }
    return shell.openExternal(url);
  });
}

function isPositiveNumber(value: number | undefined): value is number {
  return typeof value === "number" && value > 0;
}
