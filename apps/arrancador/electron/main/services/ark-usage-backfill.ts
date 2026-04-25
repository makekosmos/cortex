import { randomUUID } from "node:crypto";

import {
  ArkClient,
  type ArkTrackedAppRecord,
  type ArkUsageApi,
  type JsonValue,
} from "@kepler/ark";

import { openSqliteDatabase } from "../db";
import { execute, queryAll, queryOne } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import { getArkCoreRpcBinaryPath } from "./ark-game-objects";
import { resolveUsageTrackerDb } from "./ark-usage";
import {
  buildLegacyUsageBackfillPlan,
  type LegacyDailyRow,
  type LegacyGameRow,
  type LegacyUsageBackfillTrackedApp,
} from "./ark-usage-backfill/plan";

export type {
  LegacyDailyRow,
  LegacyGameRow,
  LegacyUsageBackfillPlan,
  LegacyUsageBackfillSession,
  LegacyUsageBackfillTrackedApp,
  LegacyUsageSessionKind,
} from "./ark-usage-backfill/plan";
export { buildLegacyUsageBackfillPlan } from "./ark-usage-backfill/plan";

const BACKFILL_MARKER_KEY = "arrancador.legacy_usage_backfill.v1";
const BACKFILL_DEVICE_ID_KEY = "arrancador.legacy_usage_backfill.device_id";

type BackfillUsageApi = Pick<ArkUsageApi, "loadAll"> & {
  trackedApps: Pick<ArkUsageApi["trackedApps"], "upsert">;
  sessions: Pick<ArkUsageApi["sessions"], "upsert">;
};

export interface LegacyUsageBackfillResult {
  targetDbPath: string | null;
  alreadyBackfilled: boolean;
  migratedTrackedApps: number;
  updatedTrackedApps: number;
  migratedSessions: number;
  skippedSessions: number;
}

export interface BackfillLegacyUsageToArkOptions {
  legacyDb: DbLike;
  arkDbPath: string;
  fallbackArkDbPath?: string;
  resolveUsageDb?: () => Promise<{ db: DbLike | null; path: string | null }>;
  arkUsage?: BackfillUsageApi;
  arkCoreRpcPath?: string;
  requestTimeoutMs?: number;
  force?: boolean;
}

function minTimestamp(left: string, right: string): string {
  return left <= right ? left : right;
}

function maxTimestamp(left: string, right: string): string {
  return left >= right ? left : right;
}

function mergeTrackedApp(
  existing: ArkTrackedAppRecord | undefined,
  next: LegacyUsageBackfillTrackedApp,
): LegacyUsageBackfillTrackedApp {
  if (!existing) {
    return next;
  }

  return {
    id: existing.id,
    platform: next.platform,
    exePath: existing.exePath || next.exePath,
    normalizedExePath: existing.normalizedExePath || next.normalizedExePath,
    processName: existing.processName || next.processName,
    displayName: existing.displayName ?? next.displayName,
    publisher: existing.publisher ?? next.publisher,
    iconRef: existing.iconRef ?? next.iconRef,
    firstSeenAt: minTimestamp(existing.firstSeenAt, next.firstSeenAt),
    lastSeenAt: maxTimestamp(existing.lastSeenAt, next.lastSeenAt),
  };
}

function createUsageDbResolver(options: BackfillLegacyUsageToArkOptions) {
  return (
    options.resolveUsageDb ??
    (() =>
      resolveUsageTrackerDb({
        arkDbPath: options.arkDbPath,
        fallbackArkDbPath: options.fallbackArkDbPath,
        openDb: (filePath) => {
          try {
            return openSqliteDatabase(filePath, {
              fileMustExist: true,
              timeoutMs: 2000,
            });
          } catch {
            return null;
          }
        },
      }))
  );
}

async function listLegacyGamesWithUsage(db: DbLike): Promise<LegacyGameRow[]> {
  return await queryAll<LegacyGameRow>(
    db,
    `SELECT id, name, exe_path, exe_name, date_added, total_playtime, last_played
     FROM games
     WHERE total_playtime > 0
        OR EXISTS (
          SELECT 1
          FROM playtime_daily
          WHERE playtime_daily.game_id = games.id
            AND playtime_daily.seconds > 0
        )
     ORDER BY date_added ASC, name ASC`,
  );
}

async function listLegacyDailyRows(db: DbLike): Promise<LegacyDailyRow[]> {
  return await queryAll<LegacyDailyRow>(
    db,
    `SELECT game_id, date, seconds
     FROM playtime_daily
     WHERE seconds > 0
     ORDER BY date ASC, game_id ASC`,
  );
}

async function getSyncKv(db: DbLike, key: string): Promise<string | null> {
  const row = await queryOne<{ value: string }>(
    db,
    "SELECT value FROM sync_kv WHERE key = ?1",
    [key],
  );
  return row?.value ?? null;
}

async function setSyncKv(db: DbLike, key: string, value: string): Promise<void> {
  await execute(
    db,
    "INSERT OR REPLACE INTO sync_kv (key, value) VALUES (?1, ?2)",
    [key, value],
  );
}

