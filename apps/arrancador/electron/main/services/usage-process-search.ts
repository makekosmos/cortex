import { ArkClient, type ArkUsageApi, type ArkUsageProcessCandidate } from "@kosmos/ark";
import type { UsageProcessCandidate } from "../../../src/types";
import { queryAll } from "../helpers/db";
import type { DbLike, DbValue } from "../helpers/shared";
import { resolveUsageTrackerDb } from "./ark-usage";
import { getArkCoreRpcBinaryPath } from "./ark-game-objects";
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
  arkUsage?: Pick<ArkUsageApi, "processes">;
  arkCoreRpcPath?: string;
  requestTimeoutMs?: number;
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

function mapArkProcessCandidate(candidate: ArkUsageProcessCandidate): UsageProcessCandidate {
  return {
    tracked_app_id: candidate.trackedAppId,
    display_name: candidate.displayName,
    exe_path: candidate.exePath,
    process_name: candidate.processName,
    last_seen_at: candidate.lastSeenAt,
    session_count: candidate.sessionCount,
    binding_match_type: candidate.bindingMatchType,
    binding_match_value: candidate.bindingMatchValue,
    binding_normalized_value: candidate.bindingNormalizedValue,
  };
}

async function runUsageProcessQuery(
  arkDb: DbLike,
  whereSql: string,
  params: readonly DbValue[],
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
  arkUsage,
  arkCoreRpcPath,
  requestTimeoutMs,
  resolveUsageDb,
}: UsageProcessSearchOptions): UsageProcessSearchService {
  const arkClients = new Map<string, ArkClient>();
  const getArkUsage = (dbPath: string): Pick<ArkUsageApi, "processes"> => {
    if (arkUsage) {
      return arkUsage;
    }

    let arkClient = arkClients.get(dbPath);
    if (!arkClient) {
      arkClient = new ArkClient({
        spaceId: "arrancador",
        deviceId: "arrancador-main",
        deviceName: "Arrancador",
        dbPath,
        sidecarPath: arkCoreRpcPath ?? getArkCoreRpcBinaryPath(),
        requestTimeoutMs: requestTimeoutMs ?? 10_000,
      });
      arkClients.set(dbPath, arkClient);
    }
    return arkClient.usage;
  };
  const arkDbCandidates = [arkDbPath, fallbackArkDbPath]
    .filter((value, index, values): value is string => Boolean(value) && values.indexOf(value) === index);
  const resolveDb =
    resolveUsageDb ??
    (() =>
      resolveUsageTrackerDb({
        arkDbPath,
        fallbackArkDbPath,
      }));

  const runFallbackQuery = async (whereSql: string, params: readonly DbValue[]) => {
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

  const runRecentQuery = async (limit: number) => {
    try {
      for (const dbPath of arkDbCandidates) {
        const candidates = await getArkUsage(dbPath).processes.recent(limit);
        if (candidates.length > 0 || arkUsage || dbPath === arkDbCandidates.at(-1)) {
          return candidates.map(mapArkProcessCandidate);
        }
      }
    } catch {
      // Fall back to read-only SQLite when the ARK runtime is unavailable.
    }

    return runFallbackQuery(
      `WHERE NULLIF(COALESCE(tracked_apps.exe_path, tracked_apps.process_name), '') IS NOT NULL`,
      [limit],
    );
  };

  const runSearchQuery = async (query: string, limit: number) => {
    try {
      for (const dbPath of arkDbCandidates) {
        const candidates = await getArkUsage(dbPath).processes.search(query, limit);
        if (candidates.length > 0 || arkUsage || dbPath === arkDbCandidates.at(-1)) {
          return candidates.map(mapArkProcessCandidate);
        }
      }
    } catch {
      // Fall back to read-only SQLite when the ARK runtime is unavailable.
    }

    const pattern = `%${query}%`;
    return runFallbackQuery(
      `WHERE (
          LOWER(COALESCE(tracked_apps.display_name, '')) LIKE ?1
          OR LOWER(COALESCE(tracked_apps.process_name, '')) LIKE ?1
          OR LOWER(COALESCE(tracked_apps.exe_path, '')) LIKE ?1
        )`,
      [pattern, limit],
    );
  };

  return {
    getRecentProcesses(limit = 10) {
      const boundedLimit = clampLimit(limit);
      return runRecentQuery(boundedLimit);
    },

    searchProcesses(query, limit = 10) {
      const trimmed = query.trim().toLowerCase();
      if (!trimmed) {
        return this.getRecentProcesses(limit);
      }

      const boundedLimit = clampLimit(limit);
      return runSearchQuery(trimmed, boundedLimit);
    },
  };
}
