import path from "node:path";

import { execute, queryAll, queryOne } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import { deleteBackup as deleteBackupArtifact } from "./backup";
import { loadGameManifestCache, manifestCachePath } from "./backup/manifest";
import { resolveSavePathTemplate } from "./backup/save-locator";
import { getSetting } from "./settings-store";

type BackupRow = {
  id: string;
  game_id: string;
  backup_path: string;
  backup_size: number;
  created_at: string;
  is_auto: number;
  notes: string | null;
};

export type GameBackupState = {
  id: string;
  name: string;
  released: string | null;
  save_path: string | null;
  backup_enabled: number;
};

export type BackupListItem = {
  id: string;
  game_id: string;
  backup_path: string;
  backup_size: number;
  created_at: string;
  is_auto: boolean;
  notes: string | null;
};

export function defaultBackupDirectory(userDataPath: string) {
  return path.join(userDataPath, "backups");
}

export async function getBackupRoot(
  db: DbLike,
  userDataPath: string,
): Promise<string> {
  const custom = (await getSetting(db, "backup_directory"))?.trim() ?? "";
  return custom || defaultBackupDirectory(userDataPath);
}

export async function getGameBackupState(
  db: DbLike,
  id: string,
): Promise<GameBackupState> {
  const row = await queryOne<GameBackupState>(
    db,
    `SELECT id, name, released, save_path, backup_enabled
     FROM games
     WHERE id = ?1`,
    [id],
  );

  if (!row) {
    throw new Error("Game not found");
  }

  return row;
}

function mapBackupRow(row: BackupRow): BackupListItem {
  return {
    id: row.id,
    game_id: row.game_id,
    backup_path: row.backup_path,
    backup_size: Number(row.backup_size ?? 0),
    created_at: row.created_at,
    is_auto: Number(row.is_auto ?? 0) === 1,
    notes: row.notes ?? null,
  };
}

export function releasedYear(released: string | null): string | null {
  const year = released?.split("-")[0]?.trim();
  return year ? year : null;
}

export async function loadManifestFromCache(userDataPath: string) {
  return await loadGameManifestCache(manifestCachePath(userDataPath));
}

export async function resolveGameOverridePath(
  db: DbLike,
  gameId: string,
  rawSavePath: string | null,
): Promise<string | null> {
  if (!rawSavePath?.trim()) {
    return null;
  }

  const row = await queryOne<{ exe_path: string }>(
    db,
    "SELECT exe_path FROM games WHERE id = ?1",
    [gameId],
  );
  if (!row?.exe_path) {
    return rawSavePath;
  }

  return await resolveSavePathTemplate(row.exe_path, rawSavePath);
}

export async function listGameBackups(db: DbLike, gameId: string) {
  const rows = await queryAll<BackupRow>(
    db,
    `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
     FROM backups
     WHERE game_id = ?1
     ORDER BY created_at DESC`,
    [gameId],
  );
  return rows.map(mapBackupRow);
}

export async function getLatestBackup(db: DbLike, gameId: string) {
  const row = await queryOne<BackupRow>(
    db,
    `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
     FROM backups
     WHERE game_id = ?1
     ORDER BY created_at DESC
     LIMIT 1`,
    [gameId],
  );
  return row ? mapBackupRow(row) : null;
}

export async function reconcileBackupRows(
  db: DbLike,
  gameId: string,
  maxBackups: number,
) {
  const backups = await listGameBackups(db, gameId);
  const extra = backups.slice(maxBackups);

  for (const backup of extra) {
    await execute(db, "DELETE FROM backups WHERE id = ?1", [backup.id]);
    await deleteBackupArtifact({ backupPath: backup.backup_path }).catch(() => undefined);
  }

  const latest = backups[0] ?? null;
  await execute(
    db,
    `UPDATE games
     SET backup_count = ?1, last_backup = ?2
     WHERE id = ?3`,
    [Math.max(0, backups.length - extra.length), latest?.created_at ?? null, gameId],
  );
}
