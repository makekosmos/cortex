import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { app } from "electron";
import { keplerDataDir } from "./data-dir";

export function resolvePackagedMemoriaExecutable(
  resourcesPath = process.resourcesPath,
  platform = process.platform,
): string | null {
  if (platform !== "win32" || !resourcesPath) return null;
  const candidate = path.join(resourcesPath, "components", "memoria", "Kosmos Memoria.exe");
  return fs.existsSync(candidate) ? candidate : null;
}

// Launch contract mirrors openManager()/openAgenda(): the packaged GPUI
// Memoria is a sibling component, so the child inherits the instance data
// dir through KOSMOS_DATA_DIR and discovers the same engine.lock.json as
// Manager. KOSMOS_MEMORIA_EXECUTABLE overrides the binary for dev/local runs.
export function openMemoria(): void {
  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return;
  const executable = process.env.KOSMOS_MEMORIA_EXECUTABLE?.trim();
  const memoriaEnv = {
    ...process.env,
    KOSMOS_DATA_DIR: keplerDataDir(),
    KOSMOS_APP_EXECUTABLE: process.execPath,
    KOSMOS_DESKTOP_VERSION: app.isPackaged ? app.getVersion() : "",
  };
  if (executable) {
    const resolved = path.resolve(executable);
    if (fs.existsSync(resolved)) {
      const child = spawn(resolved, [], {
        detached: true,
        stdio: "ignore",
        windowsHide: true,
        env: memoriaEnv,
      });
      child.unref();
    } else {
      console.warn(`[kepler-shell] KOSMOS_MEMORIA_EXECUTABLE not found: ${resolved}`);
    }
    return;
  }
  const packaged = resolvePackagedMemoriaExecutable();
  if (packaged) {
    const child = spawn(packaged, [], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
      env: memoriaEnv,
    });
    child.unref();
  } else {
    console.warn("[kepler-shell] components/memoria/Kosmos Memoria.exe is not packaged");
  }
}
