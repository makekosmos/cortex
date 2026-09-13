import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";
import {
  processInfo,
  processIdentityFromLock,
  sameProcessIdentity,
  stopProcessTree,
} from "../../scripts/dev-run-process.mjs";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
type ProcessIdentity = NonNullable<ReturnType<typeof processInfo>>;
const launchedPids = new Map<
  string,
  {
    shell: ProcessIdentity;
    backend: ProcessIdentity | null;
  }
>();
const delay = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

async function waitForBackendIdentity(
  dataDir: string,
  launchStartedAt: number,
  expectedExecutable: string,
  timeout = 30_000,
): Promise<ReturnType<typeof processInfo>> {
  const lockPath = path.join(dataDir, "engine.lock.json");
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    const identity = processIdentityFromLock(lockPath, launchStartedAt, expectedExecutable);
    if (identity) return identity;
    await delay(100);
  }
  return null;
}
export function freshDataDir(prefix: string): string {
  fs.mkdirSync(path.join(appRoot, ".e2e", "runs"), { recursive: true });
  return fs.mkdtempSync(path.join(appRoot, ".e2e", "runs", `cortex-${prefix}-`));
}
export async function launchKeplerWithDataDir(
  dataDir: string,
  envOverrides: NodeJS.ProcessEnv = {},
  userDataDir = path.join(dataDir, "userdata"),
  runId = path.basename(dataDir),
): Promise<ElectronApplication> {
  const launchStartedAt = Date.now();
  const expectedBackendExe =
    envOverrides.KEPLER_BACKEND_EXE ??
    process.env.KEPLER_BACKEND_EXE ??
    path.resolve(appRoot, "../target/debug/kepler-backend.exe");
  fs.mkdirSync(userDataDir, { recursive: true });
  const app = await electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [
      path.join(appRoot, "dist-electron", "main.js"),
      `--user-data-dir=${userDataDir}`,
      `--kosmos-run-id=${runId}`,
    ],
    env: {
      ...process.env,
      ...envOverrides,
      NODE_ENV: "test",
      KEPLER_INSTANCE: process.env.KEPLER_INSTANCE ?? `test-${runId}`,
      KOSMOS_RUN_ID: process.env.KOSMOS_RUN_ID ?? runId,
      KEPLER_BACKEND_EXE: expectedBackendExe,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_TEST_MODE: "1",
      KOSMOS_HEADLESS: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
    },
    timeout: 20_000,
  });
  const pid = app.process().pid;
  const info = processInfo(pid);
  if (!info?.startTime) throw new Error(`Electron PID ${pid} identity is unavailable`);
  const owned = {
    shell: info,
    backend: null,
  };
  launchedPids.set(dataDir, owned);
  owned.backend = await waitForBackendIdentity(dataDir, launchStartedAt, expectedBackendExe);
  return app;
}
export function shutdownKeplerEngine(dataDir: string): void {
  const owned = launchedPids.get(dataDir);
  if (owned) {
    if (owned.backend) {
      const current = processInfo(owned.backend.pid);
      if (sameProcessIdentity(current, owned.backend))
        stopProcessTree(
          owned.backend.pid,
          owned.backend.startTime,
          owned.backend.commandLine,
          "backend",
        );
    }
    stopProcessTree(owned.shell.pid, owned.shell.startTime, owned.shell.commandLine);
    launchedPids.delete(dataDir);
    return;
  }
}
