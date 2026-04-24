import type { Game } from "./types";

export const GAME_SELECT = `SELECT
  id, ark_object_id, name, exe_path, exe_name, rawg_id, description, released,
  background_image, metacritic, rating, genres, platforms, developers, publishers,
  cover_image, icon_image, is_favorite, play_count, total_playtime, last_played, date_added,
  backup_enabled, last_backup, backup_count, save_path, user_rating, user_note, play_status
FROM games`;

export type GameDbRow = Record<string, unknown>;

function readString(row: GameDbRow, key: string): string {
  const value = row[key];
  return typeof value === "string" ? value : "";
}

function readStringOrNull(row: GameDbRow, key: string): string | null {
  const value = row[key];
  return typeof value === "string" ? value : null;
}

function readNumberOrNull(row: GameDbRow, key: string): number | null {
  const value = row[key];
  if (value === null || value === undefined) {
    return null;
  }
  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function readNumber(row: GameDbRow, key: string): number {
  const value = row[key];
  const number = Number(value ?? 0);
  return Number.isFinite(number) ? number : 0;
}

function readBoolean(row: GameDbRow, key: string): boolean {
  return Number(row[key] ?? 0) === 1;
}

export function mapGameRow(row: GameDbRow): Game {
  return {
    id: readString(row, "id"),
    ark_object_id: readStringOrNull(row, "ark_object_id"),
    name: readString(row, "name"),
    exe_path: readString(row, "exe_path"),
    exe_name: readString(row, "exe_name"),
    process_bindings: [],
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
