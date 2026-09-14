import { lstat, mkdir, realpath } from "node:fs/promises";
import path from "node:path";
import type { DatabaseBackup as ManagerDatabaseBackup } from "../src/manager-api";
import { isNumber, isObject, isString, type Input } from "./manager-contract";

const BACKUP_NAME = /^ark\.db\.backup-\d{4}-\d{2}-\d{2}-\d{6}$/;

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
