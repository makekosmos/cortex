import assert from "node:assert/strict";

import { migrateArkGames } from "../../../apps/arrancador/electron/main/services/ark-game-migration.ts";

const timestamp = "2026-04-24T00:00:00.000Z";

function sourceObject(overrides = {}) {
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

function objectType(overrides = {}) {
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

function sdkObject(overrides = {}) {
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

function sourceDb(options) {
  const calls = { all: 0, get: 0, run: 0, close: 0 };
  return {
    calls,
    db: {
      all: async () => {
        calls.all += 1;
        return options.objects ?? [];
      },
      get: async () => {
        calls.get += 1;
        return options.objectType;
      },
      run: async () => {
        calls.run += 1;
        throw new Error("source DB should not be written");
      },
      close: async () => {
        calls.close += 1;
      },
    },
  };
}

function migrationTarget(options = {}) {
  const calls = {
    typeUpserts: [],
    objectUpserts: [],
    close: 0,
  };
  return {
    calls,
    target: {
      objectTypes: {
        list: async () => options.types ?? [],
        upsert: async (record) => {
          calls.typeUpserts.push(record);
        },
      },
      objects: {
        list: async () => options.objects ?? [],
        upsert: async (record) => {
          calls.objectUpserts.push(record);
        },
      },
      close: async () => {
        calls.close += 1;
      },
    },
  };
}

async function runMigration({ db, target }) {
  return await migrateArkGames({
    sourceDbPath: "source.db",
    targetDbPath: "target.db",
    openSourceDb: () => db,
    arkTarget: target,
  });
}

{
  const { db, calls: dbCalls } = sourceDb({
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
  const { target, calls } = migrationTarget();
  const result = await runMigration({ db, target });
  assert.deepEqual(result, {
    migratedTypes: 1,
    migratedObjects: 0,
    mergedObjects: 0,
    skippedObjects: 0,
  });
  assert.deepEqual(calls.typeUpserts, [objectType()]);
  assert.equal(dbCalls.run, 0);
}

{
  const { db, calls: dbCalls } = sourceDb({ objects: [sourceObject()] });
  const { target, calls } = migrationTarget({ types: [objectType()] });
  const result = await runMigration({ db, target });
  assert.equal(result.migratedObjects, 1);
  assert.equal(calls.objectUpserts.length, 1);
  assert.deepEqual(calls.objectUpserts[0].contentJson, { type: "doc", content: [] });
  assert.equal(calls.objectUpserts[0].propsJson.arrancador_game_id, "game-1");
  assert.equal(dbCalls.run, 0);
}

{
  const { db } = sourceDb({
    objects: [
      sourceObject({
        props_json: JSON.stringify({
          exe_path: "C:/Games/Control/Control.exe",
          source_value: "new",
        }),
      }),
    ],
  });
  const { target, calls } = migrationTarget({
    types: [objectType()],
    objects: [
      sdkObject({
        createdAt: "2026-04-23T00:00:00.000Z",
        updatedAt: "2026-04-25T00:00:00.000Z",
        propsJson: {
          exe_path: "C:\\Games\\Control\\Control.exe",
          keep: "target",
        },
      }),
    ],
  });
  const result = await runMigration({ db, target });
  assert.equal(result.mergedObjects, 1);
  assert.equal(calls.objectUpserts[0].id, "target-object");
  assert.equal(calls.objectUpserts[0].createdAt, "2026-04-23T00:00:00.000Z");
  assert.equal(calls.objectUpserts[0].updatedAt, "2026-04-25T00:00:00.000Z");
  assert.equal(calls.objectUpserts[0].propsJson.keep, "target");
  assert.equal(calls.objectUpserts[0].propsJson.source_value, "new");
}

{
  const row = sourceObject();
  const { db } = sourceDb({ objects: [row] });
  const { target, calls } = migrationTarget({
    types: [objectType()],
    objects: [
      sdkObject({
        id: row.id,
        title: row.title,
        propsJson: JSON.parse(row.props_json),
        createdAt: row.created_at,
        updatedAt: row.updated_at,
      }),
    ],
  });
  const result = await runMigration({ db, target });
  assert.equal(result.skippedObjects, 1);
  assert.equal(calls.objectUpserts.length, 0);
}

console.log("verify-ark-game-migration PASS");
