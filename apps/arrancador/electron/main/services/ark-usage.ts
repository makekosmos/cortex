import { openSqliteDatabase } from "../db";
import { queryAll } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import type { ArkGameObjectService } from "./ark-game-objects";
import {
  applyUsageAggregatesToGames,
  buildParameterizedList,
  buildUsageBindingIndexFromGames,
  buildUsageWhereClause,
  findGameIdForTrackedRow,
  type TrackedUsageAggregateRow,
  type TrackedUsageDailyRow,
  type UsageBindingIndex,
  zeroUsageFields,
} from "./ark-usage/bindings";
import type { PlaytimeStatsRepository } from "./contracts";
import { listGameProcessBindingsByGameId } from "./game-process-bindings";
import type { Game } from "./games/types";

type GamePathRow = {
  id: string;
  name: string;
  normalized_path: string;
};

export interface GameUsageReadModel {
  hydrateGame(game: Game): Promise<Game>;
  hydrateGames(games: Game[]): Promise<Game[]>;
}

interface ArkUsageOptions {
  legacyDb: DbLike;
  arkDbPath: string;
  fallbackArkDbPath?: string;
  arkGameObjects?: ArkGameObjectService;
  resolveUsageDb?: () => Promise<{ db: DbLike | null; path: string | null }>;
}

function createUsageDbResolver(options: ArkUsageOptions) {
  return (
    options.resolveUsageDb ??
    (() =>
      resolveUsageTrackerDb({
        arkDbPath: options.arkDbPath,
        fallbackArkDbPath: options.fallbackArkDbPath,
      }))
  );
}

