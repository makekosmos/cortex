import { spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { app, shell } from "electron";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SAFE_ID = /^[a-z0-9][a-z0-9._-]{0,127}$/;
const HOST_SHORTCUT_ROOT = ["Microsoft", "Windows", "Start Menu", "Programs", "Kosmos"] as const;

export function resolvePackagedHostExecutable(
  resourcesPath = process.resourcesPath,
  platform = process.platform,
): string | null {
  if (platform !== "win32" || !resourcesPath) return null;
  const candidate = path.join(resourcesPath, "components", "host", "Kosmos Package Host.exe");
  return fs.existsSync(candidate) ? candidate : null;
}

function hostShortcutPath(id: string): string | null {
  if (process.platform !== "win32" || !SAFE_ID.test(id)) return null;
  const root = path.join(app.getPath("appData"), ...HOST_SHORTCUT_ROOT);
  const shortcut = path.join(root, `${id}.lnk`);
  try {
    if (!fs.statSync(shortcut).isFile()) return null;
  } catch {
    return null;
  }
  return shortcut;
}

/** Open a Package v1 app in the optional Desktop Host process. */
export async function openHostedApp(id: string): Promise<void> {
  if (process.env.KOSMOS_HEADLESS === "1" || process.env.KOSMOS_TEST_MODE === "1") return;
  if (!SAFE_ID.test(id)) {
    console.warn("[kepler-shell] Hosted App identifier is invalid");
    return;
  }
  const executable = process.env.KOSMOS_HOST_EXECUTABLE?.trim();
  if (executable) {
    const resolved = path.resolve(executable);
    if (!fs.existsSync(resolved)) {
      console.warn("[kepler-shell] Host executable is unavailable");
      return;
    }
    const child = spawn(resolved, [`--open-app=${id}`], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    child.unref();
    return;
  }
  if (process.env.KOSMOS_HOST_MAIN) {
    const hostMain = path.resolve(process.env.KOSMOS_HOST_MAIN);
    if (!fs.existsSync(hostMain))
      return console.warn("[kepler-shell] Host development entry is unavailable");
    const child = spawn(process.execPath, [hostMain, `--open-app=${id}`], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    child.unref();
    return;
  }
  if (app.isPackaged) {
    const packaged = resolvePackagedHostExecutable();
    if (packaged) {
      const child = spawn(packaged, [`--open-app=${id}`], {
        detached: true,
        stdio: "ignore",
        windowsHide: true,
      });
      child.unref();
      return;
    }
  }
  if (app.isPackaged) {
    const shortcut = hostShortcutPath(id);
    if (!shortcut) {
      console.warn("[kepler-shell] Host Start Menu shortcut is unavailable");
      return;
    }
    try {
      const error = await shell.openPath(shortcut);
      if (error) console.warn("[kepler-shell] Host Start Menu shortcut failed");
    } catch {
      console.warn("[kepler-shell] Host Start Menu shortcut failed");
    }
    return;
  }
  if (!app.isPackaged) {
    const hostMain = path.resolve(__dirname, "../../host/dist-electron/main.js");
    if (!fs.existsSync(hostMain))
      return console.warn("[kepler-shell] Host development entry is unavailable");
    const child = spawn(process.execPath, [hostMain, `--open-app=${id}`], {
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });
    child.unref();
    return;
  }
}
