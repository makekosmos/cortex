import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import * as RaycastApiModule from "@raycast/api";

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

function runtimeDir(userDataDir: string): string {
  const dir = path.join(userDataDir, ".raycast-runtime");
  mkdirSync(dir, { recursive: true });
  return dir;
}

function writeRaycastApiBridge(userDataDir: string): string {
  const bridgePath = path.join(runtimeDir(userDataDir), "raycast-api-bridge.mjs");
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
  const bridgePath = path.join(runtimeDir(userDataDir), "raycast-api-jsx-runtime-bridge.mjs");
  writeFileSync(
    bridgePath,
    `const api = globalThis.__kosmosRaycastApi;\nconst emptyProps = {};\nfunction normalizeJsxProps(props) {\n  return props == null ? emptyProps : props;\n}\nexport const Fragment = "Fragment";\nexport function jsx(type, props) {\n  const normalizedProps = normalizeJsxProps(props);\n  if (typeof type === "function") return type(normalizedProps);\n  return api.createRaycastElement(type, normalizedProps);\n}\nexport const jsxs = jsx;\n`,
    "utf8",
  );
  return bridgePath;
}

function prepareRaycastApiBridge(userDataDir: string): {
  bridgeUrl: string;
  jsxRuntimeBridgeUrl: string;
} {
  (
    globalThis as typeof globalThis & { __kosmosRaycastApi?: typeof RaycastApiModule }
  ).__kosmosRaycastApi = RaycastApiModule;
  return {
    bridgeUrl: pathToFileURL(writeRaycastApiBridge(userDataDir)).href,
    jsxRuntimeBridgeUrl: pathToFileURL(writeRaycastApiJsxRuntimeBridge(userDataDir)).href,
  };
}

export async function importRaycastCommand(
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
  const { bridgeUrl, jsxRuntimeBridgeUrl } = prepareRaycastApiBridge(userDataDir);
  const transformed = source
    .replaceAll(`from "@raycast/api/jsx-runtime"`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from '@raycast/api/jsx-runtime'`, `from "${jsxRuntimeBridgeUrl}"`)
    .replaceAll(`from "@raycast/api"`, `from "${bridgeUrl}"`)
    .replaceAll(`from '@raycast/api'`, `from "${bridgeUrl}"`);
  const dataUrl = `data:text/javascript;base64,${Buffer.from(transformed, "utf8").toString("base64")}`;
  return (await import(dataUrl)) as { default?: (props: unknown) => unknown | Promise<unknown> };
}
