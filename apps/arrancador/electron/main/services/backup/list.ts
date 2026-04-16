import path from "node:path";
import { loadBackupManifest } from "./archive";
import type { BackupArchiveManifest, BackupArtifactSummary, ListBackupsInput } from "./types";
import { pathExists, readDirectoryEntries, removeBackupArtifact, statPath } from "./utils";

const WIN_PATH = path.win32;

function parseBackupTimestamp(name: string): Date | null {
  const trimmed = name.replace(/(\.sqoba\.zip|\.zip)$/i, "");
  const match = trimmed.match(/^(\d{6})_(\d{2})(\d{2})(\d{4})(?:_\d+)?$/);
  if (!match) {
    return null;
  }

  const [, time, day, month, year] = match;
  const hours = Number.parseInt(time.slice(0, 2), 10);
  const minutes = Number.parseInt(time.slice(2, 4), 10);
  const seconds = Number.parseInt(time.slice(4, 6), 10);
  const dt = new Date(
    Number.parseInt(year, 10),
    Number.parseInt(month, 10) - 1,
    Number.parseInt(day, 10),
    hours,
    minutes,
    seconds,
    0,
  );

  return Number.isNaN(dt.getTime()) ? null : dt;
}

export function backupEntryTimestamp(backupPath: string): Date {
  const name = path.basename(backupPath);
  const parsed = parseBackupTimestamp(name);
  if (parsed) {
    return parsed;
  }

  return new Date();
}

function sanitizeFolderName(name: string): string {
  return name.replace(/[<>:"/\\|?*]/g, "").trim();
}

async function findBackupGameDirsAsync(
  backupRoot: string,
  gameName: string,
  gameYear?: string | null,
): Promise<string[]> {
  const base = sanitizeFolderName(gameName).toLowerCase();
  if (!base || !(await pathExists(backupRoot))) {
    return [];
  }

  const expectedYear = gameYear?.trim();
  const expectedParens = expectedYear ? `${base} (${expectedYear.toLowerCase()})` : null;
  const out: string[] = [];
  const entries = await readDirectoryEntries(backupRoot);

  for (const entry of entries) {
    if (!entry.isDirectory()) {
      continue;
    }
    const name = entry.name.toLowerCase();
    const matchesDefault = name === base || name.startsWith(`${base}-`);
    const matchesParens = expectedParens ? name === expectedParens : false;
    if (matchesDefault || matchesParens) {
      out.push(path.join(backupRoot, entry.name));
    }
  }

  return out;
}

function parseBackupRelative(backupPath: string): [string, string] | null {
  const parts = backupPath.split("/").filter(Boolean);
  if (parts.length < 3 || parts[0] !== "files") {
    return null;
  }

  return [parts[1], parts.slice(2).join("/")];
}

function stripSuffixPath(fullPath: string, suffix: string): string | null {
  const normalizedFull = WIN_PATH.normalize(fullPath).replace(/[\\/]+$/, "");
  const normalizedSuffix = WIN_PATH.normalize(suffix).replace(/^[\\/]+|[\\/]+$/g, "");
  const lowerFull = normalizedFull.toLowerCase();
  const lowerSuffix = normalizedSuffix.toLowerCase();

  if (lowerFull === lowerSuffix) {
    return "";
  }

  const marker = `\\${lowerSuffix}`;
  if (!lowerFull.endsWith(marker)) {
    return null;
  }

  return normalizedFull.slice(0, normalizedFull.length - marker.length);
}

export function deriveSaveRootFromManifest(
  manifest: BackupArchiveManifest,
): string | null {
  const totals = new Map<string, number>();
  const roots = new Map<string, string>();

  for (const entry of manifest.files) {
    const parsed = parseBackupRelative(entry.backupPath);
    if (!parsed) {
      continue;
    }
    const [rootLabel, rel] = parsed;
    const originalPath = entry.originalPath;
    const root = stripSuffixPath(originalPath, rel) ?? path.dirname(originalPath);
    totals.set(rootLabel, (totals.get(rootLabel) ?? 0) + entry.size);
    if (!roots.has(rootLabel)) {
      roots.set(rootLabel, root);
    }
  }

  if (totals.size === 0) {
    const first = manifest.files[0];
    return first ? path.dirname(first.originalPath) : null;
  }

  let bestLabel: string | null = null;
  let bestSize = -1;
  for (const [label, size] of totals.entries()) {
    if (size > bestSize) {
      bestLabel = label;
      bestSize = size;
    }
  }

  return bestLabel ? roots.get(bestLabel) ?? null : null;
}

async function backupSummaryFromArtifact(backupPath: string): Promise<BackupArtifactSummary | null> {
  const manifest = await loadBackupManifest(backupPath);
  if (!manifest) {
    return null;
  }

  const stat = await statPath(backupPath).catch(() => null);
  const createdAt = backupEntryTimestamp(backupPath).toISOString();
  const kind = backupPath.toLowerCase().endsWith(".zip") ? "zip" : "directory";
  const saveRoot = deriveSaveRootFromManifest(manifest);
  const backupSize = manifest.files.reduce((sum, entry) => sum + entry.size, 0);

  return {
    id: backupPath,
    backupPath,
    backupSize,
    createdAt: stat?.mtime.toISOString() ?? createdAt,
    isAuto: false,
    notes: null,
    kind,
    saveRoot,
  };
}

export async function listBackups(
  input: ListBackupsInput,
): Promise<BackupArtifactSummary[]> {
  const gameDirs = await findBackupGameDirsAsync(
    input.backupRoot,
    input.gameName,
    input.gameYear ?? null,
  );
  const out: BackupArtifactSummary[] = [];

  for (const gameDir of gameDirs) {
    const entries = await readDirectoryEntries(gameDir).catch(() => []);
    for (const entry of entries) {
      if (!entry.isDirectory() && !entry.name.toLowerCase().endsWith(".zip")) {
        continue;
      }
      const backupPath = path.join(gameDir, entry.name);
      const summary = await backupSummaryFromArtifact(backupPath);
      if (summary) {
        out.push(summary);
      }
    }
  }

  out.sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));
  return out;
}

export async function pruneBackupsByLimit(input: {
  backupRoot: string;
  gameName: string;
  gameYear?: string | null;
  maxBackups: number;
}): Promise<void> {
  const backups = await listBackups({
    backupRoot: input.backupRoot,
    gameName: input.gameName,
    gameYear: input.gameYear ?? null,
  });

  if (backups.length <= input.maxBackups) {
    return;
  }

  const toDelete = backups.slice(input.maxBackups);
  for (const backup of toDelete) {
    await removeBackupArtifact(backup.backupPath);
  }
}
