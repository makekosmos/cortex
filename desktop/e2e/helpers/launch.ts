import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

const appRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "../..");
export function freshDataDir(prefix: string): string {
  return fs.mkdtempSync(path.join(os.tmpdir(), `cortex-${prefix}-`));
}
export async function launchKeplerWithDataDir(dataDir: string): Promise<ElectronApplication> {
  const userDataDir = fs.mkdtempSync(path.join(os.tmpdir(), "cortex-electron-"));
  return electron.launch({
    executablePath: electronBinary,
    cwd: appRoot,
    args: [path.join(appRoot, "dist-electron", "main.js"), `--user-data-dir=${userDataDir}`],
    env: {
      ...process.env,
      NODE_ENV: "test",
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_TEST_MODE: "1",
      KOSMOS_HEADLESS: "1",
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
    },
    timeout: 20_000,
  });
}
export function shutdownKeplerEngine(dataDir: string): void {
  try {
    // SAFETY: the isolated test lock is written by the backend in this synthetic dataDir.
    const pid = (
      JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as {
        pid?: number;
      }
    ).pid;
    if (Number.isInteger(pid)) process.kill(pid);
  } catch {
    /* isolated test data */
  }
}
