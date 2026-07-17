import { registerMainCommands } from "./main-commands";
import { registerMainCrashesIpc } from "./main-crashes-ipc";
import { registerMainDataIpc } from "./main-data-ipc";
import { registerMainShellIpc } from "./main-shell-ipc";
import { registerLeetCodeIntegrationIpc } from "./leetcode-integration";

type RegisterMainProcessIpcOptions = {
  awaitArkReady(timeoutMs?: number): Promise<any>;
  broadcastSettingsSyncUpdated(): void;
  getArkClient(): any;
  getBackendLockPath(): any;
  hideLauncher(): void;
  isBackendRunning(): boolean;
  setLauncherExpanded(expanded: boolean): void;
};

export function registerMainProcessIpc(options: RegisterMainProcessIpcOptions) {
  const commandsController = registerMainCommands({
    getArkClient: options.getArkClient,
    hideLauncher: options.hideLauncher,
  });

  registerMainDataIpc({
    awaitArkReady: options.awaitArkReady,
    getArkClient: options.getArkClient,
    broadcastSettingsSyncUpdated: options.broadcastSettingsSyncUpdated,
  });
  registerMainCrashesIpc();
  registerLeetCodeIntegrationIpc({ awaitArkReady: options.awaitArkReady });
  registerMainShellIpc({
    getBackendLockPath: options.getBackendLockPath,
    isBackendRunning: options.isBackendRunning,
    awaitArkReady: options.awaitArkReady,
    getArkClient: options.getArkClient,
    hideLauncher: options.hideLauncher,
    setLauncherExpanded: options.setLauncherExpanded,
  });

  return commandsController;
}
