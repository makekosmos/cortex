import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";

import {
  ArkClient,
  ensureKeplerRunning,
  type ArkObjectRecord,
  type ArkObjectsApi,
  type ArkObjectTypesApi,
  type JsonValue,
} from "@kosmos/ark";
import electron from "electron";

/** Kepler optional by default. KOSMOS_REQUIRE_KEPLER=1 для строгого режима. */
function isKeplerRequired(): boolean {
  return process.env.KOSMOS_REQUIRE_KEPLER === "1";
}

function isKeplerOptional(): boolean {
  return !isKeplerRequired();
}

import type { Game } from "./games/types";

const GAME_OBJECT_TYPE_ID = "game_obj";
const { app } = electron;
const DEFAULT_CONTENT_JSON = {
  type: "doc",
  content: [{ type: "paragraph" }],
};

type GameArkObjectRecord = Omit<ArkObjectRecord, "propsJson"> & {
  propsJson: Record<string, unknown>;
};

export interface ArkGameObjectService {
  hydrateGames(games: Game[]): Promise<Game[]>;
  syncGame(game: Game): Promise<string | null>;
}

export interface ArkGameObjectServiceOptions {
  arkDbPath: string;
  now?: () => Date;
  arkObjects?: Pick<ArkObjectsApi, "list" | "listByType" | "get" | "getMany" | "upsert">;
  arkObjectTypes?: Pick<ArkObjectTypesApi, "get" | "upsert">;
  arkCoreRpcPath?: string;
  requestTimeoutMs?: number;
  spaceId?: string;
  deviceId?: string;
  deviceName?: string;
  appRoot?: string;
  isPackaged?: boolean;
  resourcesPath?: string;
  throwOnError?: boolean;
}

function normalizeExePath(exePath: string): string {
  return exePath.trim().replaceAll("/", "\\").toLowerCase();
}

function getArkCoreRpcBinaryName() {
  return process.platform === "win32" ? "ark-core-rpc.exe" : "ark-core-rpc";
}

export function getArkCoreRpcBinaryPath(
  options: Pick<
    ArkGameObjectServiceOptions,
    "appRoot" | "isPackaged" | "resourcesPath"
  > = {},
) {
  const binaryName = getArkCoreRpcBinaryName();
  const electronApp = app as typeof app | undefined;

  if (options.isPackaged ?? electronApp?.isPackaged ?? false) {
    return path.join(
      options.resourcesPath ?? process.resourcesPath,
      "ark-core",
      binaryName,
    );
  }

  const appRoot = path.resolve(options.appRoot ?? process.env.APP_ROOT ?? process.cwd());
  const repoRoot = path.basename(appRoot) === "arrancador"
    ? path.resolve(appRoot, "..", "..")
    : appRoot;
  const releasePath = path.join(
    repoRoot,
    "packages",
    "ark-core",
    "rust",
    "target",
    "release",
    binaryName,
  );
  if (fs.existsSync(releasePath)) {
    return releasePath;
  }

  return path.join(
    repoRoot,
    "packages",
    "ark-core",
    "rust",
    "target",
    "debug",
    binaryName,
  );
}

function isJsonRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value && typeof value === "object" && !Array.isArray(value));
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

async function listActiveGameObjectsFromSdk(
  arkObjects: Pick<ArkObjectsApi, "listByType">,
): Promise<GameArkObjectRecord[]> {
  const objects = await arkObjects.listByType(GAME_OBJECT_TYPE_ID);
  return objects
    .filter((object) => object.deletedAt === null)
    .map((object) => ({
      id: object.id,
      typeId: object.typeId,
      title: object.title,
      contentJson: object.contentJson,
      propsJson: isJsonRecord(object.propsJson) ? object.propsJson : {},
      createdAt: object.createdAt,
      updatedAt: object.updatedAt,
      deletedAt: object.deletedAt,
    }));
}

