import { execute, queryOne } from "../../helpers/db";
import type { DbLike } from "../../helpers/shared";
import type { NewGame } from "./types";

export async function fetchExePath(db: DbLike, id: string): Promise<string> {
  const row = await queryOne<{ exe_path: string }>(
    db,
    "SELECT exe_path FROM games WHERE id = ?1",
    [id],
  );
  if (!row) {
    throw new Error("Game not found");
  }
  return row.exe_path;
}

export async function prepareGameInsert(
  db: DbLike,
  game: NewGame,
  id: string,
  dateAdded: string,
): Promise<void> {
  const normalizedPath = game.exe_path.trim().replaceAll("/", "\\").toLowerCase();
  const bindingOwner = await queryOne<{ game_id: string }>(
    db,
    `SELECT game_id
     FROM game_process_bindings
     WHERE match_type = 'exe_path'
       AND normalized_value = ?1
     LIMIT 1`,
    [normalizedPath],
  );
  if (bindingOwner?.game_id) {
    throw new Error("exe_path is already attached to another game");
  }

  await execute(
    db,
    "INSERT INTO games (id, name, exe_path, exe_name, date_added) VALUES (?1, ?2, ?3, ?4, ?5)",
    [id, game.name, game.exe_path, game.exe_name, dateAdded],
  );
}

export function isUniqueConstraintError(error: unknown): boolean {
  return String(error).includes("UNIQUE constraint failed");
}
