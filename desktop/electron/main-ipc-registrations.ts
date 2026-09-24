import { registerMainCommands } from "./main-commands";
import { registerMainDataIpc } from "./main-data-ipc";
import { registerMainShellIpc } from "./main-shell-ipc";

type RegisterMainProcessIpcOptions = {
  awaitArkReady(timeoutMs?: number): Promise<any>;
  getArkClient(): any;
  hideLauncher(): void;
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
  });
  registerMainShellIpc({
    awaitArkReady: options.awaitArkReady,
    getArkClient: options.getArkClient,
    hideLauncher: options.hideLauncher,
    setLauncherExpanded: options.setLauncherExpanded,
  });

  return commandsController;
}
