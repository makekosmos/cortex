import path from "node:path";
import {
  configureRaycastRuntime,
  type LaunchCommandOptions,
  LaunchType,
  type LaunchTypeValue,
  type RaycastRuntimeAdapter,
} from "@raycast/api";
import type { AlertOptions } from "@raycast/api";
import type { ExtensionSource } from "../extension-permissions";
import { importCommandModule } from "./api-bridge";
import {
  loadCommandPackageManifest,
  commandPreferenceDefaults,
  resolveCommandEntry,
  type CommandPackageManifest,
} from "./manifest";
import {
  normalizeCommandNode,
  type CommandValue,
  type CommandViewCallbackRegistry,
} from "./view-model";
import { createCacheStorage, createStorage } from "./storage";
import type { CommandSnapshotNode } from "../../shared/command-ipc";
import type { CommandFeedbackEvent } from "../../shared/command-ipc";

interface ClipboardAdapter {
  writeText(text: string): void;
  readText?(): string;
  clear?(): void;
}

interface SystemAdapter {
  open(target: string): Promise<void>;
  showInFinder(path: string): Promise<void>;
  trash(path: string): Promise<void>;
}

export interface RunCommandNoViewOptions {
  extensionId: string;
  commandName: string;
  extensionDir: string;
  userDataDir: string;
  source: ExtensionSource;
  launch?: CommandLaunchProps;
  clipboard?: ClipboardAdapter;
  system?: SystemAdapter;
  launchCommand?: (options: LaunchCommandOptions) => Promise<void>;
  confirmAlert?: (options: AlertOptions) => Promise<boolean>;
  feedback?: (event: CommandFeedbackEvent) => void;
  navigation?: {
    push(target: CommandValue): void;
    pop(): void;
    popToRoot(): void;
  };
}

export interface RunCommandViewOptions extends RunCommandNoViewOptions {
  callbacks?: CommandViewCallbackRegistry;
  commandMode?: "view" | "menu-bar";
}

export interface CommandLaunchProps {
  launchType?: LaunchTypeValue;
  arguments?: object;
  fallbackText?: string;
  launchContext?: unknown;
}


function createRuntimeAdapter(
  manifest: CommandPackageManifest,
  options: RunCommandNoViewOptions,
): RaycastRuntimeAdapter {
  const local = createStorage(path.join(options.userDataDir, "raycast-local-storage.json"));
  const cacheDir = path.join(options.userDataDir, "raycast-cache");
  return {
    async showToast(toast) {
      options.feedback?.({
        kind: "toast",
        title: toast.title,
        message: toast.message,
        style: toast.style,
      });
      console.log(`[command:${options.extensionId}] toast: ${toast.title}`);
    },
    async showHUD(title) {
      options.feedback?.({ kind: "hud", title });
      console.log(`[command:${options.extensionId}] hud: ${title}`);
    },
    async confirmAlert(alert) {
      return options.confirmAlert ? options.confirmAlert(alert) : false;
    },
    async clipboardCopy(content) {
      options.clipboard?.writeText(content);
    },
    async clipboardPaste(content) {
      options.clipboard?.writeText(content);
    },
    async clipboardReadText() {
      return options.clipboard?.readText?.() ?? "";
    },
    async clipboardClear() {
      if (options.clipboard?.clear) {
        options.clipboard.clear();
        return;
      }
      options.clipboard?.writeText("");
    },
    async systemOpen(target) {
      if (!options.system) throw new Error("[kepler-shell] Command system.open is not configured");
      await options.system.open(target);
    },
    async systemShowInFinder(path) {
      if (!options.system) {
        throw new Error("[kepler-shell] Command system.showInFinder is not configured");
      }
      await options.system.showInFinder(path);
    },
    async systemTrash(path) {
      if (!options.system) throw new Error("[kepler-shell] Command system.trash is not configured");
      await options.system.trash(path);
    },
    async localStorageGetItem(key) {
      return local.get(key);
    },
    async localStorageAllItems() {
      return local.all();
    },
    async localStorageSetItem(key, value) {
      local.set(key, value);
    },
    async localStorageRemoveItem(key) {
      local.remove(key);
    },
    async localStorageClear() {
      local.clear();
    },
    async cacheGet(namespace, key) {
      return createCacheStorage(cacheDir, namespace).get(key);
    },
    async cacheSet(namespace, key, value) {
      createCacheStorage(cacheDir, namespace).set(key, value);
    },
    async cacheRemove(namespace, key) {
      createCacheStorage(cacheDir, namespace).remove(key);
    },
    async cacheClear(namespace) {
      createCacheStorage(cacheDir, namespace).clear();
    },
    getPreferenceValues() {
      // SAFETY: parsed manifest defaults contain only JSON preference values accepted by Raycast.
      return commandPreferenceDefaults(manifest, options.commandName) as ReturnType<
        RaycastRuntimeAdapter["getPreferenceValues"]
      >;
    },
    async launchCommand(commandOptions) {
      if (!options.launchCommand) {
        throw new Error("[kepler-shell] Command launchCommand is not configured");
      }
      await options.launchCommand(commandOptions);
    },
    navigationPush(target) {
      // SAFETY: Raycast navigation targets are serialized command values at this boundary.
      options.navigation?.push(target as CommandValue);
    },
    navigationPop() {
      options.navigation?.pop();
    },
    navigationPopToRoot() {
      options.navigation?.popToRoot();
    },
  };
}

