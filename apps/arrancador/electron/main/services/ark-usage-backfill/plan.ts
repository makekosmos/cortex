import { createHash } from "node:crypto";

const PLATFORM = process.platform === "win32" ? "windows" : process.platform;

export type LegacyGameRow = {
  id: string;
  name: string;
  exe_path: string;
  exe_name: string;
  date_added: string;
  total_playtime: number;
  last_played: string | null;
};

export type LegacyDailyRow = {
  game_id: string;
  date: string;
  seconds: number;
};

export type LegacyUsageSessionKind = "daily" | "residual";

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
  const endedAt = preferredEndAt?.startsWith(date) ? preferredEndAt : defaultEnd;
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

export function buildLegacyUsageBackfillPlan(
  games: readonly LegacyGameRow[],
  dailyRows: readonly LegacyDailyRow[],
  deviceId: string,
  deviceName: string,
): LegacyUsageBackfillPlan {
  const dailyRowsByGame = new Map<string, LegacyDailyRow[]>();
  for (const row of dailyRows) {
    if (Number(row.seconds ?? 0) <= 0) {
      continue;
    }

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
