import { Database } from "bun:sqlite";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "bun:test";
import { recoverDatabaseRestore, restoreDatabaseBackup } from "./database-backups";

const roots: string[] = [];
type SqliteIntegrityRow = { integrity_check: string };
type SqliteTableRow = { name: string };
type SqliteObjectRow = { id: string };
type ArkBackupInspection = { objectIds: string[] };

function makeRoot(): string {
  const root = fsTempDir();
  roots.push(root);
  mkdirSync(path.join(root, "backups"), { recursive: true });
  return root;
}

function fsTempDir(): string {
  return os.tmpdir() + path.sep + `kosmos-db-backups-${Date.now()}-${Math.random()}`;
}

function createArkDatabase(file: string, objectId: string): void {
  const database = new Database(file);
  database.run(
    `
      CREATE TABLE object_types (id TEXT PRIMARY KEY, name TEXT NOT NULL, schema_json TEXT NOT NULL,
        ui_schema_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
        system_locked INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE objects (id TEXT PRIMARY KEY, type_id TEXT NOT NULL, type_version TEXT NOT NULL,
        title TEXT NOT NULL, content_json TEXT NOT NULL, props_json TEXT NOT NULL,
        created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted_at TEXT);
      CREATE TABLE object_links (id TEXT PRIMARY KEY, source_object_id TEXT NOT NULL,
        target_object_id TEXT NOT NULL, link_type TEXT NOT NULL, created_at TEXT NOT NULL);
      CREATE TABLE sync_kv (key TEXT PRIMARY KEY, value TEXT NOT NULL);
      CREATE TABLE sync_tombstones (id TEXT PRIMARY KEY, entity_type TEXT NOT NULL,
        hlc TEXT NOT NULL, deleted_at TEXT NOT NULL);
      CREATE INDEX idx_objects_type_id ON objects(type_id);
      CREATE INDEX idx_objects_updated_at ON objects(updated_at DESC);
      CREATE INDEX idx_objects_deleted_at ON objects(deleted_at);
      CREATE INDEX idx_object_links_source ON object_links(source_object_id);
      CREATE INDEX idx_object_links_target ON object_links(target_object_id);
    `,
  );
  database.run(
    "INSERT INTO object_types (id, name, schema_json, ui_schema_json, created_at, updated_at) VALUES (?, ?, '{}', '{}', 'c', 'u')",
    ["com.kosmos.note", "Note"],
  );
  database.run(
    "INSERT INTO objects (id, type_id, type_version, title, content_json, props_json, created_at, updated_at, deleted_at) VALUES (?, 'com.kosmos.note', '0.0.0-legacy', 'Note', '{}', '{}', 'c', 'u', NULL)",
    [objectId],
  );
  database.close();
}

function createMinimalArkDatabase(file: string, objectId: string): void {
  const database = new Database(file);
  database.run(
    "CREATE TABLE object_types (id TEXT PRIMARY KEY); CREATE TABLE objects (id TEXT PRIMARY KEY, deleted_at TEXT);",
  );
  database.run("INSERT INTO object_types VALUES ('fake')");
  database.run("INSERT INTO objects VALUES (?, NULL)", [objectId]);
  database.close();
}

function inspectBackup(file: string): ArkBackupInspection {
  const database = new Database(file, { readonly: true });
  try {
    // SAFETY: this SQLite query returns the single named integrity result.
    const integrity = database.query("PRAGMA integrity_check").get() as SqliteIntegrityRow;
    if (integrity.integrity_check !== "ok") throw new Error("SQLite integrity check failed");
    // SAFETY: this SQLite query selects the named table field from sqlite_master.
    const tables = database
      .query("SELECT name FROM sqlite_master WHERE type = 'table'")
      .all() as SqliteTableRow[];
    if (
      !tables.some(({ name }) => name === "objects") ||
      !tables.some(({ name }) => name === "object_types") ||
      !tables.some(({ name }) => name === "object_links") ||
      !tables.some(({ name }) => name === "sync_kv") ||
      !tables.some(({ name }) => name === "sync_tombstones")
    )
      throw new Error("Not an ARK backup");
    for (const [table, columns] of [
      ["object_types", ["id", "name", "schema_json", "ui_schema_json", "created_at", "updated_at"]],
      [
        "objects",
        [
          "id",
          "type_id",
          "type_version",
          "title",
          "content_json",
          "props_json",
          "created_at",
          "updated_at",
          "deleted_at",
        ],
      ],
      ["object_links", ["id", "source_object_id", "target_object_id", "link_type", "created_at"]],
      ["sync_kv", ["key", "value"]],
      ["sync_tombstones", ["id", "entity_type", "hlc", "deleted_at"]],
    ] as const) {
      // SAFETY: table names are fixed literals from the test ARK schema contract.
      const actual = database.query(`PRAGMA table_info('${table}')`).all() as { name: string }[];
      if (columns.some((column) => !actual.some(({ name }) => name === column)))
        throw new Error("Incompatible ARK schema");
    }
    // SAFETY: this SQLite query selects index names from sqlite_master.
    const indexes = database.query("SELECT name FROM sqlite_master WHERE type = 'index'").all() as {
      name: string;
    }[];
    if (
      [
        "idx_objects_type_id",
        "idx_objects_updated_at",
        "idx_objects_deleted_at",
        "idx_object_links_source",
        "idx_object_links_target",
      ].some((index) => !indexes.some(({ name }) => name === index))
    )
      throw new Error("Incomplete ARK schema");
    // SAFETY: this SQLite query selects the object id field from the test ARK schema.
    const rows = database
      .query("SELECT id FROM objects WHERE deleted_at IS NULL")
      .all() as SqliteObjectRow[];
    return { objectIds: rows.map(({ id }) => id) };
  } finally {
    database.close();
  }
}

