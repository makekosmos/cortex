import { execFile } from "node:child_process";
import { readFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import { findGameEntry, similarityScore } from "./manifest";
import type {
  BackupInfo,
  FindSavePathInput,
  GameManifest,
  GameManifestEntry,
  SaveDiscovery,
  SaveFile,
  SavePathLookup,
  SaveRoot,
} from "./types";
import { isDirectory, isFile, pathExists, readDirectoryEntries, statPath } from "./utils";

const execFileAsync = promisify(execFile);
const SAVE_PATH_GAME_TOKEN = "{PATHTOGAME}";
const WIN_PATH = path.win32;

interface PathResolutionContext {
  home: string | null;
  documents: string | null;
  appdata: string | null;
  localAppData: string | null;
  localLow: string | null;
  savedGames: string | null;
  publicPath: string | null;
  publicDocuments: string | null;
  programData: string | null;
  steam: string | null;
  steamUserData: string | null;
}

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
      // ignore and try the next location
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

function replaceToken(
  base: string,
  token: string,
  value: string | null,
  missing: { value: boolean },
): string {
  if (!base.includes(token)) {
    return base;
  }
  if (!value) {
    missing.value = true;
    return base;
  }
  return base.replaceAll(token, value);
}

function expandEnvVars(input: string): string {
  return input.replace(/%([^%]+)%/g, (match, key) => process.env[key] ?? match);
}

function expandTilde(input: string, home: string | null): string {
  if (!home || !input.startsWith("~")) {
    return input;
  }
  return `${home}${input.slice(1)}`;
}

function hasGlobChars(segment: string): boolean {
  return /[*?]/.test(segment);
}

function globToRegExp(segment: string): RegExp {
  const escaped = segment.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  const regex = escaped.replace(/\*/g, ".*").replace(/\?/g, ".");
  return new RegExp(`^${regex}$`, "i");
}

async function expandGlobPattern(pattern: string): Promise<string[]> {
  const normalized = pattern.replaceAll("\\", "/");
  const root = WIN_PATH.parse(pattern).root || "";
  const rootNormalized = root.replaceAll("\\", "/");
  const rest = normalized.startsWith(rootNormalized) ? normalized.slice(rootNormalized.length) : normalized;
  const segments = rest.split("/").filter(Boolean);

  let current = [root || pattern.slice(0, 2) || ""].filter(Boolean);
  if (current.length === 0) {
    current = [path.parse(pattern).root || ""].filter(Boolean);
  }

  for (const segment of segments) {
    const next: string[] = [];
    for (const base of current) {
      const candidateBase = base || root || "";
      if (!candidateBase) {
        continue;
      }

      if (!hasGlobChars(segment)) {
        const candidate = WIN_PATH.join(candidateBase, segment);
        if (await pathExists(candidate)) {
          next.push(candidate);
        }
        continue;
      }

      if (!(await pathExists(candidateBase)) || !(await isDirectory(candidateBase))) {
        continue;
      }

      const entries = await readDirectoryEntries(candidateBase);
      const matcher = globToRegExp(segment);
      for (const entry of entries) {
        if (matcher.test(entry.name)) {
          next.push(WIN_PATH.join(candidateBase, entry.name));
        }
      }
    }
    current = next;
    if (current.length === 0) {
      break;
    }
  }

  return current;
}

async function resolvePath(
  rawPath: string,
  context: PathResolutionContext,
): Promise<string[]> {
  let pathText = rawPath;
  const missing = { value: false };

  pathText = replaceToken(pathText, "<home>", context.home, missing);
  pathText = replaceToken(pathText, "<winDocuments>", context.documents, missing);
  pathText = replaceToken(pathText, "<documents>", context.documents, missing);
  pathText = replaceToken(pathText, "<winAppData>", context.appdata, missing);
  pathText = replaceToken(pathText, "<winLocalAppData>", context.localAppData, missing);
  pathText = replaceToken(pathText, "<winLocalAppDataLow>", context.localLow, missing);
  pathText = replaceToken(pathText, "<winLocalLow>", context.localLow, missing);
  pathText = replaceToken(pathText, "<winSavedGames>", context.savedGames, missing);
  pathText = replaceToken(pathText, "<winPublic>", context.publicPath, missing);
  pathText = replaceToken(pathText, "<winPublicDocuments>", context.publicDocuments, missing);
  pathText = replaceToken(pathText, "<winProgramData>", context.programData, missing);
  pathText = replaceToken(pathText, "<steam>", context.steam, missing);
  pathText = replaceToken(pathText, "<steamUserData>", context.steamUserData, missing);
  pathText = replaceToken(pathText, "<steamuserdata>", context.steamUserData, missing);
  pathText = pathText.replaceAll("<storeUserId>", "*");

  if (missing.value) {
    return [];
  }

  pathText = expandEnvVars(pathText);
  pathText = expandTilde(pathText, context.home);

  if (pathText.includes("*") || pathText.includes("?")) {
    return expandGlobPattern(pathText);
  }

  return (await pathExists(pathText)) ? [pathText] : [];
}

async function manifestRoots(
  entry: GameManifestEntry,
  context: PathResolutionContext,
): Promise<string[]> {
  const roots: string[] = [];
  const filesMap = entry.files ?? {};
  for (const paths of Object.values(filesMap)) {
    for (const rawPath of paths) {
      const resolved = await resolvePath(rawPath, context);
      roots.push(...resolved);
    }
  }
  return roots;
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

async function heuristicRoots(gameName: string, context: PathResolutionContext): Promise<string[]> {
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

async function buildRoots(paths: string[]): Promise<SaveRoot[]> {
  const seen = new Set<string>();
  const roots: SaveRoot[] = [];

  for (const p of paths) {
    const normalized = WIN_PATH.normalize(p).toLowerCase();
    if (seen.has(normalized)) {
      continue;
    }
    seen.add(normalized);
    roots.push({
      label: `root-${roots.length}`,
      path: p,
    });
  }

  return roots;
}

async function collectFiles(roots: SaveRoot[]): Promise<SaveDiscovery> {
  const files: SaveFile[] = [];
  const seen = new Set<string>();
  let totalSize = 0;

  for (const root of roots) {
    if (await isFile(root.path)) {
      const size = (await statPath(root.path)).size;
      if (seen.has(WIN_PATH.normalize(root.path))) {
        continue;
      }
      seen.add(WIN_PATH.normalize(root.path));
      files.push({
        path: root.path,
        rootLabel: root.label,
        relativePath: path.basename(root.path),
        size,
      });
      totalSize += size;
      continue;
    }

    if (!(await isDirectory(root.path))) {
      continue;
    }

    const stack = [root.path];
    while (stack.length > 0) {
      const currentDir = stack.pop() as string;
      const entries = await readDirectoryEntries(currentDir);
      for (const entry of entries) {
        const childPath = path.join(currentDir, entry.name);
        if (entry.isDirectory()) {
          stack.push(childPath);
          continue;
        }
        if (!entry.isFile()) {
          continue;
        }

        const normalized = WIN_PATH.normalize(childPath);
        if (seen.has(normalized)) {
          continue;
        }
        seen.add(normalized);

        const metadata = await statPath(childPath);
        files.push({
          path: childPath,
          rootLabel: root.label,
          relativePath: path.relative(root.path, childPath),
          size: metadata.size,
        });
        totalSize += metadata.size;
      }
    }
  }

  return {
    roots,
    files,
    totalSize,
  };
}

export async function findGameSaveRoots(
  gameName: string,
  manifest: GameManifest | null | undefined,
  overridePath?: string | null,
): Promise<SaveRoot[]> {
  const context = await createContext();
  const roots: string[] = [];

  if (overridePath) {
    if (!(await pathExists(overridePath))) {
      throw new Error(`Save path does not exist: ${overridePath}`);
    }
    roots.push(overridePath);
  }

  if (roots.length === 0) {
    const entry = findGameEntry(manifest, gameName);
    if (entry) {
      roots.push(...(await manifestRoots(entry[1], context)));
    }
  }

  if (roots.length === 0) {
    roots.push(...(await heuristicRoots(gameName, context)));
  }

  return buildRoots(roots);
}

export async function findGameSaves(
  gameName: string,
  manifest: GameManifest | null | undefined,
  overridePath?: string | null,
): Promise<SaveDiscovery | null> {
  const roots = await findGameSaveRoots(gameName, manifest, overridePath);
  if (roots.length === 0) {
    return null;
  }

  const discovery = await collectFiles(roots);
  if (discovery.files.length === 0) {
    return null;
  }

  return discovery;
}

export async function findSavePath(
  input: FindSavePathInput,
): Promise<SavePathLookup> {
  const roots = await findGameSaveRoots(
    input.gameName,
    input.manifest,
    input.overridePath ?? null,
  );

  return {
    savePath: roots.length === 1 ? roots[0].path : null,
    candidates: roots.map((root) => root.path),
  };
}

export async function discoverBackupInfo(
  gameName: string,
  manifest: GameManifest | null | undefined,
  overridePath?: string | null,
): Promise<BackupInfo | null> {
  const discovery = await findGameSaves(gameName, manifest, overridePath);
  if (!discovery) {
    return null;
  }

  const firstRoot = discovery.roots[0]?.path ?? null;
  return {
    gameName,
    savePath: discovery.roots.length === 1 ? firstRoot : null,
    registryPath: null,
    totalSize: discovery.totalSize,
    files: discovery.files.map((file) => file.path),
  };
}

export async function resolveSavePathTemplate(
  gameExePath: string,
  rawPath: string,
): Promise<string> {
  if (!rawPath.includes(SAVE_PATH_GAME_TOKEN)) {
    return rawPath;
  }

  const exeDir = path.dirname(gameExePath);
  return rawPath.replaceAll(SAVE_PATH_GAME_TOKEN, exeDir);
}

export async function tokenizeSavePath(
  gameExePath: string,
  savePath: string,
): Promise<string> {
  const savePathText = savePath.trim();
  if (!savePathText || savePathText.includes(SAVE_PATH_GAME_TOKEN)) {
    return savePathText;
  }

  if (!path.isAbsolute(savePathText) || !(await pathExists(savePathText))) {
    return savePathText;
  }

  const gameDir = path.dirname(gameExePath);
  if (!(await pathExists(gameDir))) {
    return savePathText;
  }

  const relative = path.relative(gameDir, savePathText);
  if (!relative || relative.startsWith("..")) {
    return savePathText;
  }

  return relative === "." ? SAVE_PATH_GAME_TOKEN : path.join(SAVE_PATH_GAME_TOKEN, relative);
}
