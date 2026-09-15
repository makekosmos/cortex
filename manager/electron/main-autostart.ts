import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { isString } from "./manager-contract";
import type { JsonValue } from "../src/manager-api";

// Windows login-item plumbing for the Manager "Запускать Kosmos при входе"
// toggle. Electron keys Run values by `name`; `openAtLogin:false` deletes the
// whole value regardless of `path`, and `getLoginItemSettings().openAtLogin`
// only inspects the value named by the app user model id — so both reads and
// cleanup must go through `launchItems`, which lists every Run entry matching
// the queried executable path.
export const AUTOSTART_ARGS = ["--autostart"];
export const ENGINE_AUTOSTART_ARGS = ["--start"];
export const AUTOSTART_NAME = "Kosmos";
export const ENGINE_AUTOSTART_NAME = "Kosmos Engine";
// Run-value names written by previous product generations.
export const LEGACY_AUTOSTART_NAMES = [
  "CosCast",
  "com.kazui.kepler",
  "Kepler",
  "KeplerKosmos",
  "KosmosKepler",
];

export interface LaunchItemLike {
  name?: string;
  path?: string;
  args?: string[];
  enabled?: boolean;
}

export interface LoginItemApi {
  getLoginItemSettings(options: { path: string; args: string[] }): {
    launchItems?: LaunchItemLike[];
  };
  setLoginItemSettings(settings: {
    openAtLogin: boolean;
    name?: string;
    path?: string;
    args?: string[];
  }): void;
}

// launchItems match on executable path alone; `args` only feeds the legacy
// openAtLogin comparison, which this module does not use.
function itemsForPath(api: LoginItemApi, exePath: string): LaunchItemLike[] {
  try {
    return api.getLoginItemSettings({ path: exePath, args: [] }).launchItems ?? [];
  } catch {
    return [];
  }
}

function hasActiveEntry(api: LoginItemApi, exePath: string): boolean {
  return itemsForPath(api, exePath).some((item) => item.enabled !== false);
}

// Every executable location a Kosmos-flavoured Run entry could point at, so a
// surviving entry from an older install still reports (and is cleaned) here.
export function autostartExeCandidates(
  appExecutable: string,
  localAppData = process.env.LOCALAPPDATA,
): string[] {
  const dir = path.dirname(appExecutable);
  const candidates = [
    appExecutable,
    path.join(dir, "Kosmos.exe"),
    path.join(dir, "CosCast.exe"),
    path.join(dir, "Kepler.exe"),
  ];
  if (localAppData) {
    candidates.push(path.resolve(localAppData, "Programs", "Kosmos", "Kosmos.exe"));
    candidates.push(path.resolve(localAppData, "Programs", "Kosmos", "CosCast.exe"));
    candidates.push(path.resolve(localAppData, "Programs", "Kepler", "Kepler.exe"));
  }
  return Array.from(new Set(candidates.map((candidate) => candidate.toLowerCase())));
}

// The installed Engine backend registered as `Kosmos Engine ... --start`, or
// the packaged fallback shipped inside the Desktop install.
export function engineAutostartExe(
  appExecutable: string,
  localAppData = process.env.LOCALAPPDATA,
): string {
  try {
    if (localAppData) {
      const root = path.join(localAppData, "Kosmos", "Engine");
      // SAFETY: current.json is untrusted input; version is validated before use.
      const pointer = JSON.parse(readFileSync(path.join(root, "current.json"), "utf8")) as {
        version?: JsonValue;
      };
      if (isString(pointer.version) && /^\d+\.\d+\.\d+$/.test(pointer.version)) {
        const exe = path.join(root, "versions", pointer.version, "kepler-backend.exe");
        if (existsSync(exe)) return exe;
      }
    }
  } catch {
    /* fall through to the packaged path */
  }
  return path.join(path.dirname(appExecutable), "resources", "Kosmos Runtime.exe");
}

export function isAutostartEnabled(
  api: LoginItemApi,
  appExecutable: string,
  engineExe: string,
): boolean {
  const appEnabled = autostartExeCandidates(appExecutable).some((exe) => hasActiveEntry(api, exe));
  if (appEnabled) return true;
  return hasActiveEntry(api, engineExe);
}

export function setAutostartEnabled(
  api: LoginItemApi,
  appExecutable: string,
  engineExe: string,
  enabled: boolean,
): boolean {
  api.setLoginItemSettings({
    openAtLogin: enabled,
    name: AUTOSTART_NAME,
    path: appExecutable,
    args: AUTOSTART_ARGS,
  });
  // Engine autostart is owned by the app toggle here: a surviving entry would
  // still start Kosmos after the user switched it off.
  try {
    api.setLoginItemSettings({ openAtLogin: false, name: ENGINE_AUTOSTART_NAME });
  } catch {
    /* best-effort cleanup */
  }
  for (const name of LEGACY_AUTOSTART_NAMES) {
    try {
      api.setLoginItemSettings({ openAtLogin: false, name });
    } catch {
      /* best-effort cleanup */
    }
  }
  // Any Run entry still pointing at a Kosmos executable under another name is
  // a duplicate owner — remove it so the toggle reflects a single source.
  for (const exe of autostartExeCandidates(appExecutable)) {
    for (const item of itemsForPath(api, exe)) {
      if (!item.name || item.name === AUTOSTART_NAME) continue;
      try {
        api.setLoginItemSettings({ openAtLogin: false, name: item.name });
      } catch {
        /* best-effort cleanup */
      }
    }
  }
  return isAutostartEnabled(api, appExecutable, engineExe);
}
