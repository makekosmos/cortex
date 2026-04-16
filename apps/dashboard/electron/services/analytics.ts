import fs from "node:fs";
import path from "node:path";
import { openSqliteDatabase } from "../db/sqlite.ts";
import type {
  DashboardLoadOptions,
  DashboardSnapshot,
  DailyTrendPoint,
  DatabaseStatus,
  HourlyHeatmapCell,
  RecentSessionEntry,
  TopAppEntry,
  UsageSummary,
} from "../../shared/analytics.ts";

type SummaryRow = {
  tracked_app_count: number | null;
  session_count: number | null;
  event_count: number | null;
  total_foreground_ms: number | null;
  total_idle_ms: number | null;
  first_recorded_at: string | null;
  last_recorded_at: string | null;
};

type DailyTrendRow = {
  date: string;
  foreground_ms: number | null;
  idle_ms: number | null;
  sessions: number | null;
};

type HourlyHeatmapRow = {
  weekday: number | null;
  hour: number | null;
  foreground_ms: number | null;
};

type TopAppRow = {
  id: string;
  display_name: string | null;
  process_name: string;
  normalized_path: string;
  foreground_ms: number | null;
  idle_ms: number | null;
  sessions: number | null;
  last_seen_at: string | null;
};

type RecentSessionRow = {
  id: string;
  tracked_app_id: string;
  display_name: string | null;
  process_name: string;
  platform: string;
  device_name: string;
  started_at: string;
  ended_at: string | null;
  foreground_ms: number | null;
  idle_ms: number | null;
  window_title: string | null;
};

function clampPositiveInt(value: number | undefined, fallback: number): number {
  if (!Number.isFinite(value)) {
    return fallback;
  }
  return Math.max(1, Math.floor(value ?? fallback));
}

function isoDateDaysAgo(daysAgo: number): string {
  const now = new Date();
  now.setUTCHours(0, 0, 0, 0);
  now.setUTCDate(now.getUTCDate() - daysAgo);
  return now.toISOString().slice(0, 10);
}

function enumerateDates(rangeDays: number): string[] {
  return Array.from({ length: rangeDays }, (_, index) =>
    isoDateDaysAgo(rangeDays - index - 1),
  );
}

function emptySummary(): UsageSummary {
  return {
    trackedAppCount: 0,
    sessionCount: 0,
    eventCount: 0,
    totalForegroundMs: 0,
    totalIdleMs: 0,
    firstRecordedAt: null,
    lastRecordedAt: null,
  };
}

function emptySnapshot(status: DatabaseStatus): DashboardSnapshot {
  return {
    generatedAt: new Date().toISOString(),
    status,
    summary: emptySummary(),
    dailyTrend: [],
    hourlyHeatmap: [],
    topApps: [],
    recentSessions: [],
  };
}

export function resolveDefaultArkDbPath(): string {
  if (process.env.ARK_DB_PATH && process.env.ARK_DB_PATH.trim().length > 0) {
    return process.env.ARK_DB_PATH;
  }

  const appData = process.env.APPDATA ?? process.env.LOCALAPPDATA;
  if (!appData) {
    return path.join(process.cwd(), "ark.db");
  }

  return path.join(appData, "Kepler", "ark.db");
}