function buildObjectIndexes(objects: readonly GameArkObjectRecord[]) {
  const byGameId = new Map<string, GameArkObjectRecord>();
  const byExePath = new Map<string, GameArkObjectRecord>();

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
  directById: ReadonlyMap<string, GameArkObjectRecord>,
  fallback?: {
    byGameId: ReadonlyMap<string, GameArkObjectRecord>;
    byExePath: ReadonlyMap<string, GameArkObjectRecord>;
  },
): GameArkObjectRecord | null {
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

function mergeArkGameProps(game: Game, object: GameArkObjectRecord): Game {
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

async function ensureGameObjectType(
  arkObjectTypes: Pick<ArkObjectTypesApi, "get" | "upsert">,
  timestamp: string,
) {
  const existing = await arkObjectTypes.get(GAME_OBJECT_TYPE_ID);
  if (existing) {
    return;
  }

  await arkObjectTypes.upsert({
    id: GAME_OBJECT_TYPE_ID,
    name: "Игра",
    schemaJson: JSON.stringify({
      fields: [
        { id: "description", label: "Описание", kind: "long_text", required: false },
        { id: "play_status", label: "Статус", kind: "select", required: false },
        { id: "genres", label: "Жанры", kind: "multi_select", required: false },
        { id: "exe_path", label: "Путь к игре", kind: "text", required: false, system: true },
        { id: "exe_name", label: "Имя exe", kind: "text", required: false, system: true },
        { id: "total_playtime_seconds", label: "Время игры", kind: "number", required: false, read_only: true, system: true },
        { id: "last_played_at", label: "Последний запуск", kind: "date", required: false, read_only: true, system: true },
        { id: "play_count", label: "Запусков", kind: "number", required: false, read_only: true, system: true },
      ],
    }),
    uiSchemaJson: JSON.stringify({
      collection_name: "Игры",
      visible_fields: ["play_status", "genres", "total_playtime_seconds", "last_played_at"],
      hidden_fields: ["description", "exe_path", "exe_name", "play_count"],
      read_only_fields: ["total_playtime_seconds", "last_played_at", "play_count", "exe_name"],
      field_order: [
        "play_status",
        "genres",
        "total_playtime_seconds",
        "last_played_at",
        "description",
        "exe_path",
        "exe_name",
        "play_count",
      ],
    }),
    createdAt: timestamp,
    updatedAt: timestamp,
    systemLocked: true,
  });
}

async function loadExistingObjectForSync(
  arkObjects: Pick<ArkObjectsApi, "listByType" | "get">,
  game: Game,
): Promise<GameArkObjectRecord | null> {
  if (game.ark_object_id) {
    const object = await arkObjects.get(game.ark_object_id);
    if (object?.typeId === GAME_OBJECT_TYPE_ID) {
      return {
        id: object.id,
        typeId: object.typeId,
        title: object.title,
        contentJson: object.contentJson,
        propsJson: isJsonRecord(object.propsJson) ? object.propsJson : {},
        createdAt: object.createdAt,
        updatedAt: object.updatedAt,
        deletedAt: object.deletedAt,
      };
    }
  }

  const objects = await listActiveGameObjectsFromSdk(arkObjects);
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
  let arkClient: ArkClient | null = null;
  let arkClientPromise: Promise<ArkClient> | null = null;

  const resolveArkClient = async (): Promise<ArkClient> => {
    if (arkClient) return arkClient;
    if (arkClientPromise) return arkClientPromise;
    arkClientPromise = (async () => {
      const spaceId = options.spaceId ?? "arrancador";
      const deviceId = options.deviceId ?? "arrancador-main";
      const deviceName = options.deviceName ?? "Arrancador";
      const requestTimeoutMs = options.requestTimeoutMs ?? 10_000;

      // Phase 3: kepler-aware resolution.
      const state = await ensureKeplerRunning({
        appDataPath: app.getPath("appData"),
        waitMs: 10000,
        autoLaunch: !isKeplerOptional(),
      });

      switch (state.kind) {
        case "connected": {
          console.log(
            `[arrancador.ark] using Kepler host (pid ${state.lock.pid}, ws_port ${state.lock.ws_port})`,
          );
          const client = new ArkClient({
            spaceId,
            deviceId,
            deviceName,
            keplerLock: state.lock,
            requestTimeoutMs,
          });
          arkClient = client;
          return client;
        }
        case "incompatible-version": {
          throw new Error(
            `Kepler protocol mismatch: server ${state.keplerVersion.major}.${state.keplerVersion.minor}.${state.keplerVersion.patch}, ` +
              `client expects ${state.clientMajor}.x.`,
          );
        }
        case "launch-failed":
        case "not-installed": {
          if (isKeplerRequired()) {
            const detail =
              state.kind === "not-installed"
                ? `checked: ${state.checkedPaths.join(", ") || "(no candidates)"}`
                : state.reason;
            throw new Error(
              `Arrancador запущен с KOSMOS_REQUIRE_KEPLER=1, но Kepler ${state.kind} (${detail}). ` +
                `Установи Kosmos Kepler или сними флаг.`,
            );
          }
          console.log(
            `[arrancador.ark] Kepler ${state.kind} — standalone mode, sync disabled`,
          );
          const client = new ArkClient({
            spaceId,
            deviceId,
            deviceName,
            dbPath: options.arkDbPath,
            sidecarPath: options.arkCoreRpcPath ?? getArkCoreRpcBinaryPath(options),
            requestTimeoutMs,
          });
          arkClient = client;
          return client;
        }
      }
    })();
    return arkClientPromise;
  };

  const getArkObjects = async (): Promise<
    Pick<ArkObjectsApi, "list" | "listByType" | "get" | "getMany" | "upsert">
  > => {
    if (options.arkObjects) {
      return options.arkObjects;
    }
    const client = await resolveArkClient();
    return client.objects;
  };

  const getArkObjectTypes = async (): Promise<
    Pick<ArkObjectTypesApi, "get" | "upsert">
  > => {
    if (options.arkObjectTypes) {
      return options.arkObjectTypes;
    }
    const client = await resolveArkClient();
    return client.objectTypes;
  };

  return {
    async hydrateGames(games: Game[]): Promise<Game[]> {
      if (games.length === 0) {
        return games;
      }

      try {
        const arkObjects = await getArkObjects();
        const linkedIds = games
          .map((game) => game.ark_object_id)
          .filter((value): value is string => typeof value === "string" && value.length > 0);
        const directObjects = await arkObjects.getMany(linkedIds);
        const directById = new Map(
          directObjects
            .filter((object) => object.typeId === GAME_OBJECT_TYPE_ID && object.deletedAt === null)
            .map((object) => [
              object.id,
              {
                ...object,
                propsJson: isJsonRecord(object.propsJson) ? object.propsJson : {},
              },
            ]),
        );
        const needsFallback = games.some(
          (game) => !game.ark_object_id || !directById.has(game.ark_object_id),
        );
        const fallback = needsFallback ? buildObjectIndexes(await listActiveGameObjectsFromSdk(arkObjects)) : null;

        return games.map((game) => {
          const object = resolveLinkedObject(game, directById, fallback ?? undefined);
          return object ? mergeArkGameProps(game, object) : game;
        });
      } catch (error) {
        if (options.throwOnError) {
          throw error;
        }

        return games;
      }
    },

    async syncGame(game: Game): Promise<string | null> {
      try {
        const arkObjects = await getArkObjects();
        const existing = await loadExistingObjectForSync(arkObjects, game);
        const objectId = existing?.id ?? game.ark_object_id ?? randomUUID();
        const timestamp = now().toISOString();
        await ensureGameObjectType(await getArkObjectTypes(), timestamp);
        const createdAt = existing?.createdAt ?? timestamp;
        const contentJson = existing?.contentJson ?? DEFAULT_CONTENT_JSON;
        const propsJson = buildGameObjectProps(game, existing?.propsJson ?? {});

        await arkObjects.upsert({
          id: objectId,
          typeId: GAME_OBJECT_TYPE_ID,
          title: game.name,
          contentJson: contentJson as JsonValue,
          propsJson: propsJson as JsonValue,
          createdAt,
          updatedAt: timestamp,
          deletedAt: null,
        });

        return objectId;
      } catch (error) {
        if (options.throwOnError) {
          throw error;
        }

        return null;
      }
    },
  };
}
