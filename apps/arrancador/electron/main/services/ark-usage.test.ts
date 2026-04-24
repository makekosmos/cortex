import { describe, expect, it } from "vitest";
import type { DbLike } from "../helpers/shared";
import {
  createGameUsageReadModel,
  createPlaytimeStatsRepository,
  resolveUsageTrackerDb,
} from "./ark-usage";
import type { Game } from "./games/types";

function createDb(_label: string): DbLike {
  return {
    all: () => [],
    get: () => undefined,
    run: () => ({ changes: 0 }),
    close: () => undefined,
  } as unknown as DbLike & { label: string };
}

type LegacyGameRow = {
  id: string;
  name: string;
  normalized_path: string;
};

type TrackerAggregateRow = {
  normalized_path: string;
  total_seconds: number;
  session_count: number;
  last_played: string | null;
};

type TrackerDailyRow = {
  normalized_path: string;
  date: string;
  seconds: number;
};

function createLegacyGamesDb(games: LegacyGameRow[]): DbLike {
  return {
    all(sql, params) {
      if (!sql.includes("SELECT id, name, LOWER(REPLACE(exe_path")) {
        return [];
      }

      if (!params || params.length === 0) {
        return games;
      }

      const ids = new Set(params.map((value) => String(value)));
      return games.filter((game) => ids.has(game.id));
    },
    get: () => undefined,
    run: () => ({ changes: 0 }),
    close: () => undefined,
  };
}

function createTrackerDb(
  aggregates: TrackerAggregateRow[],
  dailyRows: TrackerDailyRow[],
  onClose?: () => void,
): DbLike {
  return {
    all(sql, params) {
      const normalizedPaths = new Set(
        (params ?? [])
          .slice(0, sql.includes("BETWEEN") ? -2 : undefined)
          .map((value) => String(value)),
      );

      if (sql.includes("SUM(CASE WHEN usage_sessions.foreground_ms > 0")) {
        return aggregates.filter((row) => normalizedPaths.has(row.normalized_path));
      }

      if (sql.includes("SUBSTR(COALESCE(usage_sessions.ended_at")) {
        let rows = dailyRows.filter((row) => normalizedPaths.has(row.normalized_path));
        if (sql.includes("BETWEEN")) {
          const [start, end] = (params ?? []).slice(-2).map((value) => String(value));
          rows = rows.filter((row) => row.date >= start && row.date <= end);
        }
        return rows;
      }

      return [];
    },
    get: () => undefined,
    run: () => ({ changes: 0 }),
    close: () => {
      onClose?.();
    },
  };
}

describe("resolveUsageTrackerDb", () => {
  it("falls back to root ark db when the selected-space db has no usage rows", async () => {
    const selectedDb = createDb("selected");
    const rootDb = createDb("root");
    let hasTrackerRowsCalls = 0;
    const openDb = (targetPath: string) => {
      if (targetPath === "selected.db") return selectedDb;
      if (targetPath === "root.db") return rootDb;
      return null;
    };
    const hasTrackerRows = async (db: DbLike) => {
      hasTrackerRowsCalls += 1;
      return db === rootDb;
    };

    const result = await resolveUsageTrackerDb({
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      openDb,
      hasTrackerRows,
    });

    expect(result.path).toBe("root.db");
    expect(result.db).toBe(rootDb);
    expect(hasTrackerRowsCalls).toBe(2);
  });

  it("keeps the selected-space db when it already has tracker rows", async () => {
    const selectedDb = createDb("selected");
    const rootDb = createDb("root");
    let hasTrackerRowsCalls = 0;
    const openDb = (targetPath: string) => {
      if (targetPath === "selected.db") return selectedDb;
      if (targetPath === "root.db") return rootDb;
      return null;
    };
    const hasTrackerRows = async (db: DbLike) => {
      hasTrackerRowsCalls += 1;
      return db === selectedDb;
    };

    const result = await resolveUsageTrackerDb({
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      openDb,
      hasTrackerRows,
    });

    expect(result.path).toBe("selected.db");
    expect(result.db).toBe(selectedDb);
    expect(hasTrackerRowsCalls).toBe(1);
  });
});

