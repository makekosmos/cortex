import { lstat, mkdir, realpath } from "node:fs/promises";
import path from "node:path";
import type {
  DatabaseBackup as ManagerDatabaseBackup,
  DbBackupRestoreResult,
  DbBackupValidation,
} from "../src/manager-api";
import { isNumber, isObject, isString, type Input } from "./manager-contract";

// Ровно формат `run_backup_now` в runtime и `validate_snapshot_id` в Core:
// только basename `ark.db.backup-YYYY-MM-DD-HHMMSS`, никаких путей.
const BACKUP_NAME = /^ark\.db\.backup-\d{4}-\d{2}-\d{2}-\d{6}$/;

export function validDbBackupId(value: Input): value is string {
  return isString(value) && BACKUP_NAME.test(value);
}

function isWithin(root: string, candidate: string): boolean {
  const relative = path.relative(root, candidate);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== "..");
}

export async function resolveSafeDbBackupsFolder(dataRoot: string): Promise<string> {
  const root = path.resolve(dataRoot);
  await mkdir(root, { recursive: true });
  const canonicalRoot = await realpath(root);
  const directory = path.join(root, "backups");
  await mkdir(directory, { recursive: true });
  const metadata = await lstat(directory);
  if (!metadata.isDirectory() || metadata.isSymbolicLink())
    throw new Error("backup directory must be a real directory");
  const canonicalDirectory = await realpath(directory);
  if (!isWithin(canonicalRoot, canonicalDirectory))
    throw new Error("backup directory escapes data directory");
  return canonicalDirectory;
}

// Core `db_backup_list` отдаёт { id, size_bytes, modified_ms? } — Manager
// получает только нормализованные поля, без файловых путей.
export function normalizeDbBackups(value: Input): ManagerDatabaseBackup[] {
  const raw = isObject(value) && Array.isArray(value.backups) ? value.backups : [];
  return raw
    .flatMap((item: Input) => {
      if (
        !isObject(item) ||
        !validDbBackupId(item.id) ||
        !isNumber(item.size_bytes) ||
        !Number.isSafeInteger(item.size_bytes) ||
        item.size_bytes < 0 ||
        (item.modified_ms !== undefined &&
          item.modified_ms !== null &&
          (!isNumber(item.modified_ms) || !Number.isSafeInteger(item.modified_ms)))
      )
        return [];
      const modified =
        isNumber(item.modified_ms) && item.modified_ms >= 0
          ? new Date(item.modified_ms).toISOString()
          : null;
      return [
        {
          name: item.id,
          size: item.size_bytes,
          modified_at: modified?.slice(0, 64) ?? null,
        },
      ];
    })
    .sort((a, b) => b.name.localeCompare(a.name));
}

// Typed verdict Core `db_backup_validate` — fail-closed: неполный или
// деформированный ответ трактуется как невалидный snapshot.
export function normalizeDbBackupValidation(value: Input): DbBackupValidation {
  const raw = isObject(value) ? value : {};
  const id = isString(raw.id) ? raw.id.slice(0, 256) : "";
  const exists = raw.exists === true;
  const integrity = raw.integrity_ok === true;
  const schema = raw.schema_match === true;
  return {
    id,
    exists,
    integrity_ok: integrity,
    schema_match: schema,
    valid: raw.valid === true && exists && integrity && schema,
    error: isString(raw.error) ? raw.error.slice(0, 512) : null,
  };
}

// Typed result Core `db_backup_restore`: { id, restored, objects, links }.
// `restored !== true` или отсутствующие счётчики — не «успех».
export function normalizeDbBackupRestore(value: Input): DbBackupRestoreResult | null {
  if (!isObject(value) || !validDbBackupId(value.id) || value.restored !== true) return null;
  const objects = value.objects;
  const links = value.links;
  if (
    !isNumber(objects) ||
    !Number.isSafeInteger(objects) ||
    objects < 0 ||
    !isNumber(links) ||
    !Number.isSafeInteger(links) ||
    links < 0
  )
    return null;
  return { id: value.id, restored: true, objects, links };
}
