import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { findGameEntry, similarityScore } from "./manifest";
import {
  type PathResolutionContext,
  resolveManifestRoots,
} from "./path-expansion";
import type { GameManifest } from "./types";
import { pathExists, readDirectoryEntries } from "./utils";

const execFileAsync = promisify(execFile);
const WIN_PATH = path.win32;

export const SAVE_PATH_GAME_TOKEN = "{PATHTOGAME}";

async function queryRegistryInstallPath(): Promise<string | null> {
  if (process.platform !== "win32") {
    return null;
  }

  const queries = [
    "HKLM\\SOFTWARE\\Wow6432Node\\Valve\\Steam",
    "HKLM\\SOFTWARE\\Valve\\Steam",
  ];

  for (const key of queries) {
    try {
      const { stdout } = await execFileAsync("reg", ["query", key, "/v", "InstallPath"], {
        windowsHide: true,
        maxBuffer: 1024 * 1024,
      });

      const lines = stdout.split(/\r?\n/);
      for (const line of lines) {
        const match = line.match(/InstallPath\s+REG_SZ\s+(.+)$/i);
        if (match?.[1]) {
          const value = match[1].trim();
          if (value) {
            return value;
          }
        }
      }
    } catch {
      // Ignore and try the next location.
    }
  }

  return null;
}

async function findSteamPath(): Promise<string | null> {
  if (process.platform !== "win32") {
    return null;
  }

  const registry = await queryRegistryInstallPath();
  if (registry && (await pathExists(registry))) {
    return registry;
  }

  const candidates = [
    process.env["ProgramFiles(x86)"]
      ? path.join(process.env["ProgramFiles(x86)"] as string, "Steam")
      : null,
    process.env.ProgramFiles ? path.join(process.env.ProgramFiles, "Steam") : null,
    "C:\\Program Files (x86)\\Steam",
    "C:\\Program Files\\Steam",
  ].filter((candidate): candidate is string => Boolean(candidate));

  for (const candidate of candidates) {
    if (await pathExists(candidate)) {
      return candidate;
    }
  }

  return null;
}

async function createContext(): Promise<PathResolutionContext> {
  const home = os.homedir() || null;
  const documents = process.env.USERPROFILE ? path.join(process.env.USERPROFILE, "Documents") : null;
  const appdata = process.env.APPDATA || null;
  const localAppData = process.env.LOCALAPPDATA || null;
  const localLow = localAppData ? path.join(path.dirname(localAppData), "LocalLow") : null;
  const savedGames = home ? path.join(home, "Saved Games") : null;
  const publicPath = process.env.PUBLIC || null;
  const publicDocuments = publicPath ? path.join(publicPath, "Documents") : null;
  const programData = process.env.ProgramData || null;
  const steam = await findSteamPath();
  const steamUserData = steam ? path.join(steam, "userdata") : null;

  return {
    home,
    documents,
    appdata,
    localAppData,
    localLow,
    savedGames,
    publicPath,
    publicDocuments,
    programData,
    steam,
    steamUserData,
  };
}

