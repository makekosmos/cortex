import { copyFile, readFile } from "node:fs/promises";
import path from "node:path";
import { expandBackupArchive, readBackupManifestFromDirectory } from "./archive";
import type {
  BackupArchiveManifest,
  ProgressListener,
} from "./types";
import { createTempDir, ensureDir, isDirectory, isFile, mapLimit, pathExists, removeBackupArtifact } from "./utils";

const WIN_PATH = path.win32;

function validateBackupRelPath(rel: string): void {
  const parts = rel.split("/").filter(Boolean);
  if (parts.length === 0) {
    throw new Error(`Invalid backup path in manifest: ${rel}`);
  }

  for (const part of parts) {
    if (part === "." || part === "..") {
      throw new Error(`Invalid backup path in manifest: ${rel}`);
    }
    if (WIN_PATH.isAbsolute(part) || part.includes(":")) {
      throw new Error(`Invalid backup path in manifest: ${rel}`);
    }
  }
}

function pathFromBackupRel(rel: string): string {
  const parts = rel.split("/").filter(Boolean);
  return path.join(...parts);
}

function validateRestoreTargetPath(original: string): string {
  const rawParts = original.split(/[\\/]+/);
  if (rawParts.some((part) => part === "." || part === "..")) {
    throw new Error(`Invalid restore target path in manifest: ${original}`);
  }

  const target = WIN_PATH.normalize(original);
  if (!WIN_PATH.isAbsolute(target)) {
    throw new Error(`Invalid restore target path in manifest: ${original}`);
  }
  return target;
}

function splitDriveForRestore(
  original: string,
  inverseDrives: Map<string, string>,
): { sourceRoot: string; relative: string } {
  const match = original.match(/^([A-Za-z]):[\\/](.*)$/);
  if (match) {
    const letter = match[1].toUpperCase();
    const rest = match[2].replaceAll("\\", "/");
    const prefix = `${letter}:`;
    return {
      sourceRoot: inverseDrives.get(prefix) ?? `drive-${letter}`,
      relative: rest,
    };
  }

  return {
    sourceRoot: "drive-0",
    relative: original.replaceAll("\\", "/"),
  };
}

