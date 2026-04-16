import { randomUUID } from "node:crypto";
import { stat } from "node:fs/promises";
import path from "node:path";
import { compressBackupDirectory, loadBackupManifest } from "./archive";
import { copyDiscoveryToDirectory } from "./copy";
import { pruneBackupsByLimit, listBackups as scanBackups } from "./list";
import { restoreBackupArtifact } from "./restore";
import { discoverBackupInfo, findGameSaves, findSavePath } from "./save-locator";
import type {
  BackupArtifactSummary,
  BackupInfo,
  BackupProgress,
  BackupRecord,
  CheckBackupNeededInput,
  CheckRestoreNeededInput,
  CreateBackupInput,
  DeleteBackupInput,
  FindSavePathInput,
  GameManifest,
  ListBackupsInput,
  RestoreBackupInput,
  RestoreCheck,
  SaveDiscovery,
  SavePathLookup,
} from "./types";
import { buildUniqueBackupPath, createTempDir, ensureDir, removeBackupArtifact } from "./utils";

function formatTimestamp(date: Date): string {
  const pad2 = (value: number) => String(value).padStart(2, "0");
  const pad3 = (value: number) => String(value).padStart(3, "0");

  return [
    `${pad2(date.getHours())}${pad2(date.getMinutes())}${pad2(date.getSeconds())}`,
    `${pad3(date.getMilliseconds())}`,
    `${pad2(date.getDate())}${pad2(date.getMonth() + 1)}${date.getFullYear()}`,
  ].join("_");
}

async function emitProgress(
  listener: CreateBackupInput["onProgress"] | RestoreBackupInput["onProgress"],
  progress: BackupProgress,
): Promise<void> {
  if (!listener) {
    return;
  }
  await listener(progress);
}

async function ensureBackupRoot(backupRoot: string): Promise<void> {
  await ensureDir(backupRoot);
}

export async function createBackup(
  input: CreateBackupInput,
): Promise<BackupRecord> {
  const discovery = await findGameSaves(
    input.gameName,
    input.manifest ?? null,
    input.overridePath ?? null,
  );

  if (!discovery || discovery.files.length === 0) {
    throw new Error(`No save data found for ${input.gameName}`);
  }

  await ensureBackupRoot(input.backupRoot);

  const gameFolder = input.gameYear
    ? `${sanitizeGameFolder(input.gameName)}-${input.gameYear}`
    : sanitizeGameFolder(input.gameName);
  const gameBackupDir = path.join(input.backupRoot, gameFolder);
  await ensureDir(gameBackupDir);

  const timestamp = formatTimestamp(new Date());
  const useZip = input.mode === "zip" && !input.skipCompressionOnce;
  const backupPath = await buildUniqueBackupPath(gameBackupDir, timestamp, useZip);
  const createdAt = new Date().toISOString();

  await emitProgress(input.onProgress, {
    stage: "scan",
    current: "Scanning save files",
    done: 0,
    total: 0,
  });

  let totalBytes = 0;
  let stagingDir: string | null = null;

  try {
    if (useZip) {
      stagingDir = await createTempDir("arrancador-backup-");
      const { totalBytes: stagedBytes } = await copyDiscoveryToDirectory(stagingDir, discovery, {
        onProgress: input.onProgress,
      });
      totalBytes = stagedBytes;
      await compressBackupDirectory(backupPath, stagingDir, input.compressionLevel);
    } else {
      const result = await copyDiscoveryToDirectory(backupPath, discovery, {
        onProgress: input.onProgress,
      });
      totalBytes = result.totalBytes;
    }
  } catch (error) {
    await removeBackupArtifact(backupPath);
    if (stagingDir) {
      await removeBackupArtifact(stagingDir);
    }
    throw error;
  }

  if (totalBytes === 0) {
    await removeBackupArtifact(backupPath);
    if (stagingDir) {
      await removeBackupArtifact(stagingDir);
    }
    throw new Error("No save data found for this game");
  }

  if (stagingDir) {
    await removeBackupArtifact(stagingDir);
  }

  const backup: BackupRecord = {
    id: randomUUID(),
    gameId: input.gameId,
    backupPath,
    backupSize: totalBytes,
    createdAt,
    isAuto: Boolean(input.isAuto),
    notes: input.notes ?? null,
  };

  if (input.maxBackupsPerGame && input.maxBackupsPerGame > 0) {
    await pruneBackupsByLimit({
      backupRoot: input.backupRoot,
      gameName: input.gameName,
      gameYear: input.gameYear ?? null,
      maxBackups: input.maxBackupsPerGame,
    });
  }

  await emitProgress(input.onProgress, {
    stage: "done",
    current: "Backup created",
    done: 0,
    total: 0,
  });

  return backup;
}

export async function restoreBackup(
  input: RestoreBackupInput,
): Promise<void> {
  await restoreBackupArtifact(input.backupPath, input.onProgress ?? null);
  await emitProgress(input.onProgress, {
    stage: "done",
    current: "Restore completed",
    done: 0,
    total: 0,
  });
}

export async function listBackups(
  input: ListBackupsInput,
): Promise<BackupArtifactSummary[]> {
  return scanBackups(input);
}

export async function deleteBackup(
  input: DeleteBackupInput,
): Promise<void> {
  await removeBackupArtifact(input.backupPath);
}

export async function checkBackupNeeded(
  input: CheckBackupNeededInput,
): Promise<boolean> {
  if (!input.currentSave) {
    return false;
  }

  if (!input.lastBackup) {
    return true;
  }

  const backupTime = Date.parse(input.lastBackup.createdAt);
  if (Number.isNaN(backupTime)) {
    return true;
  }

  for (const filePath of input.currentSave.files) {
    try {
      const fileStat = await stat(filePath);
      if (fileStat.mtimeMs > backupTime) {
        return true;
      }
    } catch {
      // ignore inaccessible files and keep scanning
    }
  }

  return false;
}

export async function checkRestoreNeeded(
  input: CheckRestoreNeededInput,
): Promise<RestoreCheck> {
  if (!input.currentSave) {
    return {
      shouldRestore: false,
      backupId: null,
      currentSize: 0,
      backupSize: input.lastBackup?.backupSize ?? 0,
    };
  }

  if (!input.lastBackup) {
    return {
      shouldRestore: false,
      backupId: null,
      currentSize: input.currentSave.totalSize,
      backupSize: 0,
    };
  }

  return {
    shouldRestore: input.currentSave.totalSize < input.lastBackup.backupSize,
    backupId: input.lastBackup.id ?? null,
    currentSize: input.currentSave.totalSize,
    backupSize: input.lastBackup.backupSize,
  };
}

export { findSavePath, discoverBackupInfo, findGameSaves, loadBackupManifest };
export type {
  BackupArtifactSummary,
  BackupInfo,
  BackupProgress,
  BackupRecord,
  CheckBackupNeededInput,
  CheckRestoreNeededInput,
  CreateBackupInput,
  DeleteBackupInput,
  FindSavePathInput,
  GameManifest,
  ListBackupsInput,
  RestoreBackupInput,
  RestoreCheck,
  SaveDiscovery,
  SavePathLookup,
};

function sanitizeGameFolder(name: string): string {
  return name.replace(/[<>:"/\\|?*]/g, "").trim() || "game";
}
