import { existsSync, readFileSync, realpathSync } from "node:fs";
import path from "node:path";

export const AUTOSTART_ARGS: string[] = ["--start"];
export const AUTOSTART_NAME = "Kosmos Engine";
export const LEGACY_AUTOSTART_NAMES = [
  "CosCast",
  "com.kazui.kepler",
  "Kepler",
  "KeplerKosmos",
  "KosmosKepler",
  "Kosmos",
];

export function engineAutostartPath(execPath = process.execPath): string {
  const configuredRoot = process.env.KOSMOS_ENGINE_ROOT?.trim();
  const root =
    configuredRoot ||
    (process.env.LOCALAPPDATA ? path.join(process.env.LOCALAPPDATA, "Kosmos", "Engine") : "");
  const fallback = path.join(path.dirname(execPath), "resources", "Kosmos Runtime.exe");
  try {
    const pointer = JSON.parse(readFileSync(path.join(root, "current.json"), "utf8")) as {
      schema_version?: unknown;
      version?: unknown;
    };
    if (pointer.schema_version !== 1 || !isEngineVersion(pointer.version)) return fallback;
    const engineRoot = path.resolve(root);
    const versionsRoot = path.resolve(engineRoot, "versions");
    const backend = path.resolve(versionsRoot, pointer.version, "kepler-backend.exe");
    if (!isWithinRoot(backend, versionsRoot) || !existsSync(backend)) return fallback;
    const realVersionsRoot = realpathSync(versionsRoot);
    const realBackend = realpathSync(backend);
    if (isWithinRoot(realBackend, realVersionsRoot)) return realBackend;
  } catch {}
  return fallback;
}

function isEngineVersion(value: unknown): value is string {
  return typeof value === "string" && /^\d+\.\d+\.\d+$/.test(value);
}

function isWithinRoot(candidate: string, root: string): boolean {
  const relative = path.relative(root, candidate);
  return (
    relative !== "" &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
}

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
