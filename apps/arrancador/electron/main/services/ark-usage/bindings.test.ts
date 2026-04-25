import { describe, expect, it } from "vitest";
import type { Game } from "../games/types";
import {
  applyUsageAggregatesToGames,
  buildParameterizedList,
  buildUsageBindingIndexFromGames,
  buildUsageWhereClause,
  findGameIdForTrackedRow,
} from "./bindings";

function createGame(overrides: Partial<Game>): Game {
  return {
    id: "game-1",
    ark_object_id: null,
    name: "Chronicle",
    exe_path: "C:\\Games\\Chronicle\\Chronicle.exe",
    exe_name: "Chronicle.exe",
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
    total_playtime: 999,
    last_played: "2026-04-20T00:00:00.000Z",
    date_added: "2026-01-01T00:00:00.000Z",
    backup_enabled: false,
    last_backup: null,
    backup_count: 0,
    save_path: null,
    user_rating: null,
    user_note: null,
    ...overrides,
  };
}

describe("Ark usage binding helpers", () => {
  it("builds stable parameter placeholders with optional start index", () => {
    expect(buildParameterizedList(["a", "b"])).toBe("?1, ?2");
    expect(buildParameterizedList(["a", "b"], 3)).toBe("?3, ?4");
  });

  it("indexes primary paths and explicit process bindings", () => {
    const index = buildUsageBindingIndexFromGames([
      createGame({
        process_bindings: [
          {
            id: 1,
            game_id: "game-1",
            match_type: "exe_path",
            match_value: "C:/Games/Chronicle/Chronicle_DX12.exe",
            created_at: "2026-04-24T00:00:00.000Z",
          },
          {
            id: 2,
            game_id: "game-1",
            match_type: "process_name",
            match_value: "Chronicle Helper.exe",
            created_at: "2026-04-24T00:00:00.000Z",
          },
        ],
      }),
    ]);

    expect(index.gameNamesById.get("game-1")).toBe("Chronicle");
    expect(index.gameIdsByPath.get("c:\\games\\chronicle\\chronicle.exe")).toBe(
      "game-1",
    );
    expect(
      index.gameIdsByPath.get("c:\\games\\chronicle\\chronicle_dx12.exe"),
    ).toBe("game-1");
    expect(index.gameIdsByProcessName.get("chronicle helper.exe")).toBe("game-1");
  });

  it("builds a combined SQL predicate for path and process-name bindings", () => {
    const index = buildUsageBindingIndexFromGames([
      createGame({
        process_bindings: [
          {
            id: 1,
            game_id: "game-1",
            match_type: "process_name",
            match_value: "Chronicle Helper.exe",
            created_at: "2026-04-24T00:00:00.000Z",
          },
        ],
      }),
    ]);

    const where = buildUsageWhereClause(index);

    expect(where.whereSql).toContain("tracked_apps.normalized_exe_path IN (?1)");
    expect(where.whereSql).toContain("LOWER(COALESCE(tracked_apps.process_name, '')) IN (?2)");
    expect(where.params).toEqual([
      "c:\\games\\chronicle\\chronicle.exe",
      "chronicle helper.exe",
    ]);
  });

  it("matches tracked rows by normalized path first, then process name", () => {
    const index = buildUsageBindingIndexFromGames([
      createGame({
        process_bindings: [
          {
            id: 1,
            game_id: "game-1",
            match_type: "process_name",
            match_value: "Chronicle Helper.exe",
            created_at: "2026-04-24T00:00:00.000Z",
          },
        ],
      }),
    ]);

    expect(
      findGameIdForTrackedRow(
        {
          normalized_path: "c:/games/chronicle/chronicle.exe",
          normalized_process_name: "other.exe",
        },
        index,
      ),
    ).toBe("game-1");
    expect(
      findGameIdForTrackedRow(
        {
          normalized_path: null,
          normalized_process_name: "Chronicle Helper.exe",
        },
        index,
      ),
    ).toBe("game-1");
  });

  it("applies aggregate usage rows and clears stale local usage when unmatched", () => {
    const games = [
      createGame({ id: "matched" }),
      createGame({
        id: "stale",
        name: "Stale",
        exe_path: "C:\\Games\\Stale\\Stale.exe",
      }),
    ];
    const index = buildUsageBindingIndexFromGames(games);

    const hydrated = applyUsageAggregatesToGames(
      games,
      [
        {
          tracked_app_id: "tracked-main",
          normalized_path: "c:\\games\\chronicle\\chronicle.exe",
          normalized_process_name: "chronicle.exe",
          total_seconds: 120,
          session_count: 1,
          last_played: "2026-04-24T00:00:00.000Z",
        },
        {
          tracked_app_id: "tracked-main-2",
          normalized_path: "c:\\games\\chronicle\\chronicle.exe",
          normalized_process_name: "chronicle.exe",
          total_seconds: 180,
          session_count: 2,
          last_played: "2026-04-25T00:00:00.000Z",
        },
      ],
      index,
    );

    expect(hydrated.find((game) => game.id === "matched")).toMatchObject({
      play_count: 3,
      total_playtime: 300,
      last_played: "2026-04-25T00:00:00.000Z",
    });
    expect(hydrated.find((game) => game.id === "stale")).toMatchObject({
      play_count: 0,
      total_playtime: 0,
      last_played: null,
    });
  });
});
