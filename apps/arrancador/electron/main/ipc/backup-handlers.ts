import { ipcMain } from "electron";
import { execute, queryAll, queryOne, runInTransaction } from "../helpers/db";
import {
  createBackup as createBackupArtifact,
  deleteBackup as deleteBackupArtifact,
  discoverBackupInfo,
  checkBackupNeeded as evaluateBackupNeeded,
  checkRestoreNeeded as evaluateRestoreNeeded,
  findGameSaves as findGameSaveArtifacts,
  findSavePath,
  restoreBackup as restoreBackupArtifact,
} from "../services/backup";
import {
  getBackupRoot,
  getGameBackupState,
  getLatestBackup,
  listGameBackups,
  loadManifestFromCache,
  reconcileBackupRows,
  releasedYear,
  resolveGameOverridePath,
} from "../services/backup-ipc-support";
import { getSetting, setSetting } from "../services/settings-store";
import type { WithRuntime } from "./types";

type BackupRecordRow = {
  id: string;
  game_id: string;
  backup_path: string;
  backup_size: number;
  created_at: string;
  is_auto: number;
  notes: string | null;
};

export interface BackupIpcDeps {
  withRuntime: WithRuntime;
  getUserDataPath: () => string;
  emitRendererEvent: (channel: string, payload: unknown) => void;
}

