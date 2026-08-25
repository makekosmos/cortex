import { existsSync, renameSync } from "node:fs";
// SAFETY: The surrounding boundary establishes this documented contract.
import * as fs from "node:fs/promises";
import path from "node:path";
import { keplerDataDir } from "./data-dir";

interface ExtensionManifest {
  id: string;
  appId?: string;
  name: string;
  kind?: "app" | "native" | string;
  version?: string;
  description?: string;
  author?: string;
  icon?: string;
  entryHtml?: string;
  keplerApiVersion?: string;
  keepAliveInBackground?: boolean;
  native?: {
    executable?: string;
    devExecutable?: string;
  };
}

export interface InstalledExtensionInfo {
  id: string;
  /** Immutable application identity. Null only for pre-appId extensions. */
  appId: string | null;
  name: string;
  kind: ExtensionManifest["kind"] | null;
  version: string | null;
  description: string | null;
  author: string | null;
  iconDataUri: string | null;
  backupCount: number;
  backupTimestamps: string[];
  source: "installed" | "dev";
}

const MAX_BACKUPS = 5;

function copyDirAsync(src: string, dst: string): Promise<void> {
  // dereference: false (default) keeps the old backup behavior for symlinks.
  return fs.cp(src, dst, { recursive: true });
}

function userExtensionsRoot(): string {
  return path.join(keplerDataDir(), "extensions");
}

function extensionsBackupsRoot(): string {
  return path.join(keplerDataDir(), "extensions-backups");
}

function extensionsTmpRoot(): string {
  return path.join(keplerDataDir(), "extensions-tmp");
}

async function readManifestSafe(dir: string): Promise<ExtensionManifest | null> {
  const manifestPath = path.join(dir, "manifest.json");
  if (!existsSync(manifestPath)) return null;
  try {
// SAFETY: The surrounding boundary establishes this documented contract.
    return JSON.parse(await fs.readFile(manifestPath, "utf8")) as ExtensionManifest;
  } catch {
    return null;
  }
}

async function readIconDataUri(dir: string, manifest: ExtensionManifest): Promise<string | null> {
  if (!manifest.icon) return null;
  const iconPath = path.join(dir, manifest.icon);
  if (!existsSync(iconPath)) return null;
  const ext = path.extname(manifest.icon).toLowerCase();
  const mime =
    ext === ".svg"
      ? "image/svg+xml"
      : ext === ".jpg" || ext === ".jpeg"
        ? "image/jpeg"
        : "image/png";
  return `data:${mime};base64,${(await fs.readFile(iconPath)).toString("base64")}`;
}

async function pruneBackups(backupsRoot: string): Promise<void> {
  if (!existsSync(backupsRoot)) return;
  const entries: string[] = [];
  try {
    for (const n of await fs.readdir(backupsRoot)) {
      try {
        const stat = await fs.stat(path.join(backupsRoot, n));
        if (stat.isDirectory()) entries.push(n);
      } catch {
        // skip unreadable entry
      }
    }
  } catch {
    return;
  }
  entries.sort();
  while (entries.length > MAX_BACKUPS) {
    const old = entries.shift()!;
    try {
      await fs.rm(path.join(backupsRoot, old), { recursive: true, force: true });
    } catch (e) {
      console.warn(`[ext:install] prune backup ${old} failed:`, e);
    }
  }
}

export async function backupExtension(id: string): Promise<string | null> {
  const currentDir = path.join(userExtensionsRoot(), id);
  if (!existsSync(currentDir)) return null;
  const backupsRoot = path.join(extensionsBackupsRoot(), id);
  await fs.mkdir(backupsRoot, { recursive: true });
  const timestamp = new Date().toISOString().replace(/[:.]/g, "-");
  const target = path.join(backupsRoot, timestamp);
  await copyDirAsync(currentDir, target);
  await pruneBackups(backupsRoot);
  return target;
}

