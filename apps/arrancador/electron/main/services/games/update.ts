import fs from "node:fs";
import path from "node:path";

import { queryOne } from "../../helpers/db";
import type { DbLike, DbValue } from "../../helpers/shared";
import type { UpdateGame } from "./types";

const GAME_PATH_TOKEN = "{PATHTOGAME}";

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

  const row = await queryOne<{ exe_path: string }>(
    db,
    "SELECT exe_path FROM games WHERE id = ?1",
    [gameId],
  );
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

export async function buildUpdateClause(
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
    const normalizedLookup = normalized.replaceAll("/", "\\").toLowerCase();
    const bindingOwner = await queryOne<{ game_id: string }>(
      db,
      `SELECT game_id
       FROM game_process_bindings
       WHERE match_type = 'exe_path'
         AND normalized_value = ?1
         AND game_id != ?2
       LIMIT 1`,
      [normalizedLookup, update.id],
    );
    if (bindingOwner?.game_id) {
      throw new Error("exe_path is already attached to another game");
    }
    const exeName = path.basename(normalized);
    if (!exeName) {
      throw new Error("Invalid exe path");
    }
    push("exe_path", normalized);
    push("exe_name", exeName);
  }

  if (update.ark_object_id !== undefined) {
    push("ark_object_id", update.ark_object_id);
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
