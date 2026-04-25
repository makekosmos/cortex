import type {
  ArkObjectTypeRecord,
  ArkObjectRecord as SdkArkObjectRecord,
} from "@kepler/ark";
import { describe, expect, it, vi } from "vitest";

import type { DbLike } from "../helpers/shared";
import { migrateArkGames } from "./ark-game-migration";

const timestamp = "2026-04-24T00:00:00.000Z";

type SourceObjectRow = {
  id: string;
  type_id: string;
  title: string;
  content_json: string;
  props_json: string;
  created_at: string;
  updated_at: string;
  deleted_at: string | null;
};

function sourceObject(overrides: Partial<SourceObjectRow> = {}): SourceObjectRow {
  return {
    id: "source-object",
    type_id: "game_obj",
    title: "Control",
    content_json: '{"type":"doc","content":[]}',
    props_json: JSON.stringify({
      arrancador_game_id: "game-1",
      exe_path: "C:\\Games\\Control\\Control.exe",
    }),
    created_at: timestamp,
    updated_at: timestamp,
    deleted_at: null,
    ...overrides,
  };
}

function objectType(overrides: Partial<ArkObjectTypeRecord> = {}): ArkObjectTypeRecord {
  return {
    id: "game_obj",
    name: "Game",
    schemaJson: "{}",
    uiSchemaJson: "{}",
    createdAt: timestamp,
    updatedAt: timestamp,
    systemLocked: true,
    ...overrides,
  };
}

function sdkObject(overrides: Partial<SdkArkObjectRecord> = {}): SdkArkObjectRecord {
  return {
    id: "target-object",
    typeId: "game_obj",
    title: "Control",
    contentJson: { type: "doc", content: [] },
    propsJson: {
      arrancador_game_id: "game-1",
      exe_path: "C:\\Games\\Control\\Control.exe",
    },
    createdAt: timestamp,
    updatedAt: timestamp,
    deletedAt: null,
    ...overrides,
  };
}

function sourceDb(options: {
  objectType?: {
    id: string;
    name: string;
    schema_json: string;
    ui_schema_json: string;
    created_at: string;
    updated_at: string;
    system_locked: number;
  };
  objects?: SourceObjectRow[];
}) {
  return {
    all: vi.fn(async () => options.objects ?? []),
    get: vi.fn(async () => options.objectType),
    run: vi.fn(async () => {
      throw new Error("source DB should not be written");
    }),
    close: vi.fn(async () => undefined),
  } satisfies DbLike;
}

function migrationTarget(options: {
  types?: ArkObjectTypeRecord[];
  objects?: SdkArkObjectRecord[];
} = {}) {
  return {
    objectTypes: {
      list: vi.fn(async () => options.types ?? []),
      upsert: vi.fn(async () => undefined),
    },
    objects: {
      list: vi.fn(async () => options.objects ?? []),
      upsert: vi.fn(async () => undefined),
    },
    close: vi.fn(async () => undefined),
  };
}

describe("migrateArkGames", () => {
  it("creates the target game object type through the Ark SDK", async () => {
    const db = sourceDb({
      objectType: {
        id: "game_obj",
        name: "Game",
        schema_json: "{}",
        ui_schema_json: "{}",
        created_at: timestamp,
        updated_at: timestamp,
        system_locked: 1,
      },
      objects: [],
    });
    const target = migrationTarget();

    await expect(
      migrateArkGames({
        sourceDbPath: "source.db",
        targetDbPath: "target.db",
        openSourceDb: () => db,
        arkTarget: target,
      }),
    ).resolves.toEqual({
      migratedTypes: 1,
      migratedObjects: 0,
      mergedObjects: 0,
      skippedObjects: 0,
    });

    expect(target.objectTypes.upsert).toHaveBeenCalledWith(objectType());
    expect(db.run).not.toHaveBeenCalled();
  });

  it("migrates new game objects through the Ark SDK", async () => {
    const db = sourceDb({
      objectType: undefined,
      objects: [sourceObject()],
    });
    const target = migrationTarget({ types: [objectType()] });

    const result = await migrateArkGames({
      sourceDbPath: "source.db",
      targetDbPath: "target.db",
      openSourceDb: () => db,
      arkTarget: target,
    });

    expect(result).toEqual({
      migratedTypes: 0,
      migratedObjects: 1,
      mergedObjects: 0,
      skippedObjects: 0,
    });
    expect(target.objects.upsert).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "source-object",
        typeId: "game_obj",
        title: "Control",
        contentJson: { type: "doc", content: [] },
        propsJson: expect.objectContaining({ arrancador_game_id: "game-1" }),
        deletedAt: null,
      }),
    );
    expect(db.run).not.toHaveBeenCalled();
  });

  it("merges matching target objects by executable path", async () => {
    const db = sourceDb({
      objects: [
        sourceObject({
          id: "source-object",
          created_at: "2026-04-24T00:00:00.000Z",
          updated_at: "2026-04-24T00:00:00.000Z",
          props_json: JSON.stringify({
            exe_path: "C:/Games/Control/Control.exe",
            source_value: "new",
          }),
        }),
      ],
    });
    const target = migrationTarget({
      types: [objectType()],
      objects: [
        sdkObject({
          id: "target-object",
          createdAt: "2026-04-23T00:00:00.000Z",
          updatedAt: "2026-04-25T00:00:00.000Z",
          propsJson: {
            exe_path: "C:\\Games\\Control\\Control.exe",
            keep: "target",
          },
        }),
      ],
    });

    const result = await migrateArkGames({
      sourceDbPath: "source.db",
      targetDbPath: "target.db",
      openSourceDb: () => db,
      arkTarget: target,
    });

    expect(result.mergedObjects).toBe(1);
    expect(target.objects.upsert).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "target-object",
        createdAt: "2026-04-23T00:00:00.000Z",
        updatedAt: "2026-04-25T00:00:00.000Z",
        propsJson: expect.objectContaining({
          keep: "target",
          source_value: "new",
        }),
      }),
    );
  });

  it("skips unchanged target objects", async () => {
    const row = sourceObject();
    const db = sourceDb({ objects: [row] });
    const target = migrationTarget({
      types: [objectType()],
      objects: [
        sdkObject({
          id: row.id,
          title: row.title,
          contentJson: { type: "doc", content: [] },
          propsJson: JSON.parse(row.props_json) as Record<string, unknown>,
          createdAt: row.created_at,
          updatedAt: row.updated_at,
        }),
      ],
    });

    const result = await migrateArkGames({
      sourceDbPath: "source.db",
      targetDbPath: "target.db",
      openSourceDb: () => db,
      arkTarget: target,
    });

    expect(result).toEqual({
      migratedTypes: 0,
      migratedObjects: 0,
      mergedObjects: 0,
      skippedObjects: 1,
    });
    expect(target.objects.upsert).not.toHaveBeenCalled();
  });
});
