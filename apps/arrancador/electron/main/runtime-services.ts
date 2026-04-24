import { execute, queryAll, runInTransaction } from "./helpers/db";
import type { DbLike, GameSnapshot } from "./helpers/shared";
import { createAchievementsService } from "./services/achievements";
import { createArkGameObjectService } from "./services/ark-game-objects";
import { createGameUsageReadModel } from "./services/ark-usage";
import { createCatalogueService } from "./services/catalogue";
import type { SettingsRepository } from "./services/contracts";
import { createGamesService } from "./services/games";
import { createMetadataService } from "./services/metadata";
import { createNotificationsService } from "./services/notifications";
import { createPlaytimeStatsRepository } from "./services/playtime-stats";
import { createSettingsService } from "./services/settings";
import { getSetting, getSettingsMap, setSetting } from "./services/settings-store";
import { createStatsService } from "./services/stats";
import { createSystemService } from "./services/system";
import { createUsageProcessSearchService } from "./services/usage-process-search";

export interface RuntimeServicesDeps {
  db: DbLike;
  arkDbPath: string;
  fallbackArkDbPath: string;
}

function createSettingsRepository(db: DbLike): SettingsRepository {
  return {
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
}

export function createRuntimeServices({
  db,
  arkDbPath,
  fallbackArkDbPath,
}: RuntimeServicesDeps) {
  const settingsRepository = createSettingsRepository(db);
  const arkGameObjects = createArkGameObjectService({ arkDbPath });
  const usageReadModel = createGameUsageReadModel({
    legacyDb: db,
    arkDbPath,
    fallbackArkDbPath,
    arkGameObjects,
  });
  const playtimeRepository = createPlaytimeStatsRepository(
    db,
    arkDbPath,
    fallbackArkDbPath,
  );

  const games = createGamesService({
    db,
    usageReadModel,
    arkGameObjectSync: arkGameObjects,
  });
  const settings = createSettingsService(settingsRepository);
  const stats = createStatsService(playtimeRepository);
  const usageProcessSearch = createUsageProcessSearchService({
    arkDbPath,
    fallbackArkDbPath,
  });
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
    usageProcessSearch,
    achievements,
    notifications,
    system,
    metadata,
    catalogue,
  };
}
