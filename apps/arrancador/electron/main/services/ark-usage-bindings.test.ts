import { describe, expect, it } from "vitest";
import type { DbLike } from "../helpers/shared";
import {
  createGameUsageReadModel,
  createPlaytimeStatsRepository,
} from "./ark-usage";
import type { Game, GameProcessBinding } from "./games/types";

const unavailableArkUsage = {
  loadAll: async () => {
    throw new Error("mock ark runtime unavailable");
  },
};

type LegacyGameRow = {
  id: string;
  name: string;
  normalized_path: string;
};

type LegacyBindingRow = {
  id: number;
  game_id: string;
  match_type: "exe_path" | "process_name";
  match_value: string;
  normalized_value: string;
  created_at: string;
};

type AggregateRow = {
  tracked_app_id: string;
  normalized_path: string | null;
  normalized_process_name: string | null;
  total_seconds: number;
  session_count: number;
  last_played: string | null;
};

type DailyRow = {
  tracked_app_id: string;
  normalized_path: string | null;
  normalized_process_name: string | null;
  date: string;
  seconds: number;
};

function createLegacyDb(
  games: LegacyGameRow[],
  bindings: LegacyBindingRow[],
): DbLike {
  return {
    all(sql, params) {
      if (sql.includes("FROM game_process_bindings")) {
        const ids = new Set((params ?? []).map((value) => String(value)));
        return bindings.filter((binding) => ids.size === 0 || ids.has(binding.game_id));
      }

      if (sql.includes("FROM games")) {
        const ids = new Set((params ?? []).map((value) => String(value)));
        return games.filter((game) => ids.size === 0 || ids.has(game.id));
      }

      return [];
    },
    get: () => undefined,
    run: () => ({ changes: 0 }),
    close: () => undefined,
  } as unknown as DbLike;
}

function createTrackerDb(
  aggregates: AggregateRow[],
  dailyRows: DailyRow[],
  onClose?: () => void,
): DbLike {
  return {
    all(sql) {
      if (sql.includes("AS total_seconds")) {
        return aggregates;
      }
      if (sql.includes("AS date")) {
        return dailyRows;
      }
      return [];
    },
    get: () => undefined,
    run: () => ({ changes: 0 }),
    close: () => {
      onClose?.();
    },
  } as unknown as DbLike;
}

function createBinding(
  id: number,
  match_type: "exe_path" | "process_name",
  match_value: string,
): GameProcessBinding {
  return {
    id,
    game_id: "game-1",
    match_type,
    match_value,
    created_at: "2026-04-23T10:00:00.000Z",
  };
}

function createGameWithBindings(bindings: GameProcessBinding[]): Game {
  return {
    id: "game-1",
    ark_object_id: null,
    name: "Chronicle",
    exe_path: "C:\\Games\\Chronicle\\Chronicle.exe",
    exe_name: "Chronicle.exe",
    process_bindings: bindings,
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
    play_count: 0,
    total_playtime: 0,
    last_played: null,
    date_added: "2026-01-01T00:00:00.000Z",
    backup_enabled: false,
    last_backup: null,
    backup_count: 0,
    save_path: null,
    user_rating: null,
    user_note: null,
  };
}

describe("multi-process Ark usage bindings", () => {
  it("hydrates a game from primary path, extra exe path, and process-name bindings", async () => {
    let closeCalls = 0;
    const readModel = createGameUsageReadModel({
      legacyDb: createLegacyDb([], []),
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      arkUsage: unavailableArkUsage,
      resolveUsageDb: async () => ({
        db: createTrackerDb(
          [
            {
              tracked_app_id: "tracked-main",
              normalized_path: "c:\\games\\chronicle\\chronicle.exe",
              normalized_process_name: "chronicle.exe",
              total_seconds: 1800,
              session_count: 1,
              last_played: "2026-04-20T10:00:00.000Z",
            },
            {
              tracked_app_id: "tracked-alt",
              normalized_path: "c:\\games\\chronicle\\chronicle_dx12.exe",
              normalized_process_name: "chronicle_dx12.exe",
              total_seconds: 7200,
              session_count: 2,
              last_played: "2026-04-21T09:00:00.000Z",
            },
            {
              tracked_app_id: "tracked-helper",
              normalized_path: null,
              normalized_process_name: "chronicle helper.exe",
              total_seconds: 600,
              session_count: 1,
              last_played: "2026-04-22T08:00:00.000Z",
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

    const hydrated = await readModel.hydrateGame(
      createGameWithBindings([
        createBinding(1, "exe_path", "C:\\Games\\Chronicle\\Chronicle_DX12.exe"),
        createBinding(2, "process_name", "Chronicle Helper.exe"),
      ]),
    );

    expect(hydrated.play_count).toBe(4);
    expect(hydrated.total_playtime).toBe(9600);
    expect(hydrated.last_played).toBe("2026-04-22T08:00:00.000Z");
    expect(closeCalls).toBe(1);
  });

  it("aggregates playtime stats across explicit bindings", async () => {
    const repository = createPlaytimeStatsRepository({
      legacyDb: createLegacyDb(
        [
          {
            id: "game-1",
            name: "Chronicle",
            normalized_path: "c:\\games\\chronicle\\chronicle.exe",
          },
        ],
        [
          {
            id: 1,
            game_id: "game-1",
            match_type: "exe_path",
            match_value: "C:\\Games\\Chronicle\\Chronicle_DX12.exe",
            normalized_value: "c:\\games\\chronicle\\chronicle_dx12.exe",
            created_at: "2026-04-23T10:00:00.000Z",
          },
          {
            id: 2,
            game_id: "game-1",
            match_type: "process_name",
            match_value: "Chronicle Helper.exe",
            normalized_value: "chronicle helper.exe",
            created_at: "2026-04-23T10:01:00.000Z",
          },
        ],
      ),
      arkDbPath: "selected.db",
      fallbackArkDbPath: "root.db",
      arkUsage: unavailableArkUsage,
      resolveUsageDb: async () => ({
        db: createTrackerDb(
          [],
          [
            {
              tracked_app_id: "tracked-main",
              normalized_path: "c:\\games\\chronicle\\chronicle.exe",
              normalized_process_name: "chronicle.exe",
              date: "2026-04-20",
              seconds: 1800,
            },
            {
              tracked_app_id: "tracked-alt",
              normalized_path: "c:\\games\\chronicle\\chronicle_dx12.exe",
              normalized_process_name: "chronicle_dx12.exe",
              date: "2026-04-20",
              seconds: 3600,
            },
            {
              tracked_app_id: "tracked-helper",
              normalized_path: null,
              normalized_process_name: "chronicle helper.exe",
              date: "2026-04-21",
              seconds: 600,
            },
          ],
        ),
        path: "root.db",
      }),
    });

    await expect(repository.getRangeStats("2026-04-20", "2026-04-21")).resolves.toEqual({
      dailyTotals: [
        { date: "2026-04-20", seconds: 5400 },
        { date: "2026-04-21", seconds: 600 },
      ],
      perGameTotals: [{ id: "game-1", name: "Chronicle", seconds: 6000 }],
    });
  });
});
