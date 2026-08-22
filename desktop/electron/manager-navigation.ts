import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { keplerDataDir } from "./data-dir";

export function resolvePackagedManagerExecutable(
  resourcesPath = process.resourcesPath,
  platform = process.platform,
): string | null {
  if (platform !== "win32" || !resourcesPath) return null;
  const candidate = path.join(resourcesPath, "components", "manager", "Kosmos Manager.exe");
  return fs.existsSync(candidate) ? candidate : null;
}

export function openManager(): void {
  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return;
  const executable = process.env.KOSMOS_MANAGER_EXECUTABLE?.trim();
  const managerEnv = {
    ...process.env,
    KOSMOS_APP_EXECUTABLE: process.execPath,
    KOSMOS_UPDATE_STATE_FILE: path.join(keplerDataDir(), "update-state.json"),
  };
  if (executable) {
    const resolved = path.resolve(executable);
    if (fs.existsSync(resolved)) {
      const child = spawn(resolved, [], {
        detached: true,
        stdio: "ignore",
        windowsHide: true,
        env: managerEnv,
      });
      child.unref();
    }
    return;
  }
  const main = process.env.KOSMOS_MANAGER_MAIN?.trim();
  if (main) {
    const resolved = path.resolve(main);
    if (fs.existsSync(resolved)) {
      const child = spawn(process.execPath, [resolved], {
        detached: true,
        stdio: "ignore",
        windowsHide: true,
        env: managerEnv,
      });
      child.unref();
    }
    return;
  }
  const packaged = resolvePackagedManagerExecutable();
  if (packaged) {
    const child = spawn(packaged, [], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
      env: managerEnv,
    });
    child.unref();
  }
}
