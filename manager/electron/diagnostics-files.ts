import { lstat, mkdir, readdir, unlink } from "node:fs/promises";
import path from "node:path";

export type CrashReportMetadata = { name: string; size: number; mtime: string };

export function resolveManagerDataDir(appDataPath: string): string {
  return process.env.KOSMOS_DATA_DIR || path.join(appDataPath, "Kosmos");
}

function allowed(name: string): boolean {
  return name.length > 0 && name.length <= 256 && /\.(?:log|dmp)$/i.test(name);
}

function normalizeMtime(value: Date): string {
  const iso = value.toISOString();
  return iso.length <= 64 ? iso : iso.slice(0, 64);
}

async function diagnosticsDir(root: string, kind: "logs" | "crashes"): Promise<string | null> {
  const dir = path.join(root, kind);
  try {
    const stat = await lstat(dir);
    if (!stat.isDirectory() || stat.isSymbolicLink())
      throw new Error("diagnostics root is not a real directory");
    return dir;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
    throw error;
  }
}

export async function listCrashReports(root: string): Promise<CrashReportMetadata[]> {
  const dir = await diagnosticsDir(root, "crashes");
  if (!dir) return [];
  const names = await readdir(dir);
  const result: CrashReportMetadata[] = [];
  for (const name of names) {
    if (!allowed(name)) continue;
    try {
      const stat = await lstat(path.join(dir, name));
      if (!stat.isFile() || stat.size < 0 || stat.size > Number.MAX_SAFE_INTEGER) continue;
      result.push({ name, size: stat.size, mtime: normalizeMtime(stat.mtime) });
    } catch {
      // Files can disappear while the directory is being inspected.
    }
  }
  return result.sort((a, b) => b.mtime.localeCompare(a.mtime));
}

export async function clearCrashReports(root: string): Promise<number> {
  const dir = await diagnosticsDir(root, "crashes");
  if (!dir) return 0;
  const names = await readdir(dir);
  let removed = 0;
  for (const name of names) {
    if (!allowed(name)) continue;
    try {
      const stat = await lstat(path.join(dir, name));
      if (!stat.isFile()) continue;
      await unlink(path.join(dir, name));
      removed += 1;
    } catch {
      // Best effort: never recurse or follow links.
    }
  }
  return removed;
}

export async function ensureDiagnosticsDir(root: string, kind: "logs" | "crashes") {
  const dir = path.join(root, kind);
  const existing = await diagnosticsDir(root, kind);
  if (existing) return existing;
  await mkdir(dir, { recursive: true });
  if (!(await diagnosticsDir(root, kind))) throw new Error("diagnostics root was not created");
  return dir;
}
