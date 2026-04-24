import type { UsageProcessCandidate } from "../../../src/types";
import { queryAll } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import { resolveUsageTrackerDb } from "./ark-usage";
import { normalizeGameProcessBindingValue } from "./game-process-bindings";

type UsageProcessRow = {
  tracked_app_id: string;
  display_name: string | null;
  exe_path: string | null;
  process_name: string | null;
  last_seen_at: string | null;
  session_count: number;
};

interface UsageProcessSearchOptions {
  arkDbPath: string;
  fallbackArkDbPath?: string;
  resolveUsageDb?: () => Promise<{ db: DbLike | null; path: string | null }>;
}

export interface UsageProcessSearchService {
  getRecentProcesses(limit?: number): Promise<UsageProcessCandidate[]>;
  searchProcesses(query: string, limit?: number): Promise<UsageProcessCandidate[]>;
}

function clampLimit(limit: number | undefined) {
  const next = Number(limit ?? 10);
  if (!Number.isFinite(next)) {
    return 10;
  }
  return Math.min(25, Math.max(1, Math.floor(next)));
}

function buildCandidate(row: UsageProcessRow): UsageProcessCandidate | null {
  const exePath = row.exe_path?.trim() || null;
  const processName = row.process_name?.trim() || null;
  const matchType = exePath ? "exe_path" : processName ? "process_name" : null;
  const matchValue = exePath ?? processName;
  if (!matchType || !matchValue) {
    return null;
  }

  return {
    tracked_app_id: row.tracked_app_id,
    display_name:
      row.display_name?.trim() ||
      processName ||
      exePath ||
      row.tracked_app_id,
    exe_path: exePath,
    process_name: processName,
    last_seen_at: row.last_seen_at ?? null,
    session_count: Number(row.session_count ?? 0),
    binding_match_type: matchType,
    binding_match_value: matchValue,
    binding_normalized_value: normalizeGameProcessBindingValue(matchType, matchValue),
  };
}

async function runUsageProcessQuery(
  arkDb: DbLike,
  whereSql: string,
  params: unknown[],
): Promise<UsageProcessCandidate[]> {
  const rows = await queryAll<UsageProcessRow>(
    arkDb,
    `SELECT tracked_apps.id AS tracked_app_id,
            NULLIF(COALESCE(tracked_apps.display_name, tracked_apps.process_name, tracked_apps.exe_path), '') AS display_name,
            NULLIF(tracked_apps.exe_path, '') AS exe_path,
            NULLIF(tracked_apps.process_name, '') AS process_name,
            MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at,
            SUM(CASE WHEN usage_sessions.foreground_ms > 0 THEN 1 ELSE 0 END) AS session_count
     FROM usage_sessions
     JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
     ${whereSql}
     GROUP BY tracked_apps.id, tracked_apps.display_name, tracked_apps.exe_path, tracked_apps.process_name
     ORDER BY last_seen_at DESC
     LIMIT ?${params.length}`,
    params,
  );

  return rows
    .map(buildCandidate)
    .filter((candidate): candidate is UsageProcessCandidate => candidate !== null);
}

export function createUsageProcessSearchService({
  arkDbPath,
  fallbackArkDbPath,
  resolveUsageDb,
}: UsageProcessSearchOptions): UsageProcessSearchService {
  const resolveDb =
    resolveUsageDb ??
    (() =>
      resolveUsageTrackerDb({
        arkDbPath,
        fallbackArkDbPath,
      }));

  const runQuery = async (whereSql: string, params: unknown[]) => {
    const { db } = await resolveDb();
    if (!db) {
      return [];
    }

    try {
      return await runUsageProcessQuery(db, whereSql, params);
    } finally {
      await Promise.resolve(db.close?.());
    }
  };

  return {
    getRecentProcesses(limit = 10) {
      const boundedLimit = clampLimit(limit);
      return runQuery(
        `WHERE NULLIF(COALESCE(tracked_apps.exe_path, tracked_apps.process_name), '') IS NOT NULL`,
        [boundedLimit],
      );
    },

    searchProcesses(query, limit = 10) {
      const trimmed = query.trim().toLowerCase();
      if (!trimmed) {
        return this.getRecentProcesses(limit);
      }

      const boundedLimit = clampLimit(limit);
      const pattern = `%${trimmed}%`;
      return runQuery(
        `WHERE (
            LOWER(COALESCE(tracked_apps.display_name, '')) LIKE ?1
            OR LOWER(COALESCE(tracked_apps.process_name, '')) LIKE ?1
            OR LOWER(COALESCE(tracked_apps.exe_path, '')) LIKE ?1
          )`,
        [pattern, boundedLimit],
      );
    },
  };
}
