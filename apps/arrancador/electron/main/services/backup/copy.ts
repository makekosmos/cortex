import { copyFile, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  copyBackupDirectoryWithSidecar,
  isArrancadorSidecarUnavailableError,
} from "../../sidecar/arrancador-sidecar";
import { buildBackupManifest, writeBackupManifestToDirectory, writeBackupReadmeToDirectory } from "./archive";
import type { BackupArchiveManifest, BackupFileEntry, ProgressListener, SaveDiscovery } from "./types";
import { ensureDir, mapLimit } from "./utils";

function assertSafeBackupSegment(kind: string, value: string) {
  if (
    !value ||
    value === "." ||
    value === ".." ||
    value.includes(":") ||
    value.includes("/") ||
    value.includes("\\") ||
    path.isAbsolute(value)
  ) {
    throw new Error(`Invalid backup ${kind}: ${value}`);
  }
}

function normalizeSafeRelativePath(relativePath: string): string {
  const normalized = relativePath.replaceAll("\\", "/");
  if (normalized.startsWith("/") || /^[a-zA-Z]:/.test(normalized)) {
    throw new Error(`Invalid backup relative path: ${relativePath}`);
  }

  if (!normalized) {
    return "";
  }

  for (const segment of normalized.split("/")) {
    assertSafeBackupSegment("relative path segment", segment);
  }

  return normalized;
}

export function buildBackupRelPath(rootLabel: string, relativePath: string): string {
  assertSafeBackupSegment("root label", rootLabel);
  const rel = normalizeSafeRelativePath(relativePath);
  if (!rel) {
    return `files/${rootLabel}/file`;
  }
  return `files/${rootLabel}/${rel}`;
}

async function fileMtime(filePath: string): Promise<number | null> {
  try {
    const metadata = await stat(filePath);
    const mtime = metadata.mtimeMs;
    return Number.isFinite(mtime) ? Math.trunc(mtime) : null;
  } catch {
    return null;
  }
}

export interface CopyDiscoveryOptions {
  concurrency?: number;
  onProgress?: ProgressListener | null;
}

export interface CopyDiscoveryResult {
  manifest: BackupArchiveManifest;
  totalBytes: number;
}

export async function copyDiscoveryToDirectory(
  destination: string,
  discovery: SaveDiscovery,
  options: CopyDiscoveryOptions = {},
): Promise<CopyDiscoveryResult> {
  const plannedEntries = discovery.files.map((file) => ({
    file,
    backupPath: buildBackupRelPath(file.rootLabel, file.relativePath),
  }));

  if (process.env.ARRANCADOR_BACKUP_BACKEND !== "ts") {
    try {
      const result = await copyBackupDirectoryWithSidecar(destination, discovery, {
        onProgress: options.onProgress,
      });
      return {
        manifest: buildBackupManifest(
          plannedEntries.map(({ file, backupPath }) => ({
            backupPath,
            originalPath: file.path,
            size: file.size,
            mtime: null,
          })),
        ),
        totalBytes: result.totalBytes,
      };
    } catch (error) {
      if (!isArrancadorSidecarUnavailableError(error)) {
        throw error;
      }
      console.warn(
        "[Arrancador] Rust backup sidecar unavailable, falling back to TypeScript copy.",
      );
    }
  }

  await ensureDir(destination);

  const concurrency = options.concurrency ?? Math.max(1, Math.min(8, os.cpus().length || 1));
  const entries: BackupFileEntry[] = [];
  let completed = 0;
  let totalBytes = 0;

  await mapLimit(plannedEntries, concurrency, async ({ file, backupPath }, index) => {
    const targetPath = path.join(destination, backupPath);
    await ensureDir(path.dirname(targetPath));
    await copyFile(file.path, targetPath);

    const size = file.size;
    const entry: BackupFileEntry = {
      backupPath,
      originalPath: file.path,
      size,
      mtime: await fileMtime(file.path),
    };
    entries[index] = entry;
    totalBytes += size;

    completed += 1;
    if (options.onProgress && (completed === discovery.files.length || completed % 50 === 0)) {
      await options.onProgress({
        stage: "copy",
        current: file.path,
        done: completed,
        total: discovery.files.length,
      });
    }
    return entry;
  });

  const manifest = buildBackupManifest(entries);
  await writeBackupManifestToDirectory(destination, manifest);
  await writeBackupReadmeToDirectory(destination);

  return {
    manifest,
    totalBytes,
  };
}
