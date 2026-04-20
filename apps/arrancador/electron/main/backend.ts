import path from "node:path";

import {
  app,
  BrowserWindow,
  dialog,
  ipcMain,
  shell,
  type OpenDialogOptions,
} from "electron";

import { openGameDatabase, openSqliteDatabase } from "./db";
import { execute, queryAll, queryOne, runInTransaction } from "./helpers/db";
import type { DbLike, DbValue, GameSnapshot } from "./helpers/shared";
import { createAchievementsService } from "./services/achievements";
import { createArkGameObjectService } from "./services/ark-game-objects";
import { createCatalogueService } from "./services/catalogue";
import type {
  AppSettings,
  SettingsRepository,
} from "./services/contracts";
import { createGamesService } from "./services/games";
import { createMetadataService } from "./services/metadata";
import { createNotificationsService } from "./services/notifications";
import { createGameUsageReadModel } from "./services/ark-usage";
import { createPlaytimeStatsRepository } from "./services/playtime-stats";
import { createScanCancellation, getRunningProcesses, scanExecutablesStream } from "./services/scan";
import { createSettingsService } from "./services/settings";
import { createStatsService } from "./services/stats";
import { createSystemService } from "./services/system";
import {
  checkBackupNeeded as evaluateBackupNeeded,
  checkRestoreNeeded as evaluateRestoreNeeded,
  createBackup as createBackupArtifact,
  deleteBackup as deleteBackupArtifact,
  discoverBackupInfo,
  findGameSaves,
  findSavePath,
  restoreBackup as restoreBackupArtifact,
} from "./services/backup";
import { loadGameManifestCache, manifestCachePath } from "./services/backup/manifest";
import { resolveSavePathTemplate } from "./services/backup/save-locator";

type BackupRow = {
  id: string;
  game_id: string;
  backup_path: string;
  backup_size: number;
  created_at: string;
  is_auto: number;
  notes: string | null;
};

type GameBackupState = {
  id: string;
  name: string;
  released: string | null;
  save_path: string | null;
  backup_enabled: number;
};

type RuntimeServices = ReturnType<typeof createRuntimeServices>;

interface RuntimeState {
  db: DbLike;
  services: RuntimeServices;
  currentScan: ReturnType<typeof createScanCancellation> | null;
}

let runtimeState: RuntimeState | null = null;
let ipcRegistered = false;

function getDbPath() {
  return path.join(app.getPath("userData"), "arrancador.db");
}

function getArkDbPath() {
  const override = process.env.ARK_DB_PATH?.trim();
  if (override) {
    return override;
  }
  return path.join(app.getPath("appData"), "Kepler", "ark.db");
}

async function getSettingsMap(db: DbLike): Promise<Record<string, string>> {
  const rows = await queryAll<{ key: string; value: string }>(
    db,
    "SELECT key, value FROM settings",
  );

  return Object.fromEntries(rows.map((row) => [row.key, row.value]));
}

async function getSetting(db: DbLike, key: string): Promise<string | null> {
  const row = await queryOne<{ value: string }>(
    db,
    "SELECT value FROM settings WHERE key = ?1",
    [key],
  );
  return row?.value ?? null;
}

async function setSetting(db: DbLike, key: string, value: string): Promise<void> {
  await execute(
    db,
    "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
    [key, value],
  );
}