function commandLaunchProps(options: RunCommandNoViewOptions): CommandLaunchProps {
  return {
    launchType: options.launch?.launchType ?? LaunchType.UserInitiated,
    arguments: options.launch?.arguments ?? {},
    fallbackText: options.launch?.fallbackText,
    launchContext: options.launch?.launchContext,
  };
}

export async function runCommandNoView(options: RunCommandNoViewOptions): Promise<void> {
  if (options.source === "user") {
    throw new Error(
      `[kepler-shell] refusing to execute user-installed command '${options.extensionId}:${options.commandName}' without an isolated runtime`,
    );
  }

  const manifest = loadCommandPackageManifest(options.extensionDir);
  const command = manifest?.commands.find((item) => item.name === options.commandName);
  if (!manifest || !command || command.mode !== "no-view") {
    throw new Error(
      `[kepler-shell] no-view command not found: ${options.extensionId}:${options.commandName}`,
    );
  }

  const entry = resolveCommandEntry(options.extensionDir, manifest, options.commandName);
  if (!entry) {
    throw new Error(
      `[kepler-shell] command '${options.extensionId}:${options.commandName}' has no built JS entry`,
    );
  }

  configureRaycastRuntime(createRuntimeAdapter(manifest, options));
  const imported = await importCommandModule(entry, options.userDataDir);
  if (!imported.default) {
    throw new Error(`[kepler-shell] command '${entry}' must export default function`);
  }
  // SAFETY: command launch props are produced by the local Raycast runtime adapter.
  await imported.default(commandLaunchProps(options) as CommandValue);
}

export async function runCommandView(options: RunCommandViewOptions): Promise<CommandSnapshotNode> {
  if (options.source === "user") {
    throw new Error(
      `[kepler-shell] refusing to execute user-installed view command '${options.extensionId}:${options.commandName}' without an isolated runtime`,
    );
  }

  const manifest = loadCommandPackageManifest(options.extensionDir);
  const command = manifest?.commands.find((item) => item.name === options.commandName);
  const commandMode = options.commandMode ?? "view";
  if (!manifest || !command || command.mode !== commandMode) {
    throw new Error(
      `[kepler-shell] ${commandMode} command not found: ${options.extensionId}:${options.commandName}`,
    );
  }

  const entry = resolveCommandEntry(options.extensionDir, manifest, options.commandName);
  if (!entry) {
    throw new Error(
      `[kepler-shell] command '${options.extensionId}:${options.commandName}' has no built JS entry`,
    );
  }

  configureRaycastRuntime(createRuntimeAdapter(manifest, options));
  const imported = await importCommandModule(entry, options.userDataDir);
  if (!imported.default) {
    throw new Error(`[kepler-shell] command '${entry}' must export default function`);
  }
  // SAFETY: command launch props are produced by the local Raycast runtime adapter.
  const root = await imported.default(commandLaunchProps(options) as CommandValue);
  // SAFETY: the command module runs inside the Raycast bridge and returns the documented element tree.
  const snapshot = normalizeCommandNode(root as CommandValue, options.callbacks);
  if (!snapshot) {
    throw new Error(
      `[kepler-shell] view command '${options.extensionId}:${options.commandName}' returned no UI`,
    );
  }
  return snapshot;
}
