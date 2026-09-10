import { spawn, type ChildProcess } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { keplerDataDir } from "./data-dir";
import type { JsonRecord } from "./extension-permissions";

interface BackendProcessLogger {
  error(scope: string, message: string, data?: JsonRecord): void;
  info(scope: string, message: string, data?: JsonRecord): void;
}

interface SpawnBackendProcessArgs {
  desktopAuthorityCredential: string;
  env: NodeJS.ProcessEnv;
  instanceSlot: string;
  log: BackendProcessLogger;
  resolveBackendExe(): string;
}

export interface SpawnedBackendProcess {
  lockPath: string;
  proc: ChildProcess | null;
}

export function spawnBackendProcess({
  desktopAuthorityCredential,
  env,
  instanceSlot,
  log,
  resolveBackendExe,
}: SpawnBackendProcessArgs): SpawnedBackendProcess {
  const exe = resolveBackendExe();
  if (!existsSync(exe)) {
    log.error("backend", "Kosmos Runtime not found", { exe });
    return { lockPath: "", proc: null };
  }

  const dataDir = keplerDataDir();
  const lockPath = path.join(dataDir, "engine.lock.json");
  log.info("backend", "spawning backend", { exe, dataDir });
  const testModeEnabled = env.KOSMOS_TEST_MODE === "1";
  const headlessEnabled = env.KOSMOS_HEADLESS === "1";
  const testGroqApiKey =
    testModeEnabled || headlessEnabled ? (env.KOSMOS_TEST_GROQ_API_KEY?.trim() ?? "") : "";
  const backendEnv: NodeJS.ProcessEnv = {
    ...env,
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_TEST_MODE: testModeEnabled ? "1" : env.KOSMOS_TEST_MODE,
    KOSMOS_HEADLESS: headlessEnabled ? "1" : env.KOSMOS_HEADLESS,
    KEPLER_INSTANCE: instanceSlot,
    KOSMOS_DESKTOP_ROLE_CREDENTIAL: desktopAuthorityCredential,
    KOSMOS_DESKTOP_ROLE_PID: String(process.pid),
    RUST_BACKTRACE: "1",
  };
  if (
    process.platform === "win32" &&
    path.basename(process.execPath).toLowerCase() === "kosmos.exe"
  ) {
    backendEnv.KOSMOS_CORTEX_EXECUTABLE ??= process.execPath;
  }
  if (testGroqApiKey) {
    backendEnv.KOSMOS_TEST_GROQ_API_KEY = testGroqApiKey;
  } else {
    delete backendEnv.KOSMOS_TEST_GROQ_API_KEY;
  }

  const proc = spawn(exe, ["--start"], {
    detached: true,
    stdio: "ignore",
    env: backendEnv,
    windowsHide: true,
  });
  proc.unref();
  return { lockPath, proc };
}

export function restartBackendProcess(resolveBackendExe: () => string): Promise<void> {
  return new Promise((resolve, reject) => {
    const proc = spawn(resolveBackendExe(), ["--restart-core"], {
      stdio: "ignore",
      windowsHide: true,
    });
    proc.once("error", reject);
    proc.once("exit", (code) => {
      if (code === 0 || code === 2) resolve();
      else reject(new Error(`Kosmos Runtime restart exited with code ${code}`));
    });
  });
}

export function isBackendLockProcessAlive(lockPath: string): boolean {
  if (!lockPath || !existsSync(lockPath)) return false;
  try {
    const pid = JSON.parse(readFileSync(lockPath, "utf8")).pid;
    if (!Number.isInteger(pid) || pid <= 0) return false;
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}
