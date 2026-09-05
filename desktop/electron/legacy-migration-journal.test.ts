import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  migrationJournalPath,
  readMigrationJournal,
  recoverPreparedMigration,
  validateMigrationJournal,
  writeMigrationJournal,
} from "./legacy-migration-journal";

const journal = {
  schema_version: 1 as const,
  target_id: "com.kosmos.arcadia" as const,
  source_ids: ["arcadia", "arrancador"],
  replacement: { version: "1.2.3", sha256: "a".repeat(64), catalog_sequence: 13 },
  phase: "prepared" as const,
  grant_policy: "reconsent" as const,
  records_policy: "opaque-preserve" as const,
};

test("validates and durably writes the strict journal schema", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  try {
    validateMigrationJournal(journal);
    await writeMigrationJournal(root, journal);
    expect(JSON.parse(await readFile(migrationJournalPath(root, journal.target_id), "utf8"))).toEqual(journal);
    await expect(
      Promise.resolve().then(() => validateMigrationJournal({ ...journal, secret: "must-not-persist" })),
    ).rejects.toThrow();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("prepared recovery restores before-state before removing visibility marker", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  try {
    await writeMigrationJournal(root, journal);
    const events: string[] = [];
    expect(await recoverPreparedMigration(root, journal.target_id, async () => events.push("restored"))).toBe(true);
    expect(events).toEqual(["restored"]);
    expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
