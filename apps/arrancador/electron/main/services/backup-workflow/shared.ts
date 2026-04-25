import type { BackupRecord } from "../backup/types";
import type { GameBackupState } from "../backup-ipc-support";
import type {
  BackupProgressEvent,
  BackupRecordRow,
  BackupWorkflowContext,
} from "./types";

export async function loadManifest({ deps, io }: BackupWorkflowContext) {
  return await io.loadManifestFromCache(deps.userDataPath);
}

export async function getOverridePath(
  { deps, io }: BackupWorkflowContext,
  gameId: string | undefined,
  gameState: GameBackupState | null,
) {
  return gameId && gameState
    ? await io.resolveGameOverridePath(deps.db, gameId, gameState.save_path)
    : null;
}

export async function getBackupRecord(
  { deps, io }: BackupWorkflowContext,
  backupId: string,
) {
  return await io.queryOne<BackupRecordRow>(
    deps.db,
    `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
     FROM backups
     WHERE id = ?1`,
    [backupId],
  );
}

export function toBackupRecord(row: BackupRecordRow): BackupRecord {
  return {
    id: row.id,
    gameId: row.game_id,
    backupPath: row.backup_path,
    backupSize: row.backup_size,
    createdAt: row.created_at,
    isAuto: Boolean(row.is_auto),
    notes: row.notes,
  };
}

export function toBackupProgressPayload(
  gameId: string,
  progress: {
    stage: string;
    current: string;
    done: number;
    total: number;
  },
): BackupProgressEvent {
  return {
    game_id: gameId,
    stage: progress.stage,
    message: progress.current,
    done: progress.done,
    total: progress.total,
  };
}
