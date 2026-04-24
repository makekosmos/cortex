import { describe, expect, it } from "vitest";
import type { DbLike, DbRunResult, DbValue } from "../helpers/shared";
import {
  addGameProcessBindings,
  listGameProcessBindings,
} from "./game-process-bindings";

type GameRow = {
  id: string;
  exe_path: string;
};

type BindingRow = {
  id: number;
  game_id: string;
  match_type: "exe_path" | "process_name";
  match_value: string;
  normalized_value: string;
  created_at: string;
};

function normalizePath(exePath: string) {
  return exePath.trim().replaceAll("/", "\\").toLowerCase();
}

function createBindingsDb(
  games: GameRow[],
  bindings: BindingRow[] = [],
): DbLike {
  let nextId = bindings.reduce((max, binding) => Math.max(max, binding.id), 0) + 1;
  const db: DbLike = {
    all(sql, params = []) {
      if (sql.includes("FROM game_process_bindings")) {
        const ids = new Set((params ?? []).map((value) => String(value)));
        const rows =
          ids.size === 0
            ? bindings
            : bindings.filter((binding) => ids.has(binding.game_id));
        return rows.map((binding) => ({ ...binding }));
      }

      return [];
    },

    get(sql, params = []) {
      if (sql.includes("SELECT LOWER(REPLACE(exe_path")) {
        const gameId = String(params[0] ?? "");
        const game = games.find((entry) => entry.id === gameId);
        return game ? { normalized_path: normalizePath(game.exe_path) } : undefined;
      }

      if (sql.includes("SELECT id") && sql.includes("FROM games")) {
        const normalizedPath = String(params[0] ?? "");
        const game = games.find((entry) => normalizePath(entry.exe_path) === normalizedPath);
        return game ? { id: game.id } : undefined;
      }

      if (sql.includes("SELECT game_id") && sql.includes("FROM game_process_bindings")) {
        const matchType = String(params[0] ?? "");
        const normalizedValue = String(params[1] ?? "");
        const binding = bindings.find(
          (entry) =>
            entry.match_type === matchType &&
            entry.normalized_value === normalizedValue,
        );
        return binding ? { game_id: binding.game_id } : undefined;
      }

      return undefined;
    },

    run(sql, params: readonly DbValue[] = []): DbRunResult {
      if (sql.includes("INSERT INTO game_process_bindings")) {
        bindings.push({
          id: nextId++,
          game_id: String(params[0] ?? ""),
          match_type: params[1] as "exe_path" | "process_name",
          match_value: String(params[2] ?? ""),
          normalized_value: String(params[3] ?? ""),
          created_at: String(params[4] ?? ""),
        });
        return { changes: 1 };
      }

      return { changes: 0 };
    },

    transaction: async (fn) => await Promise.resolve(fn(db)),
    close: () => undefined,
  };

  return db;
}

describe("addGameProcessBindings", () => {
  it("stores unique bindings, skips primary-path duplicates, and trims values", async () => {
    const db = createBindingsDb([
      { id: "game-1", exe_path: "C:\\Games\\Chronicle\\Chronicle.exe" },
    ]);

    await addGameProcessBindings(
      db,
      "game-1",
      [
        { match_type: "exe_path", match_value: "C:/Games/Chronicle/Chronicle.exe" },
        { match_type: "process_name", match_value: " Chronicle Helper.exe " },
        { match_type: "process_name", match_value: "chronicle helper.exe" },
      ],
      () => new Date("2026-04-23T12:00:00.000Z"),
    );

    await expect(listGameProcessBindings(db, ["game-1"])).resolves.toEqual([
      {
        id: 1,
        game_id: "game-1",
        match_type: "process_name",
        match_value: "Chronicle Helper.exe",
        created_at: "2026-04-23T12:00:00.000Z",
      },
    ]);
  });

  it("rejects bindings that collide with another game's primary executable", async () => {
    const db = createBindingsDb([
      { id: "game-1", exe_path: "C:\\Games\\Chronicle\\Chronicle.exe" },
      { id: "game-2", exe_path: "C:\\Games\\Other\\Other.exe" },
    ]);

    await expect(
      addGameProcessBindings(db, "game-1", [
        {
          match_type: "exe_path",
          match_value: "C:\\Games\\Other\\Other.exe",
        },
      ]),
    ).rejects.toThrow("primary process for another game");
  });
});
