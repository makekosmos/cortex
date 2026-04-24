import { randomUUID } from "node:crypto";
import fs from "node:fs";

import { execute, queryAll, queryOne, runInTransaction } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import {
  addGameProcessBindings,
  listGameProcessBindingsByGameId,
  removeGameProcessBinding,
} from "./game-process-bindings";
import {
  fetchExePath,
  isUniqueConstraintError,
  prepareGameInsert,
} from "./games/persistence";
import {
  countRunningInstancesByMatches,
  killMatchingProcessesByMatches,
  type ProcessMatch,
  resolveShortcutTarget,
  spawnGameProcess,
} from "./games/process";
import { GAME_SELECT, type GameDbRow, mapGameRow } from "./games/rows";
import type {
  Game,
  NewGame,
  NewGameProcessBinding,
  UpdateGame,
} from "./games/types";
import { buildUpdateClause } from "./games/update";

export interface GamesServiceDeps {
  db: DbLike;
  usageReadModel?: {
    hydrateGame(game: Game): Promise<Game>;
    hydrateGames(games: Game[]): Promise<Game[]>;
  };
  arkGameObjectSync?: {
    syncGame(game: Game): Promise<string | null>;
  };
  now?: () => Date;
  fileExists?: (filePath: string) => boolean;
  log?: Pick<Console, "error" | "warn">;
  resolveShortcutTarget?: (inputPath: string) => Promise<string>;
  countRunningInstances?: (matches: ProcessMatch[]) => Promise<number>;
  killMatchingProcesses?: (matches: ProcessMatch[]) => Promise<number>;
  spawnGameProcess?: (exePath: string) => Promise<void>;
}

export interface GamesService {
  getGame(id: string): Promise<Game | null>;
  addGame(game: NewGame): Promise<Game>;
  addGamesBatch(games: NewGame[]): Promise<Game[]>;
  getAllGames(): Promise<Game[]>;
  getFavorites(): Promise<Game[]>;
  updateGame(update: UpdateGame): Promise<Game>;
  toggleFavorite(id: string): Promise<Game>;
  deleteGame(id: string): Promise<void>;
  recordGameLaunch(id: string): Promise<Game>;
  searchGames(query: string): Promise<Game[]>;
  gameExistsByPath(exePath: string): Promise<boolean>;
  resolveShortcutTarget(path: string): Promise<string>;
  isGameInstalled(id: string): Promise<boolean>;
  getRunningInstances(id: string): Promise<number>;
  killGameProcesses(id: string): Promise<number>;
  launchGame(id: string): Promise<void>;
  addProcessBindings(id: string, bindings: NewGameProcessBinding[]): Promise<Game>;
  removeProcessBinding(id: string, bindingId: number): Promise<Game>;
  syncAllGamesToArk(): Promise<GamesArkSyncResult>;
}

export interface GamesArkSyncResult {
  total: number;
  synced: number;
  failed: number;
}

async function getRowById(db: DbLike, id: string): Promise<Game | null> {
  const row = await queryOne<GameDbRow>(db, `${GAME_SELECT} WHERE id = ?1`, [id]);
  return row ? mapGameRow(row) : null;
}

async function hydrateExplicitBindings(db: DbLike, games: Game[]): Promise<Game[]> {
  if (games.length === 0) {
    return games;
  }

  const bindingsByGameId = await listGameProcessBindingsByGameId(
    db,
    games.map((game) => game.id),
  );

  return games.map((game) => ({
    ...game,
    process_bindings: bindingsByGameId.get(game.id) ?? [],
  }));
}

