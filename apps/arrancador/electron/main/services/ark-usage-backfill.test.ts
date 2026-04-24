import { describe, expect, it } from "vitest";
import type { DbLike, DbRunResult, DbValue } from "../helpers/shared";
import {
  backfillLegacyUsageToArk,
  buildLegacyUsageBackfillPlan,
} from "./ark-usage-backfill";

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

function createLegacyDb(
  games: LegacyGameRow[],
  dailyRows: LegacyDailyRow[],
): DbLike {
  return {
    all(sql) {
      if (sql.includes("FROM games")) {
        return games;
      }

      if (sql.includes("FROM playtime_daily")) {
        return dailyRows;
      }

      return [];
    },
    get: () => undefined,
    run: () => ({ changes: 0 }),
    close: () => undefined,
  };
}

function createArkDb() {
  const syncKv = new Map<string, string>();
  const trackedApps = new Map<string, Record<string, unknown>>();
  const usageSessions = new Map<string, Record<string, unknown>>();

  const db: DbLike = {
    all: () => [],
    get(sql, params = []) {
      if (sql.includes("SELECT value FROM sync_kv")) {
        const value = syncKv.get(String(params[0] ?? ""));
        return value === undefined ? undefined : { value };
      }

      if (sql.includes("FROM tracked_apps")) {
        return trackedApps.get(String(params[0] ?? ""));
      }

      if (sql.includes("FROM usage_sessions")) {
        const id = String(params[0] ?? "");
        return usageSessions.has(id) ? { id } : undefined;
      }

      return undefined;
    },
    run(sql, params: readonly DbValue[] = []): DbRunResult {
      if (sql.includes("INSERT OR REPLACE INTO sync_kv")) {
        syncKv.set(String(params[0] ?? ""), String(params[1] ?? ""));
        return { changes: 1 };
      }

      if (sql.includes("INSERT OR REPLACE INTO tracked_apps")) {
        trackedApps.set(String(params[0] ?? ""), {
          id: String(params[0] ?? ""),
          platform: String(params[1] ?? ""),
          exe_path: String(params[2] ?? ""),
          normalized_exe_path: String(params[3] ?? ""),
          process_name: String(params[4] ?? ""),
          display_name: params[5] ?? null,
          publisher: params[6] ?? null,
          icon_ref: params[7] ?? null,
          first_seen_at: String(params[8] ?? ""),
          last_seen_at: String(params[9] ?? ""),
        });
        return { changes: 1 };
      }

      if (sql.includes("INSERT INTO usage_sessions")) {
        usageSessions.set(String(params[0] ?? ""), {
          id: String(params[0] ?? ""),
          tracked_app_id: String(params[1] ?? ""),
        });
        return { changes: 1 };
      }

      return { changes: 0 };
    },
    transaction: async (fn) => await Promise.resolve(fn(db)),
    close: () => undefined,
  };

  return {
    db,
    syncKv,
    trackedApps,
    usageSessions,
  };
}

describe("buildLegacyUsageBackfillPlan", () => {
  it("builds daily and residual synthetic sessions from legacy totals", () => {
    const plan = buildLegacyUsageBackfillPlan(
      [
        {
          id: "valorant",
          name: "VALORANT",
          exe_path: "C:\\VALORANT.exe",
          exe_name: "VALORANT.exe",
          date_added: "2026-01-01T00:00:00.000Z",
          total_playtime: 1200,
          last_played: "2026-04-16T10:00:00.000Z",
        },
      ],
      [
        {
          game_id: "valorant",
          date: "2026-04-15",
          seconds: 600,
        },
      ],
      "device-1",
      "Test Device",
    );

    expect(plan.trackedApps).toHaveLength(1);
    expect(plan.trackedApps[0]?.normalizedExePath).toBe("c:\\valorant.exe");
    expect(plan.sessions).toHaveLength(2);
    expect(plan.sessions.map((session) => session.kind)).toEqual(["daily", "residual"]);
    expect(plan.sessions.map((session) => session.foregroundMs)).toEqual([600000, 600000]);

    const samePlan = buildLegacyUsageBackfillPlan(
      [
        {
          id: "valorant",
          name: "VALORANT",
          exe_path: "C:\\VALORANT.exe",
          exe_name: "VALORANT.exe",
          date_added: "2026-01-01T00:00:00.000Z",
          total_playtime: 1200,
          last_played: "2026-04-16T10:00:00.000Z",
        },
      ],
      [
        {
          game_id: "valorant",
          date: "2026-04-15",
          seconds: 600,
        },
      ],
      "device-1",
      "Test Device",
    );

    expect(samePlan.sessions.map((session) => session.id)).toEqual(
      plan.sessions.map((session) => session.id),
    );
  });
});

describe("backfillLegacyUsageToArk", () => {
  it("imports legacy usage into Ark and stays idempotent on forced reruns", async () => {
    const legacyDb = createLegacyDb(
      [
        {
          id: "valorant",
          name: "VALORANT",
          exe_path: "C:\\VALORANT.exe",
          exe_name: "VALORANT.exe",
          date_added: "2026-01-01T00:00:00.000Z",
          total_playtime: 1200,
          last_played: "2026-04-16T10:00:00.000Z",
        },
      ],
      [
        {
          game_id: "valorant",
          date: "2026-04-15",
          seconds: 600,
        },
      ],
    );
    const ark = createArkDb();

    const first = await backfillLegacyUsageToArk({
      legacyDb,
      arkDbPath: "selected.db",
      resolveUsageDb: async () => ({
        db: ark.db,
        path: "selected.db",
      }),
    });

    expect(first.alreadyBackfilled).toBe(false);
    expect(first.migratedTrackedApps).toBe(1);
    expect(first.migratedSessions).toBe(2);
    expect(ark.trackedApps.size).toBe(1);
    expect(ark.usageSessions.size).toBe(2);
    expect(ark.syncKv.has("arrancador.legacy_usage_backfill.v1")).toBe(true);
    expect(ark.syncKv.has("lan_sync.version_vector")).toBe(true);

    const second = await backfillLegacyUsageToArk({
      legacyDb,
      arkDbPath: "selected.db",
      resolveUsageDb: async () => ({
        db: ark.db,
        path: "selected.db",
      }),
      force: true,
    });

    expect(second.alreadyBackfilled).toBe(false);
    expect(second.migratedTrackedApps).toBe(0);
    expect(second.migratedSessions).toBe(0);
    expect(second.skippedSessions).toBe(2);
    expect(ark.trackedApps.size).toBe(1);
    expect(ark.usageSessions.size).toBe(2);
  });
});
