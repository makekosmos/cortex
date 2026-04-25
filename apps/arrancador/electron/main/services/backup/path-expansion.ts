import path from "node:path";
import type { GameManifestEntry } from "./types";
import { isDirectory, pathExists, readDirectoryEntries } from "./utils";

const WIN_PATH = path.win32;

export interface PathResolutionContext {
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
  const rest = normalized.startsWith(rootNormalized)
    ? normalized.slice(rootNormalized.length)
    : normalized;
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

export async function resolveRootPathPattern(
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

export async function resolveManifestRoots(
  entry: GameManifestEntry,
  context: PathResolutionContext,
): Promise<string[]> {
  const roots: string[] = [];
  const filesMap = entry.files ?? {};
  for (const paths of Object.values(filesMap)) {
    for (const rawPath of paths) {
      const resolved = await resolveRootPathPattern(rawPath, context);
      roots.push(...resolved);
    }
  }
  return roots;
}