export function createGamesService(deps: GamesServiceDeps): GamesService {
  const usageReadModel = deps.usageReadModel;
  const arkGameObjectSync = deps.arkGameObjectSync;
  const now = deps.now ?? (() => new Date());
  const log = deps.log ?? console;
  const exists = deps.fileExists ?? fs.existsSync;
  const resolveShortcut = deps.resolveShortcutTarget ?? resolveShortcutTarget;
  const countInstances = deps.countRunningInstances ?? countRunningInstancesByMatches;
  const killProcesses = deps.killMatchingProcesses ?? killMatchingProcessesByMatches;
  const spawnProcess = deps.spawnGameProcess ?? spawnGameProcess;

  const loadGame = async (id: string) => {
    const fetched = await getRowById(deps.db, id);
    if (!fetched) {
      return null;
    }
    const [withBindings] = await hydrateExplicitBindings(deps.db, [fetched]);
    return withBindings ?? null;
  };

  const loadGames = async (rows: DbRow[]) =>
    await hydrateExplicitBindings(deps.db, rows.map(mapGameRow));

  const getProcessMatchesForGame = async (id: string): Promise<ProcessMatch[]> => {
    const game = await loadGame(id);
    if (!game) {
      throw new Error("Game not found");
    }

    const matches: ProcessMatch[] = [
      { matchType: "exe_path", value: game.exe_path },
      ...game.process_bindings.map((binding) => ({
        matchType: binding.match_type,
        value: binding.match_value,
      })),
    ];

    const deduped = new Map<string, ProcessMatch>();
    for (const match of matches) {
      deduped.set(`${match.matchType}:${match.value.trim().toLowerCase()}`, match);
    }
    return [...deduped.values()];
  };

  const persistArkObjectId = async (gameId: string, arkObjectId: string): Promise<void> => {
    await execute(
      deps.db,
      "UPDATE games SET ark_object_id = ?1 WHERE id = ?2",
      [arkObjectId, gameId],
    );
  };

  const syncArkGame = async (game: Game): Promise<Game> => {
    if (!arkGameObjectSync) {
      return game;
    }

    const arkObjectId = await arkGameObjectSync.syncGame(game);
    if (!arkObjectId) {
      throw new Error(`Failed to sync game ${game.id} to Ark`);
    }

    if (arkObjectId !== game.ark_object_id) {
      await persistArkObjectId(game.id, arkObjectId);
    }

    return {
      ...game,
      ark_object_id: arkObjectId,
    };
  };

  const syncArkGameIfPossible = async (game: Game): Promise<Game> => {
    try {
      return await syncArkGame(game);
    } catch (error) {
      log.warn?.(`Failed to sync game ${game.id} to Ark`, error);
      return game;
    }
  };

  const recordGameLaunch = async (id: string): Promise<Game> => {
    const fetched = await getRowById(deps.db, id);
    if (!fetched) {
      throw new Error("Game not found");
    }
    return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
  };

  return {
    async getGame(id: string): Promise<Game | null> {
      const game = await loadGame(id);
      if (!game) {
        return null;
      }
      return usageReadModel ? await usageReadModel.hydrateGame(game) : game;
    },

    async addGame(game: NewGame): Promise<Game> {
      const id = randomUUID();
      const dateAdded = now().toISOString();
      await prepareGameInsert(deps.db, game, id, dateAdded);
      const fetched = await loadGame(id);
      if (!fetched) {
        throw new Error("Failed to fetch inserted game");
      }
      const hydrated = usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
      return await syncArkGameIfPossible(hydrated);
    },

    async addGamesBatch(games: NewGame[]): Promise<Game[]> {
      if (games.length === 0) {
        return [];
      }

      const inserted: Array<{ id: string; name: string }> = [];
      await runInTransaction(deps.db, async (tx) => {
        for (const game of games) {
          const id = randomUUID();
          const dateAdded = now().toISOString();
          try {
            await prepareGameInsert(tx, game, id, dateAdded);
            inserted.push({ id, name: game.name });
          } catch (error) {
            if (!isUniqueConstraintError(error)) {
              log.error?.(`Error adding game ${game.name}:`, error);
            }
          }
        }
      });

      const result: Game[] = [];
      for (const item of inserted) {
        const fetched = await loadGame(item.id);
        if (fetched) {
          const hydrated = usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
          result.push(await syncArkGameIfPossible(hydrated));
        } else {
          log.error?.(`Error fetching new game ${item.id}:`, item.name);
        }
      }

      return result;
    },

    async getAllGames(): Promise<Game[]> {
      const rows = await queryAll<DbRow>(deps.db, `${GAME_SELECT} ORDER BY name ASC`);
      const games = await loadGames(rows);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },

    async getFavorites(): Promise<Game[]> {
      const rows = await queryAll<DbRow>(
        deps.db,
        `${GAME_SELECT} WHERE is_favorite = 1 ORDER BY name ASC`,
      );
      const games = await loadGames(rows);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },

    async updateGame(update: UpdateGame): Promise<Game> {
      const { sql, values } = await buildUpdateClause(update, deps.db);
      if (sql) {
        await execute(deps.db, sql, values);
      }

      const fetched = await loadGame(update.id);
      if (!fetched) {
        throw new Error("Game not found");
      }
      const hydrated = usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
      return await syncArkGameIfPossible(hydrated);
    },

    async toggleFavorite(id: string): Promise<Game> {
      await execute(
        deps.db,
        "UPDATE games SET is_favorite = CASE WHEN is_favorite = 1 THEN 0 ELSE 1 END WHERE id = ?1",
        [id],
      );

      const fetched = await loadGame(id);
      if (!fetched) {
        throw new Error("Game not found");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
    },

    async deleteGame(id: string): Promise<void> {
      await execute(deps.db, "DELETE FROM games WHERE id = ?1", [id]);
    },

    async recordGameLaunch(id: string): Promise<Game> {
      return await recordGameLaunch(id);
    },

    async searchGames(query: string): Promise<Game[]> {
      const pattern = `%${query}%`;
      const rows = await queryAll<DbRow>(
        deps.db,
        `${GAME_SELECT} WHERE name LIKE ?1 OR exe_name LIKE ?1 ORDER BY name ASC`,
        [pattern],
      );
      const games = await loadGames(rows);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },

    async gameExistsByPath(exePath: string): Promise<boolean> {
      const normalizedPath = exePath.trim().replaceAll("/", "\\").toLowerCase();
      const row = await queryOne<{ count: number }>(
        deps.db,
        `SELECT (
            SELECT COUNT(*)
            FROM games
            WHERE LOWER(REPLACE(exe_path, '/', '\\')) = ?1
          ) + (
            SELECT COUNT(*)
            FROM game_process_bindings
            WHERE match_type = 'exe_path'
              AND normalized_value = ?1
          ) AS count`,
        [normalizedPath],
      );
      return Number(row?.count ?? 0) > 0;
    },

    async resolveShortcutTarget(pathname: string): Promise<string> {
      return await resolveShortcut(pathname);
    },

    async isGameInstalled(id: string): Promise<boolean> {
      const matches = await getProcessMatchesForGame(id);
      return matches.some(
        (match) => match.matchType === "exe_path" && exists(match.value),
      );
    },

    async getRunningInstances(id: string): Promise<number> {
      return await countInstances(await getProcessMatchesForGame(id));
    },

    async killGameProcesses(id: string): Promise<number> {
      return await killProcesses(await getProcessMatchesForGame(id));
    },

    async launchGame(id: string): Promise<void> {
      const exePath = await fetchExePath(deps.db, id);
      await spawnProcess(exePath);
      await recordGameLaunch(id);
    },

    async addProcessBindings(id: string, bindings: NewGameProcessBinding[]): Promise<Game> {
      await addGameProcessBindings(deps.db, id, bindings, now);
      const fetched = await loadGame(id);
      if (!fetched) {
        throw new Error("Game not found");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
    },

    async removeProcessBinding(id: string, bindingId: number): Promise<Game> {
      await removeGameProcessBinding(deps.db, id, bindingId);
      const fetched = await loadGame(id);
      if (!fetched) {
        throw new Error("Game not found");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
    },

    async syncAllGamesToArk(): Promise<GamesArkSyncResult> {
      const rows = await queryAll<DbRow>(deps.db, `${GAME_SELECT} ORDER BY date_added ASC, name ASC`);
      const games = await loadGames(rows);
      const hydratedGames = usageReadModel ? await usageReadModel.hydrateGames(games) : games;
      const result: GamesArkSyncResult = {
        total: hydratedGames.length,
        synced: 0,
        failed: 0,
      };

      if (!arkGameObjectSync || hydratedGames.length === 0) {
        return result;
      }

      for (const game of hydratedGames) {
        try {
          await syncArkGame(game);
          result.synced += 1;
        } catch (error) {
          result.failed += 1;
          log.warn?.(`Failed to sync game ${game.id} to Ark`, error);
        }
      }

      return result;
    },
  };
}
