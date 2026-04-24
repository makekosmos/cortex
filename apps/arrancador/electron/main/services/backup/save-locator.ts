import path from "node:path";
import {
  resolveSaveRootPaths,
  SAVE_PATH_GAME_TOKEN,
} from "./save-root-resolver";
import type {
  BackupInfo,
  FindSavePathInput,
  GameManifest,
  SaveDiscovery,
  SaveFile,
  SavePathLookup,
  SaveRoot,
} from "./types";
import { isDirectory, isFile, pathExists, readDirectoryEntries, statPath } from "./utils";

const WIN_PATH = path.win32;

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
  const roots: string[] = [];

  if (overridePath) {
    if (!(await pathExists(overridePath))) {
      throw new Error(`Save path does not exist: ${overridePath}`);
    }
    roots.push(overridePath);
  }

  if (roots.length === 0) {
    roots.push(...(await resolveSaveRootPaths(gameName, manifest)));
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
