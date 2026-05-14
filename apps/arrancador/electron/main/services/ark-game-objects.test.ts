import path from "node:path";

import type { ArkObjectRecord, ArkObjectsApi, ArkObjectTypesApi } from "@kosmos/ark";
import { describe, expect, it, vi } from "vitest";

import {
  createArkGameObjectService,
  getArkCoreRpcBinaryPath,
} from "./ark-game-objects";
import type { Game } from "./games/types";

const timestamp = "2026-04-24T00:00:00.000Z";

function game(overrides: Partial<Game> = {}): Game {
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
    date_added: timestamp,
    backup_enabled: 1,
    last_backup: null,
    backup_count: 0,
    save_path: null,
    user_rating: null,
    user_note: null,
    play_status: "not_started",
    process_bindings: [],
    ...overrides,
  };
}

function arkObject(overrides: Partial<ArkObjectRecord> = {}): ArkObjectRecord {
  return {
    id: "ark-game-1",
    typeId: "game_obj",
    title: "Control",
    contentJson: { type: "doc", content: [{ type: "paragraph" }] },
    propsJson: {
      sync_source: "arrancador",
      arrancador_game_id: "game-1",
      exe_path: "C:\\Games\\Control\\Control.exe",
    },
    createdAt: "2026-04-23T00:00:00.000Z",
    updatedAt: "2026-04-23T00:00:00.000Z",
    deletedAt: null,
    ...overrides,
  };
}

function arkObjects(overrides: Partial<ArkObjectsApi> = {}) {
  return {
    list: vi.fn(async () => []),
    listByType: vi.fn(async () => []),
    get: vi.fn(async () => null),
    getMany: vi.fn(async () => []),
    upsert: vi.fn(async () => undefined),
    delete: vi.fn(async () => undefined),
    search: vi.fn(async () => []),
    ...overrides,
  } satisfies ArkObjectsApi;
}

function arkObjectTypes(overrides: Partial<ArkObjectTypesApi> = {}) {
  return {
    list: vi.fn(async () => []),
    get: vi.fn(async () => null),
    upsert: vi.fn(async () => undefined),
    delete: vi.fn(async () => undefined),
    ...overrides,
  } satisfies ArkObjectTypesApi;
}

describe("createArkGameObjectService", () => {
  it("writes game objects through the Ark object API", async () => {
    const objects = arkObjects();
    const objectTypes = arkObjectTypes();
    const service = createArkGameObjectService({
      arkDbPath: "unused.db",
      arkObjects: objects,
      arkObjectTypes: objectTypes,
      now: () => new Date(timestamp),
    });

    await expect(service.syncGame(game())).resolves.toEqual(expect.any(String));

    expect(objectTypes.upsert).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "game_obj",
        name: "Игра",
        systemLocked: true,
      }),
    );
    expect(objects.upsert).toHaveBeenCalledWith(
      expect.objectContaining({
        typeId: "game_obj",
        title: "Control",
        createdAt: timestamp,
        updatedAt: timestamp,
        deletedAt: null,
        propsJson: expect.objectContaining({
          sync_source: "arrancador",
          arrancador_game_id: "game-1",
          exe_path: "C:\\Games\\Control\\Control.exe",
        }),
      }),
    );
  });

  it("reuses an existing object by ark_object_id", async () => {
    const existing = arkObject({
      id: "existing-object",
      propsJson: {
        custom: "keep",
        arrancador_game_id: "game-1",
      },
    });
    const objects = arkObjects({
      get: vi.fn(async () => existing),
    });
    const objectTypes = arkObjectTypes({
      get: vi.fn(async () => ({
        id: "game_obj",
        name: "Игра",
        schemaJson: "{}",
        uiSchemaJson: "{}",
        createdAt: timestamp,
        updatedAt: timestamp,
        systemLocked: true,
      })),
    });
    const service = createArkGameObjectService({
      arkDbPath: "unused.db",
      arkObjects: objects,
      arkObjectTypes: objectTypes,
      now: () => new Date(timestamp),
    });

    await expect(
      service.syncGame(game({ ark_object_id: "existing-object" })),
    ).resolves.toBe("existing-object");

    expect(objects.list).not.toHaveBeenCalled();
    expect(objects.upsert).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "existing-object",
        createdAt: existing.createdAt,
        propsJson: expect.objectContaining({ custom: "keep" }),
      }),
    );
  });

  it("reuses an existing object by game id or executable path fallback", async () => {
    const objects = arkObjects({
      listByType: vi.fn(async () => [
        arkObject({
          id: "by-game-id",
          propsJson: { arrancador_game_id: "game-1" },
        }),
        arkObject({
          id: "by-exe",
          propsJson: { exe_path: "C:\\Games\\Control\\Control.exe" },
        }),
      ]),
    });
    const objectTypes = arkObjectTypes();
    const service = createArkGameObjectService({
      arkDbPath: "unused.db",
      arkObjects: objects,
      arkObjectTypes: objectTypes,
      now: () => new Date(timestamp),
    });

    await expect(service.syncGame(game())).resolves.toBe("by-game-id");
    expect(objects.upsert).toHaveBeenCalledWith(
      expect.objectContaining({ id: "by-game-id" }),
    );
  });

  it("resolves ark-core-rpc paths for packaged and dev runtimes", () => {
    expect(
      getArkCoreRpcBinaryPath({
        isPackaged: true,
        resourcesPath: "C:\\App\\resources",
      }),
    ).toBe(path.join("C:\\App\\resources", "ark-core", "ark-core-rpc.exe"));

    expect(
      getArkCoreRpcBinaryPath({
        isPackaged: false,
        appRoot: "D:\\repo\\apps\\arrancador",
      }),
    ).toBe(path.join(
      "D:\\repo",
      "packages",
      "ark-core",
      "rust",
      "target",
      "debug",
      "ark-core-rpc.exe",
    ));
  });
});
