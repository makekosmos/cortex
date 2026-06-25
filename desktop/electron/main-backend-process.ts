import { spawn, type ChildProcess } from "node:child_process";
import { existsSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { keplerDataDir } from "./data-dir";

interface BackendProcessLogger {
  error(scope: string, message: string, data?: Record<string, unknown>): void;
  info(scope: string, message: string, data?: Record<string, unknown>): void;
}

interface SpawnBackendProcessArgs {
  env: NodeJS.ProcessEnv;
  instanceSlot: string;
  isUsageTrackerEnabled(): boolean;
  log: BackendProcessLogger;
  resolveBackendExe(): string;
}

export interface SpawnedBackendProcess {
  lockPath: string;
  proc: ChildProcess | null;
}

export function spawnBackendProcess({
  env,
  instanceSlot,
  isUsageTrackerEnabled,
  log,
  resolveBackendExe,
}: SpawnBackendProcessArgs): SpawnedBackendProcess {
  const exe = resolveBackendExe();
  if (!existsSync(exe)) {
    log.error("backend", "Kosmos Runtime not found", { exe });
    return { lockPath: "", proc: null };
  }

  const dataDir = keplerDataDir();
  const lockPath = path.join(dataDir, "kepler.lock.json");
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
    KEPLER_USAGE_TRACKER: isUsageTrackerEnabled() ? "1" : "0",
    KOSMOS_DEVICE_NAME: env.KOSMOS_DEVICE_NAME || os.hostname(),
    RUST_BACKTRACE: "1",
  };
  if (testGroqApiKey) {
    backendEnv.KOSMOS_TEST_GROQ_API_KEY = testGroqApiKey;
  } else {
    delete backendEnv.KOSMOS_TEST_GROQ_API_KEY;
  }

  const proc = spawn(exe, [], {
    detached: false,
    stdio: ["ignore", "pipe", "pipe"],
    env: backendEnv,
  });
  proc.stdout?.on("data", (b) => process.stderr.write(`[kepler-backend] ${b.toString()}`));
  proc.stderr?.on("data", (b) => process.stderr.write(`[kepler-backend] ${b.toString()}`));
  return { lockPath, proc };
}

export function killBackendTree(proc: ChildProcess): void {
  const pid = proc.pid;
  console.error(`[kepler-shell] will-quit: killing backend tree pid=${pid}`);
  if (process.platform === "win32" && pid) {
    try {
      spawn("taskkill", ["/T", "/F", "/PID", String(pid)], {
        stdio: "ignore",
        windowsHide: true,
      });
      return;
    } catch (e) {
      console.error("[kepler-shell] taskkill failed:", e);
    }
  }
  proc.kill();
}
