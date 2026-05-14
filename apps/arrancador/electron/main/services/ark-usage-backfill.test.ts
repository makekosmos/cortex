import type { ArkTrackedAppRecord, ArkUsageApi, ArkUsageSessionRecord } from "@kosmos/ark";
import { describe, expect, it } from "vitest";
import type { DbLike } from "../helpers/shared";
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
  const db: DbLike = {
    all: () => [],
    get: () => undefined,
    run: () => ({ changes: 0 }),
    transaction: async (fn) => await Promise.resolve(fn(db)),
    close: () => undefined,
  };

  return {
    db,
  };
}

function createArkKv() {
  const values = new Map<string, string>();

  return {
    kv: {
      get: async (key: string) => values.get(key) ?? null,
      set: async (key: string, value: string) => {
        values.set(key, value);
      },
    },
    values,
  };
}

function createArkUsage() {
  const trackedApps = new Map<string, ArkTrackedAppRecord>();
  const usageSessions = new Map<string, ArkUsageSessionRecord>();
  const usage: Pick<ArkUsageApi, "loadAll"> & {
    trackedApps: Pick<ArkUsageApi["trackedApps"], "upsert">;
    sessions: Pick<ArkUsageApi["sessions"], "upsert">;
  } = {
    loadAll: async () => ({
      trackedApps: [...trackedApps.values()],
      usageSessions: [...usageSessions.values()],
      usageEvents: [],
    }),
    trackedApps: {
      upsert: async (record) => {
        trackedApps.set(record.id, record);
      },
    },
    sessions: {
      upsert: async (record) => {
        usageSessions.set(record.id, record);
      },
    },
  };

  return {
    usage,
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
    const arkKv = createArkKv();
    const arkUsage = createArkUsage();

    const first = await backfillLegacyUsageToArk({
      legacyDb,
      arkDbPath: "selected.db",
      arkUsage: arkUsage.usage,
      arkKv: arkKv.kv,
      resolveUsageDb: async () => ({
        db: ark.db,
        path: "selected.db",
      }),
    });

    expect(first.alreadyBackfilled).toBe(false);
    expect(first.migratedTrackedApps).toBe(1);
    expect(first.migratedSessions).toBe(2);
    expect(arkUsage.trackedApps.size).toBe(1);
    expect(arkUsage.usageSessions.size).toBe(2);
    expect(arkKv.values.has("arrancador.legacy_usage_backfill.v1")).toBe(true);

    const second = await backfillLegacyUsageToArk({
      legacyDb,
      arkDbPath: "selected.db",
      arkUsage: arkUsage.usage,
      arkKv: arkKv.kv,
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
    expect(arkUsage.trackedApps.size).toBe(1);
    expect(arkUsage.usageSessions.size).toBe(2);
  });
});
