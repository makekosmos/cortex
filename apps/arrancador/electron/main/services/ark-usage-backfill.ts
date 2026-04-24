import { createHash, randomUUID } from "node:crypto";

import { openSqliteDatabase } from "../db";
import { execute, queryAll, queryOne, runInTransaction } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import { resolveUsageTrackerDb } from "./ark-usage";

const BACKFILL_MARKER_KEY = "arrancador.legacy_usage_backfill.v1";
const BACKFILL_DEVICE_ID_KEY = "arrancador.legacy_usage_backfill.device_id";
const VERSION_VECTOR_KEY = "lan_sync.version_vector";
const PLATFORM = process.platform === "win32" ? "windows" : process.platform;

type LegacyGameRow = {
  id: string;
  name: string;
  exe_path: string;
  exe_name: string;
  date_added: string;
  total_playtime: number;
  last_played: string | null;
};

type LegacyDailyRow = {
  game_id: string;
  date: string;
  seconds: number;
};

type ExistingTrackedAppRow = {
  id: string;
  exe_path: string;
  normalized_exe_path: string;
  process_name: string;
  display_name: string | null;
  publisher: string | null;
  icon_ref: string | null;
  first_seen_at: string;
  last_seen_at: string;
};

type LegacyUsageSessionKind = "daily" | "residual";

export type LegacyUsageBackfillTrackedApp = {
  id: string;
  platform: string;
  exePath: string;
  normalizedExePath: string;
  processName: string;
  displayName: string | null;
  publisher: string | null;
  iconRef: string | null;
  firstSeenAt: string;
  lastSeenAt: string;
};

export type LegacyUsageBackfillSession = {
  id: string;
  trackedAppId: string;
  deviceId: string;
  deviceName: string;
  platform: string;
  startedAt: string;
  endedAt: string;
  foregroundMs: number;
  idleMs: number;
  windowTitle: string | null;
  processName: string;
  exePath: string;
  pidStart: number | null;
  pidEnd: number | null;
  metaJson: string;
  kind: LegacyUsageSessionKind;
};

export interface LegacyUsageBackfillPlan {
  trackedApps: LegacyUsageBackfillTrackedApp[];
  sessions: LegacyUsageBackfillSession[];
}

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
  force?: boolean;
}

function normalizeExePath(exePath: string): string {
  return exePath.trim().replaceAll("/", "\\").toLowerCase();
}

function hashToHex(input: string): string {
  return createHash("sha256").update(input).digest("hex");
}

function createTrackedAppId(exePath: string): string {
  return hashToHex(`${PLATFORM}:${normalizeExePath(exePath)}`);
}

function createSyntheticSessionId(
  trackedAppId: string,
  kind: LegacyUsageSessionKind,
  date: string,
  seconds: number,
): string {
  return hashToHex(`arrancador-legacy-usage:${trackedAppId}:${kind}:${date}:${seconds}`);
}

function toIsoDate(value: string | null | undefined): string | null {
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  return trimmed.length >= 10 ? trimmed.slice(0, 10) : null;
}

function toIsoTimestamp(date: Date): string {
  return date.toISOString();
}

function deriveSessionBounds(
  date: string,
  seconds: number,
  preferredEndAt?: string | null,
): { startedAt: string; endedAt: string } {
  const safeSeconds = Math.max(0, Number(seconds || 0));
  const defaultEnd = `${date}T21:00:00.000Z`;
  const endedAt =
    preferredEndAt?.startsWith(date)
      ? preferredEndAt
      : defaultEnd;
  const endMs = Date.parse(endedAt);
  const startedAt = Number.isFinite(endMs)
    ? toIsoTimestamp(new Date(endMs - safeSeconds * 1000))
    : `${date}T00:00:00.000Z`;

  return {
    startedAt,
    endedAt,
  };
}

function buildSessionMeta(
  game: LegacyGameRow,
  kind: LegacyUsageSessionKind,
  date: string,
): string {
  return JSON.stringify({
    source: "arrancador-legacy-backfill",
    imported: true,
    version: 1,
    kind,
    legacy_game_id: game.id,
    legacy_date: date,
  });
}

function minTimestamp(left: string, right: string): string {
  return left <= right ? left : right;
}

function maxTimestamp(left: string, right: string): string {
  return left >= right ? left : right;
}

