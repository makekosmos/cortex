import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";

import { execute, queryAll, queryOne, runInTransaction } from "../helpers/db";
import type { DbLike, DbValue } from "../helpers/shared";
import type { Game, NewGame, UpdateGame } from "./games/types";
import {
  countRunningInstances,
  killMatchingProcesses,
  resolveShortcutTarget,
  spawnGameProcess,
} from "./games/process";

const GAME_SELECT = `SELECT
  id, name, exe_path, exe_name, rawg_id, description, released,
  background_image, metacritic, rating, genres, platforms, developers, publishers,
  cover_image, icon_image, is_favorite, play_count, total_playtime, last_played, date_added,
  backup_enabled, last_backup, backup_count, save_path, user_rating, user_note, play_status
FROM games`;

const GAME_PATH_TOKEN = "{PATHTOGAME}";

export interface GamesServiceDeps {
  db: DbLike;
  usageReadModel?: {
    hydrateGame(game: Game): Promise<Game>;
    hydrateGames(games: Game[]): Promise<Game[]>;
  };
  now?: () => Date;
  fileExists?: (filePath: string) => boolean;
  log?: Pick<Console, "error" | "warn">;
  resolveShortcutTarget?: (inputPath: string) => Promise<string>;
  countRunningInstances?: (exePath: string) => Promise<number>;
  killMatchingProcesses?: (exePath: string) => Promise<number>;
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
}

type DbRow = Record<string, unknown>;

function readString(row: DbRow, key: string): string {
  const value = row[key];
  return typeof value === "string" ? value : "";
}

function readStringOrNull(row: DbRow, key: string): string | null {
  const value = row[key];
  return typeof value === "string" ? value : null;
}