function stripQuotes(input: string): string {
  return input.replace(/^['"]|['"]$/g, "");
}

function normalizeDrivePrefix(value: string): string {
  const trimmed = stripQuotes(value.trim()).replaceAll("/", "\\");
  const match = trimmed.match(/^([A-Za-z]):/);
  return match ? `${match[1].toUpperCase()}:` : trimmed.replace(/[\\/]+$/, "");
}

interface ParsedLegacyMapping {
  drives: Map<string, string>;
  files: string[];
}

function parseLegacyMapping(text: string): ParsedLegacyMapping {
  const drives = new Map<string, string>();
  const backups: Array<{ files: string[] }> = [];
  let section: "root" | "drives" | "backups" = "root";
  let currentBackup: { files: string[] } | null = null;
  let inFiles = false;

  for (const rawLine of text.split(/\r?\n/)) {
    const line = rawLine.replace(/\t/g, "    ");
    const trimmed = line.trim();

    if (trimmed === "drives:") {
      section = "drives";
      inFiles = false;
      continue;
    }
    if (trimmed === "backups:") {
      section = "backups";
      inFiles = false;
      continue;
    }

    if (section === "drives") {
      const match = line.match(/^\s{2}([^:]+):\s*(.+)\s*$/);
      if (match) {
        drives.set(stripQuotes(match[1].trim()), normalizeDrivePrefix(match[2].trim()));
      }
      continue;
    }

    if (section === "backups") {
      if (/^\s{2}-\s+/.test(line)) {
        currentBackup = { files: [] };
        backups.push(currentBackup);
        inFiles = false;
        continue;
      }

      if (/^\s{4}files:\s*$/.test(line)) {
        inFiles = true;
        continue;
      }

      if (inFiles && currentBackup) {
        const match = line.match(/^\s{6,}(?:"([^"]+)"|'([^']+)'|([^:]+)):\s*$/);
        if (match) {
          const value = match[1] ?? match[2] ?? match[3];
          if (value) {
            currentBackup.files.push(stripQuotes(value.trim()));
          }
        }
      }
    }
  }

  return {
    drives,
    files: backups.at(-1)?.files ?? [],
  };
}

async function restoreFromEntries(
  entries: Array<{ source: string; target: string }>,
  onProgress?: ProgressListener | null,
): Promise<void> {
  const total = entries.length;
  let completed = 0;

  await mapLimit(entries, Math.min(8, total || 1), async ({ source, target }) => {
    await ensureDir(path.dirname(target));
    await copyFile(source, target);

    completed += 1;
    if (onProgress && (completed === total || completed % 50 === 0)) {
      await onProgress({
        stage: "restore",
        current: target,
        done: completed,
        total,
      });
    }
  });
}

async function restoreManifestDirectory(
  backupRoot: string,
  manifest: BackupArchiveManifest,
  onProgress?: ProgressListener | null,
): Promise<void> {
  const entries = manifest.files.map((entry) => {
    validateBackupRelPath(entry.backupPath);
    const source = path.join(backupRoot, pathFromBackupRel(entry.backupPath));
    const target = validateRestoreTargetPath(entry.originalPath);
    return { source, target };
  });

  for (const entry of entries) {
    if (!(await pathExists(entry.source))) {
      throw new Error(`Backup file is missing: ${entry.source}`);
    }
  }

  await restoreFromEntries(entries, onProgress);
}

async function restoreLegacyMapping(
  backupRoot: string,
  mappingPath: string,
  onProgress?: ProgressListener | null,
): Promise<void> {
  const mappingText = await readFile(mappingPath, "utf8");
  const mapping = parseLegacyMapping(mappingText);

  const entries = mapping.files.map((originalPath) => {
    const { sourceRoot, relative } = splitDriveForRestore(originalPath, mapping.drives);
    const sourceRel = `${sourceRoot}/${relative}`.replace(/\/+/g, "/");
    validateBackupRelPath(sourceRel);
    const source = path.join(backupRoot, pathFromBackupRel(sourceRel));
    const target = validateRestoreTargetPath(originalPath.replaceAll("/", "\\"));
    return { source, target };
  });

  for (const entry of entries) {
    if (!(await pathExists(entry.source))) {
      throw new Error(`Backup file is missing: ${entry.source}`);
    }
  }

  await restoreFromEntries(entries, onProgress);
}

export async function restoreBackupDirectory(
  backupRoot: string,
  onProgress?: ProgressListener | null,
): Promise<void> {
  const manifest = await readBackupManifestFromDirectory(backupRoot);
  if (manifest) {
    await restoreManifestDirectory(backupRoot, manifest, onProgress);
    return;
  }

  const mappingPath = path.join(backupRoot, "mapping.yaml");
  if (await isFile(mappingPath)) {
    await restoreLegacyMapping(backupRoot, mappingPath, onProgress);
    return;
  }

  throw new Error("Backup manifest is missing");
}

export async function restoreBackupArtifact(
  backupPath: string,
  onProgress?: ProgressListener | null,
): Promise<void> {
  if (await isDirectory(backupPath)) {
    await restoreBackupDirectory(backupPath, onProgress);
    return;
  }

  if (!(await isFile(backupPath))) {
    throw new Error(`Backup does not exist: ${backupPath}`);
  }

  if (!backupPath.toLowerCase().endsWith(".zip")) {
    throw new Error(`Unsupported backup artifact: ${backupPath}`);
  }

  const tempDir = await createTempDir("arrancador-restore-");
  try {
    await expandBackupArchive(backupPath, tempDir);
    await restoreBackupDirectory(tempDir, onProgress);
  } finally {
    await removeBackupArtifact(tempDir);
  }
}
