import { randomUUID } from "node:crypto";
import fs from "node:fs";

import { openSqliteDatabase } from "../db";
import { queryAll, queryOne } from "../helpers/db";
import type { DbLike } from "../helpers/shared";
import type { Game } from "./games/types";

const GAME_OBJECT_TYPE_ID = "game_obj";
const DEFAULT_CONTENT_JSON = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

type ArkObjectRow = {
  id: string;
  title: string;
  content_json: string;
  props_json: string;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
};

type ArkObjectRecord = {
  id: string;
  title: string;
  contentJson: unknown;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
};

export interface ArkGameObjectService {
  hydrateGames(games: Game[]): Promise<Game[]>;
  syncGame(game: Game): Promise<string | null>;
}

export interface ArkGameObjectServiceOptions {
  arkDbPath: string;
  now?: () => Date;
}

function normalizeExePath(exePath: string): string {
  return exePath.trim().replaceAll("/", "\\").toLowerCase();
}

function buildParameterizedList(values: readonly string[], startIndex = 1): string {
  return values.map((_, index) => `?${index + startIndex}`).join(", ");
}

function tryOpenArkDb(
  arkDbPath: string,
  options: { readonly?: boolean } = {},
): DbLike | null {
  if (!arkDbPath || !fs.existsSync(arkDbPath)) {
    return null;
  }

  try {
    return openSqliteDatabase(arkDbPath, {
      readonly: options.readonly ?? false,
      fileMustExist: true,
      timeoutMs: 2000,
    });
  } catch {
    return null;
  }
}

function parseJsonRecord(value: string | null | undefined): Record<string, unknown> {
  if (!value) {
    return {};
  }

  try {
    const parsed = JSON.parse(value) as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, unknown>)
      : {};
  } catch {
    return {};
  }
}

function parseJsonValue(value: string | null | undefined): unknown {
  if (!value) {
    return DEFAULT_CONTENT_JSON;
  }

  try {
    return JSON.parse(value) as unknown;
  } catch {
    return DEFAULT_CONTENT_JSON;
  }
}

