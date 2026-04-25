import {
  getBackupRecord,
  loadManifest,
  toBackupProgressPayload,
} from "./shared";
import type {
  BackupWorkflowContext,
  CreateBackupPayload,
} from "./types";

export function createBackupWriteUseCases(context: BackupWorkflowContext) {
  const { deps, io } = context;

  return {
    async setLudusaviPath(path: string): Promise<void> {
      await io.setSetting(deps.db, "ludusavi_path", path || "native");
    },

    async setBackupDirectory(path: string): Promise<void> {
      await io.setSetting(deps.db, "backup_directory", path.trim());
    },

    async createBackup(payload: CreateBackupPayload) {
      const gameState = await io.getGameBackupState(deps.db, payload.gameId);
      const manifest = await loadManifest(context);
      const overridePath = await io.resolveGameOverridePath(
        deps.db,
        payload.gameId,
        gameState.save_path,
      );
      const backupRoot = await io.getBackupRoot(deps.db, deps.userDataPath);
      const settings = await deps.settings.getAllSettings();

      const backup = await io.createBackupArtifact({
        gameId: payload.gameId,
        gameName: payload.gameName,
        backupRoot,
        mode:
          settings.backup_compression_enabled &&
          !settings.backup_skip_compression_once
            ? "zip"
            : "directory",
        compressionLevel: settings.backup_compression_level,
        skipCompressionOnce: settings.backup_skip_compression_once,
        overridePath,
        manifest,
        isAuto: payload.isAuto,
        notes: payload.notes,
        gameYear: io.releasedYear(gameState.released),
        maxBackupsPerGame: settings.max_backups_per_game,
        onProgress: async (progress) => {
          deps.emitRendererEvent(
            "backup:progress",
            toBackupProgressPayload(payload.gameId, progress),
          );
        },
      });

      await io.execute(
        deps.db,
        `INSERT INTO backups (id, game_id, backup_path, backup_size, created_at, is_auto, notes)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)`,
        [
          backup.id ?? backup.backupPath,
          payload.gameId,
          backup.backupPath,
          backup.backupSize,
          backup.createdAt,
          payload.isAuto ? 1 : 0,
          backup.notes ?? null,
        ],
      );

      await io.execute(
        deps.db,
        `UPDATE games
         SET last_backup = ?1,
             backup_count = backup_count + 1,
             backup_enabled = 1
         WHERE id = ?2`,
        [backup.createdAt, payload.gameId],
      );

      await io.reconcileBackupRows(
        deps.db,
        payload.gameId,
        settings.max_backups_per_game,
      );

      if (settings.backup_skip_compression_once) {
        await io.setSetting(deps.db, "backup_skip_compression_once", "false");
      }

      await deps.achievements.recordAchievementEvent("backup_restore");

      return {
        id: backup.id ?? backup.backupPath,
        game_id: payload.gameId,
        backup_path: backup.backupPath,
        backup_size: backup.backupSize,
        created_at: backup.createdAt,
        is_auto: Boolean(backup.isAuto),
        notes: backup.notes ?? null,
      };
    },

    async restoreBackup(backupId: string): Promise<void> {
      const backup = await getBackupRecord(context, backupId);
      if (!backup) {
        throw new Error("Backup not found");
      }
      const gameState = await io.getGameBackupState(deps.db, backup.game_id);
      const manifest = await loadManifest(context);
      const overridePath = await io.resolveGameOverridePath(
        deps.db,
        backup.game_id,
        gameState.save_path,
      );
      const currentSave = await io.findGameSaveArtifacts(
        gameState.name,
        manifest,
        overridePath,
      );
      const allowedRestoreRoots =
        currentSave?.roots.map((root) => root.path) ??
        (overridePath ? [overridePath] : []);

      await io.restoreBackupArtifact({
        backupPath: backup.backup_path,
        allowedRestoreRoots,
        onProgress: async (progress) => {
          deps.emitRendererEvent(
            "restore:progress",
            toBackupProgressPayload(backup.game_id, progress),
          );
        },
      });

      await deps.achievements.recordAchievementEvent("backup_restore");
    },

    async deleteBackup(backupId: string): Promise<void> {
      const backup = await getBackupRecord(context, backupId);
      if (!backup) {
        return;
      }

      await io.deleteBackupArtifact({ backupPath: backup.backup_path });
      await io.execute(deps.db, "DELETE FROM backups WHERE id = ?1", [backupId]);
      await io.reconcileBackupRows(
        deps.db,
        backup.game_id,
        Number.MAX_SAFE_INTEGER,
      );
    },

    async updateBackupSettings(settings: Record<string, string>): Promise<void> {
      await io.runInTransaction(deps.db, async (tx) => {
        for (const [key, value] of Object.entries(settings)) {
          await io.execute(
            tx,
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            [key, String(value)],
          );
        }
      });
    },
  };
}
