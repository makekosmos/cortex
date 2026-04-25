import { describe, expect, it, vi } from "vitest";
import type { DbLike } from "../helpers/shared";
import { createGamesService } from "./games";
import type { GameDbRow } from "./games/rows";

function gameRow(overrides: Partial<GameDbRow> = {}): GameDbRow {
  return {
    id: "game-1",
    ark_object_id: null,
    name: "Control",
    exe_path: "C:\\Games\\Control\\Control.exe",
    exe_name: "Control.exe",
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
    is_favorite: 0,
    play_count: 0,
    total_playtime: 0,
    last_played: null,
    date_added: "2026-04-24T00:00:00.000Z",
    backup_enabled: 1,
    last_backup: null,
    backup_count: 0,
    save_path: null,
    user_rating: null,
    user_note: null,
    play_status: "not_started",
    ...overrides,
  };
}

function createDb(options: {
  get?: DbLike["get"];
  all?: DbLike["all"];
  run?: DbLike["run"];
  transaction?: DbLike["transaction"];
} = {}): DbLike {
  return {
    get: options.get ?? (async () => undefined),
    all: options.all ?? (async () => []),
    run: options.run ?? (async () => ({ changes: 0 })),
    transaction: options.transaction,
  };
}

describe("createGamesService", () => {
  it("records launches by loading and hydrating the current game snapshot", async () => {
    const hydrateGame = vi.fn(async (game) => ({
      ...game,
      play_count: 4,
      total_playtime: 600,
    }));
    const db = createDb({
      get: async () => gameRow(),
      all: async () => [],
    });
    const service = createGamesService({
      db,
      usageReadModel: {
        hydrateGame,
        hydrateGames: async (games) => games,
      },
    });

    await expect(service.recordGameLaunch("game-1")).resolves.toMatchObject({
      id: "game-1",
      name: "Control",
      play_count: 4,
      total_playtime: 600,
    });
    expect(hydrateGame).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "game-1",
        exe_path: "C:\\Games\\Control\\Control.exe",
      }),
    );
  });

  it("checks primary executable and explicit exe-path bindings for path ownership", async () => {
    const get = vi.fn(async () => ({ count: 1 }));
    const service = createGamesService({
      db: createDb({ get }),
    });

    await expect(
      service.gameExistsByPath("C:/Games/Control/Control.exe"),
    ).resolves.toBe(true);

    expect(get).toHaveBeenCalledWith(
      expect.stringContaining("game_process_bindings"),
      ["c:\\games\\control\\control.exe"],
    );
  });

  it("adds single and batch games through the db boundary", async () => {
    const run = vi.fn(async () => ({ changes: 1 }));
    const get = vi.fn(async (sql: string) => {
      if (sql.includes("game_process_bindings")) {
        return undefined;
      }
      return gameRow({ name: "Control Ultimate" });
    });
    const service = createGamesService({
      db: createDb({ all: async () => [], get, run }),
      now: () => new Date("2026-04-24T10:00:00.000Z"),
      arkGameObjectSync: {
        syncGame: async (game) => `ark-${game.id}`,
      },
    });

    await expect(service.getGame("game-1")).resolves.toMatchObject({
      id: "game-1",
      name: "Control Ultimate",
    });
    await expect(
      service.addGame({
        name: "Control",
        exe_path: "C:\\Games\\Control\\Control.exe",
        exe_name: "Control.exe",
      }),
    ).resolves.toMatchObject({ ark_object_id: "ark-game-1" });
    await expect(
      service.addGamesBatch([
        {
          name: "Alan Wake",
          exe_path: "C:\\Games\\AlanWake\\AlanWake.exe",
          exe_name: "AlanWake.exe",
        },
      ]),
    ).resolves.toHaveLength(1);

    expect(run).toHaveBeenCalledWith(
      expect.stringContaining("INSERT INTO games"),
      expect.any(Array),
    );
  });

  it("lists, filters, and searches games through stable SQL query boundaries", async () => {
    const all = vi.fn(async (sql: string) => {
      if (sql.includes("game_process_bindings")) {
        return [];
      }
      return [
        gameRow({ id: "game-1", name: "Control" }),
        gameRow({ id: "game-2", name: "Alan Wake" }),
      ];
    });
    const service = createGamesService({
      db: createDb({ all }),
    });

    await expect(service.getAllGames()).resolves.toHaveLength(2);
    await expect(service.getFavorites()).resolves.toHaveLength(2);
    await expect(service.searchGames("control")).resolves.toHaveLength(2);

    expect(all).toHaveBeenCalledWith(
      expect.stringContaining("ORDER BY name ASC"),
      [],
    );
    expect(all).toHaveBeenCalledWith(
      expect.stringContaining("WHERE name LIKE ?1 OR exe_name LIKE ?1"),
      ["%control%"],
    );
  });

  it("updates, toggles favorites, and deletes a game through command SQL", async () => {
    const run = vi.fn(async () => ({ changes: 1 }));
    const service = createGamesService({
      db: createDb({
        get: async () => gameRow({ name: "Control Ultimate" }),
        all: async () => [],
        run,
      }),
    });

    await expect(service.updateGame({ id: "game-1", name: "Control 2" }))
      .resolves.toMatchObject({ name: "Control Ultimate" });
    await expect(service.toggleFavorite("game-1")).resolves.toMatchObject({
      id: "game-1",
    });
    await expect(service.deleteGame("game-1")).resolves.toBeUndefined();

    expect(run).toHaveBeenCalledWith(
      expect.stringContaining("DELETE FROM games"),
      ["game-1"],
    );
  });

  it("uses injected process operations for install, running, kill, and launch flows", async () => {
    const spawnGameProcess = vi.fn(async () => undefined);
    const countRunningInstances = vi.fn(async () => 2);
    const killMatchingProcesses = vi.fn(async () => 2);
    const db = createDb({
      get: async (sql: string) => {
        if (sql.includes("SELECT exe_path FROM games")) {
          return { exe_path: "C:\\Games\\Control\\Control.exe" };
        }
        return gameRow();
      },
      all: async (sql: string) =>
        sql.includes("game_process_bindings")
          ? [
              {
                id: 7,
                game_id: "game-1",
                match_type: "process_name",
                match_value: "ControlHelper.exe",
                normalized_value: "controlhelper.exe",
                created_at: "2026-04-24T10:00:00.000Z",
              },
            ]
          : [],
    });
    const service = createGamesService({
      db,
      fileExists: (filePath) => filePath.endsWith("Control.exe"),
      spawnGameProcess,
      countRunningInstances,
      killMatchingProcesses,
    });

    await expect(service.isGameInstalled("game-1")).resolves.toBe(true);
    await expect(service.getRunningInstances("game-1")).resolves.toBe(2);
    await expect(service.killGameProcesses("game-1")).resolves.toBe(2);
    await expect(service.launchGame("game-1")).resolves.toBeUndefined();

    expect(countRunningInstances).toHaveBeenCalledWith(
      expect.arrayContaining([
        { matchType: "exe_path", value: "C:\\Games\\Control\\Control.exe" },
        { matchType: "process_name", value: "ControlHelper.exe" },
      ]),
    );
    expect(spawnGameProcess).toHaveBeenCalledWith("C:\\Games\\Control\\Control.exe");
  });

  it("adds and removes process bindings before returning a refreshed game", async () => {
    const run = vi.fn(async () => ({ changes: 1 }));
    const db = createDb({
      get: async (sql: string) => {
        if (sql.includes("LOWER(REPLACE(exe_path")) {
          return { normalized_path: "c:\\games\\control\\control.exe" };
        }
        if (sql.includes("WHERE id = ?1") && sql.includes("AND game_id = ?2")) {
          return { id: 7 };
        }
        if (sql.includes("FROM games")) {
          return gameRow();
        }
        return undefined;
      },
      all: async () => [],
      run,
      transaction: async (fn) => await fn(createDb({ run })),
    });
    const service = createGamesService({ db });

    await expect(
      service.addProcessBindings("game-1", [
        { match_type: "process_name", match_value: " ControlHelper.exe " },
      ]),
    ).resolves.toMatchObject({ id: "game-1" });
    await expect(service.removeProcessBinding("game-1", 7)).resolves.toMatchObject({
      id: "game-1",
    });

    expect(run).toHaveBeenCalledWith(
      expect.stringContaining("DELETE FROM game_process_bindings"),
      [7, "game-1"],
    );
  });

  it("syncs all games to Ark and reports failed items without aborting the batch", async () => {
    const warn = vi.fn();
    const syncedIds: string[] = [];
    const service = createGamesService({
      db: createDb({
        all: async (sql: string) =>
          sql.includes("game_process_bindings")
            ? []
            : [
                gameRow({ id: "game-1", name: "Control" }),
                gameRow({ id: "game-2", name: "Broken" }),
              ],
        run: async () => ({ changes: 1 }),
      }),
      log: { error: vi.fn(), warn },
      arkGameObjectSync: {
        syncGame: async (game) => {
          if (game.id === "game-2") {
            throw new Error("Ark write failed");
          }
          syncedIds.push(game.id);
          return `ark-${game.id}`;
        },
      },
    });

    await expect(service.syncAllGamesToArk()).resolves.toEqual({
      total: 2,
      synced: 1,
      failed: 1,
    });
    expect(syncedIds).toEqual(["game-1"]);
    expect(warn).toHaveBeenCalledWith(
      "Failed to sync game game-2 to Ark",
      expect.any(Error),
    );
  });
});
