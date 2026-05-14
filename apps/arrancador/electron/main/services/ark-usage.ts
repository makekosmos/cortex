import {
  ArkClient,
  type ArkUsageApi,
  type ArkUsageGamePlaytimeBinding,
  type ArkUsageGamePlaytimeSummary,
  type ArkUsageSnapshot,
} from "@kosmos/ark";
import { openSqliteDatabase } from "../db";
import { queryAll } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import { getArkCoreRpcBinaryPath, type ArkGameObjectService } from "./ark-game-objects";
import {
  applyUsageAggregatesToGames,
  buildParameterizedList,
  buildUsageBindingIndexFromGames,
  buildUsageWhereClause,
  findGameIdForTrackedRow,
  normalizeExePath,
  normalizeProcessName,
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

export interface ArkUsageSnapshotLoaderOptions {
  arkDbPath: string;
  fallbackArkDbPath?: string;
  arkUsage?: Pick<ArkUsageApi, "loadAll"> & Partial<Pick<ArkUsageApi, "gamePlaytime">>;
  arkCoreRpcPath?: string;
  requestTimeoutMs?: number;
}

interface ArkUsageOptions extends ArkUsageSnapshotLoaderOptions {
  legacyDb: DbLike;
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

function uniqueCandidates(...paths: Array<string | undefined>) {
  return paths.filter((value, index, values): value is string => Boolean(value) && values.indexOf(value) === index);
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

function buildArkPlaytimeBindingsFromGames(
  games: readonly Pick<Game, "id" | "name" | "exe_path" | "process_bindings">[],
): ArkUsageGamePlaytimeBinding[] {
  const bindings: ArkUsageGamePlaytimeBinding[] = [];
  const seen = new Set<string>();

  const pushBinding = (
    game: Pick<Game, "id" | "name">,
    matchType: ArkUsageGamePlaytimeBinding["matchType"],
    matchValue: string,
  ) => {
    const normalized = matchType === "exe_path"
      ? normalizeExePath(matchValue)
      : normalizeProcessName(matchValue);
    if (!normalized) {
      return;
    }

    const key = `${game.id}\0${matchType}\0${normalized}`;
    if (seen.has(key)) {
      return;
    }
    seen.add(key);
    bindings.push({
      gameId: game.id,
      gameName: game.name,
      matchType,
      matchValue,
    });
  };

  for (const game of games) {
    pushBinding(game, "exe_path", game.exe_path);
    for (const binding of game.process_bindings) {
      pushBinding(game, binding.match_type, binding.match_value);
    }
  }

  return bindings;
}

async function buildArkPlaytimeBindingsFromLegacyDb(
  legacyDb: DbLike,
): Promise<ArkUsageGamePlaytimeBinding[]> {
  const gameRows = await loadGamePathIndex(legacyDb);
  const bindingsByGameId = await listGameProcessBindingsByGameId(
    legacyDb,
    gameRows.map((game) => game.id),
  );
  return buildArkPlaytimeBindingsFromGames(
    gameRows.map((row) => ({
      id: row.id,
      name: row.name,
      exe_path: row.normalized_path,
      process_bindings: bindingsByGameId.get(row.id) ?? [],
    })) as Array<Pick<Game, "id" | "name" | "exe_path" | "process_bindings">>,
  );
}

function applyPlaytimeSummaryToGames(
  games: readonly Game[],
  summary: ArkUsageGamePlaytimeSummary,
) {
  const aggregatesByGameId = new Map(summary.aggregates.map((aggregate) => [aggregate.gameId, aggregate]));
  return games.map((game) => {
    const aggregate = aggregatesByGameId.get(game.id);
    if (!aggregate) {
      return zeroUsageFields(game);
    }

    return {
      ...game,
      play_count: aggregate.sessionCount,
      total_playtime: aggregate.totalSeconds,
      last_played: aggregate.lastPlayed,
    };
  });
}

function createArkUsageApiResolver(options: ArkUsageSnapshotLoaderOptions) {
  if (options.arkUsage) {
    return () => options.arkUsage;
  }

  const clients = new Map<string, ArkClient>();

  return (dbPath: string): Pick<ArkUsageApi, "loadAll" | "gamePlaytime"> => {
    let client = clients.get(dbPath);
    if (!client) {
      client = new ArkClient({
        spaceId: "arrancador",
        deviceId: "arrancador-main",
        deviceName: "Arrancador",
        dbPath,
        sidecarPath: options.arkCoreRpcPath ?? getArkCoreRpcBinaryPath(),
        requestTimeoutMs: options.requestTimeoutMs ?? 10_000,
      });
      clients.set(dbPath, client);
    }

    return client.usage;
  };
}

export function createArkGamePlaytimeSummaryLoader(options: ArkUsageSnapshotLoaderOptions) {
  const getArkUsage = createArkUsageApiResolver(options);

  return async (
    bindings: ArkUsageGamePlaytimeBinding[],
    rangeStart?: string,
    rangeEnd?: string,
  ): Promise<ArkUsageGamePlaytimeSummary | null> => {
    if (bindings.length === 0) {
      return {
        aggregates: [],
        dailyTotals: [],
        perGameTotals: [],
      };
    }

    let firstSummary: ArkUsageGamePlaytimeSummary | null = null;
    for (const dbPath of uniqueCandidates(options.arkDbPath, options.fallbackArkDbPath)) {
      const usage = getArkUsage(dbPath);
      if (!usage?.gamePlaytime?.summary) {
        return null;
      }

      const summary = await usage.gamePlaytime.summary({
        bindings,
        rangeStart,
        rangeEnd,
      });
      firstSummary ??= summary;
      if (summary.aggregates.length > 0 || summary.dailyTotals.length > 0 || summary.perGameTotals.length > 0) {
        return summary;
      }
    }

    return firstSummary;
  };
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

function rowMatchesUsageIndex(
  row: Pick<TrackedUsageAggregateRow, "normalized_path" | "normalized_process_name">,
  index: UsageBindingIndex,
) {
  return Boolean(findGameIdForTrackedRow(row, index));
}

function loadTrackedRowsFromSnapshot(
  snapshot: ArkUsageSnapshot,
  index: UsageBindingIndex,
): TrackedUsageAggregateRow[] {
  const trackedAppsById = new Map(snapshot.trackedApps.map((app) => [app.id, app]));
  const rowsByTrackedApp = new Map<string, TrackedUsageAggregateRow>();

  for (const session of snapshot.usageSessions) {
    const trackedApp = trackedAppsById.get(session.trackedAppId);
    if (!trackedApp) {
      continue;
    }

    const normalizedPath = trackedApp.normalizedExePath
      ? normalizeExePath(trackedApp.normalizedExePath)
      : normalizeExePath(trackedApp.exePath || session.exePath);
    const normalizedProcessName = normalizeProcessName(trackedApp.processName || session.processName);
    const rowKey = `${trackedApp.id}\0${normalizedPath}\0${normalizedProcessName}`;
    const row = rowsByTrackedApp.get(rowKey) ?? {
      tracked_app_id: trackedApp.id,
      normalized_path: normalizedPath || null,
      normalized_process_name: normalizedProcessName || null,
      total_seconds: 0,
      session_count: 0,
      last_played: null,
    };

    if (!rowMatchesUsageIndex(row, index)) {
      continue;
    }

    row.total_seconds += Math.trunc(Number(session.foregroundMs ?? 0) / 1000);
    if (Number(session.foregroundMs ?? 0) > 0) {
      row.session_count += 1;
    }

    const lastSeen = session.endedAt ?? session.startedAt;
    if (lastSeen && (!row.last_played || lastSeen > row.last_played)) {
      row.last_played = lastSeen;
    }
    rowsByTrackedApp.set(rowKey, row);
  }

  return [...rowsByTrackedApp.values()];
}

function loadDailyRowsFromSnapshot(
  snapshot: ArkUsageSnapshot,
  index: UsageBindingIndex,
  rangeStart: string,
  rangeEnd: string,
): TrackedUsageDailyRow[] {
  const trackedAppsById = new Map(snapshot.trackedApps.map((app) => [app.id, app]));
  const rowsByDay = new Map<string, TrackedUsageDailyRow>();

  for (const session of snapshot.usageSessions) {
    const trackedApp = trackedAppsById.get(session.trackedAppId);
    if (!trackedApp) {
      continue;
    }

    const date = (session.endedAt ?? session.startedAt).slice(0, 10);
    if (date < rangeStart || date > rangeEnd) {
      continue;
    }

    const seconds = Math.trunc(Number(session.foregroundMs ?? 0) / 1000);
    if (seconds <= 0) {
      continue;
    }

    const normalizedPath = trackedApp.normalizedExePath
      ? normalizeExePath(trackedApp.normalizedExePath)
      : normalizeExePath(trackedApp.exePath || session.exePath);
    const normalizedProcessName = normalizeProcessName(trackedApp.processName || session.processName);
    const candidate = {
      normalized_path: normalizedPath || null,
      normalized_process_name: normalizedProcessName || null,
    };
    if (!rowMatchesUsageIndex(candidate, index)) {
      continue;
    }

    const rowKey = `${trackedApp.id}\0${normalizedPath}\0${normalizedProcessName}\0${date}`;
    const row = rowsByDay.get(rowKey) ?? {
      tracked_app_id: trackedApp.id,
      normalized_path: normalizedPath || null,
      normalized_process_name: normalizedProcessName || null,
      date,
      seconds: 0,
    };
    row.seconds += seconds;
    rowsByDay.set(rowKey, row);
  }

  return [...rowsByDay.values()].sort((left, right) => left.date.localeCompare(right.date));
}

export function createArkUsageSnapshotLoader(options: ArkUsageSnapshotLoaderOptions) {
  if (options.arkUsage) {
    return () => options.arkUsage?.loadAll() ?? Promise.resolve(null);
  }

  const clients = new Map<string, ArkClient>();

  return async (): Promise<ArkUsageSnapshot | null> => {
    let firstSnapshot: ArkUsageSnapshot | null = null;
    for (const dbPath of uniqueCandidates(options.arkDbPath, options.fallbackArkDbPath)) {
      let client = clients.get(dbPath);
      if (!client) {
        client = new ArkClient({
          spaceId: "arrancador",
          deviceId: "arrancador-main",
          deviceName: "Arrancador",
          dbPath,
          sidecarPath: options.arkCoreRpcPath ?? getArkCoreRpcBinaryPath(),
          requestTimeoutMs: options.requestTimeoutMs ?? 10_000,
        });
        clients.set(dbPath, client);
      }

      const snapshot = await client.usage.loadAll();
      firstSnapshot ??= snapshot;
      if (snapshot.usageSessions.length > 0) {
        return snapshot;
      }
    }

    return firstSnapshot;
  };
}

export function createGameUsageReadModel({
  legacyDb,
  arkDbPath,
  fallbackArkDbPath,
  arkGameObjects,
  arkUsage,
  arkCoreRpcPath,
  requestTimeoutMs,
  resolveUsageDb,
}: ArkUsageOptions): GameUsageReadModel {
  const resolveEffectiveUsageDb = createUsageDbResolver({
    legacyDb,
    arkDbPath,
    fallbackArkDbPath,
    arkGameObjects,
    arkUsage,
    arkCoreRpcPath,
    requestTimeoutMs,
    resolveUsageDb,
  });
  const loadArkPlaytimeSummary = createArkGamePlaytimeSummaryLoader({
    arkDbPath,
    fallbackArkDbPath,
    arkUsage,
    arkCoreRpcPath,
    requestTimeoutMs,
  });

  const hydrateGames = async (games: Game[]): Promise<Game[]> => {
    if (games.length === 0) {
      return games;
    }

    const usageIndex = buildUsageBindingIndexFromGames(games);

    try {
      const summary = await loadArkPlaytimeSummary(buildArkPlaytimeBindingsFromGames(games));
      if (!summary) {
        throw new Error("ARK playtime summary endpoint is unavailable");
      }
      const hydrated = applyPlaytimeSummaryToGames(games, summary);

      return arkGameObjects
        ? await arkGameObjects.hydrateGames(hydrated)
        : hydrated;
    } catch {
      const { db: arkDb } = await resolveEffectiveUsageDb();
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
  arkUsage,
  arkCoreRpcPath,
  requestTimeoutMs,
  resolveUsageDb,
}: ArkUsageOptions): PlaytimeStatsRepository {
  const resolveEffectiveUsageDb = createUsageDbResolver({
    legacyDb,
    arkDbPath,
    fallbackArkDbPath,
    arkUsage,
    arkCoreRpcPath,
    requestTimeoutMs,
    resolveUsageDb,
  });
  const loadArkPlaytimeSummary = createArkGamePlaytimeSummaryLoader({
    arkDbPath,
    fallbackArkDbPath,
    arkUsage,
    arkCoreRpcPath,
    requestTimeoutMs,
  });

  const loadArkRangeStats = async (rangeStart: string, rangeEnd: string) => {
    try {
      const summary = await loadArkPlaytimeSummary(
        await buildArkPlaytimeBindingsFromLegacyDb(legacyDb),
        rangeStart,
        rangeEnd,
      );
      if (!summary) {
        throw new Error("ARK playtime summary endpoint is unavailable");
      }

      return {
        dailyTotals: summary.dailyTotals,
        perGameTotals: summary.perGameTotals.map((game) => ({
          id: game.gameId,
          name: game.gameName,
          seconds: game.seconds,
        })),
      };
    } catch {
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