export function loadDashboardSnapshot(
  options: DashboardLoadOptions = {},
  selectedDbPath?: string | null,
): DashboardSnapshot {
  const rangeDays = clampPositiveInt(options.rangeDays, 21);
  const topAppsLimit = clampPositiveInt(options.topAppsLimit, 8);
  const recentSessionsLimit = clampPositiveInt(options.recentSessionsLimit, 24);
  const explicitDbPath = options.dbPath ?? null;
  const resolvedPath = explicitDbPath ?? selectedDbPath ?? resolveDefaultArkDbPath();
  const source: DatabaseStatus["source"] = explicitDbPath
    ? "explicit"
    : selectedDbPath
      ? "selected"
      : "default";

  if (!resolvedPath || !fs.existsSync(resolvedPath)) {
    return emptySnapshot({
      path: resolvedPath,
      exists: false,
      readable: false,
      source,
      message:
        "Файл Ark DB не найден. Выберите существующую базу или дождитесь первых usage-данных.",
    });
  }

  let db;
  try {
    db = openSqliteDatabase(resolvedPath, {
      readonly: true,
      fileMustExist: true,
      timeoutMs: 2_000,
    });
  } catch (error) {
    return emptySnapshot({
      path: resolvedPath,
      exists: true,
      readable: false,
      source,
      message: `Не удалось открыть Ark DB: ${(error as Error).message}`,
    });
  }

  try {
    const rangeStart = isoDateDaysAgo(rangeDays - 1);
    const rangeEnd = isoDateDaysAgo(0);

    const summaryRow = db.get<SummaryRow>(
      `SELECT COUNT(DISTINCT tracked_apps.id) AS tracked_app_count,
              COUNT(DISTINCT usage_sessions.id) AS session_count,
              (SELECT COUNT(*) FROM usage_events) AS event_count,
              COALESCE(SUM(usage_sessions.foreground_ms), 0) AS total_foreground_ms,
              COALESCE(SUM(usage_sessions.idle_ms), 0) AS total_idle_ms,
              MIN(usage_sessions.started_at) AS first_recorded_at,
              MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_recorded_at
       FROM tracked_apps
       LEFT JOIN usage_sessions ON usage_sessions.tracked_app_id = tracked_apps.id`,
    );

    const summary: UsageSummary = {
      trackedAppCount: Number(summaryRow?.tracked_app_count ?? 0),
      sessionCount: Number(summaryRow?.session_count ?? 0),
      eventCount: Number(summaryRow?.event_count ?? 0),
      totalForegroundMs: Number(summaryRow?.total_foreground_ms ?? 0),
      totalIdleMs: Number(summaryRow?.total_idle_ms ?? 0),
      firstRecordedAt: summaryRow?.first_recorded_at ?? null,
      lastRecordedAt: summaryRow?.last_recorded_at ?? null,
    };

    const trendRows = db.all<DailyTrendRow>(
      `SELECT SUBSTR(COALESCE(ended_at, started_at), 1, 10) AS date,
              COALESCE(SUM(foreground_ms), 0) AS foreground_ms,
              COALESCE(SUM(idle_ms), 0) AS idle_ms,
              COUNT(*) AS sessions
       FROM usage_sessions
       WHERE SUBSTR(COALESCE(ended_at, started_at), 1, 10) BETWEEN ?1 AND ?2
       GROUP BY date
       ORDER BY date ASC`,
      [rangeStart, rangeEnd],
    );
    const trendByDate = new Map(
      trendRows.map((row) => [
        row.date,
        {
          foregroundMs: Number(row.foreground_ms ?? 0),
          idleMs: Number(row.idle_ms ?? 0),
          sessions: Number(row.sessions ?? 0),
        },
      ]),
    );
    const dailyTrend: DailyTrendPoint[] = enumerateDates(rangeDays).map((date) => {
      const item = trendByDate.get(date);
      return {
        date,
        foregroundMs: item?.foregroundMs ?? 0,
        idleMs: item?.idleMs ?? 0,
        sessions: item?.sessions ?? 0,
      };
    });

    const hourlyHeatmap = db
      .all<HourlyHeatmapRow>(
        `SELECT CAST(STRFTIME('%w', started_at) AS INTEGER) AS weekday,
                CAST(STRFTIME('%H', started_at) AS INTEGER) AS hour,
                COALESCE(SUM(foreground_ms), 0) AS foreground_ms
         FROM usage_sessions
         WHERE SUBSTR(COALESCE(ended_at, started_at), 1, 10) BETWEEN ?1 AND ?2
         GROUP BY weekday, hour`,
        [rangeStart, rangeEnd],
      )
      .filter((row) => row.weekday !== null && row.hour !== null)
      .map<HourlyHeatmapCell>((row) => ({
        weekday: Number(row.weekday),
        hour: Number(row.hour),
        foregroundMs: Number(row.foreground_ms ?? 0),
      }));

    const topApps = db
      .all<TopAppRow>(
        `SELECT tracked_apps.id,
                tracked_apps.display_name,
                tracked_apps.process_name,
                tracked_apps.normalized_exe_path AS normalized_path,
                COALESCE(SUM(usage_sessions.foreground_ms), 0) AS foreground_ms,
                COALESCE(SUM(usage_sessions.idle_ms), 0) AS idle_ms,
                COUNT(usage_sessions.id) AS sessions,
                MAX(COALESCE(usage_sessions.ended_at, usage_sessions.started_at)) AS last_seen_at
         FROM tracked_apps
         JOIN usage_sessions ON usage_sessions.tracked_app_id = tracked_apps.id
         GROUP BY tracked_apps.id
         ORDER BY foreground_ms DESC, last_seen_at DESC
         LIMIT ?1`,
        [topAppsLimit],
      )
      .map<TopAppEntry>((row) => ({
        id: row.id,
        displayName: row.display_name?.trim() || row.process_name,
        processName: row.process_name,
        normalizedPath: row.normalized_path,
        foregroundMs: Number(row.foreground_ms ?? 0),
        idleMs: Number(row.idle_ms ?? 0),
        sessions: Number(row.sessions ?? 0),
        lastSeenAt: row.last_seen_at ?? null,
      }));

    const recentSessions = db
      .all<RecentSessionRow>(
        `SELECT usage_sessions.id,
                usage_sessions.tracked_app_id,
                tracked_apps.display_name,
                usage_sessions.process_name,
                usage_sessions.platform,
                usage_sessions.device_name,
                usage_sessions.started_at,
                usage_sessions.ended_at,
                usage_sessions.foreground_ms,
                usage_sessions.idle_ms,
                usage_sessions.window_title
         FROM usage_sessions
         JOIN tracked_apps ON tracked_apps.id = usage_sessions.tracked_app_id
         ORDER BY usage_sessions.started_at DESC
         LIMIT ?1`,
        [recentSessionsLimit],
      )
      .map<RecentSessionEntry>((row) => ({
        id: row.id,
        trackedAppId: row.tracked_app_id,
        displayName: row.display_name?.trim() || row.process_name,
        processName: row.process_name,
        platform: row.platform,
        deviceName: row.device_name,
        startedAt: row.started_at,
        endedAt: row.ended_at,
        foregroundMs: Number(row.foreground_ms ?? 0),
        idleMs: Number(row.idle_ms ?? 0),
        windowTitle: row.window_title ?? null,
      }));

    return {
      generatedAt: new Date().toISOString(),
      status: {
        path: resolvedPath,
        exists: true,
        readable: true,
        source,
        message:
          summary.sessionCount > 0
            ? null
            : "База читается, но usage-сессии пока не записаны.",
      },
      summary,
      dailyTrend,
      hourlyHeatmap,
      topApps,
      recentSessions,
    };
  } finally {
    db.close?.();
  }
}
