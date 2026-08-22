import { setTimeout as delay } from "node:timers/promises";
import { type IpcMainInvokeEvent } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { CommandRecord } from "../shared/ipc-types";
import { safeHandle } from "./ipc-safe";
import { COMMANDS, findCommand } from "./commands";
import { keplerLog } from "./logging";

interface MainCommandsOptions {
  getArkClient: () => ArkClient | null;
  hideLauncher: () => void;
}

export interface MainCommandsController {
  invokeCommandById(id: string, event?: IpcMainInvokeEvent): Promise<void>;
  broadcastCommandsUpdated(): void;
}

export function registerMainCommands(options: MainCommandsOptions): MainCommandsController {
  const { getArkClient, hideLauncher } = options;

  function broadcastCommandsUpdated(): void {
    void import("electron").then(({ BrowserWindow }) => {
      for (const win of BrowserWindow.getAllWindows()) {
        if (!win.isDestroyed()) win.webContents.send("kepler:commands:updated");
      }
    });
  }

  async function staticCommands(): Promise<CommandRecord[]> {
    const records: CommandRecord[] = [];
    for (const c of COMMANDS) {
      const shortcut =
        typeof c.shortcut === "function"
          ? await Promise.resolve(c.shortcut()).catch(() => undefined)
          : c.shortcut;
      records.push({
        id: c.id,
        title: c.title,
        subtitle: c.subtitle,
        category: c.category,
        kind: c.kind,
        appName: c.appName,
        icon: c.icon?.(),
        shortcut,
      });
    }
    return records;
  }

  function mergeDynamicCommands(byId: Map<string, CommandRecord>, dynamic: unknown): void {
    if (!Array.isArray(dynamic)) {
      keplerLog.warn("commands", "commands.list returned non-array");
      return;
    }
    for (const c of dynamic) {
      if (!c || typeof c !== "object" || typeof c.id !== "string" || byId.has(c.id)) continue;
      const d = c as CommandRecord & { kind?: "app" | "command"; appName?: string };
      byId.set(c.id, {
        id: c.id,
        title: c.title,
        subtitle: c.subtitle,
        category: c.category,
        kind: d.kind,
        appName: d.appName,
        shortcut: d.shortcut,
      });
    }
  }

  safeHandle("kepler:commands:list", async (): Promise<CommandRecord[]> => {
    const byId = new Map<string, CommandRecord>();
    for (const c of await staticCommands()) byId.set(c.id, c);
    const arkClient = getArkClient();
    if (arkClient) {
      try {
        mergeDynamicCommands(
          byId,
          await Promise.race([
            arkClient.commands.list(),
            delay(2000).then(() => {
              throw new Error("dynamic commands timeout");
            }),
          ]),
        );
      } catch (e) {
        keplerLog.warn("commands", "commands.list failed", { err: String(e) });
      }
    }
    return Array.from(byId.values());
  });

  async function invokeCommandById(id: string, event?: IpcMainInvokeEvent): Promise<void> {
    const internal = findCommand(id);
    if (internal) {
      try {
        await internal.exec(event);
      } catch (e) {
        console.error(`[kepler-shell] command ${id} failed:`, e);
      }
      if (!internal.keepsLauncherOpen) hideLauncher();
      return;
    }
    const arkClient = getArkClient();
    if (arkClient) {
      try {
        await arkClient.commands.invoke(id);
      } catch (e) {
        console.error(`[kepler-shell] dynamic command ${id} invoke failed:`, e);
      }
    } else {
      console.warn(`[kepler-shell] unknown command (no arkClient): ${id}`);
    }
    hideLauncher();
  }

  safeHandle("kepler:commands:invoke", async (event, id: string): Promise<void> => {
    await invokeCommandById(id, event);
  });

  return { invokeCommandById, broadcastCommandsUpdated };
}
