import { copyFile, lstat, open, readFile, rename, unlink } from "node:fs/promises";
import path from "node:path";
import { randomUUID } from "node:crypto";
import type { DatabaseBackup as ManagerDatabaseBackup } from "../src/manager-api";
import { isNumber, isObject, isString, type Input } from "./manager-contract";

const BACKUP_NAME = /^ark\.db\.backup-\d{4}-\d{2}-\d{2}-\d{6}$/;
const SIDECARS = ["-wal", "-shm"] as const;
const RESTORE_JOURNAL = ".ark.db.restore-journal.json";

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

type RestoreJournal = {
  database: string;
  rollback: string;
  moved: string;
  staged: string;
  sidecars: string[];
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

function journalFile(dataDir: string): string {
  return path.join(path.resolve(dataDir), RESTORE_JOURNAL);
}

async function syncFile(file: string): Promise<void> {
  if (process.versions.bun) return;
  const handle = await open(file, "r");
  try {
    await handle.sync();
  } finally {
    await handle.close();
  }
}

async function writeJournal(file: string, journal: RestoreJournal): Promise<void> {
  const temporary = `${file}.${randomUUID()}.tmp`;
  const handle = await open(temporary, "w");
  try {
    await handle.writeFile(JSON.stringify(journal));
    await handle.sync();
  } finally {
    await handle.close();
  }
  await rename(temporary, file);
}

function parseJournal(value: string, dataRoot: string): RestoreJournal {
  // SAFETY: JSON.parse output is checked as an object before fields are read.
  const parsed = JSON.parse(value) as Input;
  if (
    !isObject(parsed) ||
    !isString(parsed.database) ||
    !isString(parsed.rollback) ||
    !isString(parsed.moved) ||
    !isString(parsed.staged) ||
    !Array.isArray(parsed.sidecars) ||
    parsed.sidecars.some((sidecar) => !isString(sidecar))
  )
    throw new Error("Invalid ARK restore journal");
  const sidecars = parsed.sidecars.flatMap((sidecar) => (isString(sidecar) ? [sidecar] : []));
  const paths = [parsed.database, parsed.rollback, parsed.moved, parsed.staged];
  if (
    paths.some((file) => path.dirname(path.resolve(file)) !== dataRoot) ||
    sidecars.some((sidecar) => !new Set<string>(SIDECARS).has(sidecar))
  )
    throw new Error("Invalid ARK restore journal paths");
  return {
    database: parsed.database,
    rollback: parsed.rollback,
    moved: parsed.moved,
    staged: parsed.staged,
    sidecars,
  };
}

async function present(file: string): Promise<boolean> {
  try {
    await lstat(file);
    return true;
  } catch (error) {
    // SAFETY: fs/promises errors expose the standard Node errno code here.
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw error;
  }
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

export async function recoverDatabaseRestore(dataDir: string): Promise<void> {
  const dataRoot = path.resolve(dataDir);
  const journal = journalFile(dataRoot);
  let contents: string;
  try {
    contents = await readFile(journal, "utf8");
  } catch (error) {
    // SAFETY: fs/promises errors expose the standard Node errno code here.
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
  const record = parseJournal(contents, dataRoot);
  const primaryExists = await present(record.database);
  if (!primaryExists) {
    if (await present(record.rollback)) await rename(record.rollback, record.database);
    else if (await present(record.moved)) await rename(record.moved, record.database);
    else throw new Error("ARK restore journal has no recoverable database");
    for (const suffix of record.sidecars) {
      const moved = `${record.moved}${suffix}`;
      if (await present(moved)) await rename(moved, `${record.database}${suffix}`);
    }
  }
  await removeIfPresent(record.staged);
  await removeIfPresent(record.moved);
  for (const suffix of record.sidecars) await removeIfPresent(`${record.moved}${suffix}`);
  await removeIfPresent(record.rollback);
  await removeIfPresent(journal);
}

let restoreTail: Promise<void> = Promise.resolve();

async function restoreDatabaseBackupLocked(
  dataDir: string,
  name: string,
  hooks: DatabaseBackupHooks,
): Promise<DatabaseBackup & { objectCount: number }> {
  await recoverDatabaseRestore(dataDir);
  const dataRoot = path.resolve(dataDir);
  const source = backupFile(dataRoot, name);
  const metadata = await regularFile(source);
  const inspection = await hooks.inspectBackup(source);
  const database = path.join(dataRoot, "ark.db");
  await regularFile(database);

  const token = randomUUID();
  const staged = path.join(dataRoot, `.ark.db.restore-new-${token}`);
  const rollback = path.join(dataRoot, `.ark.db.restore-rollback-${token}`);
  const moved = path.join(dataRoot, `.ark.db.restore-live-${token}`);
  const journal = journalFile(dataRoot);
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
    await syncFile(staged);
    await copyFile(database, rollback);
    await syncFile(rollback);
    await writeJournal(journal, {
      database,
      rollback,
      moved,
      staged,
      sidecars: [...SIDECARS],
    });
    await rename(database, moved);
    databaseMoved = true;
    for (const suffix of SIDECARS) {
      try {
        await rename(`${database}${suffix}`, `${moved}${suffix}`);
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

    cleanupOld = true;
    await removeIfPresent(journal).catch(() => undefined);
    return { name, path: source, ...metadata, objectCount: expectedIds.length };
  } catch (error) {
    if (databaseMoved) {
      try {
        await hooks.stop();
        await hooks.waitUntilStopped();
        if (databaseInstalled) await removeIfPresent(database);
        for (const suffix of SIDECARS) await removeIfPresent(`${database}${suffix}`);
        for (const suffix of [...movedSidecars].reverse()) {
          await rename(`${moved}${suffix}`, `${database}${suffix}`);
        }
        await rename(rollback, database);
        await removeIfPresent(journal);
        if (!(await hooks.start()) || !(await hooks.waitUntilReady()))
          throw new Error("Engine did not become ready after ARK restore rollback");
        cleanupOld = true;
      } catch (rollbackError) {
        throw new Error(`ARK restore failed and rollback failed: ${String(rollbackError)}`);
      }
    } else if (stopped) {
      await removeIfPresent(journal).catch(() => undefined);
      if (!(await hooks.start()) || !(await hooks.waitUntilReady()))
        throw new Error("Engine did not become ready after failed ARK restore");
    }
    throw error;
  } finally {
    await removeIfPresent(staged).catch(() => undefined);
    if (cleanupOld) {
      for (const suffix of movedSidecars)
        await removeIfPresent(`${moved}${suffix}`).catch(() => undefined);
      await removeIfPresent(moved).catch(() => undefined);
      await removeIfPresent(rollback).catch(() => undefined);
    }
  }
}

export async function restoreDatabaseBackup(
  dataDir: string,
  name: string,
  hooks: DatabaseBackupHooks,
): Promise<DatabaseBackup & { objectCount: number }> {
  const previous = restoreTail;
  let release!: () => void;
  restoreTail = new Promise((resolve) => {
    release = resolve;
  });
  await previous;
  try {
    return await restoreDatabaseBackupLocked(dataDir, name, hooks);
  } finally {
    release();
  }
}