export async function listBackups(id: string): Promise<string[]> {
  const root = path.join(extensionsBackupsRoot(), id);
  if (!existsSync(root)) return [];
  const entries: string[] = [];
  try {
    for (const n of await fs.readdir(root)) {
      try {
        const stat = await fs.stat(path.join(root, n));
        if (stat.isDirectory()) entries.push(n);
      } catch {
        // skip unreadable entry
      }
    }
  } catch {
    return [];
  }
  return entries.sort().reverse();
}

export async function revertExtension(id: string, timestamp: string | undefined): Promise<boolean> {
  const backups = await listBackups(id);
  if (backups.length === 0) return false;
  const chosen = timestamp ?? backups[0]!;
  const backupDir = path.join(extensionsBackupsRoot(), id, chosen);
  if (!existsSync(path.join(backupDir, "manifest.json"))) {
    throw new Error(`backup not found or corrupt: ${id}/${chosen}`);
  }
  const target = path.join(userExtensionsRoot(), id);

  if (existsSync(target)) {
    await backupExtension(id);
  }

  const stamp = Date.now().toString(36) + Math.random().toString(36).slice(2, 6);
  const tmpDir = path.join(extensionsTmpRoot(), `${id}-revert-${stamp}`);
  await fs.mkdir(path.dirname(tmpDir), { recursive: true });

  try {
    await copyDirAsync(backupDir, tmpDir);
    let oldDir: string | null = null;
    if (existsSync(target)) {
      oldDir = `${target}.old-${stamp}`;
      renameSync(target, oldDir);
    }
    try {
      renameSync(tmpDir, target);
    } catch (e) {
      if (oldDir && !existsSync(target)) renameSync(oldDir, target);
      throw e;
    }
    if (oldDir && existsSync(oldDir)) {
      await fs.rm(oldDir, { recursive: true, force: true });
    }
    return true;
  } catch (e) {
    await fs.rm(tmpDir, { recursive: true, force: true });
    throw e;
  }
}

async function scanExtensionsDir(
  root: string,
  source: "installed" | "dev",
): Promise<InstalledExtensionInfo[]> {
  if (!existsSync(root)) return [];
  const out: InstalledExtensionInfo[] = [];
  let entries: string[];
  try {
    entries = await fs.readdir(root);
  } catch {
    return [];
  }
  for (const id of entries) {
    const dir = path.join(root, id);
    let stat;
    try {
      stat = await fs.stat(dir);
    } catch {
      continue;
    }
    if (!stat.isDirectory()) continue;
    const manifest = await readManifestSafe(dir);
    if (!manifest) continue;
    if (source === "dev") {
      if (manifest.kind === "native") {
        const native = manifest.native;
        const rel = native?.devExecutable ?? native?.executable;
        if (!rel || !existsSync(path.resolve(dir, rel))) continue;
      } else {
        const entryHtml = manifest.entryHtml ?? "dist/index.html";
        const entryPath = path.join(dir, entryHtml);
        if (!existsSync(entryPath)) continue;
      }
    }
    const iconDataUri = await readIconDataUri(dir, manifest);
    const backups = source === "installed" ? await listBackups(id) : [];
    out.push({
      id,
      appId: manifest.appId ?? null,
      name: manifest.name,
      kind: manifest.kind ?? null,
      version: manifest.version ?? null,
      description: manifest.description ?? null,
      author: manifest.author ?? null,
      iconDataUri,
      backupCount: backups.length,
      backupTimestamps: backups,
      source,
    });
  }
  return out;
}

export async function listInstalledUserExtensions(
  userRoot: string,
  devRoot: string | null,
): Promise<InstalledExtensionInfo[]> {
  const devList = devRoot ? await scanExtensionsDir(devRoot, "dev") : [];
  const installedList = await scanExtensionsDir(userRoot, "installed");
  const seen = new Set(devList.map((e) => e.id));
  return [...devList, ...installedList.filter((e) => !seen.has(e.id))];
}

export async function uninstallExtension(userRoot: string, id: string): Promise<boolean> {
  const dir = path.join(userRoot, id);
  if (!existsSync(dir)) return false;
  await fs.rm(dir, { recursive: true, force: true });
  return true;
}
