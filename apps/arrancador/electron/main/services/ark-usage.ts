import { queryAll } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import { openSqliteDatabase } from "../db";
import type { PlaytimeStatsRepository } from "./contracts";
import type { ArkGameObjectService } from "./ark-game-objects";
import type { Game } from "./games/types";

type GamePathRow = {
  id: string;
  name: string;
  normalized_path: string;
};

type UsageAggregateRow = {
  normalized_path: string;
  total_seconds: number;
  last_played: string | null;
};

type DailyTotalRow = {
  date: string;
  seconds: number;
};

type GameUsageAggregate = {
  totalSeconds: number;
  lastPlayed: string | null;
};

export interface GameUsageReadModel {
  hydrateGame(game: Game): Promise<Game>;
  hydrateGames(games: Game[]): Promise<Game[]>;
}

interface ArkUsageOptions {
  legacyDb: DbLike;
  arkDbPath: string;
  arkGameObjects?: ArkGameObjectService;
}

function normalizeExePath(exePath: string): string {
  return exePath.trim().replaceAll("/", "\\").toLowerCase();
}

function buildParameterizedList(values: readonly string[], startIndex = 1): string {
  return values.map((_, index) => `?${index + startIndex}`).join(", ");
}

function createLegacyPlaytimeStatsRepository(db: DbLike): PlaytimeStatsRepository {
  return {
    async getDailyTotals(rangeStart, rangeEnd) {
      return await queryAll<{ date: string; seconds: number }>(
        db,
        `SELECT date, SUM(seconds) AS seconds
         FROM playtime_daily
         WHERE date BETWEEN ?1 AND ?2
         GROUP BY date
         ORDER BY date`,
        [rangeStart, rangeEnd],
      );
    },

    async getPerGameTotals(rangeStart, rangeEnd) {
      return await queryAll<{ id: string; name: string; seconds: number }>(
        db,
        `SELECT games.id, games.name, SUM(playtime_daily.seconds) AS seconds
         FROM playtime_daily
         JOIN games ON games.id = playtime_daily.game_id
         WHERE playtime_daily.date BETWEEN ?1 AND ?2
         GROUP BY games.id, games.name
         HAVING seconds > 0
         ORDER BY seconds DESC`,
        [rangeStart, rangeEnd],
      );
    },
  };
}

async function loadGamePathIndex(
  legacyDb: DbLike,
  ids?: readonly string[],
): Promise<GamePathRow[]> {
  if (ids && ids.length === 0) {
    return [];
  }

  if (ids && ids.length > 0) {
    const placeholders = buildParameterizedList(ids);
    return await queryAll<GamePathRow>(
      legacyDb,
      `SELECT id, name, LOWER(REPLACE(exe_path, '/', '\\')) AS normalized_path
       FROM games
       WHERE id IN (${placeholders})
       ORDER BY name ASC`,
      ids,
    );
  }

  return await queryAll<GamePathRow>(
    legacyDb,
    `SELECT id, name, LOWER(REPLACE(exe_path, '/', '\\')) AS normalized_path
     FROM games
     ORDER BY name ASC`,
  );
}

function tryOpenArkDb(arkDbPath: string): DbLike | null {
  try {
    return openSqliteDatabase(arkDbPath, {
      readonly: true,
      fileMustExist: true,
      timeoutMs: 2000,
    });
  } catch {
    return null;
  }
}

async function queryUsageAggregatesByPath(
  arkDb: DbLike,
  normalizedPaths: readonly string[],
): Promise<Map<string, GameUsageAggregate>> {
  if (normalizedPaths.length === 0) {
    return new Map();
  }

  const placeholders = buildParameterizedList(normalizedPaths);
  const rows = await queryAll<UsageAggregateRow>(
    arkDb,
    `SELECT tracked_apps.normalized_exe_path AS normalized_path,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS total_seconds,
            MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_played
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     WHERE tracked_apps.normalized_exe_path IN (${placeholders})
     GROUP BY tracked_apps.normalized_exe_path`,
    normalizedPaths,
  );

  return new Map(
    rows.map((row) => [
      row.normalized_path,
      {
        totalSeconds: Number(row.total_seconds ?? 0),
        lastPlayed: row.last_played ?? null,
      },
    ]),
  );
}

async function queryDailyTotals(
  arkDb: DbLike,
  normalizedPaths: readonly string[],
  rangeStart: string,
  rangeEnd: string,
): Promise<DailyTotalRow[]> {
  if (normalizedPaths.length === 0) {
    return [];
  }

  const placeholders = buildParameterizedList(normalizedPaths);
  const dateStartIndex = normalizedPaths.length + 1;
  const dateEndIndex = normalizedPaths.length + 2;
  return await queryAll<DailyTotalRow>(
    arkDb,
    `SELECT SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10) AS date,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS seconds
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     WHERE tracked_apps.normalized_exe_path IN (${placeholders})
       AND SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10)
           BETWEEN ?${dateStartIndex} AND ?${dateEndIndex}
     GROUP BY date
     ORDER BY date`,
    [...normalizedPaths, rangeStart, rangeEnd],
  );
}

