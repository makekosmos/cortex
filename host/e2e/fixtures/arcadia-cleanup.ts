import fs from "node:fs";
import path from "node:path";
import type { ChildProcess } from "node:child_process";
import type { ElectronApplication } from "playwright";
import {
  cargoTarget,
  closeHost,
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
  const engineBinary = path.join(cargoTarget(), "debug", "kepler-backend.exe");
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
    fs.rmSync(root, { recursive: true, force: true, maxRetries: 50, retryDelay: 100 });
    if (fs.existsSync(root)) throw new Error(`Arcadia E2E cleanup root remains: ${root}`);
  } catch (error) {
    errors.push(error);
  }
  if (errors.length) throw new AggregateError(errors, "Arcadia E2E cleanup failed");
}
