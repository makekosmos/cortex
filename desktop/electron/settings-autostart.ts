import path from "node:path";

export const AUTOSTART_ARGS: string[] = ["--autostart"];
export const AUTOSTART_NAME = "Kosmos";
export const LEGACY_AUTOSTART_NAMES = [
  "CosCast",
  "com.kazui.kepler",
  "Kepler",
  "KeplerKosmos",
  "KosmosKepler",
];

export type WindowsLaunchItem = {
  name?: string;
  path?: string;
  args?: string[];
  enabled?: boolean;
};

function normalizeWinExecutablePath(value: string): string {
  return path.normalize(value).toLowerCase();
}

function sameArgs(actual: string[] | undefined, expected: string[]): boolean {
  if (!actual) return expected.length === 0;
  return actual.length === expected.length && actual.every((arg, i) => arg === expected[i]);
}

export function launchItemMatchesAutostart(
  item: WindowsLaunchItem,
  execPath = process.execPath,
): boolean {
  if (!item.path || item.enabled === false) return false;
  return (
    normalizeWinExecutablePath(item.path) === normalizeWinExecutablePath(execPath) &&
    sameArgs(item.args, AUTOSTART_ARGS)
  );
}

export function legacyAutostartPathCandidates(
  execPath = process.execPath,
  localAppData = process.env.LOCALAPPDATA,
): string[] {
  const dir = path.dirname(execPath);
  const paths = [
    path.join(dir, "CosCast.exe"),
    path.join(dir, "Kosmos.exe"),
    path.join(dir, "Kepler.exe"),
  ];
  if (localAppData) {
    paths.push(path.resolve(localAppData, "Programs", "Kosmos", "CosCast.exe"));
    paths.push(path.resolve(localAppData, "Programs", "Kosmos", "Kosmos.exe"));
    paths.push(path.resolve(localAppData, "Programs", "Kepler", "Kepler.exe"));
  }
  return Array.from(new Set(paths));
}