describe("createPlaytimeStatsRepository", () => {
  it("reads statistics from Ark usage rows only", async () => {
    const legacyDb = createLegacyGamesDb([
      { id: "valorant", name: "VALORANT", normalized_path: "c:\\valorant.exe" },
      { id: "doom", name: "DOOM", normalized_path: "c:\\doom.exe" },
    ]);
    const trackerDb = createTrackerDb(
      [
        {
          normalized_path: "c:\\valorant.exe",
          total_seconds: 8400,
          session_count: 2,
          last_played: "2026-04-17T12:00:00.000Z",
        },
        {
          normalized_path: "c:\\doom.exe",
          total_seconds: 1200,
          session_count: 1,
          last_played: "2026-04-16T08:00:00.000Z",
        },
      ],
      [
        { normalized_path: "c:\\valorant.exe", date: "2026-04-16", seconds: 5400 },
        { normalized_path: "c:\\valorant.exe", date: "2026-04-17", seconds: 3000 },
        { normalized_path: "c:\\doom.exe", date: "2026-04-16", seconds: 1200 },
      ],
    );

    let closeCalls = 0;
    let resolveCalls = 0;
    const repository = createPlaytimeStatsRepository({
      legacyDb,
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      resolveUsageDb: async () => {
        resolveCalls += 1;
        return {
          db: trackerDb,
          path: "root.db",
        };
      },
    });

    trackerDb.close = () => {
      closeCalls += 1;
    };

    await expect(repository.getRangeStats("2026-04-16", "2026-04-17")).resolves.toEqual({
      dailyTotals: [
        { date: "2026-04-16", seconds: 6600 },
        { date: "2026-04-17", seconds: 3000 },
      ],
      perGameTotals: [
        { id: "valorant", name: "VALORANT", seconds: 8400 },
        { id: "doom", name: "DOOM", seconds: 1200 },
      ],
    });
    expect(resolveCalls).toBe(1);
    expect(closeCalls).toBe(1);
  });

  it("does not fall back to legacy stats when Ark usage data is unavailable", async () => {
    const repository = createPlaytimeStatsRepository({
      legacyDb: createLegacyGamesDb([
        { id: "valorant", name: "VALORANT", normalized_path: "c:\\valorant.exe" },
      ]),
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      resolveUsageDb: async () => ({
        db: null,
        path: null,
      }),
    });

    await expect(repository.getDailyTotals("2026-04-16", "2026-04-17")).resolves.toEqual([]);
    await expect(repository.getPerGameTotals("2026-04-16", "2026-04-17")).resolves.toEqual([]);
  });
});

describe("createGameUsageReadModel", () => {
  it("hydrates play_count, total_playtime, and last_played from Ark only", async () => {
    let closeCalls = 0;
    const readModel = createGameUsageReadModel({
      legacyDb: createLegacyGamesDb([]),
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      resolveUsageDb: async () => ({
        db: createTrackerDb(
          [
            {
              normalized_path: "c:\\valorant.exe",
              total_seconds: 8400,
              session_count: 3,
              last_played: "2026-04-17T12:00:00.000Z",
            },
          ],
          [],
          () => {
            closeCalls += 1;
          },
        ),
        path: "root.db",
      }),
    });

    const inputGame: Game = {
      id: "valorant",
      ark_object_id: null,
      name: "VALORANT",
      exe_path: "C:\\VALORANT.exe",
      exe_name: "VALORANT.exe",
      process_bindings: [],
      play_status: "not_started",
      rawg_id: null,
      description: null,
      released: null,
      background_image: null,
      metacritic: null,
      rating: null,
      genres: null,
      platforms: null,
      developers: null,
      publishers: null,
      cover_image: null,
      icon_image: null,
      is_favorite: false,
      play_count: 99,
      total_playtime: 10000,
      last_played: "2026-04-16T19:54:26.571Z",
      date_added: "2026-01-01T00:00:00.000Z",
      backup_enabled: false,
      last_backup: null,
      backup_count: 0,
      save_path: null,
      user_rating: null,
      user_note: null,
    };

    await expect(readModel.hydrateGame(inputGame)).resolves.toMatchObject({
      id: "valorant",
      play_count: 3,
      total_playtime: 8400,
      last_played: "2026-04-17T12:00:00.000Z",
    });
    expect(closeCalls).toBe(1);
  });

  it("clears stale local usage fields when Ark has no usage rows for a game", async () => {
    const readModel = createGameUsageReadModel({
      legacyDb: createLegacyGamesDb([]),
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      resolveUsageDb: async () => ({
        db: createTrackerDb([], []),
        path: "root.db",
      }),
    });

    const inputGame: Game = {
      id: "valorant",
      ark_object_id: null,
      name: "VALORANT",
      exe_path: "C:\\VALORANT.exe",
      exe_name: "VALORANT.exe",
      process_bindings: [],
      play_status: "not_started",
      rawg_id: null,
      description: null,
      released: null,
      background_image: null,
      metacritic: null,
      rating: null,
      genres: null,
      platforms: null,
      developers: null,
      publishers: null,
      cover_image: null,
      icon_image: null,
      is_favorite: false,
      play_count: 12,
      total_playtime: 4800,
      last_played: "2026-04-16T19:54:26.571Z",
      date_added: "2026-01-01T00:00:00.000Z",
      backup_enabled: false,
      last_backup: null,
      backup_count: 0,
      save_path: null,
      user_rating: null,
      user_note: null,
    };

    await expect(readModel.hydrateGame(inputGame)).resolves.toMatchObject({
      id: "valorant",
      play_count: 0,
      total_playtime: 0,
      last_played: null,
    });
  });
});