function readNumberOrNull(row: DbRow, key: string): number | null {
  const value = row[key];
  if (value === null || value === undefined) {
    return null;
  }
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function readNumber(row: DbRow, key: string): number {
  const value = row[key];
  const number = Number(value ?? 0);
  return Number.isFinite(number) ? number : 0;
}

function readBoolean(row: DbRow, key: string): boolean {
  return Number(row[key] ?? 0) === 1;
}

function mapGameRow(row: DbRow): Game {
  return {
    id: readString(row, "id"),
    name: readString(row, "name"),
    exe_path: readString(row, "exe_path"),
    exe_name: readString(row, "exe_name"),
    play_status: readStringOrNull(row, "play_status") ?? "not_started",

    rawg_id: readNumberOrNull(row, "rawg_id"),
    description: readStringOrNull(row, "description"),
    released: readStringOrNull(row, "released"),
    background_image: readStringOrNull(row, "background_image"),
    metacritic: readNumberOrNull(row, "metacritic"),
    rating: readNumberOrNull(row, "rating"),
    genres: readStringOrNull(row, "genres"),
    platforms: readStringOrNull(row, "platforms"),
    developers: readStringOrNull(row, "developers"),
    publishers: readStringOrNull(row, "publishers"),

    cover_image: readStringOrNull(row, "cover_image"),
    icon_image: readStringOrNull(row, "icon_image"),
    is_favorite: readBoolean(row, "is_favorite"),
    play_count: readNumber(row, "play_count"),
    total_playtime: readNumber(row, "total_playtime"),
    last_played: readStringOrNull(row, "last_played"),
    date_added: readString(row, "date_added"),

    backup_enabled: readBoolean(row, "backup_enabled"),
    last_backup: readStringOrNull(row, "last_backup"),
    backup_count: readNumber(row, "backup_count"),
    save_path: readStringOrNull(row, "save_path"),

    user_rating: readNumberOrNull(row, "user_rating"),
    user_note: readStringOrNull(row, "user_note"),
  };
}

async function getRowById(db: DbLike, id: string): Promise<Game | null> {
  const row = await queryOne<DbRow>(db, `${GAME_SELECT} WHERE id = ?1`, [id]);
  return row ? mapGameRow(row) : null;
}

async function fetchExePath(db: DbLike, id: string): Promise<string> {
  const row = await queryOne<{ exe_path: string }>(db, "SELECT exe_path FROM games WHERE id = ?1", [
    id,
  ]);
  if (!row) {
    throw new Error("Game not found");
  }
  return row.exe_path;
}

async function normalizeSavePathIfPossible(
  db: DbLike,
  gameId: string,
  savePath: string,
): Promise<string> {
  if (savePath.includes(GAME_PATH_TOKEN)) {
    return savePath;
  }

  const absolutePath = path.isAbsolute(savePath) ? savePath : "";
  if (!absolutePath || !fs.existsSync(absolutePath)) {
    return savePath;
  }

  const row = await queryOne<{ exe_path: string }>(db, "SELECT exe_path FROM games WHERE id = ?1", [
    gameId,
  ]);
  if (!row) {
    return savePath;
  }

  const gameDir = path.dirname(row.exe_path);
  let canonicalGameDir: string;
  let canonicalSavePath: string;
  try {
    canonicalGameDir = fs.realpathSync.native(gameDir);
    canonicalSavePath = fs.realpathSync.native(absolutePath);
  } catch {
    return savePath;
  }

  const relative = path.relative(canonicalGameDir, canonicalSavePath);
  if (!relative || relative === "") {
    return GAME_PATH_TOKEN;
  }

  if (relative.startsWith("..")) {
    return savePath;
  }

  return path.join(GAME_PATH_TOKEN, relative);
}

async function prepareGameInsert(
  db: DbLike,
  game: NewGame,
  id: string,
  dateAdded: string,
): Promise<void> {
  await execute(
    db,
    "INSERT INTO games (id, name, exe_path, exe_name, date_added) VALUES (?1, ?2, ?3, ?4, ?5)",
    [id, game.name, game.exe_path, game.exe_name, dateAdded],
  );
}

async function buildUpdateClause(
  update: UpdateGame,
  db: DbLike,
): Promise<{ sql: string; values: DbValue[] }> {
  const updates: string[] = [];
  const values: DbValue[] = [];

  const push = (column: string, value: DbValue) => {
    updates.push(`${column} = ?`);
    values.push(value);
  };

  if (update.name !== undefined) {
    if (update.name === null || update.name.trim() === "") {
      throw new Error("name cannot be empty");
    }
    push("name", update.name);
  }

  if (update.exe_path !== undefined) {
    if (update.exe_path === null || update.exe_path.trim() === "") {
      throw new Error("exe_path cannot be empty");
    }
    const normalized = update.exe_path.trim();
    const exeName = path.basename(normalized);
    if (!exeName) {
      throw new Error("Invalid exe path");
    }
    push("exe_path", normalized);
    push("exe_name", exeName);
  }

  if (update.description !== undefined) {
    push("description", update.description);
  }
  if (update.cover_image !== undefined) {
    push("cover_image", update.cover_image);
  }
  if (update.icon_image !== undefined) {
    push("icon_image", update.icon_image);
  }
  if (update.is_favorite !== undefined) {
    push("is_favorite", update.is_favorite ? 1 : 0);
  }
  if (update.backup_enabled !== undefined) {
    push("backup_enabled", update.backup_enabled ? 1 : 0);
  }
  if (update.save_path !== undefined) {
    const normalized =
      update.save_path === null || update.save_path.trim() === ""
        ? null
        : await normalizeSavePathIfPossible(db, update.id, update.save_path);
    push("save_path", normalized);
    push("save_path_checked", normalized !== null ? 1 : 0);
  }
  if (update.rawg_id !== undefined) {
    push("rawg_id", update.rawg_id);
  }
  if (update.released !== undefined) {
    push("released", update.released);
  }
  if (update.background_image !== undefined) {
    push("background_image", update.background_image);
  }
  if (update.metacritic !== undefined) {
    push("metacritic", update.metacritic);
  }
  if (update.rating !== undefined) {
    push("rating", update.rating);
  }
  if (update.genres !== undefined) {
    push("genres", update.genres);
  }
  if (update.platforms !== undefined) {
    push("platforms", update.platforms);
  }
  if (update.developers !== undefined) {
    push("developers", update.developers);
  }
  if (update.publishers !== undefined) {
    push("publishers", update.publishers);
  }
  if (update.user_rating !== undefined) {
    push("user_rating", update.user_rating);
  }
  if (update.user_note !== undefined) {
    push("user_note", update.user_note);
  }
  if (update.play_status !== undefined) {
    push("play_status", update.play_status);
  }

  if (updates.length === 0) {
    return { sql: "", values: [] };
  }

  values.push(update.id);
  return {
    sql: `UPDATE games SET ${updates.join(", ")} WHERE id = ?`,
    values,
  };
}

function isUniqueConstraintError(error: unknown): boolean {
  return String(error).includes("UNIQUE constraint failed");
}

export function createGamesService(deps: GamesServiceDeps): GamesService {
  const usageReadModel = deps.usageReadModel;
  const now = deps.now ?? (() => new Date());
  const log = deps.log ?? console;
  const exists = deps.fileExists ?? fs.existsSync;
  const resolveShortcut = deps.resolveShortcutTarget ?? resolveShortcutTarget;
  const countInstances = deps.countRunningInstances ?? countRunningInstances;
  const killProcesses = deps.killMatchingProcesses ?? killMatchingProcesses;
  const spawnProcess = deps.spawnGameProcess ?? spawnGameProcess;

  const recordGameLaunch = async (id: string): Promise<Game> => {
    const launchedAt = now().toISOString();
    await execute(
      deps.db,
      "UPDATE games SET play_count = play_count + 1, last_played = ?1 WHERE id = ?2",
      [launchedAt, id],
    );

    const fetched = await getRowById(deps.db, id);
    if (!fetched) {
      throw new Error("Game not found");
    }
    return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
  };

  return {
    async getGame(id: string): Promise<Game | null> {
      const game = await getRowById(deps.db, id);
      if (!game) {
        return null;
      }
      return usageReadModel ? await usageReadModel.hydrateGame(game) : game;
    },

    async addGame(game: NewGame): Promise<Game> {
      const id = randomUUID();
      const dateAdded = now().toISOString();
      await prepareGameInsert(deps.db, game, id, dateAdded);
      const fetched = await getRowById(deps.db, id);
      if (!fetched) {
        throw new Error("Failed to fetch inserted game");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
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
        const fetched = await getRowById(deps.db, item.id);
        if (fetched) {
          result.push(usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched);
        } else {
          log.error?.(`Error fetching new game ${item.id}:`, item.name);
        }
      }

      return result;
    },

    async getAllGames(): Promise<Game[]> {
      const rows = await queryAll<DbRow>(deps.db, `${GAME_SELECT} ORDER BY name ASC`);
      const games = rows.map(mapGameRow);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },

    async getFavorites(): Promise<Game[]> {
      const rows = await queryAll<DbRow>(
        deps.db,
        `${GAME_SELECT} WHERE is_favorite = 1 ORDER BY name ASC`,
      );
      const games = rows.map(mapGameRow);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },

    async updateGame(update: UpdateGame): Promise<Game> {
      const { sql, values } = await buildUpdateClause(update, deps.db);
      if (sql) {
        await execute(deps.db, sql, values);
      }

      const fetched = await getRowById(deps.db, update.id);
      if (!fetched) {
        throw new Error("Game not found");
      }
      return usageReadModel ? await usageReadModel.hydrateGame(fetched) : fetched;
    },

    async toggleFavorite(id: string): Promise<Game> {
      await execute(
        deps.db,
        "UPDATE games SET is_favorite = CASE WHEN is_favorite = 1 THEN 0 ELSE 1 END WHERE id = ?1",
        [id],
      );

      const fetched = await getRowById(deps.db, id);
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
      const games = rows.map(mapGameRow);
      return usageReadModel ? await usageReadModel.hydrateGames(games) : games;
    },

    async gameExistsByPath(exePath: string): Promise<boolean> {
      const row = await queryOne<{ count: number }>(
        deps.db,
        "SELECT COUNT(*) AS count FROM games WHERE exe_path = ?1",
        [exePath],
      );
      return Number(row?.count ?? 0) > 0;
    },

    async resolveShortcutTarget(pathname: string): Promise<string> {
      return await resolveShortcut(pathname);
    },

    async isGameInstalled(id: string): Promise<boolean> {
      const exePath = await fetchExePath(deps.db, id);
      return exists(exePath);
    },

    async getRunningInstances(id: string): Promise<number> {
      const exePath = await fetchExePath(deps.db, id);
      return await countInstances(exePath);
    },

    async killGameProcesses(id: string): Promise<number> {
      const exePath = await fetchExePath(deps.db, id);
      return await killProcesses(exePath);
    },

    async launchGame(id: string): Promise<void> {
      const exePath = await fetchExePath(deps.db, id);
      await spawnProcess(exePath);
      await recordGameLaunch(id);
    },
  };
}
