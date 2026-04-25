import { normalizeGameProcessBindingValue } from "../game-process-bindings";
import type { Game } from "../games/types";

export type TrackedUsageAggregateRow = {
  tracked_app_id: string;
  normalized_path: string | null;
  normalized_process_name: string | null;
  total_seconds: number;
  session_count: number;
  last_played: string | null;
};

export type TrackedUsageDailyRow = {
  tracked_app_id: string;
  normalized_path: string | null;
  normalized_process_name: string | null;
  date: string;
  seconds: number;
};

export type GameUsageAggregate = {
  totalSeconds: number;
  sessionCount: number;
  lastPlayed: string | null;
};

export type UsageBindingIndex = {
  gameNamesById: Map<string, string>;
  gameIdsByPath: Map<string, string>;
  gameIdsByProcessName: Map<string, string>;
};

export function normalizeExePath(exePath: string): string {
  return normalizeGameProcessBindingValue("exe_path", exePath);
}

export function normalizeProcessName(processName: string) {
  return normalizeGameProcessBindingValue("process_name", processName);
}

export function buildParameterizedList(values: readonly string[], startIndex = 1): string {
  return values.map((_, index) => `?${index + startIndex}`).join(", ");
}

export function buildUsageBindingIndexFromGames(
  games: readonly Game[],
): UsageBindingIndex {
  const gameNamesById = new Map<string, string>();
  const gameIdsByPath = new Map<string, string>();
  const gameIdsByProcessName = new Map<string, string>();

  for (const game of games) {
    gameNamesById.set(game.id, game.name);
    gameIdsByPath.set(normalizeExePath(game.exe_path), game.id);

    for (const binding of game.process_bindings) {
      const normalizedValue = normalizeGameProcessBindingValue(
        binding.match_type,
        binding.match_value,
      );
      if (!normalizedValue) {
        continue;
      }

      if (binding.match_type === "exe_path") {
        gameIdsByPath.set(normalizedValue, game.id);
      } else {
        gameIdsByProcessName.set(normalizedValue, game.id);
      }
    }
  }

  return {
    gameNamesById,
    gameIdsByPath,
    gameIdsByProcessName,
  };
}

export function buildUsageWhereClause(index: UsageBindingIndex) {
  const pathBindings = [...index.gameIdsByPath.keys()];
  const nameBindings = [...index.gameIdsByProcessName.keys()];
  const clauses: string[] = [];
  const params: string[] = [];

  if (pathBindings.length > 0) {
    const placeholders = buildParameterizedList(pathBindings, params.length + 1);
    clauses.push(`tracked_apps.normalized_exe_path IN (${placeholders})`);
    params.push(...pathBindings);
  }

  if (nameBindings.length > 0) {
    const placeholders = buildParameterizedList(nameBindings, params.length + 1);
    clauses.push(`LOWER(COALESCE(tracked_apps.process_name, '')) IN (${placeholders})`);
    params.push(...nameBindings);
  }

  return {
    whereSql: clauses.length > 0 ? `WHERE ${clauses.join(" OR ")}` : "",
    params,
  };
}

export function findGameIdForTrackedRow(
  row: {
    normalized_path: string | null;
    normalized_process_name: string | null;
  },
  index: UsageBindingIndex,
) {
  const pathMatch = row.normalized_path
    ? index.gameIdsByPath.get(normalizeExePath(row.normalized_path))
    : null;
  if (pathMatch) {
    return pathMatch;
  }

  const processName = row.normalized_process_name
    ? normalizeProcessName(row.normalized_process_name)
    : "";
  if (!processName) {
    return null;
  }
  return index.gameIdsByProcessName.get(processName) ?? null;
}

export function zeroUsageFields(game: Game): Game {
  return {
    ...game,
    play_count: 0,
    total_playtime: 0,
    last_played: null,
  };
}

export function applyUsageAggregatesToGames(
  games: readonly Game[],
  rows: readonly TrackedUsageAggregateRow[],
  index: UsageBindingIndex,
) {
  const totalsByGameId = new Map<string, GameUsageAggregate>();

  for (const row of rows) {
    const gameId = findGameIdForTrackedRow(row, index);
    if (!gameId) {
      continue;
    }

    const current = totalsByGameId.get(gameId) ?? {
      totalSeconds: 0,
      sessionCount: 0,
      lastPlayed: null,
    };

    current.totalSeconds += Number(row.total_seconds ?? 0);
    current.sessionCount += Number(row.session_count ?? 0);
    if (!current.lastPlayed || (row.last_played && row.last_played > current.lastPlayed)) {
      current.lastPlayed = row.last_played ?? current.lastPlayed;
    }

    totalsByGameId.set(gameId, current);
  }

  return games.map((game) => {
    const metrics = totalsByGameId.get(game.id);
    if (!metrics) {
      return zeroUsageFields(game);
    }

    return {
      ...game,
      play_count: metrics.sessionCount,
      total_playtime: metrics.totalSeconds,
      last_played: metrics.lastPlayed,
    };
  });
}
