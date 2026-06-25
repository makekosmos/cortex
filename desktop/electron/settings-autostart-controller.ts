import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { app } from "electron";
import { resolveInstance } from "./instance";
import {
  AUTOSTART_ARGS,
  AUTOSTART_NAME,
  LEGACY_AUTOSTART_NAMES,
  launchItemMatchesAutostart,
  legacyAutostartPathCandidates,
} from "./settings-autostart";

const execFileAsync = promisify(execFile);

function isLegacyAutostartEnabled(): boolean {
  if (process.platform !== "win32") return false;
  return legacyAutostartPathCandidates().some((legacyPath) => {
    try {
      return app.getLoginItemSettings({
        path: legacyPath,
        args: AUTOSTART_ARGS,
      }).openAtLogin;
    } catch {
      return false;
    }
  });
}

async function removeLegacyAutostartEntries(): Promise<void> {
  if (process.platform !== "win32") return;
  for (const legacyPath of legacyAutostartPathCandidates()) {
    for (const name of LEGACY_AUTOSTART_NAMES) {
      try {
        app.setLoginItemSettings({
          openAtLogin: false,
          name,
          path: legacyPath,
          args: AUTOSTART_ARGS,
        });
      } catch {
        /* best-effort cleanup */
      }
    }
  }

  try {
    await execFileAsync(
      "reg.exe",
      ["delete", "HKCU\\Software\\Microsoft\\Windows\\CurrentVersion\\Run", "/v", "Kepler", "/f"],
      { windowsHide: true },
    );
  } catch {
    /* value absent or registry unavailable */
  }
}

export function isAutostartEnabled(): boolean {
  const settings = app.getLoginItemSettings({
    path: process.execPath,
    args: AUTOSTART_ARGS,
  });
  const launchItems = Array.isArray(settings.launchItems) ? settings.launchItems : [];
  return (
    settings.openAtLogin ||
    launchItems.some((item) => launchItemMatchesAutostart(item)) ||
    isLegacyAutostartEnabled()
  );
}

export async function setAutostartEnabled(enabled: boolean): Promise<void> {
  if (!resolveInstance().autorunEnabled) {
    console.warn(
      `[kepler-shell] autostart toggle ignored for slot ${resolveInstance().slot} (only prod)`,
    );
    return;
  }
  app.setLoginItemSettings({
    openAtLogin: enabled,
    name: AUTOSTART_NAME,
    path: process.execPath,
    args: AUTOSTART_ARGS,
  });
  await removeLegacyAutostartEntries();
  try {
    const verify = app.getLoginItemSettings({
      path: process.execPath,
      args: AUTOSTART_ARGS,
    });
    const launchItems = Array.isArray(verify.launchItems) ? verify.launchItems : [];
    const launchItemVerified = launchItems.some((item) => launchItemMatchesAutostart(item));
    console.log(
      `[kepler-shell] autostart set → enabled=${enabled}, verified openAtLogin=${verify.openAtLogin}, launchItem=${launchItemVerified}, execPath=${process.execPath}`,
    );
  } catch (e) {
    console.warn("[kepler-shell] autostart verify failed:", e);
  }
}

export function isAutostartAllowed(): boolean {
  return resolveInstance().autorunEnabled;
}
