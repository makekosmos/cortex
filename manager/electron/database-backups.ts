import type { DatabaseBackup as ManagerDatabaseBackup } from "../src/manager-api";
import { isNumber, isObject, isString, type Input } from "./manager-contract";

const BACKUP_NAME = /^ark\.db\.backup-\d{4}-\d{2}-\d{2}-\d{6}$/;

export function normalizeDbBackups(value: Input): ManagerDatabaseBackup[] {
  const raw = isObject(value) && Array.isArray(value.backups) ? value.backups : [];
  return raw.flatMap((item: Input) => {
    if (
      !isObject(item) ||
      !isString(item.name) ||
      !BACKUP_NAME.test(item.name) ||
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
