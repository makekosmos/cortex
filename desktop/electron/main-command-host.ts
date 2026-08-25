import { clipboard, dialog, shell } from "electron";
import { LaunchType, type AlertOptions, type LaunchCommandOptions } from "@raycast/api";
import {
  extensionUserDataDir,
  findDeclaredCommand,
  openExtension,
  commandRuntimeContext,
  type DeclaredCommand,
} from "./extension-host";
import { runCommandNoView, type CommandLaunchProps } from "./command-host/command-runner";
import { openCommandViewCommand } from "./command-host/view-host";

interface CommandSystemAdapter {
  open(target: string): Promise<void>;
  showInFinder(path: string): Promise<void>;
  trash(path: string): Promise<void>;
}

function commandLaunchFromOptions(options: LaunchCommandOptions): CommandLaunchProps {
  return {
    launchType: options.type ?? LaunchType.LaunchCommand,
    arguments: options.arguments ?? {},
    fallbackText: options.fallbackText,
    launchContext: options.context,
  };
}

async function launchCommandFromOptions(
  originExtensionId: string,
  options: LaunchCommandOptions,
): Promise<void> {
  const extensionId = options.extensionName ?? originExtensionId;
  const target = findDeclaredCommand(`${extensionId}:${options.name}`);
  if (!target) {
    console.warn(
      `[kepler-shell] Command launchCommand target not found: ${extensionId}:${options.name}`,
    );
    return;
  }
  await launchCommandDeclaredCommand(target, commandLaunchFromOptions(options));
}

async function openCommandSystemTarget(target: string): Promise<void> {
  if (/^https?:\/\//i.test(target)) {
    await shell.openExternal(target);
    return;
  }

  const error = await shell.openPath(target);
  if (error) throw new Error(error);
}

function commandSystemAdapter(): CommandSystemAdapter {
  return {
    open: openCommandSystemTarget,
    async showInFinder(target) {
      shell.showItemInFolder(target);
    },
    async trash(target) {
      await shell.trashItem(target);
    },
  };
}

async function confirmCommandAlert(alert: AlertOptions): Promise<boolean> {
  const result = await dialog.showMessageBox({
    type: "question",
    buttons: [alert.primaryAction?.title ?? "OK", alert.dismissAction?.title ?? "Отмена"],
    defaultId: 0,
    cancelId: 1,
    title: alert.title,
    message: alert.title,
    detail: alert.message,
  });
  return result.response === 0;
}

export async function launchCommandDeclaredCommand(
  declared: DeclaredCommand,
  launch?: CommandLaunchProps,
): Promise<boolean> {
  if (declared.mode === "open") {
    await openExtension(declared.extensionId, declared.route);
    return true;
  }

  if (
    declared.mode !== "command-view" &&
    declared.mode !== "command-no-view" &&
    declared.mode !== "command-menu-bar"
  ) {
    return false;
  }

  const context = commandRuntimeContext(declared.extensionId);
  if (!context || !declared.commandName) {
    console.warn(`[kepler-shell] Command context not found: ${declared.id}`);
    return false;
  }

  const launchCommand = (options: LaunchCommandOptions) =>
    launchCommandFromOptions(declared.extensionId, options);

  if (declared.mode === "command-view" || declared.mode === "command-menu-bar") {
    await openCommandViewCommand({
      extensionId: declared.extensionId,
      extensionName: declared.appName,
      commandName: declared.commandName,
      commandTitle: declared.title,
      commandMode: declared.mode === "command-menu-bar" ? "menu-bar" : "view",
      extensionDir: context.dir,
      source: context.source,
      launch,
      system: commandSystemAdapter(),
      launchCommand,
    });
    return true;
  }

  await runCommandNoView({
    extensionId: declared.extensionId,
    commandName: declared.commandName,
    extensionDir: context.dir,
    userDataDir: extensionUserDataDir(declared.extensionId),
    source: context.source,
    launch,
    clipboard,
    system: commandSystemAdapter(),
    confirmAlert: confirmCommandAlert,
    launchCommand,
  });
  return true;
}