function readOptionalString(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function readOptionalNumber(value: unknown): number | null {
  if (value === null || value === undefined || value === "") {
    return null;
  }

  const number = Number(value);
  return Number.isFinite(number) ? number : null;
}

function readOptionalBoolean(value: unknown): boolean | null {
  if (typeof value === "boolean") {
    return value;
  }

  if (value === 1 || value === "1" || value === "true") {
    return true;
  }

  if (value === 0 || value === "0" || value === "false") {
    return false;
  }

  return null;
}

function readPlayStatus(value: unknown): Game["play_status"] | null {
  return value === "not_started" ||
    value === "in_progress" ||
    value === "completed" ||
    value === "abandoned"
    ? value
    : null;
}

function mapArkObjectRow(row: ArkObjectRow): ArkObjectRecord {
  return {
    id: row.id,
    title: row.title,
    contentJson: parseJsonValue(row.content_json),
    propsJson: parseJsonRecord(row.props_json),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

async function listActiveGameObjects(arkDb: DbLike): Promise<ArkObjectRecord[]> {
  const rows = await queryAll<ArkObjectRow>(
    arkDb,
    `SELECT id, title, content_json, props_json, created_at, updated_at, deleted_at
     FROM objects
     WHERE type_id = ?1 AND deleted_at IS NULL
     ORDER BY updated_at DESC, created_at DESC`,
    [GAME_OBJECT_TYPE_ID],
  );

  return rows.map(mapArkObjectRow);
}

async function listObjectsById(
  arkDb: DbLike,
  ids: readonly string[],
): Promise<Map<string, ArkObjectRecord>> {
  if (ids.length === 0) {
    return new Map();
  }

  const placeholders = buildParameterizedList(ids);
  const rows = await queryAll<ArkObjectRow>(
    arkDb,
    `SELECT id, title, content_json, props_json, created_at, updated_at, deleted_at
     FROM objects
     WHERE type_id = ?1 AND deleted_at IS NULL AND id IN (${placeholders})
     ORDER BY updated_at DESC, created_at DESC`,
    [GAME_OBJECT_TYPE_ID, ...ids],
  );

  return new Map(rows.map((row) => [row.id, mapArkObjectRow(row)]));
}

function buildObjectIndexes(objects: readonly ArkObjectRecord[]) {
  const byGameId = new Map<string, ArkObjectRecord>();
  const byExePath = new Map<string, ArkObjectRecord>();

  for (const object of objects) {
    const gameId = readOptionalString(object.propsJson.arrancador_game_id);
    if (gameId && !byGameId.has(gameId)) {
      byGameId.set(gameId, object);
    }

    const exePath = readOptionalString(object.propsJson.exe_path);
    if (exePath) {
      const normalized = normalizeExePath(exePath);
      if (!byExePath.has(normalized)) {
        byExePath.set(normalized, object);
      }
    }
  }

  return { byGameId, byExePath };
}

function resolveLinkedObject(
  game: Game,
  directById: ReadonlyMap<string, ArkObjectRecord>,
  fallback?: {
    byGameId: ReadonlyMap<string, ArkObjectRecord>;
    byExePath: ReadonlyMap<string, ArkObjectRecord>;
  },
): ArkObjectRecord | null {
  if (game.ark_object_id) {
    const direct = directById.get(game.ark_object_id);
    if (direct) {
      return direct;
    }
  }

  if (!fallback) {
    return null;
  }

  return (
    fallback.byGameId.get(game.id) ??
    fallback.byExePath.get(normalizeExePath(game.exe_path)) ??
    null
  );
}

function mergeArkGameProps(game: Game, object: ArkObjectRecord): Game {
  const props = object.propsJson;
  const nextName = readOptionalString(object.title) ?? game.name;
  const nextDescription = readOptionalString(props.description) ?? game.description;
  const nextUserRating = readOptionalNumber(props.user_rating) ?? game.user_rating;
  const nextPlayStatus = readPlayStatus(props.play_status) ?? game.play_status;
  const nextGenres = readOptionalString(props.genres) ?? game.genres;
  const nextCoverImage = readOptionalString(props.cover_image) ?? game.cover_image;
  const nextBackgroundImage =
    readOptionalString(props.background_image) ?? game.background_image;
  const nextRawgId = game.rawg_id ?? readOptionalNumber(props.rawg_id);

  const remoteSavePath = readOptionalString(props.save_path);
  const nextSavePath = game.save_path ?? remoteSavePath;

  const remotePlayCount = readOptionalNumber(props.play_count);
  const nextPlayCount =
    remotePlayCount !== null ? Math.max(game.play_count, remotePlayCount) : game.play_count;

  const remoteLastPlayed = readOptionalString(props.last_played_at);
  const nextLastPlayed = game.last_played ?? remoteLastPlayed;

  const remoteTotalPlaytime = readOptionalNumber(props.total_playtime_seconds);
  const nextTotalPlaytime =
    remoteTotalPlaytime !== null
      ? Math.max(game.total_playtime, remoteTotalPlaytime)
      : game.total_playtime;

  const remoteSaveExists = readOptionalBoolean(props.save_exists);
  const savePathFromExists =
    game.save_path ?? (remoteSaveExists === false ? null : nextSavePath);

  return {
    ...game,
    ark_object_id: object.id,
    name: nextName,
    description: nextDescription,
    user_rating: nextUserRating,
    play_status: nextPlayStatus,
    genres: nextGenres,
    cover_image: nextCoverImage,
    background_image: nextBackgroundImage,
    rawg_id: nextRawgId,
    save_path: savePathFromExists,
    play_count: nextPlayCount,
    last_played: nextLastPlayed,
    total_playtime: nextTotalPlaytime,
  };
}

function buildGameObjectProps(
  game: Game,
  existingProps: Record<string, unknown>,
): Record<string, unknown> {
  return {
    ...existingProps,
    sync_source: "arrancador",
    arrancador_game_id: game.id,
    description: game.description,
    user_rating: game.user_rating,
    play_status: game.play_status,
    genres: game.genres,
    cover_image: game.cover_image,
    background_image: game.background_image,
    exe_path: game.exe_path,
    save_path: game.save_path,
    total_playtime_seconds: game.total_playtime,
    last_played_at: game.last_played,
    play_count: game.play_count,
    save_exists: Boolean(game.save_path),
    rawg_id: game.rawg_id === null ? null : String(game.rawg_id),
    exe_name: game.exe_name,
  };
}

async function loadExistingObjectForSync(
  arkDb: DbLike,
  game: Game,
): Promise<ArkObjectRecord | null> {
  if (game.ark_object_id) {
    const row = await queryOne<ArkObjectRow>(
      arkDb,
      `SELECT id, title, content_json, props_json, created_at, updated_at, deleted_at
       FROM objects
       WHERE id = ?1 AND type_id = ?2`,
      [game.ark_object_id, GAME_OBJECT_TYPE_ID],
    );
    if (row) {
      return mapArkObjectRow(row);
    }
  }

  const objects = await listActiveGameObjects(arkDb);
  const { byGameId, byExePath } = buildObjectIndexes(objects);
  return (
    byGameId.get(game.id) ??
    byExePath.get(normalizeExePath(game.exe_path)) ??
    null
  );
}

export function createArkGameObjectService(
  options: ArkGameObjectServiceOptions,
): ArkGameObjectService {
  const now = options.now ?? (() => new Date());

  return {
    async hydrateGames(games: Game[]): Promise<Game[]> {
      if (games.length === 0) {
        return games;
      }

      const arkDb = tryOpenArkDb(options.arkDbPath, { readonly: true });
      if (!arkDb) {
        return games;
      }

      try {
        const linkedIds = games
          .map((game) => game.ark_object_id)
          .filter((value): value is string => typeof value === "string" && value.length > 0);
        const directById = await listObjectsById(arkDb, linkedIds);
        const needsFallback = games.some(
          (game) => !game.ark_object_id || !directById.has(game.ark_object_id),
        );
        const fallback = needsFallback ? buildObjectIndexes(await listActiveGameObjects(arkDb)) : null;

        return games.map((game) => {
          const object = resolveLinkedObject(game, directById, fallback ?? undefined);
          return object ? mergeArkGameProps(game, object) : game;
        });
      } catch {
        return games;
      }
    },

    async syncGame(game: Game): Promise<string | null> {
      const arkDb = tryOpenArkDb(options.arkDbPath);
      if (!arkDb) {
        return null;
      }

      try {
        const existing = await loadExistingObjectForSync(arkDb, game);
        const objectId = existing?.id ?? game.ark_object_id ?? randomUUID();
        const timestamp = now().toISOString();
        const createdAt = existing?.createdAt ?? timestamp;
        const contentJson = existing?.contentJson ?? DEFAULT_CONTENT_JSON;
        const propsJson = buildGameObjectProps(game, existing?.propsJson ?? {});

        await arkDb.run(
          `INSERT OR REPLACE INTO objects
             (id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)`,
          [
            objectId,
            GAME_OBJECT_TYPE_ID,
            game.name,
            JSON.stringify(contentJson),
            JSON.stringify(propsJson),
            createdAt,
            timestamp,
          ],
        );

        return objectId;
      } catch {
        return null;
      }
    },
  };
}
