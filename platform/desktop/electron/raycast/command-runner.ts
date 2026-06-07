import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import * as RaycastApiModule from "@raycast/api";
import {
  configureRaycastRuntime,
  type LaunchCommandOptions,
  LaunchType,
  type LaunchTypeValue,
  type RaycastRuntimeAdapter,
} from "@raycast/api";
import type { AlertOptions } from "@raycast/api";
import type { ExtensionSource } from "../extension-permissions";
import {
  loadRaycastPackageManifest,
  raycastPreferenceDefaults,
  resolveRaycastCommandEntry,
  type RaycastPackageManifest,
} from "./manifest";
import { normalizeRaycastNode, type RaycastViewCallbackRegistry } from "./view-model";
import type { RaycastSnapshotNode } from "../../shared/raycast-ipc";
import type { RaycastFeedbackEvent } from "../../shared/raycast-ipc";

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

export interface RunRaycastNoViewCommandOptions {
  extensionId: string;
  commandName: string;
  extensionDir: string;
  userDataDir: string;
  source: ExtensionSource;
  launch?: RaycastCommandLaunchProps;
  clipboard?: ClipboardAdapter;
  system?: SystemAdapter;
  launchCommand?: (options: LaunchCommandOptions) => Promise<void>;
  confirmAlert?: (options: AlertOptions) => Promise<boolean>;
  feedback?: (event: RaycastFeedbackEvent) => void;
  navigation?: {
    push(target: unknown): void;
    pop(): void;
    popToRoot(): void;
  };
}

export interface RunRaycastViewCommandOptions extends RunRaycastNoViewCommandOptions {
  callbacks?: RaycastViewCallbackRegistry;
  commandMode?: "view" | "menu-bar";
}

export interface RaycastCommandLaunchProps {
  launchType?: LaunchTypeValue;
  arguments?: Record<string, unknown>;
  fallbackText?: string;
  launchContext?: unknown;
}

function readJsonFile(filePath: string): Record<string, string> {
  if (!existsSync(filePath)) return {};
  try {
    const parsed = JSON.parse(readFileSync(filePath, "utf8")) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, string>)
      : {};
  } catch {
    return {};
  }
}

function writeJsonFile(filePath: string, value: Record<string, string>): void {
  mkdirSync(path.dirname(filePath), { recursive: true });
  writeFileSync(filePath, JSON.stringify(value, null, 2), "utf8");
}

function createStorage(filePath: string): {
  get(key: string): string | undefined;
  all(): Record<string, string>;
  set(key: string, value: string): void;
  remove(key: string): void;
  clear(): void;
} {
  return {
    get(key) {
      return readJsonFile(filePath)[key];
    },
    all() {
      return { ...readJsonFile(filePath) };
    },
    set(key, value) {
      const data = readJsonFile(filePath);
      data[key] = value;
      writeJsonFile(filePath, data);
    },
    remove(key) {
      const data = readJsonFile(filePath);
      delete data[key];
      writeJsonFile(filePath, data);
    },
    clear() {
      writeJsonFile(filePath, {});
    },
  };
}

function createCacheStorage(filePath: string, namespace: string): ReturnType<typeof createStorage> {
  return createStorage(path.join(filePath, `${namespace}.json`));
}

function createRuntimeAdapter(
  manifest: RaycastPackageManifest,
  options: RunRaycastNoViewCommandOptions,
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
      console.log(`[raycast:${options.extensionId}] toast: ${toast.title}`);
    },
    async showHUD(title) {
      options.feedback?.({ kind: "hud", title });
      console.log(`[raycast:${options.extensionId}] hud: ${title}`);
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
      if (!options.system) throw new Error("[kepler-shell] Raycast system.open is not configured");
      await options.system.open(target);
    },
    async systemShowInFinder(path) {
      if (!options.system) {
        throw new Error("[kepler-shell] Raycast system.showInFinder is not configured");
      }
      await options.system.showInFinder(path);
    },
    async systemTrash(path) {
      if (!options.system) throw new Error("[kepler-shell] Raycast system.trash is not configured");
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
      return raycastPreferenceDefaults(manifest, options.commandName);
    },
    async launchCommand(commandOptions) {
      if (!options.launchCommand) {
        throw new Error("[kepler-shell] Raycast launchCommand is not configured");
      }
      await options.launchCommand(commandOptions);
    },
    navigationPush(target) {
      options.navigation?.push(target);
    },
    navigationPop() {
      options.navigation?.pop();
    },
    navigationPopToRoot() {
      options.navigation?.popToRoot();
    },
  };
}

function commandLaunchProps(options: RunRaycastNoViewCommandOptions): {
  launchType: LaunchTypeValue;
  arguments: Record<string, unknown>;
  fallbackText?: string;
  launchContext?: unknown;
} {
  return {
    launchType: options.launch?.launchType ?? LaunchType.UserInitiated,
    arguments: options.launch?.arguments ?? {},
    fallbackText: options.launch?.fallbackText,
    launchContext: options.launch?.launchContext,
  };
}

const RAYCAST_API_BRIDGE_EXPORTS = [
  "Action",
  "ActionPanel",
  "Cache",
  "Clipboard",
  "Detail",
  "Form",
  "Grid",
  "LaunchType",
  "List",
  "LocalStorage",
  "MenuBarExtra",
  "Toast",
  "confirmAlert",
  "environment",
  "getPreferenceValues",
  "launchCommand",
  "open",
  "showInFinder",
  "showHUD",
  "showToast",
  "trash",
  "useNavigation",
];

