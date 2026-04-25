import type { BackupListItem } from "../backup-ipc-support";
import {
  getOverridePath,
  loadManifest,
  toBackupRecord,
} from "./shared";
import type {
  BackupCheckPayload,
  BackupWorkflowContext,
  FindGameSavePathsPayload,
} from "./types";

export function createBackupReadUseCases(context: BackupWorkflowContext) {
  const { deps, io } = context;

  return {
    async checkLudusaviInstalled(): Promise<boolean> {
      return true;
    },

    async getLudusaviExecutablePath(): Promise<string> {
      return (await io.getSetting(deps.db, "ludusavi_path")) || "native";
    },

    async getBackupDirectorySetting(): Promise<string> {
      return await io.getBackupRoot(deps.db, deps.userDataPath);
    },

    async refreshSqobaManifest(): Promise<void> {
      return undefined;
    },

    async findGameSavePaths(payload: FindGameSavePathsPayload) {
      const manifest = await loadManifest(context);
      const gameState = payload.gameId
        ? await io.getGameBackupState(deps.db, payload.gameId)
        : null;
      const overridePath = await getOverridePath(context, payload.gameId, gameState);
      const result = await io.findSavePath({
        gameName: payload.gameName,
        gameId: payload.gameId ?? null,
        overridePath,
        manifest,
      });

      if (!result.savePath && payload.gameId) {
        deps.emitRendererEvent("game:save-path-missing", {
          game_id: payload.gameId,
          game_name: payload.gameName,
        });
      }

      return {
        save_path: result.savePath,
        candidates: result.candidates,
      };
    },

    async findGameSaves(payload: FindGameSavePathsPayload) {
      const manifest = await loadManifest(context);
      const gameState = payload.gameId
        ? await io.getGameBackupState(deps.db, payload.gameId)
        : null;
      const overridePath = await getOverridePath(context, payload.gameId, gameState);
      const result = await io.discoverBackupInfo(
        payload.gameName,
        manifest,
        overridePath,
      );

      return result
        ? {
            game_name: result.gameName,
            save_path: result.savePath,
            registry_path: result.registryPath,
            total_size: result.totalSize,
            files: result.files,
          }
        : null;
    },

    async getGameBackups(gameId: string): Promise<BackupListItem[]> {
      return await io.listGameBackups(deps.db, gameId);
    },

    async shouldBackupBeforeLaunch(gameId: string): Promise<boolean> {
      const game = await io.getGameBackupState(deps.db, gameId);
      const enabled =
        (await io.getSetting(deps.db, "backup_before_launch")) === "true";
      return enabled && Number(game.backup_enabled ?? 0) === 1;
    },

    async checkBackupNeeded(payload: BackupCheckPayload): Promise<boolean> {
      const game = await io.getGameBackupState(deps.db, payload.gameId);
      const manifest = await loadManifest(context);
      const overridePath = await io.resolveGameOverridePath(
        deps.db,
        payload.gameId,
        game.save_path,
      );
      const currentSave = await io.findGameSaveArtifacts(
        payload.gameName,
        manifest,
        overridePath,
      );
      const lastBackup = await io.getLatestBackup(deps.db, payload.gameId);

      return await io.evaluateBackupNeeded({
        currentSave,
        lastBackup: lastBackup
          ? {
              id: lastBackup.id,
              gameId: lastBackup.game_id,
              backupPath: lastBackup.backup_path,
              backupSize: lastBackup.backup_size,
              createdAt: lastBackup.created_at,
              isAuto: lastBackup.is_auto,
              notes: lastBackup.notes,
            }
          : null,
      });
    },

    async checkRestoreNeeded(payload: BackupCheckPayload) {
      const game = await io.getGameBackupState(deps.db, payload.gameId);
      const manifest = await loadManifest(context);
      const overridePath = await io.resolveGameOverridePath(
        deps.db,
        payload.gameId,
        game.save_path,
      );
      const currentSave = await io.findGameSaveArtifacts(
        payload.gameName,
        manifest,
        overridePath,
      );
      const lastBackup = await io.getLatestBackup(deps.db, payload.gameId);
      const result = await io.evaluateRestoreNeeded({
        currentSave,
        lastBackup: lastBackup ? toBackupRecord(lastBackup) : null,
      });

      return {
        should_restore: result.shouldRestore,
        backup_id: result.backupId,
        current_size: result.currentSize,
        backup_size: result.backupSize,
      };
    },

    async getBackupSettings(): Promise<Record<string, string>> {
      const rows = await io.queryAll<{ key: string; value: string }>(
        deps.db,
        `SELECT key, value
         FROM settings
         WHERE key LIKE 'backup%' OR key = 'ludusavi_path' OR key = 'max_backups_per_game'
         ORDER BY key ASC`,
      );
      return Object.fromEntries(rows.map((row) => [row.key, row.value]));
    },
  };
}