async function getGameBackupState(db: DbLike, id: string): Promise<GameBackupState> {
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

function defaultBackupDirectory() {
  return path.join(app.getPath("userData"), "backups");
}

async function getBackupRoot(db: DbLike): Promise<string> {
  const custom = (await getSetting(db, "backup_directory"))?.trim() ?? "";
  return custom || defaultBackupDirectory();
}

function mapBackupRow(row: BackupRow) {
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

function releasedYear(released: string | null): string | null {
  const year = released?.split("-")[0]?.trim();
  return year ? year : null;
}

function emitRendererEvent(channel: string, payload: unknown) {
  for (const window of BrowserWindow.getAllWindows()) {
    window.webContents.send(channel, payload);
  }
}

async function loadManifestFromCache() {
  const cachePath = manifestCachePath(app.getPath("userData"));
  return await loadGameManifestCache(cachePath);
}

async function resolveGameOverridePath(
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

async function listGameBackups(db: DbLike, gameId: string) {
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

async function getLatestBackup(db: DbLike, gameId: string) {
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

async function reconcileBackupRows(db: DbLike, gameId: string, maxBackups: number) {
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

function createRuntimeServices(db: DbLike) {
  const arkDbPath = getArkDbPath();
  const settingsRepository: SettingsRepository = {
    listSettings: () => getSettingsMap(db),
    upsertSettings: async (entries) => {
      await runInTransaction(db, async (tx) => {
        for (const [key, value] of Object.entries(entries)) {
          await execute(
            tx,
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            [key, value],
          );
        }
      });
    },
    getSetting: (key) => getSetting(db, key),
    upsertSetting: (key, value) => setSetting(db, key, value),
    listScanDirectories: async () => {
      const rows = await queryAll<{ path: string }>(
        db,
        "SELECT path FROM scan_directories ORDER BY path ASC",
      );
      return rows.map((row) => row.path);
    },
    addScanDirectory: async (dirPath) => {
      await execute(
        db,
        "INSERT OR IGNORE INTO scan_directories (path) VALUES (?1)",
        [dirPath],
      );
    },
    removeScanDirectory: async (dirPath) => {
      await execute(db, "DELETE FROM scan_directories WHERE path = ?1", [dirPath]);
    },
  };

  const arkGameObjects = createArkGameObjectService({ arkDbPath });
  const usageReadModel = createGameUsageReadModel({
    legacyDb: db,
    arkDbPath,
    arkGameObjects,
  });
  const playtimeRepository = createPlaytimeStatsRepository({
    legacyDb: db,
    arkDbPath,
  });

  const games = createGamesService({
    db,
    usageReadModel,
    arkGameObjectSync: arkGameObjects,
  });
  const settings = createSettingsService(settingsRepository);
  const stats = createStatsService(playtimeRepository);
  const achievements = createAchievementsService({ db });
  const notifications = createNotificationsService({ db });
  const system = createSystemService();
  const metadata = createMetadataService({
    settings: {
      getRawgApiKey: () => getSetting(db, "rawg_api_key"),
      setRawgApiKey: async (key) => {
        await setSetting(db, "rawg_api_key", key);
      },
    },
    games: {
      applyRawgMetadata: async (update) =>
        await games.updateGame({
          id: update.gameId,
          name: update.name,
          rawg_id: update.rawgId,
          description: update.description,
          released: update.released,
          background_image: update.background_image,
          metacritic: update.metacritic,
          rating: update.rating,
          genres: update.genres,
          platforms: update.platforms,
          developers: update.developers,
          publishers: update.publishers,
        }) as unknown as GameSnapshot,
    },
  });
  const catalogue = createCatalogueService({
    db,
    library: {
      listGames: async () => {
        const items = await games.getAllGames();
        return items.map((item) => ({
          id: item.id,
          name: item.name,
          exe_name: item.exe_name,
        }));
      },
    },
  });

  return {
    games,
    settings,
    stats,
    achievements,
    notifications,
    system,
    metadata,
    catalogue,
  };
}

function getRuntimeState(): RuntimeState {
  if (!runtimeState) {
    throw new Error("App runtime is not initialized");
  }
  return runtimeState;
}

function registerIpcHandlers() {
  const withRuntime = <TArgs extends unknown[], TResult>(
    handler: (runtime: RuntimeState, ...args: TArgs) => Promise<TResult> | TResult,
  ) => {
    return async (_event: Electron.IpcMainInvokeEvent, ...args: TArgs) =>
      await handler(getRuntimeState(), ...args);
  };

  ipcMain.handle("get_all_games", withRuntime(async ({ services }) => await services.games.getAllGames()));
  ipcMain.handle("get_game", withRuntime(async ({ services }, payload: { id: string }) => await services.games.getGame(payload.id)));
  ipcMain.handle("add_game", withRuntime(async ({ services }, payload: { game: { name: string; exe_path: string; exe_name: string } }) => await services.games.addGame(payload.game)));
  ipcMain.handle("add_games_batch", withRuntime(async ({ services }, payload: { games: Array<{ name: string; exe_path: string; exe_name: string }> }) => await services.games.addGamesBatch(payload.games)));
  ipcMain.handle("update_game", withRuntime(async ({ services }, payload: { update: Record<string, DbValue> & { id: string } }) => await services.games.updateGame(payload.update)));
  ipcMain.handle("delete_game", withRuntime(async ({ services }, payload: { id: string }) => {
    await services.games.deleteGame(payload.id);
  }));
  ipcMain.handle("toggle_favorite", withRuntime(async ({ services }, payload: { id: string }) => await services.games.toggleFavorite(payload.id)));
  ipcMain.handle("get_favorites", withRuntime(async ({ services }) => await services.games.getFavorites()));
  ipcMain.handle("record_game_launch", withRuntime(async ({ services }, payload: { id: string }) => await services.games.recordGameLaunch(payload.id)));
  ipcMain.handle("search_games", withRuntime(async ({ services }, payload: { query: string }) => await services.games.searchGames(payload.query)));
  ipcMain.handle("game_exists_by_path", withRuntime(async ({ services }, payload: { exePath: string }) => await services.games.gameExistsByPath(payload.exePath)));
  ipcMain.handle("is_game_installed", withRuntime(async ({ services }, payload: { id: string }) => await services.games.isGameInstalled(payload.id)));
  ipcMain.handle("launch_game", withRuntime(async ({ services }, payload: { id: string }) => {
    await services.games.launchGame(payload.id);
    await services.achievements.recordAchievementEvent("game_launch");
  }));
  ipcMain.handle("get_running_instances", withRuntime(async ({ services }, payload: { id: string }) => await services.games.getRunningInstances(payload.id)));
  ipcMain.handle("kill_game_processes", withRuntime(async ({ services }, payload: { id: string }) => await services.games.killGameProcesses(payload.id)));
  ipcMain.handle("resolve_shortcut_target", withRuntime(async ({ services }, payload: { path: string }) => await services.games.resolveShortcutTarget(payload.path)));

  ipcMain.handle("search_rawg", withRuntime(async ({ services }, payload: { query: string }) => await services.metadata.searchRawg(payload.query)));
  ipcMain.handle("get_rawg_game_details", withRuntime(async ({ services }, payload: { rawgId: number }) => await services.metadata.getRawgGameDetails(payload.rawgId)));
  ipcMain.handle("apply_rawg_metadata", withRuntime(async ({ services }, payload: { gameId: string; rawgId: number; rename: boolean }) => await services.metadata.applyRawgMetadata(payload.gameId, payload.rawgId, payload.rename)));
  ipcMain.handle("set_rawg_api_key", withRuntime(async ({ services }, payload: { key: string }) => {
    await services.metadata.setRawgApiKey(payload.key);
  }));
  ipcMain.handle("get_rawg_api_key", withRuntime(async ({ services }) => await services.metadata.getRawgApiKey()));

  ipcMain.handle("get_all_achievements", withRuntime(async ({ services }, payload: { unlockedOnly: boolean }) => await services.achievements.getAllAchievements(payload.unlockedOnly)));
  ipcMain.handle("seed_default_achievements", withRuntime(async ({ services }) => await services.achievements.seedDefaultAchievements()));
  ipcMain.handle("record_achievement_event", withRuntime(async ({ services }, payload: { eventTrigger: string; eventContext?: string }) => await services.achievements.recordAchievementEvent(payload.eventTrigger, payload.eventContext)));

  ipcMain.handle("get_all_settings", withRuntime(async ({ services }) => await services.settings.getAllSettings()));
  ipcMain.handle("update_settings", withRuntime(async ({ services }, payload: { settings: AppSettings }) => {
    await services.settings.updateSettings(payload.settings);
  }));
  ipcMain.handle("get_setting", withRuntime(async ({ services }, payload: { key: string }) => await services.settings.getSetting(payload.key)));
  ipcMain.handle("set_setting", withRuntime(async ({ services }, payload: { key: string; value: string }) => {
    await services.settings.setSetting(payload.key, payload.value);
  }));
  ipcMain.handle("add_scan_directory", withRuntime(async ({ services }, payload: { path: string }) => {
    await services.settings.addScanDirectory(payload.path);
  }));
  ipcMain.handle("get_scan_directories", withRuntime(async ({ services }) => await services.settings.getScanDirectories()));
  ipcMain.handle("remove_scan_directory", withRuntime(async ({ services }, payload: { path: string }) => {
    await services.settings.removeScanDirectory(payload.path);
  }));

  ipcMain.handle("get_playtime_stats", withRuntime(async ({ services }, payload: { start?: string; end?: string } = {}) => await services.stats.getPlaytimeStats(payload?.start, payload?.end)));
  ipcMain.handle("get_running_processes", withRuntime(async () => await getRunningProcesses()));
  ipcMain.handle("get_system_info", withRuntime(async ({ services }) => await services.system.getSystemInfo()));
  ipcMain.handle("test_disk_speed", withRuntime(async ({ services }, payload: { mountPoint: string }) => await services.system.testDiskSpeed(payload.mountPoint)));

  ipcMain.handle("list_notifications", withRuntime(async ({ services }, payload: { unread_only: boolean }) => await services.notifications.listNotifications(payload.unread_only)));
  ipcMain.handle("create_notification", withRuntime(async ({ services }, payload: { level: string; title: string; message: string; source?: string }) => await services.notifications.createNotification(payload.level, payload.title, payload.message, payload.source)));
  ipcMain.handle("mark_notification_read", withRuntime(async ({ services }, payload: { id: string }) => await services.notifications.markNotificationRead(payload.id)));
  ipcMain.handle("mark_all_notifications_read", withRuntime(async ({ services }) => await services.notifications.markAllNotificationsRead()));
  ipcMain.handle("clear_notifications", withRuntime(async ({ services }) => await services.notifications.clearNotifications()));

  ipcMain.handle("get_catalogue_items", withRuntime(async ({ services }, payload: { source: string | null }) => await services.catalogue.getCatalogueItems(payload.source)));
  ipcMain.handle("search_catalogue", withRuntime(async ({ services }, payload: { query: string; source: string | null }) => await services.catalogue.searchCatalogue(payload.query, payload.source)));
  ipcMain.handle("upsert_catalogue_item", withRuntime(async ({ services }, payload: { input: { id?: number; rawg_id: number; name: string; payload: string; source?: string } }) => await services.catalogue.upsertCatalogueItem({
    id: payload.input.id,
    rawgId: payload.input.rawg_id,
    name: payload.input.name,
    payload: payload.input.payload,
    source: payload.input.source,
  })));
  ipcMain.handle("delete_catalogue_item", withRuntime(async ({ services }, payload: { id: number }) => await services.catalogue.deleteCatalogueItem(payload.id)));
  ipcMain.handle("sync_library_to_catalogue", withRuntime(async ({ services }) => await services.catalogue.syncLibraryToCatalogue()));

  ipcMain.handle("check_ludusavi_installed", withRuntime(async () => true));
  ipcMain.handle("get_ludusavi_executable_path", withRuntime(async ({ db }) => (await getSetting(db, "ludusavi_path")) || "native"));
  ipcMain.handle("set_ludusavi_path", withRuntime(async ({ db }, payload: { path: string }) => {
    await setSetting(db, "ludusavi_path", payload.path || "native");
  }));
  ipcMain.handle("set_backup_directory", withRuntime(async ({ db }, payload: { path: string }) => {
    await setSetting(db, "backup_directory", payload.path.trim());
  }));
  ipcMain.handle("get_backup_directory_setting", withRuntime(async ({ db }) => await getBackupRoot(db)));
  ipcMain.handle("refresh_sqoba_manifest", withRuntime(async () => undefined));
  ipcMain.handle("find_game_save_paths", withRuntime(async ({ db }, payload: { gameName: string; gameId?: string }) => {
    const manifest = await loadManifestFromCache();
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
      emitRendererEvent("game:save-path-missing", {
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
    const manifest = await loadManifestFromCache();
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
    const manifest = await loadManifestFromCache();
    const overridePath = await resolveGameOverridePath(db, payload.gameId, gameState.save_path);
    const backupRoot = await getBackupRoot(db);
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
        emitRendererEvent("backup:progress", {
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
    const backup = await queryOne<BackupRow>(
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
        emitRendererEvent("restore:progress", {
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
    const backup = await queryOne<BackupRow>(
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
    const manifest = await loadManifestFromCache();
    const overridePath = await resolveGameOverridePath(db, payload.gameId, game.save_path);
    const currentSave = await findGameSaves(payload.gameName, manifest, overridePath);
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
    const manifest = await loadManifestFromCache();
    const overridePath = await resolveGameOverridePath(db, payload.gameId, game.save_path);
    const currentSave = await findGameSaves(payload.gameName, manifest, overridePath);
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

  ipcMain.handle("dialog_open", withRuntime(async (_runtime, payload: OpenDialogOptions) => {
    const window = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    const result = window
      ? await dialog.showOpenDialog(window, payload)
      : await dialog.showOpenDialog(payload);
    if (result.canceled) {
      return null;
    }
    if (payload.multiple) {
      return result.filePaths;
    }
    return result.filePaths[0] ?? null;
  }));
  ipcMain.handle("get_window_platform", withRuntime(async () => process.platform));
  ipcMain.handle("window_minimize", withRuntime(async () => {
    const window = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    window?.minimize();
  }));
  ipcMain.handle("window_toggle_maximize", withRuntime(async () => {
    const window = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    if (!window) {
      return;
    }
    if (window.isMaximized()) {
      window.unmaximize();
      return;
    }
    window.maximize();
  }));
  ipcMain.handle("window_close", withRuntime(async () => {
    const window = BrowserWindow.getFocusedWindow() ?? BrowserWindow.getAllWindows()[0];
    window?.close();
  }));
  ipcMain.handle("shell_open_path", withRuntime(async (_runtime, payload: { path: string }) => await shell.openPath(payload.path)));
  ipcMain.handle("shell_open_external", withRuntime(async (_runtime, payload: { url: string }) => {
    await shell.openExternal(payload.url);
  }));
  ipcMain.handle("get_autostart_state", withRuntime(async () => app.getLoginItemSettings().openAtLogin));
  ipcMain.handle("set_autostart_state", withRuntime(async (_runtime, payload: { enabled: boolean }) => {
    app.setLoginItemSettings({
      openAtLogin: payload.enabled,
      path: process.execPath,
    });
  }));
  ipcMain.handle("cancel_scan", withRuntime(async (runtime) => {
    runtime.currentScan?.cancel();
    runtime.currentScan = null;
  }));
  ipcMain.handle("scan_executables_stream", withRuntime(async (runtime, payload: { dir: string }) => {
    runtime.currentScan?.cancel();
    const cancellation = createScanCancellation();
    runtime.currentScan = cancellation;
    let count = 0;

    try {
      count = await scanExecutablesStream(payload.dir, {
        signal: cancellation.signal,
        onEntry: async (entry) => {
          emitRendererEvent("scan:entry", entry);
        },
      });
      await runtime.services.achievements.recordAchievementEvent("scan_complete");
      return count;
    } finally {
      if (runtime.currentScan === cancellation) {
        runtime.currentScan = null;
      }
      emitRendererEvent("scan:done", { count });
    }
  }));
}

async function createRuntime() {
  const db = await openGameDatabase(
    openSqliteDatabase(getDbPath()),
  );

  runtimeState = {
    db,
    services: createRuntimeServices(db),
    currentScan: null,
  };
}

export async function initializeBackend(): Promise<void> {
  if (!runtimeState) {
    await createRuntime();
  }

  if (!ipcRegistered) {
    registerIpcHandlers();
    ipcRegistered = true;
  }
}