function mergeTrackedApp(
  existing: ExistingTrackedAppRow | undefined,
  next: LegacyUsageBackfillTrackedApp,
): LegacyUsageBackfillTrackedApp {
  if (!existing) {
    return next;
  }

  return {
    id: existing.id,
    platform: next.platform,
    exePath: existing.exe_path || next.exePath,
    normalizedExePath: existing.normalized_exe_path || next.normalizedExePath,
    processName: existing.process_name || next.processName,
    displayName: existing.display_name ?? next.displayName,
    publisher: existing.publisher ?? next.publisher,
    iconRef: existing.icon_ref ?? next.iconRef,
    firstSeenAt: minTimestamp(existing.first_seen_at, next.firstSeenAt),
    lastSeenAt: maxTimestamp(existing.last_seen_at, next.lastSeenAt),
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

function nextHlc(existing: string | undefined, deviceId: string): string {
  const now = new Date().toISOString();
  if (!existing) {
    return `${now}:000000:${deviceId}`;
  }

  const zIndex = existing.indexOf("Z");
  if (zIndex < 0) {
    return `${now}:000000:${deviceId}`;
  }

  const firstColon = existing.indexOf(":", zIndex);
  if (firstColon < 0) {
    return `${now}:000000:${deviceId}`;
  }

  const rest = existing.slice(firstColon + 1);
  const secondColon = rest.indexOf(":");
  if (secondColon < 0) {
    return `${now}:000000:${deviceId}`;
  }

  const wallTime = existing.slice(0, firstColon);
  const counter = Number.parseInt(rest.slice(0, secondColon), 10) || 0;
  const previousDeviceId = rest.slice(secondColon + 1);

  if (previousDeviceId === deviceId) {
    if (wallTime < now) {
      return `${now}:000000:${deviceId}`;
    }

    return `${wallTime}:${String(counter + 1).padStart(6, "0")}:${deviceId}`;
  }

  if (wallTime >= now) {
    return `${wallTime}:${String(counter + 1).padStart(6, "0")}:${deviceId}`;
  }

  return `${now}:000000:${deviceId}`;
}

async function bumpVersionVector(
  db: DbLike,
  entityId: string,
  deviceId: string,
): Promise<void> {
  const encoded = await getSyncKv(db, VERSION_VECTOR_KEY);
  const vector =
    encoded && encoded.trim().length > 0
      ? (JSON.parse(encoded) as Record<string, string>)
      : {};
  vector[entityId] = nextHlc(vector[entityId], deviceId);
  await setSyncKv(db, VERSION_VECTOR_KEY, JSON.stringify(vector));
}

export function buildLegacyUsageBackfillPlan(
  games: readonly LegacyGameRow[],
  dailyRows: readonly LegacyDailyRow[],
  deviceId: string,
  deviceName: string,
): LegacyUsageBackfillPlan {
  const dailyRowsByGame = new Map<string, LegacyDailyRow[]>();
  for (const row of dailyRows) {
    const current = dailyRowsByGame.get(row.game_id) ?? [];
    current.push(row);
    dailyRowsByGame.set(row.game_id, current);
  }

  const trackedApps: LegacyUsageBackfillTrackedApp[] = [];
  const sessions: LegacyUsageBackfillSession[] = [];

  for (const game of games) {
    const normalizedExePath = normalizeExePath(game.exe_path);
    const trackedAppId = createTrackedAppId(game.exe_path);
    const gameDailyRows = [...(dailyRowsByGame.get(game.id) ?? [])].sort((left, right) =>
      left.date.localeCompare(right.date),
    );
    const importedGameSessions: LegacyUsageBackfillSession[] = [];
    const totalFromDailyRows = gameDailyRows.reduce(
      (sum, row) => sum + Math.max(0, Number(row.seconds ?? 0)),
      0,
    );

    for (const row of gameDailyRows) {
      const { startedAt, endedAt } = deriveSessionBounds(row.date, row.seconds);
      importedGameSessions.push({
        id: createSyntheticSessionId(trackedAppId, "daily", row.date, row.seconds),
        trackedAppId,
        deviceId,
        deviceName,
        platform: PLATFORM,
        startedAt,
        endedAt,
        foregroundMs: Math.max(0, Number(row.seconds ?? 0)) * 1000,
        idleMs: 0,
        windowTitle: game.name,
        processName: game.exe_name,
        exePath: game.exe_path,
        pidStart: null,
        pidEnd: null,
        metaJson: buildSessionMeta(game, "daily", row.date),
        kind: "daily",
      });
    }

    const residualSeconds = Math.max(
      0,
      Math.floor(Number(game.total_playtime ?? 0) - totalFromDailyRows),
    );
    if (residualSeconds > 0) {
      const residualDate =
        toIsoDate(game.last_played) ??
        gameDailyRows[gameDailyRows.length - 1]?.date ??
        game.date_added.slice(0, 10);
      const { startedAt, endedAt } = deriveSessionBounds(
        residualDate,
        residualSeconds,
        game.last_played,
      );
      importedGameSessions.push({
        id: createSyntheticSessionId(trackedAppId, "residual", residualDate, residualSeconds),
        trackedAppId,
        deviceId,
        deviceName,
        platform: PLATFORM,
        startedAt,
        endedAt,
        foregroundMs: residualSeconds * 1000,
        idleMs: 0,
        windowTitle: game.name,
        processName: game.exe_name,
        exePath: game.exe_path,
        pidStart: null,
        pidEnd: null,
        metaJson: buildSessionMeta(game, "residual", residualDate),
        kind: "residual",
      });
    }

    if (importedGameSessions.length === 0) {
      continue;
    }

    importedGameSessions.sort((left, right) => left.endedAt.localeCompare(right.endedAt));
    const firstSeenAt = importedGameSessions[0]?.startedAt ?? game.date_added;
    const lastSeenAt =
      importedGameSessions[importedGameSessions.length - 1]?.endedAt ??
      game.last_played ??
      game.date_added;

    trackedApps.push({
      id: trackedAppId,
      platform: PLATFORM,
      exePath: game.exe_path,
      normalizedExePath,
      processName: game.exe_name,
      displayName: game.name,
      publisher: null,
      iconRef: null,
      firstSeenAt,
      lastSeenAt,
    });
    sessions.push(...importedGameSessions);
  }

  return { trackedApps, sessions };
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

    return await runInTransaction(arkDb, async (tx) => {
      const result: LegacyUsageBackfillResult = {
        ...emptyResult,
        targetDbPath,
        alreadyBackfilled: false,
      };

      for (const trackedApp of plan.trackedApps) {
        const existing = await queryOne<ExistingTrackedAppRow>(
          tx,
          `SELECT id, exe_path, normalized_exe_path, process_name, display_name, publisher, icon_ref,
                  first_seen_at, last_seen_at
           FROM tracked_apps
           WHERE id = ?1`,
          [trackedApp.id],
        );

        const merged = mergeTrackedApp(existing ?? undefined, trackedApp);
        await execute(
          tx,
          `INSERT OR REPLACE INTO tracked_apps
             (id, platform, exe_path, normalized_exe_path, process_name,
              display_name, publisher, icon_ref, first_seen_at, last_seen_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)`,
          [
            merged.id,
            merged.platform,
            merged.exePath,
            merged.normalizedExePath,
            merged.processName,
            merged.displayName,
            merged.publisher,
            merged.iconRef,
            merged.firstSeenAt,
            merged.lastSeenAt,
          ],
        );

        if (!existing) {
          result.migratedTrackedApps += 1;
        } else if (
          existing.first_seen_at !== merged.firstSeenAt ||
          existing.last_seen_at !== merged.lastSeenAt ||
          (existing.display_name ?? null) !== (merged.displayName ?? null)
        ) {
          result.updatedTrackedApps += 1;
        }

        await bumpVersionVector(tx, merged.id, deviceId);
      }

      for (const session of plan.sessions) {
        const existing = await queryOne<{ id: string }>(
          tx,
          "SELECT id FROM usage_sessions WHERE id = ?1",
          [session.id],
        );
        if (existing) {
          result.skippedSessions += 1;
          continue;
        }

        await execute(
          tx,
          `INSERT INTO usage_sessions
             (id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
              foreground_ms, idle_ms, window_title, process_name, exe_path, pid_start, pid_end, meta_json)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)`,
          [
            session.id,
            session.trackedAppId,
            session.deviceId,
            session.deviceName,
            session.platform,
            session.startedAt,
            session.endedAt,
            session.foregroundMs,
            session.idleMs,
            session.windowTitle,
            session.processName,
            session.exePath,
            session.pidStart,
            session.pidEnd,
            session.metaJson,
          ],
        );
        result.migratedSessions += 1;
        await bumpVersionVector(tx, session.id, deviceId);
      }

      await setSyncKv(
        tx,
        BACKFILL_MARKER_KEY,
        JSON.stringify({
          version: 1,
          imported_at: new Date().toISOString(),
          sessions: plan.sessions.length,
        }),
      );

      return result;
    });
  } finally {
    await Promise.resolve(arkDb.close?.());
  }
}
