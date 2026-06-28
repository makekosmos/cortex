import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import * as RaycastApiModule from "@raycast/api";

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

function prepareCommandApiBridge(userDataDir: string): {
  bridgeUrl: string;
  jsxRuntimeBridgeUrl: string;
} {
  (
    globalThis as typeof globalThis & { __kosmosCommandApi?: typeof RaycastApiModule }
  ).__kosmosCommandApi = RaycastApiModule;
  return {
    bridgeUrl: pathToFileURL(writeCommandApiBridge(userDataDir)).href,
    jsxRuntimeBridgeUrl: pathToFileURL(writeCommandApiJsxRuntimeBridge(userDataDir)).href,
  };
}

export async function importCommandModule(
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
  const { bridgeUrl, jsxRuntimeBridgeUrl } = prepareCommandApiBridge(userDataDir);
  const transformed = source
    .replaceAll(`from "@raycast/api/jsx-runtime"`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from '@raycast/api/jsx-runtime'`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from "@raycast/api"`, `from "${bridgeUrl}"`)
    .replaceAll(`from '@raycast/api'`, `from "${bridgeUrl}"`);
  const dataUrl = `data:text/javascript;base64,${Buffer.from(transformed, "utf8").toString("base64")}`;
  return (await import(dataUrl)) as { default?: (props: unknown) => unknown | Promise<unknown> };
}