async function closeDbQuietly(db: DbLike | null): Promise<void> {
  try {
    await Promise.resolve(db?.close?.());
  } catch {
    // Ignore close failures for read-only usage snapshots.
  }
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

async function hasUsageSessionRows(arkDb: DbLike): Promise<boolean> {
  try {
    const row = await arkDb.get<{ present: number }>(
      "SELECT 1 AS present FROM usage_sessions LIMIT 1",
    );
    return Number(row?.present ?? 0) === 1;
  } catch {
    return false;
  }
}

export async function resolveUsageTrackerDb(
  options: {
    arkDbPath: string;
    fallbackArkDbPath?: string;
    openDb?: (arkDbPath: string) => DbLike | null;
    hasTrackerRows?: (arkDb: DbLike) => Promise<boolean>;
  },
): Promise<{ db: DbLike | null; path: string | null }> {
  const openDb = options.openDb ?? tryOpenArkDb;
  const hasTrackerRows = options.hasTrackerRows ?? hasUsageSessionRows;
  const candidates = [
    options.arkDbPath,
    options.fallbackArkDbPath,
  ].filter((value, index, values): value is string => Boolean(value) && values.indexOf(value) === index);

  let firstOpen: { db: DbLike; path: string } | null = null;

  for (const candidate of candidates) {
    const db = openDb(candidate);
    if (!db) {
      continue;
    }

    if (!firstOpen) {
      firstOpen = { db, path: candidate };
    }

    if (await hasTrackerRows(db)) {
      if (firstOpen && firstOpen.db !== db) {
        await Promise.resolve(firstOpen.db.close?.());
      }
      return { db, path: candidate };
    }

    if (firstOpen.db !== db) {
      await Promise.resolve(db.close?.());
    }
  }

  return firstOpen ?? { db: null, path: null };
}

async function buildUsageBindingIndexFromLegacyDb(
  legacyDb: DbLike,
  ids?: readonly string[],
): Promise<UsageBindingIndex> {
  const gameRows = await loadGamePathIndex(legacyDb, ids);
  const bindingsByGameId = await listGameProcessBindingsByGameId(
    legacyDb,
    gameRows.map((game) => game.id),
  );

  const games = gameRows.map((row) => ({
    id: row.id,
    name: row.name,
    exe_path: row.normalized_path,
    process_bindings: bindingsByGameId.get(row.id) ?? [],
  })) as Array<Pick<Game, "id" | "name" | "exe_path" | "process_bindings">>;

  const normalizedGames = games.map((game) => ({
    ...game,
    exe_path: game.exe_path,
  })) as Game[];

  return buildUsageBindingIndexFromGames(normalizedGames);
}

async function queryTrackedUsageAggregates(
  arkDb: DbLike,
  index: UsageBindingIndex,
): Promise<TrackedUsageAggregateRow[]> {
  const { whereSql, params } = buildUsageWhereClause(index);
  if (!whereSql) {
    return [];
  }

  return await queryAll<TrackedUsageAggregateRow>(
    arkDb,
    `SELECT tracked_apps.id AS tracked_app_id,
            tracked_apps.normalized_exe_path AS normalized_path,
            LOWER(COALESCE(tracked_apps.process_name, '')) AS normalized_process_name,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS total_seconds,
            SUM(CASE WHEN usage_sessions.foreground_ms > 0 THEN 1 ELSE 0 END) AS session_count,
            MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_played
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     ${whereSql}
     GROUP BY tracked_apps.id, tracked_apps.normalized_exe_path, normalized_process_name`,
    params,
  );
}

async function queryTrackedUsageDailyTotals(
  arkDb: DbLike,
  index: UsageBindingIndex,
  rangeStart: string,
  rangeEnd: string,
): Promise<TrackedUsageDailyRow[]> {
  const { whereSql, params } = buildUsageWhereClause(index);
  if (!whereSql) {
    return [];
  }

  const startIndex = params.length + 1;
  const endIndex = params.length + 2;
  return await queryAll<TrackedUsageDailyRow>(
    arkDb,
    `SELECT tracked_apps.id AS tracked_app_id,
            tracked_apps.normalized_exe_path AS normalized_path,
            LOWER(COALESCE(tracked_apps.process_name, '')) AS normalized_process_name,
            SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10) AS date,
            CAST(SUM(usage_sessions.foreground_ms) / 1000 AS INTEGER) AS seconds
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     ${whereSql}
       AND SUBSTR(COALESCE(usage_sessions.ended_at, usage_sessions.started_at), 1, 10)
           BETWEEN ?${startIndex} AND ?${endIndex}
     GROUP BY tracked_apps.id, tracked_apps.normalized_exe_path, normalized_process_name, date
     HAVING seconds > 0
     ORDER BY date ASC, tracked_apps.id ASC`,
    [...params, rangeStart, rangeEnd],
  );
}

export function createGameUsageReadModel({
  legacyDb,
  arkDbPath,
  fallbackArkDbPath,
  arkGameObjects,
  resolveUsageDb,
}: ArkUsageOptions): GameUsageReadModel {
  const resolveEffectiveUsageDb = createUsageDbResolver({
    legacyDb,
    arkDbPath,
    fallbackArkDbPath,
    arkGameObjects,
    resolveUsageDb,
  });

  const hydrateGames = async (games: Game[]): Promise<Game[]> => {
    if (games.length === 0) {
      return games;
    }

    const { db: arkDb } = await resolveEffectiveUsageDb();
    const usageIndex = buildUsageBindingIndexFromGames(games);

    try {
      const trackedRows = arkDb
        ? await queryTrackedUsageAggregates(arkDb, usageIndex)
        : [];
      const hydrated = applyUsageAggregatesToGames(games, trackedRows, usageIndex);

      return arkGameObjects
        ? await arkGameObjects.hydrateGames(hydrated)
        : hydrated;
    } catch {
      const zeroed = games.map(zeroUsageFields);
      return arkGameObjects
        ? await arkGameObjects.hydrateGames(zeroed).catch(() => zeroed)
        : zeroed;
    } finally {
      await closeDbQuietly(arkDb);
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
  fallbackArkDbPath,
  resolveUsageDb,
}: ArkUsageOptions): PlaytimeStatsRepository {
  const resolveEffectiveUsageDb = createUsageDbResolver({
    legacyDb,
    arkDbPath,
    fallbackArkDbPath,
    resolveUsageDb,
  });

  const loadArkRangeStats = async (rangeStart: string, rangeEnd: string) => {
    const { db: arkDb } = await resolveEffectiveUsageDb();
    if (!arkDb) {
      return {
        dailyTotals: [] as Array<{ date: string; seconds: number }>,
        perGameTotals: [] as Array<{ id: string; name: string; seconds: number }>,
      };
    }

    try {
      const usageIndex = await buildUsageBindingIndexFromLegacyDb(legacyDb);
      const trackerRows = await queryTrackedUsageDailyTotals(
        arkDb,
        usageIndex,
        rangeStart,
        rangeEnd,
      );

      const dailyTotals = new Map<string, number>();
      const perGameTotals = new Map<string, { id: string; name: string; seconds: number }>();

      for (const row of trackerRows) {
        const gameId = findGameIdForTrackedRow(row, usageIndex);
        if (!gameId) {
          continue;
        }

        const seconds = Number(row.seconds ?? 0);
        dailyTotals.set(row.date, (dailyTotals.get(row.date) ?? 0) + seconds);

        const currentGame = perGameTotals.get(gameId);
        if (currentGame) {
          currentGame.seconds += seconds;
          continue;
        }

        perGameTotals.set(gameId, {
          id: gameId,
          name: usageIndex.gameNamesById.get(gameId) ?? gameId,
          seconds,
        });
      }

      return {
        dailyTotals: [...dailyTotals.entries()]
          .map(([date, seconds]) => ({ date, seconds }))
          .sort((left, right) => left.date.localeCompare(right.date)),
        perGameTotals: [...perGameTotals.values()].sort((left, right) => right.seconds - left.seconds),
      };
    } finally {
      await closeDbQuietly(arkDb);
    }
  };

  return {
    getRangeStats(rangeStart, rangeEnd) {
      return loadArkRangeStats(rangeStart, rangeEnd);
    },

    async getDailyTotals(rangeStart, rangeEnd) {
      const { dailyTotals } = await loadArkRangeStats(rangeStart, rangeEnd);
      return dailyTotals;
    },

    async getPerGameTotals(rangeStart, rangeEnd) {
      const { perGameTotals } = await loadArkRangeStats(rangeStart, rangeEnd);
      return perGameTotals;
    },
  };
}