afterEach(() => {
  for (const root of roots.splice(0)) rmSync(root, { recursive: true, force: true });
});

describe("database backup transaction", () => {
  test("rejects invalid backups without stopping the Engine", async () => {
    const root = makeRoot();
    const current = path.join(root, "ark.db");
    const backup = "ark.db.backup-2026-09-13-120000";
    createArkDatabase(current, "current");
    createMinimalArkDatabase(path.join(root, "backups", backup), "fake");
    let stopped = false;

    await expect(
      restoreDatabaseBackup(root, backup, {
        stop: async () => {
          stopped = true;
        },
        waitUntilStopped: async () => undefined,
        start: async () => true,
        waitUntilReady: async () => true,
        liveObjectIds: async () => ["current"],
        inspectBackup,
      }),
    ).rejects.toThrow(/database|SQLite|ARK/i);
    expect(stopped).toBe(false);
  });

  test("restores an ARK backup and proves its objects after restart", async () => {
    const root = makeRoot();
    const current = path.join(root, "ark.db");
    const backup = "ark.db.backup-2026-09-13-120000";
    const backupPath = path.join(root, "backups", backup);
    createArkDatabase(current, "current");
    createArkDatabase(backupPath, "restored");
    const events: string[] = [];

    await expect(
      restoreDatabaseBackup(root, backup, {
        stop: async () => events.push("stop"),
        waitUntilStopped: async () => events.push("stopped"),
        start: async () => {
          events.push("start");
          return true;
        },
        waitUntilReady: async () => {
          events.push("ready");
          return true;
        },
        liveObjectIds: async () => ["restored"],
        inspectBackup,
      }),
    ).resolves.toMatchObject({ name: backup, objectCount: 1 });
    expect(events).toEqual(["stop", "stopped", "start", "ready"]);
  });

  test("rolls back when the restarted Engine does not contain backup objects", async () => {
    const root = makeRoot();
    const current = path.join(root, "ark.db");
    const backup = "ark.db.backup-2026-09-13-120000";
    createArkDatabase(current, "current");
    createArkDatabase(path.join(root, "backups", backup), "restored");
    let starts = 0;

    await expect(
      restoreDatabaseBackup(root, backup, {
        stop: async () => undefined,
        waitUntilStopped: async () => undefined,
        start: async () => {
          starts += 1;
          return true;
        },
        waitUntilReady: async () => true,
        liveObjectIds: async () => (starts === 1 ? ["wrong"] : ["current"]),
        inspectBackup,
      }),
    ).rejects.toThrow(/verification/i);
    expect(starts).toBe(2);
    const restoredCurrent = new Database(current, { readonly: true });
    expect(restoredCurrent.query("SELECT id FROM objects").get()).toEqual({ id: "current" });
    restoredCurrent.close();
  });

  test("serializes concurrent restore operations", async () => {
    const root = makeRoot();
    const current = path.join(root, "ark.db");
    const backup = "ark.db.backup-2026-09-13-120000";
    createArkDatabase(current, "current");
    createArkDatabase(path.join(root, "backups", backup), "restored");
    let active = 0;
    let maximum = 0;
    const hooks = {
      stop: async () => {
        active += 1;
        maximum = Math.max(maximum, active);
        await new Promise((resolve) => setTimeout(resolve, 10));
      },
      waitUntilStopped: async () => undefined,
      start: async () => true,
      waitUntilReady: async () => true,
      liveObjectIds: async () => {
        await new Promise((resolve) => setTimeout(resolve, 10));
        active -= 1;
        return ["restored"];
      },
      inspectBackup,
    };

    await Promise.all([
      restoreDatabaseBackup(root, backup, hooks),
      restoreDatabaseBackup(root, backup, hooks),
    ]);
    expect(maximum).toBe(1);
  });

  test("recovers a missing primary from the durable restore journal", async () => {
    const root = makeRoot();
    const current = path.join(root, "ark.db");
    const rollback = path.join(root, ".ark.db.restore-rollback-test");
    const moved = path.join(root, ".ark.db.restore-live-test");
    const staged = path.join(root, ".ark.db.restore-new-test");
    createArkDatabase(rollback, "current");
    writeFileSync(
      path.join(root, ".ark.db.restore-journal.json"),
      JSON.stringify({ database: current, rollback, moved, staged, sidecars: [] }),
    );

    await recoverDatabaseRestore(root);
    const recovered = new Database(current, { readonly: true });
    expect(recovered.query("SELECT id FROM objects").get()).toEqual({ id: "current" });
    recovered.close();
  });
});