function writeRaycastApiBridge(userDataDir: string): string {
  const runtimeDir = path.join(userDataDir, ".raycast-runtime");
  mkdirSync(runtimeDir, { recursive: true });
  const bridgePath = path.join(runtimeDir, "raycast-api-bridge.mjs");
  const namedExports = RAYCAST_API_BRIDGE_EXPORTS.map(
    (name) => `export const ${name} = api.${name};`,
  ).join("\n");
  writeFileSync(
    bridgePath,
    `const api = globalThis.__kosmosRaycastApi;\n${namedExports}\nexport default api;\n`,
    "utf8",
  );
  return bridgePath;
}

function writeRaycastApiJsxRuntimeBridge(userDataDir: string): string {
  const runtimeDir = path.join(userDataDir, ".raycast-runtime");
  mkdirSync(runtimeDir, { recursive: true });
  const bridgePath = path.join(runtimeDir, "raycast-api-jsx-runtime-bridge.mjs");
  writeFileSync(
    bridgePath,
    `const api = globalThis.__kosmosRaycastApi;\nexport const Fragment = "Fragment";\nexport function jsx(type, props) {\n  if (typeof type === "function") return type(props ?? {});\n  return api.createRaycastElement(type, props ?? {});\n}\nexport const jsxs = jsx;\n`,
    "utf8",
  );
  return bridgePath;
}

async function importRaycastCommand(
  entry: string,
  userDataDir: string,
): Promise<{
  default?: (props: unknown) => unknown | Promise<unknown>;
}> {
  const source = readFileSync(entry, "utf8");
  if (!source.includes("@raycast/api")) {
    return (await import(`${pathToFileURL(entry).href}?t=${Date.now()}`)) as {
      default?: (props: unknown) => unknown | Promise<unknown>;
    };
  }
  const bridgeUrl = pathToFileURL(writeRaycastApiBridge(userDataDir)).href;
  const jsxRuntimeBridgeUrl = pathToFileURL(writeRaycastApiJsxRuntimeBridge(userDataDir)).href;
  (
    globalThis as typeof globalThis & { __kosmosRaycastApi?: typeof RaycastApiModule }
  ).__kosmosRaycastApi = RaycastApiModule;
  const transformed = source
    .replaceAll(`from "@raycast/api/jsx-runtime"`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from '@raycast/api/jsx-runtime'`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from "@raycast/api"`, `from "${bridgeUrl}"`)
    .replaceAll(`from '@raycast/api'`, `from "${bridgeUrl}"`);
  const dataUrl = `data:text/javascript;base64,${Buffer.from(transformed, "utf8").toString("base64")}`;
  return (await import(dataUrl)) as { default?: (props: unknown) => unknown | Promise<unknown> };
}

export async function runRaycastNoViewCommand(
  options: RunRaycastNoViewCommandOptions,
): Promise<void> {
  if (options.source === "user") {
    throw new Error(
      `[kepler-shell] refusing to execute user-installed Raycast command '${options.extensionId}:${options.commandName}' without an isolated runtime`,
    );
  }

  const manifest = loadRaycastPackageManifest(options.extensionDir);
  const command = manifest?.commands.find((item) => item.name === options.commandName);
  if (!manifest || !command || command.mode !== "no-view") {
    throw new Error(
      `[kepler-shell] Raycast no-view command not found: ${options.extensionId}:${options.commandName}`,
    );
  }

  const entry = resolveRaycastCommandEntry(options.extensionDir, manifest, options.commandName);
  if (!entry) {
    throw new Error(
      `[kepler-shell] Raycast command '${options.extensionId}:${options.commandName}' has no built JS entry`,
    );
  }

  configureRaycastRuntime(createRuntimeAdapter(manifest, options));
  const imported = await importRaycastCommand(entry, options.userDataDir);
  if (typeof imported.default !== "function") {
    throw new Error(`[kepler-shell] Raycast command '${entry}' must export default function`);
  }
  await imported.default(commandLaunchProps(options));
}

export async function runRaycastViewCommand(
  options: RunRaycastViewCommandOptions,
): Promise<RaycastSnapshotNode> {
  if (options.source === "user") {
    throw new Error(
      `[kepler-shell] refusing to execute user-installed Raycast view command '${options.extensionId}:${options.commandName}' without an isolated runtime`,
    );
  }

  const manifest = loadRaycastPackageManifest(options.extensionDir);
  const command = manifest?.commands.find((item) => item.name === options.commandName);
  const commandMode = options.commandMode ?? "view";
  if (!manifest || !command || command.mode !== commandMode) {
    throw new Error(
      `[kepler-shell] Raycast ${commandMode} command not found: ${options.extensionId}:${options.commandName}`,
    );
  }

  const entry = resolveRaycastCommandEntry(options.extensionDir, manifest, options.commandName);
  if (!entry) {
    throw new Error(
      `[kepler-shell] Raycast command '${options.extensionId}:${options.commandName}' has no built JS entry`,
    );
  }

  configureRaycastRuntime(createRuntimeAdapter(manifest, options));
  const imported = await importRaycastCommand(entry, options.userDataDir);
  if (typeof imported.default !== "function") {
    throw new Error(`[kepler-shell] Raycast command '${entry}' must export default function`);
  }
  const root = await imported.default(commandLaunchProps(options));
  const snapshot = normalizeRaycastNode(root, options.callbacks);
  if (!snapshot) {
    throw new Error(
      `[kepler-shell] Raycast view command '${options.extensionId}:${options.commandName}' returned no UI`,
    );
  }
  return snapshot;
}
