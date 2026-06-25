import { clipboard, dialog, shell } from "electron";
import { LaunchType, type AlertOptions, type LaunchCommandOptions } from "@raycast/api";
import {
  extensionUserDataDir,
  findDeclaredCommand,
  openExtension,
  raycastRuntimeContext,
  type DeclaredCommand,
} from "./extension-host";
import { runRaycastNoViewCommand, type RaycastCommandLaunchProps } from "./raycast/command-runner";
import { openRaycastViewCommand } from "./raycast/view-host";

function raycastLaunchFromOptions(options: LaunchCommandOptions): RaycastCommandLaunchProps {
  return {
    launchType: options.type ?? LaunchType.LaunchCommand,
    arguments: options.arguments ?? {},
    fallbackText: options.fallbackText,
    launchContext: options.context,
  };
}

async function launchRaycastCommandFromOptions(
  originExtensionId: string,
  options: LaunchCommandOptions,
): Promise<void> {
  const extensionId = options.extensionName ?? originExtensionId;
  const target = findDeclaredCommand(`${extensionId}:${options.name}`);
  if (!target) {
    console.warn(
      `[kepler-shell] Raycast launchCommand target not found: ${extensionId}:${options.name}`,
    );
    return;
  }
  await launchRaycastDeclaredCommand(target, raycastLaunchFromOptions(options));
}

async function openRaycastSystemTarget(target: string): Promise<void> {
  if (/^https?:\/\//i.test(target)) {
    await shell.openExternal(target);
    return;
  }

  const error = await shell.openPath(target);
  if (error) throw new Error(error);
}

function raycastSystemAdapter(): {
  open(target: string): Promise<void>;
  showInFinder(path: string): Promise<void>;
  trash(path: string): Promise<void>;
} {
  return {
    open: openRaycastSystemTarget,
    async showInFinder(target) {
      shell.showItemInFolder(target);
    },
    async trash(target) {
      await shell.trashItem(target);
    },
  };
}

async function confirmRaycastAlert(alert: AlertOptions): Promise<boolean> {
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

export async function launchRaycastDeclaredCommand(
  declared: DeclaredCommand,
  launch?: RaycastCommandLaunchProps,
): Promise<boolean> {
  if (declared.mode === "open") {
    await openExtension(declared.extensionId, declared.route);
    return true;
  }

  if (
    declared.mode !== "raycast-view" &&
    declared.mode !== "raycast-no-view" &&
    declared.mode !== "raycast-menu-bar"
  ) {
    return false;
  }

  const context = raycastRuntimeContext(declared.extensionId);
  if (!context || !declared.raycastCommandName) {
    console.warn(`[kepler-shell] Raycast command context not found: ${declared.id}`);
    return false;
  }

  const launchCommand = (options: LaunchCommandOptions) =>
    launchRaycastCommandFromOptions(declared.extensionId, options);

  if (declared.mode === "raycast-view" || declared.mode === "raycast-menu-bar") {
    await openRaycastViewCommand({
      extensionId: declared.extensionId,
      extensionName: declared.appName,
      commandName: declared.raycastCommandName,
      commandTitle: declared.title,
      commandMode: declared.mode === "raycast-menu-bar" ? "menu-bar" : "view",
      extensionDir: context.dir,
      source: context.source,
      launch,
      system: raycastSystemAdapter(),
      launchCommand,
    });
    return true;
  }

  await runRaycastNoViewCommand({
    extensionId: declared.extensionId,
    commandName: declared.raycastCommandName,
    extensionDir: context.dir,
    userDataDir: extensionUserDataDir(declared.extensionId),
    source: context.source,
    launch,
    clipboard,
    system: raycastSystemAdapter(),
    confirmAlert: confirmRaycastAlert,
    launchCommand,
  });
  return true;
}
