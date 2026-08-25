import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import * as RaycastApiModule from "@raycast/api";
import type { CommandValue } from "./view-model";

const COMMAND_API_BRIDGE_EXPORTS = [
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

interface CommandBridgeUrls {
  bridgeUrl: string;
  jsxRuntimeBridgeUrl: string;
}

interface CommandModule {
  default?: (props: CommandValue) => CommandValue | Promise<CommandValue>;
}

function runtimeDir(userDataDir: string): string {
  const dir = path.join(userDataDir, ".raycast-runtime");
  mkdirSync(dir, { recursive: true });
  return dir;
}

function writeCommandApiBridge(userDataDir: string): string {
  const bridgePath = path.join(runtimeDir(userDataDir), "raycast-api-bridge.mjs");
  const namedExports = COMMAND_API_BRIDGE_EXPORTS.map(
    (name) => `export const ${name} = api.${name};`,
  ).join("\n");
  writeFileSync(
    bridgePath,
    `const api = globalThis.__kosmosCommandApi;\n${namedExports}\nexport default api;\n`,
    "utf8",
  );
  return bridgePath;
}

function writeCommandApiJsxRuntimeBridge(userDataDir: string): string {
  const bridgePath = path.join(runtimeDir(userDataDir), "raycast-api-jsx-runtime-bridge.mjs");
  writeFileSync(
    bridgePath,
    `const api = globalThis.__kosmosCommandApi;\nconst emptyProps = {};\nfunction normalizeJsxProps(props) {\n  return props == null ? emptyProps : props;\n}\nexport const Fragment = "Fragment";\nexport function jsx(type, props) {\n  const normalizedProps = normalizeJsxProps(props);\n  if (typeof type === "function") return type(normalizedProps);\n  return api.createRaycastElement(type, normalizedProps);\n}\nexport const jsxs = jsx;\n`,
    "utf8",
  );
  return bridgePath;
}

function prepareCommandApiBridge(userDataDir: string): CommandBridgeUrls {
  // SAFETY: this private global is the documented bridge channel for generated command modules.
  const runtimeGlobal = globalThis as typeof globalThis & {
    __kosmosCommandApi?: typeof RaycastApiModule;
  };
  runtimeGlobal.__kosmosCommandApi = RaycastApiModule;
  return {
    bridgeUrl: pathToFileURL(writeCommandApiBridge(userDataDir)).href,
    jsxRuntimeBridgeUrl: pathToFileURL(writeCommandApiJsxRuntimeBridge(userDataDir)).href,
  };
}

export async function importCommandModule(
  entry: string,
  userDataDir: string,
): Promise<CommandModule> {
  const source = readFileSync(entry, "utf8");
  if (!source.includes("@raycast/api")) {
    // SAFETY: the imported command module is constrained by the Raycast command contract.
    return (await import(`${pathToFileURL(entry).href}?t=${Date.now()}`)) as CommandModule;
  }
  const { bridgeUrl, jsxRuntimeBridgeUrl } = prepareCommandApiBridge(userDataDir);
  const transformed = source
    .replaceAll(`from "@raycast/api/jsx-runtime"`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from '@raycast/api/jsx-runtime'`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from "@raycast/api"`, `from "${bridgeUrl}"`)
    .replaceAll(`from '@raycast/api'`, `from "${bridgeUrl}"`);
  const dataUrl = `data:text/javascript;base64,${Buffer.from(transformed, "utf8").toString("base64")}`;
  // SAFETY: the transformed module is loaded through the generated Raycast bridge.
  return (await import(dataUrl)) as CommandModule;
}