async function queryPerGameTotals(
  arkDb: DbLike,
  gameIndex: readonly GamePathRow[],
  rangeStart: string,
  rangeEnd: string,
): Promise<Array<{ id: string; name: string; seconds: number }>> {
  if (gameIndex.length === 0) {
    return [];
  }

  const normalizedPaths = gameIndex.map((row) => row.normalized_path);
  const placeholders = buildParameterizedList(normalizedPaths);
  const dateStartIndex = normalizedPaths.length + 1;
  const dateEndIndex = normalizedPaths.length + 2;
  const rows = await queryAll<{ normalized_path: string; seconds: number }>(
    arkDb,
    `SELECT tracked_apps.normalized_exe_path AS normalized_path,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS seconds
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     WHERE tracked_apps.normalized_exe_path IN (${placeholders})
       AND SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10)
           BETWEEN ?${dateStartIndex} AND ?${dateEndIndex}
     GROUP BY tracked_apps.normalized_exe_path
     HAVING seconds > 0
     ORDER BY seconds DESC`,
    [...normalizedPaths, rangeStart, rangeEnd],
  );

  const byPath = new Map(rows.map((row) => [row.normalized_path, Number(row.seconds ?? 0)]));
  return gameIndex
    .map((game) => ({
      id: game.id,
      name: game.name,
      seconds: byPath.get(game.normalized_path) ?? 0,
    }))
    .filter((game) => game.seconds > 0)
    .sort((left, right) => right.seconds - left.seconds);
}

export function createGameUsageReadModel({
  legacyDb,
  arkDbPath,
  arkGameObjects,
}: ArkUsageOptions): GameUsageReadModel {
  const hydrateGames = async (games: Game[]): Promise<Game[]> => {
    if (games.length === 0) {
      return games;
    }

    const arkDb = tryOpenArkDb(arkDbPath);
    if (!arkDb) {
      return games;
    }

    try {
      const metricsByPath = await queryUsageAggregatesByPath(
        arkDb,
        games.map((game) => normalizeExePath(game.exe_path)),
      );

      const hydrated = games.map((game) => {
        const metrics = metricsByPath.get(normalizeExePath(game.exe_path));
        if (!metrics) {
          return game;
        }

        return {
          ...game,
          total_playtime: metrics.totalSeconds,
          last_played: metrics.lastPlayed,
        };
      });

      return arkGameObjects
        ? await arkGameObjects.hydrateGames(hydrated)
        : hydrated;
    } catch {
      return arkGameObjects
        ? await arkGameObjects.hydrateGames(games).catch(() => games)
        : games;
    }
  };

  return {
    async hydrateGame(game) {
      const [hydrated] = await hydrateGames([game]);
      return hydrated;
    },
    hydrateGames,
  };
}

export function createPlaytimeStatsRepository({
  legacyDb,
  arkDbPath,
}: ArkUsageOptions): PlaytimeStatsRepository {
  const legacy = createLegacyPlaytimeStatsRepository(legacyDb);

  return {
    async getDailyTotals(rangeStart, rangeEnd) {
      const arkDb = tryOpenArkDb(arkDbPath);
      if (!arkDb) {
        return await legacy.getDailyTotals(rangeStart, rangeEnd);
      }

      try {
        const gameIndex = await loadGamePathIndex(legacyDb);
        const rows = await queryDailyTotals(
          arkDb,
          gameIndex.map((game) => game.normalized_path),
          rangeStart,
          rangeEnd,
        );

        return rows.length > 0
          ? rows.map((row) => ({
              date: row.date,
              seconds: Number(row.seconds ?? 0),
            }))
          : await legacy.getDailyTotals(rangeStart, rangeEnd);
      } catch {
        return await legacy.getDailyTotals(rangeStart, rangeEnd);
      }
    },

    async getPerGameTotals(rangeStart, rangeEnd) {
      const arkDb = tryOpenArkDb(arkDbPath);
      if (!arkDb) {
        return await legacy.getPerGameTotals(rangeStart, rangeEnd);
      }

      try {
        const gameIndex = await loadGamePathIndex(legacyDb);
        const rows = await queryPerGameTotals(arkDb, gameIndex, rangeStart, rangeEnd);
        return rows.length > 0 ? rows : await legacy.getPerGameTotals(rangeStart, rangeEnd);
      } catch {
        return await legacy.getPerGameTotals(rangeStart, rangeEnd);
      }
    },
  };
}