export function registerBackupIpcHandlers(deps: BackupIpcDeps) {
  const { withRuntime } = deps;

  ipcMain.handle("check_ludusavi_installed", withRuntime(async () => true));
  ipcMain.handle("get_ludusavi_executable_path", withRuntime(async ({ db }) => (await getSetting(db, "ludusavi_path")) || "native"));
  ipcMain.handle("set_ludusavi_path", withRuntime(async ({ db }, payload: { path: string }) => {
    await setSetting(db, "ludusavi_path", payload.path || "native");
  }));
  ipcMain.handle("set_backup_directory", withRuntime(async ({ db }, payload: { path: string }) => {
    await setSetting(db, "backup_directory", payload.path.trim());
  }));
  ipcMain.handle("get_backup_directory_setting", withRuntime(async ({ db }) => await getBackupRoot(db, deps.getUserDataPath())));
  ipcMain.handle("refresh_sqoba_manifest", withRuntime(async () => undefined));
  ipcMain.handle("find_game_save_paths", withRuntime(async ({ db }, payload: { gameName: string; gameId?: string }) => {
    const manifest = await loadManifestFromCache(deps.getUserDataPath());
    const gameState = payload.gameId ? await getGameBackupState(db, payload.gameId) : null;
    const overridePath =
      payload.gameId && gameState
        ? await resolveGameOverridePath(db, payload.gameId, gameState.save_path)
        : null;
    const result = await findSavePath({
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
  }));
  ipcMain.handle("find_game_saves", withRuntime(async ({ db }, payload: { gameName: string; gameId?: string }) => {
    const manifest = await loadManifestFromCache(deps.getUserDataPath());
    const gameState = payload.gameId ? await getGameBackupState(db, payload.gameId) : null;
    const overridePath =
      payload.gameId && gameState
        ? await resolveGameOverridePath(db, payload.gameId, gameState.save_path)
        : null;
    const result = await discoverBackupInfo(
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
  }));
  ipcMain.handle("create_backup", withRuntime(async ({ db, services }, payload: { gameId: string; gameName: string; isAuto: boolean; notes?: string }) => {
    const gameState = await getGameBackupState(db, payload.gameId);
    const manifest = await loadManifestFromCache(deps.getUserDataPath());
    const overridePath = await resolveGameOverridePath(db, payload.gameId, gameState.save_path);
    const backupRoot = await getBackupRoot(db, deps.getUserDataPath());
    const settings = await services.settings.getAllSettings();

    const backup = await createBackupArtifact({
      gameId: payload.gameId,
      gameName: payload.gameName,
      backupRoot,
      mode:
        settings.backup_compression_enabled && !settings.backup_skip_compression_once
          ? "zip"
          : "directory",
      compressionLevel: settings.backup_compression_level,
      skipCompressionOnce: settings.backup_skip_compression_once,
      overridePath,
      manifest,
      isAuto: payload.isAuto,
      notes: payload.notes,
      gameYear: releasedYear(gameState.released),
      maxBackupsPerGame: settings.max_backups_per_game,
      onProgress: async (progress) => {
        deps.emitRendererEvent("backup:progress", {
          game_id: payload.gameId,
          stage: progress.stage,
          message: progress.current,
          done: progress.done,
          total: progress.total,
        });
      },
    });

    await execute(
      db,
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

    await execute(
      db,
      `UPDATE games
       SET last_backup = ?1,
           backup_count = backup_count + 1,
           backup_enabled = 1
       WHERE id = ?2`,
      [backup.createdAt, payload.gameId],
    );

    await reconcileBackupRows(db, payload.gameId, settings.max_backups_per_game);

    if (settings.backup_skip_compression_once) {
      await setSetting(db, "backup_skip_compression_once", "false");
    }

    await services.achievements.recordAchievementEvent("backup_restore");

    return {
      id: backup.id ?? backup.backupPath,
      game_id: payload.gameId,
      backup_path: backup.backupPath,
      backup_size: backup.backupSize,
      created_at: backup.createdAt,
      is_auto: Boolean(backup.isAuto),
      notes: backup.notes ?? null,
    };
  }));
  ipcMain.handle("get_game_backups", withRuntime(async ({ db }, payload: { gameId: string }) => await listGameBackups(db, payload.gameId)));
  ipcMain.handle("restore_backup", withRuntime(async ({ db, services }, payload: { backupId: string }) => {
    const backup = await queryOne<BackupRecordRow>(
      db,
      `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
       FROM backups
       WHERE id = ?1`,
      [payload.backupId],
    );
    if (!backup) {
      throw new Error("Backup not found");
    }

    await restoreBackupArtifact({
      backupPath: backup.backup_path,
      onProgress: async (progress) => {
        deps.emitRendererEvent("restore:progress", {
          game_id: backup.game_id,
          stage: progress.stage,
          message: progress.current,
          done: progress.done,
          total: progress.total,
        });
      },
    });

    await services.achievements.recordAchievementEvent("backup_restore");
  }));
  ipcMain.handle("delete_backup", withRuntime(async ({ db }, payload: { backupId: string }) => {
    const backup = await queryOne<BackupRecordRow>(
      db,
      `SELECT id, game_id, backup_path, backup_size, created_at, is_auto, notes
       FROM backups
       WHERE id = ?1`,
      [payload.backupId],
    );
    if (!backup) {
      return;
    }
    await deleteBackupArtifact({ backupPath: backup.backup_path });
    await execute(db, "DELETE FROM backups WHERE id = ?1", [payload.backupId]);
    await reconcileBackupRows(db, backup.game_id, Number.MAX_SAFE_INTEGER);
  }));
  ipcMain.handle("should_backup_before_launch", withRuntime(async ({ db }, payload: { gameId: string }) => {
    const game = await getGameBackupState(db, payload.gameId);
    const enabled = (await getSetting(db, "backup_before_launch")) === "true";
    return enabled && Number(game.backup_enabled ?? 0) === 1;
  }));
  ipcMain.handle("check_backup_needed", withRuntime(async ({ db }, payload: { gameId: string; gameName: string }) => {
    const game = await getGameBackupState(db, payload.gameId);
    const manifest = await loadManifestFromCache(deps.getUserDataPath());
    const overridePath = await resolveGameOverridePath(db, payload.gameId, game.save_path);
    const currentSave = await findGameSaveArtifacts(payload.gameName, manifest, overridePath);
    const lastBackup = await getLatestBackup(db, payload.gameId);
    return await evaluateBackupNeeded({
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
  }));
  ipcMain.handle("check_restore_needed", withRuntime(async ({ db }, payload: { gameId: string; gameName: string }) => {
    const game = await getGameBackupState(db, payload.gameId);
    const manifest = await loadManifestFromCache(deps.getUserDataPath());
    const overridePath = await resolveGameOverridePath(db, payload.gameId, game.save_path);
    const currentSave = await findGameSaveArtifacts(payload.gameName, manifest, overridePath);
    const lastBackup = await getLatestBackup(db, payload.gameId);
    const result = await evaluateRestoreNeeded({
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
    return {
      should_restore: result.shouldRestore,
      backup_id: result.backupId,
      current_size: result.currentSize,
      backup_size: result.backupSize,
    };
  }));
  ipcMain.handle("get_backup_settings", withRuntime(async ({ db }) => {
    const rows = await queryAll<{ key: string; value: string }>(
      db,
      `SELECT key, value
       FROM settings
       WHERE key LIKE 'backup%' OR key = 'ludusavi_path' OR key = 'max_backups_per_game'
       ORDER BY key ASC`,
    );
    return Object.fromEntries(rows.map((row) => [row.key, row.value]));
  }));
  ipcMain.handle("update_backup_settings", withRuntime(async ({ db }, payload: { settings: Record<string, string> }) => {
    await runInTransaction(db, async (tx) => {
      for (const [key, value] of Object.entries(payload.settings)) {
        await execute(
          tx,
          "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
          [key, String(value)],
        );
      }
    });
  }));
}
