import fs from "node:fs";
import path from "node:path";
import type { ChildProcess } from "node:child_process";
import type { ElectronApplication } from "playwright";
import {
  cargoTarget,
  closeHost,
  executableName,
  processTreePids,
  recordCleanup,
  terminate,
  waitForPidGone,
} from "./host-runtime";

export async function cleanupArcadiaE2e(options: {
  host?: ElectronApplication;
  engine?: ChildProcess;
  restartedEngine?: ChildProcess;
  dataDir: string;
  root: string;
  cleanupManifest: string;
  pids: Set<number>;
}) {
  const { host, engine, restartedEngine, dataDir, root, cleanupManifest, pids } = options;
  const errors: unknown[] = [];
  const attempt = async (action: () => Promise<void>) => {
    try {
      await action();
    } catch (error) {
      errors.push(error);
    }
  };
  const engineBinary = path.join(cargoTarget(), "debug", executableName("kepler-backend"));
  if (host) pids.add(host.process().pid);
  if (restartedEngine?.pid) for (const pid of processTreePids(restartedEngine.pid)) pids.add(pid);
  if (engine?.pid) for (const pid of processTreePids(engine.pid)) pids.add(pid);
  await attempt(() => closeHost(host, pids));
  await attempt(() => terminate(restartedEngine, engineBinary, dataDir, "restarted Engine"));
  await attempt(() => terminate(engine, engineBinary, dataDir, "Engine"));
  for (const pid of pids) await attempt(() => waitForPidGone(pid, "recorded teardown process"));
  try {
    recordCleanup(cleanupManifest, root, pids);
  } catch (error) {
    errors.push(error);
  }
  try {
    const deadline = Date.now() + 30_000;
    while (true) {
      try {
        await fs.promises.rm(root, {
          recursive: true,
          force: true,
          maxRetries: 50,
          retryDelay: 100,
        });
      } catch (error) {
        if (Date.now() >= deadline) throw error;
      }
      if (!fs.existsSync(root)) break;
      if (Date.now() >= deadline) throw new Error(`Arcadia E2E cleanup root remains: ${root}`);
      // ponytail: bounded retry for Windows handle races; replace with ownership tracking if this persists.
      await new Promise((resolve) => setTimeout(resolve, 100));
    }
  } catch (error) {
    errors.push(error);
  }
  if (errors.length) throw new AggregateError(errors, "Arcadia E2E cleanup failed");
}
