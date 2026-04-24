import fs from "node:fs";
import path from "node:path";

import { openSqliteDatabase } from "../db";
import { queryAll, queryOne, runInTransaction } from "../helpers/db";
import type { DbLike } from "../helpers/shared";

const GAME_OBJECT_TYPE_ID = "game_obj";

type ArkObjectTypeRow = {
  id: string;
  name: string;
  schema_json: string;
  ui_schema_json: string;
  created_at: string;
  updated_at: string;
  system_locked: number;
};

type ArkObjectRow = {
  id: string;
  type_id: string;
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
  contentJson: string;
  propsJson: Record<string, unknown>;
  createdAt: string;
  updatedAt: string;
};

export interface ArkGameMigrationResult {
  migratedTypes: number;
  migratedObjects: number;
  mergedObjects: number;
  skippedObjects: number;
}

export interface MigrateArkGamesOptions {
  sourceDbPath: string;
  targetDbPath: string;
}

function normalizeExePath(exePath: string): string {
  return exePath.trim().replaceAll("/", "\\").toLowerCase();
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

function readOptionalString(value: unknown): string | null {
  if (typeof value !== "string") {
    return null;
  }

  const trimmed = value.trim();
  return trimmed.length > 0 ? trimmed : null;
}

function mapArkObjectRow(row: ArkObjectRow): ArkObjectRecord {
  return {
    id: row.id,
    title: row.title,
    contentJson: row.content_json,
    propsJson: parseJsonRecord(row.props_json),
    createdAt: row.created_at,
    updatedAt: row.updated_at,
  };
}

async function listActiveGameObjects(arkDb: DbLike): Promise<ArkObjectRecord[]> {
  const rows = await queryAll<ArkObjectRow>(
    arkDb,
    `SELECT id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at
     FROM objects
     WHERE type_id = ?1 AND deleted_at IS NULL
     ORDER BY updated_at DESC, created_at DESC`,
    [GAME_OBJECT_TYPE_ID],
  );

  return rows.map(mapArkObjectRow);
}

function buildGameIndexes(objects: readonly ArkObjectRecord[]) {
  const byId = new Map<string, ArkObjectRecord>();
  const byArrancadorGameId = new Map<string, ArkObjectRecord>();
  const byExePath = new Map<string, ArkObjectRecord>();

  for (const object of objects) {
    byId.set(object.id, object);

    const gameId = readOptionalString(object.propsJson.arrancador_game_id);
    if (gameId && !byArrancadorGameId.has(gameId)) {
      byArrancadorGameId.set(gameId, object);
    }

    const exePath = readOptionalString(object.propsJson.exe_path);
    if (exePath) {
      const normalized = normalizeExePath(exePath);
      if (!byExePath.has(normalized)) {
        byExePath.set(normalized, object);
      }
    }
  }

  return { byId, byArrancadorGameId, byExePath };
}

function mergeObjectProps(
  sourceProps: Record<string, unknown>,
  targetProps: Record<string, unknown>,
): Record<string, unknown> {
  return {
    ...targetProps,
    ...sourceProps,
  };
}

function arePropsEqual(
  left: Record<string, unknown>,
  right: Record<string, unknown>,
): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

async function ensureGameObjectType(sourceDb: DbLike, targetDb: DbLike): Promise<number> {
  const targetType = await queryOne<{ id: string }>(
    targetDb,
    "SELECT id FROM object_types WHERE id = ?1",
    [GAME_OBJECT_TYPE_ID],
  );
  if (targetType) {
    return 0;
  }

  const sourceType = await queryOne<ArkObjectTypeRow>(
    sourceDb,
    `SELECT id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked
     FROM object_types
     WHERE id = ?1`,
    [GAME_OBJECT_TYPE_ID],
  );

  if (!sourceType) {
    return 0;
  }

  await targetDb.run(
    `INSERT INTO object_types
       (id, name, schema_json, ui_schema_json, created_at, updated_at, system_locked)
     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)`,
    [
      sourceType.id,
      sourceType.name,
      sourceType.schema_json,
      sourceType.ui_schema_json,
      sourceType.created_at,
      sourceType.updated_at,
      sourceType.system_locked,
    ],
  );

  return 1;
}

function openArkDb(filePath: string): DbLike {
  if (!filePath.trim()) {
    throw new Error("Ark DB path is empty");
  }

  if (!fs.existsSync(filePath)) {
    throw new Error(`Ark DB does not exist: ${filePath}`);
  }

  return openSqliteDatabase(filePath, {
    fileMustExist: true,
    timeoutMs: 2000,
  });
}

export async function migrateArkGames(
  options: MigrateArkGamesOptions,
): Promise<ArkGameMigrationResult> {
  if (path.resolve(options.sourceDbPath) === path.resolve(options.targetDbPath)) {
    throw new Error("Source and target Ark DB must be different");
  }

  const sourceDb = openArkDb(options.sourceDbPath);
  const targetDb = openArkDb(options.targetDbPath);

  try {
    return await runInTransaction(targetDb, async (tx) => {
      const result: ArkGameMigrationResult = {
        migratedTypes: await ensureGameObjectType(sourceDb, tx),
        migratedObjects: 0,
        mergedObjects: 0,
        skippedObjects: 0,
      };

      const sourceObjects = await listActiveGameObjects(sourceDb);
      const targetObjects = await listActiveGameObjects(tx);
      const targetIndexes = buildGameIndexes(targetObjects);

      for (const sourceObject of sourceObjects) {
        const sourceArrancadorGameId = readOptionalString(sourceObject.propsJson.arrancador_game_id);
        const sourceExePath = readOptionalString(sourceObject.propsJson.exe_path);
        const targetMatch =
          targetIndexes.byId.get(sourceObject.id) ??
          (sourceArrancadorGameId
            ? targetIndexes.byArrancadorGameId.get(sourceArrancadorGameId)
            : undefined) ??
          (sourceExePath
            ? targetIndexes.byExePath.get(normalizeExePath(sourceExePath))
            : undefined) ??
          null;

        const objectId = targetMatch?.id ?? sourceObject.id;
        const mergedProps = mergeObjectProps(sourceObject.propsJson, targetMatch?.propsJson ?? {});
        const createdAt =
          targetMatch && targetMatch.createdAt < sourceObject.createdAt
            ? targetMatch.createdAt
            : sourceObject.createdAt;
        const updatedAt =
          targetMatch && targetMatch.updatedAt > sourceObject.updatedAt
            ? targetMatch.updatedAt
            : sourceObject.updatedAt;

        if (
          targetMatch &&
          targetMatch.title === sourceObject.title &&
          targetMatch.contentJson === sourceObject.contentJson &&
          arePropsEqual(targetMatch.propsJson, mergedProps)
        ) {
          result.skippedObjects += 1;
          continue;
        }

        await tx.run(
          `INSERT OR REPLACE INTO objects
             (id, type_id, title, content_json, props_json, created_at, updated_at, deleted_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)`,
          [
            objectId,
            GAME_OBJECT_TYPE_ID,
            sourceObject.title,
            sourceObject.contentJson,
            JSON.stringify(mergedProps),
            createdAt,
            updatedAt,
          ],
        );

        const nextRecord: ArkObjectRecord = {
          id: objectId,
          title: sourceObject.title,
          contentJson: sourceObject.contentJson,
          propsJson: mergedProps,
          createdAt,
          updatedAt,
        };
        targetIndexes.byId.set(nextRecord.id, nextRecord);

        const nextArrancadorGameId = readOptionalString(nextRecord.propsJson.arrancador_game_id);
        if (nextArrancadorGameId) {
          targetIndexes.byArrancadorGameId.set(nextArrancadorGameId, nextRecord);
        }

        const nextExePath = readOptionalString(nextRecord.propsJson.exe_path);
        if (nextExePath) {
          targetIndexes.byExePath.set(normalizeExePath(nextExePath), nextRecord);
        }

        if (targetMatch) {
          result.mergedObjects += 1;
        } else {
          result.migratedObjects += 1;
        }
      }

      return result;
    });
  } finally {
    await Promise.resolve(sourceDb.close?.());
    await Promise.resolve(targetDb.close?.());
  }
}
