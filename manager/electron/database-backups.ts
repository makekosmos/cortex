import { copyFile, lstat, rename, unlink } from "node:fs/promises";
import path from "node:path";
import { randomUUID } from "node:crypto";
import type { DatabaseBackup as ManagerDatabaseBackup } from "../src/manager-api";
import { isNumber, isObject, isString, type Input } from "./manager-contract";

const BACKUP_NAME = /^ark\.db\.backup-\d{4}-\d{2}-\d{2}-\d{6}$/;
const SIDECARS = ["-wal", "-shm"] as const;

export function validDatabaseBackupName(value: string): boolean {
  return BACKUP_NAME.test(value);
}

export type DatabaseBackup = {
  name: string;
  path: string;
  size: number;
  modifiedAt: string;
};

export type BackupInspection = { objectIds: string[] };

export type DatabaseBackupHooks = {
  inspectBackup(path: string): Promise<BackupInspection> | BackupInspection;
  stop(): Promise<void>;
  waitUntilStopped(): Promise<void>;
  start(): Promise<boolean>;
  waitUntilReady(): Promise<boolean>;
  liveObjectIds(): Promise<string[]>;
};

export function normalizeDbBackups(value: Input): ManagerDatabaseBackup[] {
  const raw = isObject(value) && Array.isArray(value.backups) ? value.backups : [];
  return raw.flatMap((item: Input) => {
    if (
      !isObject(item) ||
      !isString(item.name) ||
      !/^ark\.db\.backup-\d{4}-\d{2}-\d{2}-\d{6}$/.test(item.name) ||
      !isString(item.path) ||
      !isNumber(item.size) ||
      !Number.isSafeInteger(item.size) ||
      item.size < 0 ||
      !isString(item.modified_at)
    )
      return [];
    return [
      {
        name: item.name,
        path: item.path.slice(0, 4096),
        size: item.size,
        modified_at: item.modified_at.slice(0, 64),
      },
    ];
  });
}

function backupDirectory(dataDir: string): string {
  return path.resolve(dataDir, "backups");
}

function backupFile(dataDir: string, name: string): string {
  if (!validDatabaseBackupName(name)) throw new Error("Invalid ARK backup name");
  return path.join(backupDirectory(dataDir), name);
}

async function regularFile(file: string): Promise<{ size: number; modifiedAt: string }> {
  const stat = await lstat(file);
  if (!stat.isFile() || stat.isSymbolicLink()) throw new Error("ARK backup is not a regular file");
  return { size: stat.size, modifiedAt: stat.mtime.toISOString() };
}

async function removeIfPresent(file: string): Promise<void> {
  try {
    await unlink(file);
  } catch (error) {
    // SAFETY: fs/promises errors expose the standard Node errno code here.
    if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
  }
}

export async function restoreDatabaseBackup(
  dataDir: string,
  name: string,
  hooks: DatabaseBackupHooks,
): Promise<DatabaseBackup & { objectCount: number }> {
  const dataRoot = path.resolve(dataDir);
  const source = backupFile(dataRoot, name);
  const inspection = await hooks.inspectBackup(source);
  const metadata = await regularFile(source);
  const database = path.join(dataRoot, "ark.db");
  await regularFile(database);

  const token = randomUUID();
  const staged = path.join(dataRoot, `.ark.db.restore-new-${token}`);
  const oldDatabase = path.join(dataRoot, `.ark.db.restore-old-${token}`);
  const movedSidecars: string[] = [];
  let databaseMoved = false;
  let databaseInstalled = false;
  let cleanupOld = false;
  let stopped = false;

  try {
    await hooks.stop();
    await hooks.waitUntilStopped();
    stopped = true;
    await copyFile(source, staged);
    await regularFile(staged);
    await rename(database, oldDatabase);
    databaseMoved = true;
    for (const suffix of SIDECARS) {
      const current = `${database}${suffix}`;
      const old = `${oldDatabase}${suffix}`;
      try {
        await rename(current, old);
        movedSidecars.push(suffix);
      } catch (error) {
        // SAFETY: fs/promises errors expose the standard Node errno code here.
        if ((error as NodeJS.ErrnoException).code !== "ENOENT") throw error;
      }
    }
    await rename(staged, database);
    databaseInstalled = true;
    if (!(await hooks.start()) || !(await hooks.waitUntilReady()))
      throw new Error("Engine did not become ready after restore verification");
    const liveIds = [...new Set(await hooks.liveObjectIds())].sort();
    const expectedIds = [...new Set(inspection.objectIds)].sort();
    if (
      liveIds.length !== expectedIds.length ||
      liveIds.some((id, index) => id !== expectedIds[index])
    )
      throw new Error("ARK object verification failed after restore");

    await removeIfPresent(oldDatabase);
    for (const suffix of movedSidecars) await removeIfPresent(`${oldDatabase}${suffix}`);
    cleanupOld = true;
    return { name, path: source, ...metadata, objectCount: expectedIds.length };
  } catch (error) {
    if (databaseMoved) {
      try {
        await hooks.stop();
        await hooks.waitUntilStopped();
        if (databaseInstalled) await removeIfPresent(database);
        for (const suffix of [...movedSidecars].reverse()) {
          await rename(`${oldDatabase}${suffix}`, `${database}${suffix}`);
        }
        await rename(oldDatabase, database);
        if (!(await hooks.start()) || !(await hooks.waitUntilReady()))
          throw new Error("Engine did not become ready after ARK restore rollback");
        cleanupOld = true;
      } catch (rollbackError) {
        throw new Error(`ARK restore failed and rollback failed: ${String(rollbackError)}`);
      }
    } else if (stopped) {
      if (!(await hooks.start()) || !(await hooks.waitUntilReady()))
        throw new Error("Engine did not become ready after failed ARK restore");
    }
    throw error;
  } finally {
    await removeIfPresent(staged).catch(() => undefined);
    if (cleanupOld) {
      await removeIfPresent(oldDatabase).catch(() => undefined);
      for (const suffix of movedSidecars)
        await removeIfPresent(`${oldDatabase}${suffix}`).catch(() => undefined);
    }
  }
}
