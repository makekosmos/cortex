import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";
import {
  cargoTarget,
  executableName,
  hostE2eEnvironment,
  recordCleanup,
} from "../../host/e2e/fixtures/host-runtime";

export {
  buildEngine,
  cargoTarget,
  closeHost,
  executableName,
  hostE2eEnvironment,
  processTreePids,
  recordCleanup,
  rpc,
  rpcError,
  startEngine,
  terminate,
  waitFor,
  waitForPidGone,
} from "../../host/e2e/fixtures/host-runtime";
export type { JsonValue, Lock } from "../../host/e2e/fixtures/host-runtime";

export const managerRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
export const managerMain = path.join(managerRoot, "dist-electron", "main.js");

export const engineBinaries = () => {
  const target = cargoTarget();
  return {
    engine: path.join(target, "debug", executableName("kepler-backend")),
    ark: path.join(target, "debug", executableName("ark-core-rpc")),
  };
};

export const cleanupManifest = (): string => {
  const manifest = process.env.KOSMOS_MANAGER_E2E_CLEANUP_MANIFEST;
  if (!manifest) throw new Error("KOSMOS_MANAGER_E2E_CLEANUP_MANIFEST is required");
  return manifest;
};

// Isolated run root under the OS temp dir; the run-e2e.mjs cleanup sweep only
// accepts `kosmos-manager-e2e-*` directories directly under os.tmpdir().
export const managerE2eRoot = (slug: string): string => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), `kosmos-manager-e2e-${slug}-`));
  recordCleanup(cleanupManifest(), root, new Set());
  return root;
};

// Deterministic env for both the Engine child and the Manager Electron app:
// whitelisted inheritance plus per-run APPDATA/XDG_CONFIG_HOME isolation so
// Electron never touches the developer's real profile. Linux containers
// typically lack unprivileged user namespaces for the Chromium SUID sandbox.
export const managerEnvironment = (
  root: string,
  dataDir: string,
  overrides: NodeJS.ProcessEnv = {},
): NodeJS.ProcessEnv => {
  const environment = hostE2eEnvironment({
    APPDATA: path.join(root, "appdata"),
    XDG_CONFIG_HOME: path.join(root, "xdg-config"),
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_TEST_MODE: "1",
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
    NODE_ENV: "test",
    ...overrides,
  });
  if (process.platform === "linux") environment.ELECTRON_DISABLE_SANDBOX = "1";
  return environment;
};

export const launchManager = (
  root: string,
  slot: string,
  environment: NodeJS.ProcessEnv,
  entry = managerMain,
): Promise<ElectronApplication> => {
  const userData = path.join(root, slot, "userdata");
  fs.mkdirSync(userData, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: managerRoot,
    args: [`--user-data-dir=${userData}`, entry],
    env: environment,
    timeout: 30_000,
  });
};
