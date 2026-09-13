import { Database } from "bun:sqlite";
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { afterEach, describe, expect, test } from "bun:test";
import { restoreDatabaseBackup } from "./database-backups";

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
    "CREATE TABLE object_types (id TEXT PRIMARY KEY); CREATE TABLE objects (id TEXT PRIMARY KEY, deleted_at TEXT);",
  );
  database.run("INSERT INTO object_types VALUES (?)", ["com.kosmos.note"]);
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
      !tables.some(({ name }) => name === "object_types")
    )
      throw new Error("Not an ARK backup");
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
    writeFileSync(path.join(root, "backups", backup), "not sqlite");
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
});