function sanitizeName(name: string): string {
  return name
    .replace(/[<>:"/\\|?*]/g, "")
    .trim()
    .replace(/\s+/g, " ");
}

function candidateNames(gameName: string): string[] {
  const out = new Set<string>();
  const base = sanitizeName(gameName);
  const normalized = sanitizeName(gameName.toLowerCase());
  const collapsed = base.replace(/\s+/g, "");

  for (const value of [base, normalized, collapsed]) {
    if (value.trim()) {
      out.add(value.trim());
    }
  }

  return [...out];
}

async function findNamedPaths(base: string, names: string[]): Promise<string[]> {
  const out: string[] = [];
  for (const name of names) {
    const candidate = path.join(base, name);
    if (await pathExists(candidate)) {
      out.push(candidate);
    }
  }
  return out;
}

async function findWindowsStorePaths(
  localAppData: string,
  gameName: string,
): Promise<string[]> {
  const packagesRoot = path.join(localAppData, "Packages");
  if (!(await pathExists(packagesRoot))) {
    return [];
  }

  const normalized = sanitizeName(gameName).replace(/\s+/g, "").toLowerCase();
  if (!normalized) {
    return [];
  }

  const matches: string[] = [];
  const entries = await readDirectoryEntries(packagesRoot);
  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const name = entry.name.toLowerCase();
    if (!name.includes(normalized)) {
      continue;
    }
    const root = path.join(packagesRoot, entry.name);
    for (const candidate of [
      path.join(root, "SystemAppData", "wgs"),
      path.join(root, "SystemAppData", "xgs"),
      path.join(root, "LocalState"),
    ]) {
      if (await pathExists(candidate)) {
        matches.push(candidate);
      }
    }
  }

  return matches;
}

async function readLibraryFolders(steamPath: string): Promise<string[]> {
  const libraryFile = path.join(steamPath, "steamapps", "libraryfolders.vdf");
  if (!(await pathExists(libraryFile))) {
    return [];
  }

  const text = await readFile(libraryFile, "utf8");
  const out: string[] = [];
  for (const line of text.split(/\r?\n/)) {
    const parts = line.split("\"");
    if (parts.length >= 4 && parts[1] === "path") {
      out.push(parts[3].replace(/\\\\/g, "\\"));
    }
  }

  return out;
}

async function findSteamLibraryPaths(steamPath: string): Promise<string[]> {
  const folders = [steamPath, ...(await readLibraryFolders(steamPath))];
  const seen = new Set<string>();
  const out: string[] = [];
  for (const folder of folders) {
    const normalized = WIN_PATH.normalize(folder);
    if (!seen.has(normalized)) {
      seen.add(normalized);
      out.push(folder);
    }
  }
  return out;
}

function findAcfValue(text: string, key: string): string | null {
  for (const line of text.split(/\r?\n/)) {
    const parts = line.split("\"");
    if (parts.length >= 4 && parts[1] === key) {
      return parts[3];
    }
  }
  return null;
}

function normalizeNameForMatch(name: string): string {
  const cleaned = name.toLowerCase().replace(/[^a-z0-9]+/g, " ");
  const stopWords = new Set([
    "the",
    "a",
    "an",
    "edition",
    "definitive",
    "remastered",
    "goty",
    "game",
    "of",
    "year",
    "ultimate",
    "complete",
    "collection",
    "bundle",
    "deluxe",
    "enhanced",
    "hd",
  ]);

  return cleaned
    .split(/\s+/)
    .map((token) => token.trim())
    .filter((token) => token.length > 0 && !stopWords.has(token))
    .join(" ");
}

async function findSteamAppIds(
  gameName: string,
  libraryPaths: string[],
): Promise<string[]> {
  const target = normalizeNameForMatch(gameName);
  const appIds: string[] = [];
  const seen = new Set<string>();

  for (const library of libraryPaths) {
    const steamapps = path.join(library, "steamapps");
    if (!(await pathExists(steamapps))) {
      continue;
    }

    const entries = await readDirectoryEntries(steamapps);
    for (const entry of entries) {
      if (!entry.name.endsWith(".acf")) {
        continue;
      }

      const stem = entry.name.slice(0, -4);
      if (!stem.startsWith("appmanifest_")) {
        continue;
      }
      const appId = stem.slice("appmanifest_".length);
      if (seen.has(appId)) {
        continue;
      }

      const manifestPath = path.join(steamapps, entry.name);
      const text = await readFile(manifestPath, "utf8").catch(() => null);
      const name = text ? findAcfValue(text, "name") : null;
      if (!name) {
        continue;
      }

      const normalized = normalizeNameForMatch(name);
      if (similarityScore(target, normalized) >= 0.7) {
        seen.add(appId);
        appIds.push(appId);
      }
    }
  }

  return appIds;
}

async function findSteamSavePaths(gameName: string): Promise<string[]> {
  const steamPath = await findSteamPath();
  if (!steamPath) {
    return [];
  }

  const libraryPaths = await findSteamLibraryPaths(steamPath);
  const appIds = await findSteamAppIds(gameName, libraryPaths);
  if (appIds.length === 0) {
    return [];
  }

  const userdataRoot = path.join(steamPath, "userdata");
  if (!(await pathExists(userdataRoot))) {
    return [];
  }

  const out: string[] = [];
  const users = await readDirectoryEntries(userdataRoot);
  for (const user of users) {
    if (!user.isDirectory()) {
      continue;
    }
    const userPath = path.join(userdataRoot, user.name);
    for (const appId of appIds) {
      const appRoot = path.join(userPath, appId);
      if (!(await pathExists(appRoot))) {
        continue;
      }
      for (const candidate of [path.join(appRoot, "remote"), path.join(appRoot, "local"), appRoot]) {
        if (await pathExists(candidate)) {
          out.push(candidate);
          break;
        }
      }
    }
  }

  return out;
}

async function heuristicRoots(
  gameName: string,
  context: PathResolutionContext,
): Promise<string[]> {
  const variants = candidateNames(gameName);
  const roots: string[] = [];

  if (context.documents) {
    roots.push(...(await findNamedPaths(path.join(context.documents, "My Games"), variants)));
    roots.push(...(await findNamedPaths(path.join(context.documents, "Saved Games"), variants)));
    roots.push(...(await findNamedPaths(context.documents, variants)));
  }

  if (context.savedGames) {
    roots.push(...(await findNamedPaths(context.savedGames, variants)));
  }

  if (context.appdata) {
    roots.push(...(await findNamedPaths(context.appdata, variants)));
  }

  if (context.localAppData) {
    roots.push(...(await findNamedPaths(context.localAppData, variants)));
    roots.push(...(await findWindowsStorePaths(context.localAppData, gameName)));
  }

  if (context.localLow) {
    roots.push(...(await findNamedPaths(context.localLow, variants)));
  }

  roots.push(...(await findSteamSavePaths(gameName)));
  return roots;
}

export async function resolveSaveRootPaths(
  gameName: string,
  manifest: GameManifest | null | undefined,
): Promise<string[]> {
  const context = await createContext();
  const entry = findGameEntry(manifest, gameName);

  if (entry) {
    const roots = await resolveManifestRoots(entry[1], context);
    if (roots.length > 0) {
      return roots;
    }
  }

  return await heuristicRoots(gameName, context);
}
