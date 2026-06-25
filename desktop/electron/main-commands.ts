import { setTimeout as delay } from "node:timers/promises";
import { type IpcMainInvokeEvent } from "electron";
import type { ArkClient } from "@kosmos/ark";
import type { CommandRecord } from "../shared/ipc-types";
import { safeHandle } from "./ipc-safe";
import { COMMANDS, findCommand } from "./commands";
import { keplerLog } from "./logging";
import {
  findDeclaredCommand,
  isExtensionRunning,
  loadDeclaredCommands,
  openExtension,
} from "./extension-host";
import { listInstalledUserExtensions } from "./extension-installer";
import { launchRaycastDeclaredCommand } from "./main-raycast-commands";

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
    // Lazy import avoided: BrowserWindow only needed for broadcast fan-out.
    void import("electron").then(({ BrowserWindow }) => {
      for (const win of BrowserWindow.getAllWindows()) {
        if (!win.isDestroyed()) {
          win.webContents.send("kepler:commands:updated");
        }
      }
    });
  }

  async function staticCommands(): Promise<CommandRecord[]> {
    // Filter: команды с requiresExtension показываются только если этот
    // extension реально установлен (manifest.json в %APPDATA%\Kosmos\extensions\).
    // Snapshot installed ids per-call — listInstalledUserExtensions делает
    // disk scan, дёшево (4-10 dir entries).
    let installedIds: Set<string>;
    try {
      installedIds = new Set((await listInstalledUserExtensions()).map((e) => e.id));
    } catch {
      installedIds = new Set();
    }

    const records: CommandRecord[] = [];
    for (const c of COMMANDS.filter(
      (cmd) => !cmd.requiresExtension || installedIds.has(cmd.requiresExtension),
    )) {
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
      console.warn("[kepler-shell] commands.list returned non-array:", dynamic);
      return;
    }

    for (const c of dynamic) {
      if (byId.has(c.id)) continue; // internal/manifest priority
      const d = c as CommandRecord & {
        kind?: "app" | "command";
        appName?: string;
      };
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

    // 1) Kepler-internal (settings/dashboard/check-updates).
    for (const c of await staticCommands()) byId.set(c.id, c);

    // 2) Manifest-declared из всех установленных + dev-tree extension'ов.
    try {
      for (const cmd of loadDeclaredCommands()) {
        if (byId.has(cmd.id)) continue; // internal priority
        byId.set(cmd.id, {
          id: cmd.id,
          title: cmd.title,
          subtitle: cmd.subtitle,
          category: cmd.category,
          kind: cmd.kind,
          appName: cmd.appName,
          icon: cmd.icon,
          shortcut: (cmd as CommandRecord).shortcut,
        });
      }
    } catch (e) {
      keplerLog.error("commands", "loadDeclaredCommands failed", {
        err: String(e),
      });
    }

    // 3) Runtime dynamic (commands.register от running extension'ов).
    //    Видны только пока соответствующий extension запущен.
    const arkClient = getArkClient();
    if (arkClient) {
      try {
        const COMMANDS_DYNAMIC_TIMEOUT_MS = 2000;
        const dynamic = await Promise.race([
          arkClient.commands.list(),
          delay(COMMANDS_DYNAMIC_TIMEOUT_MS).then(() => {
            throw new Error("dynamic commands timeout");
          }),
        ]);
        mergeDynamicCommands(byId, dynamic);
      } catch (e) {
        keplerLog.warn("commands", "commands.list (dynamic) failed", {
          err: String(e),
        });
      }
    }

    return Array.from(byId.values());
  });

  async function awaitExtensionCommand(
    _extensionId: string,
    fullCommandId: string,
    timeoutMs = 5000,
  ): Promise<boolean> {
    const arkClient = getArkClient();
    if (!arkClient) return false;
    const startedAt = Date.now();
    while (Date.now() - startedAt < timeoutMs) {
      try {
        const list = await arkClient.commands.list();
        if (Array.isArray(list) && list.some((c) => c.id === fullCommandId)) {
          return true;
        }
      } catch {
        // ARK rpc race — retry
      }
      await delay(100);
    }
    return false;
  }

  async function invokeCommandById(id: string, event?: IpcMainInvokeEvent): Promise<void> {
    // 1) Internal commands win — exec локально.
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

    // 2) Manifest-declared — open или action mode.
    const declared = findDeclaredCommand(id);
    if (declared) {
      if (declared.mode === "open") {
        await openExtension(declared.extensionId, declared.route);
        hideLauncher();
        return;
      }
      if (declared.mode === "raycast-view" || declared.mode === "raycast-menu-bar") {
        try {
          await launchRaycastDeclaredCommand(declared);
        } catch (e) {
          console.error(`[kepler-shell] Raycast UI command ${id} failed:`, e);
        }
        hideLauncher();
        return;
      }
      if (declared.mode === "raycast-no-view") {
        try {
          await launchRaycastDeclaredCommand(declared);
        } catch (e) {
          console.error(`[kepler-shell] Raycast no-view command ${id} failed:`, e);
        }
        hideLauncher();
        return;
      }
      // action mode: dynamic invoke через ARK. Auto-launch если extension
      // не запущен — окно открывается, ждём commands.register, dispatch'им.
      if (!isExtensionRunning(declared.extensionId)) {
        await openExtension(declared.extensionId, declared.route);
        const ready = await awaitExtensionCommand(declared.extensionId, id);
        if (!ready) {
          console.warn(
            `[kepler-shell] auto-launch для action команды ${id}: extension не зарегистрировал её в течение 5s`,
          );
          hideLauncher();
          return;
        }
      }
      const arkClient = getArkClient();
      if (arkClient) {
        try {
          await arkClient.commands.invoke(id);
        } catch (e) {
          console.error(`[kepler-shell] declared action invoke ${id} failed:`, e);
        }
      }
      hideLauncher();
      return;
    }

    // 3) Runtime dynamic — backend broadcasts command_invoked.
    const arkClient = getArkClient();
    if (arkClient) {
      const colonIdx = id.indexOf(":");
      if (colonIdx > 0) {
        const extId = id.slice(0, colonIdx);
        if (!isExtensionRunning(extId)) {
          await openExtension(extId);
          await awaitExtensionCommand(extId, id);
        }
      }
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

  return {
    invokeCommandById,
    broadcastCommandsUpdated,
  };
}