async function loadOrCreateBackfillDeviceId(db: DbLike): Promise<string> {
  const existing = await getSyncKv(db, BACKFILL_DEVICE_ID_KEY);
  if (existing && existing.trim().length > 0) {
    return existing.trim();
  }

  const generated = `arrancador-legacy-${randomUUID().replaceAll("-", "")}`;
  await setSyncKv(db, BACKFILL_DEVICE_ID_KEY, generated);
  return generated;
}

function createBackfillUsageApi(
  options: BackfillLegacyUsageToArkOptions,
  targetDbPath: string,
  deviceId: string,
): BackfillUsageApi {
  if (options.arkUsage) {
    return options.arkUsage;
  }

  return new ArkClient({
    spaceId: "arrancador",
    deviceId,
    deviceName: "Arrancador Legacy Import",
    dbPath: targetDbPath,
    sidecarPath: options.arkCoreRpcPath ?? getArkCoreRpcBinaryPath(),
    requestTimeoutMs: options.requestTimeoutMs ?? 10_000,
  }).usage;
}

function parseMetaJson(value: string): JsonValue {
  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as JsonValue)
      : {};
  } catch {
    return {};
  }
}

export async function backfillLegacyUsageToArk(
  options: BackfillLegacyUsageToArkOptions,
): Promise<LegacyUsageBackfillResult> {
  const resolveUsageDb = createUsageDbResolver(options);
  const { db: arkDb, path: targetDbPath } = await resolveUsageDb();

  const emptyResult: LegacyUsageBackfillResult = {
    targetDbPath,
    alreadyBackfilled: false,
    migratedTrackedApps: 0,
    updatedTrackedApps: 0,
    migratedSessions: 0,
    skippedSessions: 0,
  };

  if (!arkDb) {
    return emptyResult;
  }

  try {
    if (!options.force) {
      const marker = await getSyncKv(arkDb, BACKFILL_MARKER_KEY);
      if (marker && marker.trim().length > 0) {
        return {
          ...emptyResult,
          alreadyBackfilled: true,
        };
      }
    }

    const [legacyGames, legacyDailyRows, deviceId] = await Promise.all([
      listLegacyGamesWithUsage(options.legacyDb),
      listLegacyDailyRows(options.legacyDb),
      loadOrCreateBackfillDeviceId(arkDb),
    ]);
    const deviceName =
      process.env.COMPUTERNAME?.trim() ||
      process.env.HOSTNAME?.trim() ||
      "Arrancador Legacy Import";
    const plan = buildLegacyUsageBackfillPlan(
      legacyGames,
      legacyDailyRows,
      deviceId,
      deviceName,
    );
    const usage = createBackfillUsageApi(
      options,
      targetDbPath ?? options.arkDbPath,
      deviceId,
    );
    const existingUsage = await usage.loadAll();
    const existingTrackedApps = new Map(
      existingUsage.trackedApps.map((trackedApp) => [trackedApp.id, trackedApp]),
    );
    const existingSessionIds = new Set(
      existingUsage.usageSessions.map((session) => session.id),
    );

    const result: LegacyUsageBackfillResult = {
      ...emptyResult,
      targetDbPath,
      alreadyBackfilled: false,
    };

    for (const trackedApp of plan.trackedApps) {
      const existing = existingTrackedApps.get(trackedApp.id);
      const merged = mergeTrackedApp(existing, trackedApp);
      await usage.trackedApps.upsert(merged);
      existingTrackedApps.set(merged.id, merged);

      if (!existing) {
        result.migratedTrackedApps += 1;
      } else if (
        existing.firstSeenAt !== merged.firstSeenAt ||
        existing.lastSeenAt !== merged.lastSeenAt ||
        (existing.displayName ?? null) !== (merged.displayName ?? null)
      ) {
        result.updatedTrackedApps += 1;
      }
    }

    for (const session of plan.sessions) {
      if (existingSessionIds.has(session.id)) {
        result.skippedSessions += 1;
        continue;
      }

      await usage.sessions.upsert({
        id: session.id,
        trackedAppId: session.trackedAppId,
        deviceId: session.deviceId,
        deviceName: session.deviceName,
        platform: session.platform,
        startedAt: session.startedAt,
        endedAt: session.endedAt,
        foregroundMs: session.foregroundMs,
        idleMs: session.idleMs,
        windowTitle: session.windowTitle,
        processName: session.processName,
        exePath: session.exePath,
        pidStart: session.pidStart,
        pidEnd: session.pidEnd,
        metaJson: parseMetaJson(session.metaJson),
      });
      existingSessionIds.add(session.id);
      result.migratedSessions += 1;
    }

    await setSyncKv(
      arkDb,
      BACKFILL_MARKER_KEY,
      JSON.stringify({
        version: 1,
        imported_at: new Date().toISOString(),
        sessions: plan.sessions.length,
      }),
    );

    return result;
  } finally {
    await Promise.resolve(arkDb.close?.());
  }
}
