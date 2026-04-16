import { access, lstat, mkdir, mkdtemp, readdir, rm, stat } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

const INVALID_FOLDER_CHARS = /[<>:"/\\|?*]/g;

export function sanitizeFolderName(name: string): string {
  const cleaned = name.replace(INVALID_FOLDER_CHARS, "").trim();
  return cleaned.length > 0 ? cleaned : "game";
}

export async function pathExists(targetPath: string): Promise<boolean> {
  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

export async function isDirectory(targetPath: string): Promise<boolean> {
  try {
    return (await stat(targetPath)).isDirectory();
  } catch {
    return false;
  }
}

export async function isFile(targetPath: string): Promise<boolean> {
  try {
    return (await stat(targetPath)).isFile();
  } catch {
    return false;
  }
}

export async function removeBackupArtifact(targetPath: string): Promise<void> {
  if (!(await pathExists(targetPath))) {
    return;
  }

  await rm(targetPath, { recursive: true, force: true });
}

export async function buildUniqueBackupPath(
  backupRoot: string,
  timestamp: string,
  useCompression: boolean,
): Promise<string> {
  let attempt = 0;

  while (true) {
    const suffix = attempt === 0 ? timestamp : `${timestamp}_${attempt}`;
    const candidate = useCompression
      ? path.join(backupRoot, `${suffix}.sqoba.zip`)
      : path.join(backupRoot, suffix);

    if (!(await pathExists(candidate))) {
      return candidate;
    }

    attempt += 1;
  }
}

export async function createTempDir(prefix: string): Promise<string> {
  return mkdtemp(path.join(tmpdir(), prefix));
}

export async function mapLimit<T, R>(
  items: readonly T[],
  limit: number,
  worker: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  const concurrency = Math.max(1, Math.trunc(limit) || 1);
  const results = new Array<R>(items.length);
  let nextIndex = 0;

  const runners = Array.from({ length: Math.min(concurrency, items.length) }, async () => {
    while (true) {
      const current = nextIndex;
      nextIndex += 1;
      if (current >= items.length) {
        return;
      }
      results[current] = await worker(items[current], current);
    }
  });

  await Promise.all(runners);
  return results;
}

export async function readDirectoryEntries(targetPath: string) {
  return readdir(targetPath, { withFileTypes: true });
}

export async function ensureDir(targetPath: string): Promise<void> {
  await mkdir(targetPath, { recursive: true });
}

export async function statPath(targetPath: string) {
  return stat(targetPath);
}

export async function lstatPath(targetPath: string) {
  return lstat(targetPath);
}
