import { expect, test } from "bun:test";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  migrationJournalPath,
  migrationFinalizationPath,
  readMigrationJournal,
  recoverPreparedMigration,
  runLegacyMigration,
  validateMigrationJournal,
  writeMigrationJournal,
  isLegacyLaunchBlocked,
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
    expect(
      JSON.parse(await readFile(migrationJournalPath(root, journal.target_id), "utf8")),
    ).toEqual(journal);
    await expect(
      Promise.resolve().then(() =>
        validateMigrationJournal({ ...journal, secret: "must-not-persist" }),
      ),
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
    expect(
      await recoverPreparedMigration(root, journal.target_id, async () => events.push("restored")),
    ).toBe(true);
    expect(events).toEqual(["restored"]);
    expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("prepared and committed journals block legacy launch", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  try {
    expect(isLegacyLaunchBlocked(root, "eden")).toBe(false);
    await writeMigrationJournal(root, {
      ...journal,
      target_id: "com.kosmos.memoria",
      source_ids: ["eden"],
      phase: "prepared",
    });
    expect(isLegacyLaunchBlocked(root, "eden")).toBe(true);
    await writeMigrationJournal(root, {
      ...journal,
      target_id: "com.kosmos.memoria",
      source_ids: ["eden"],
      phase: "committed",
    });
    expect(isLegacyLaunchBlocked(root, "eden")).toBe(true);
    expect(isLegacyLaunchBlocked(root, "delphi")).toBe(false);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("writes prepared before cutover and restores it on activation failure", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  try {
    const events: string[] = [];
    await expect(
      runLegacyMigration({
        dataDir: root,
        journal: { ...journal, phase: "prepared" },
        verifyReplacement: async () => true,
        stopAffected: async () => events.push("stopped"),
        snapshotBefore: async () => events.push("snapshotted"),
        stageDestination: async () => events.push("staged"),
        revokeLegacyGrants: async () => events.push("revoked"),
        activateCanonical: async () => {
          events.push("activation-failed");
          throw new Error("activation failed");
        },
        restoreBefore: async () => events.push("restored"),
      }),
    ).rejects.toThrow("activation failed");
    expect(events).toEqual([
      "stopped",
      "snapshotted",
      "staged",
      "revoked",
      "activation-failed",
      "restored",
    ]);
    expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("restores the snapshot when staging fails before the prepared marker", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  try {
    const events: string[] = [];
    await expect(
      runLegacyMigration({
        dataDir: root,
        journal,
        verifyReplacement: async () => true,
        stopAffected: async () => events.push("stopped"),
        snapshotBefore: async () => events.push("snapshotted"),
        stageDestination: async () => {
          events.push("stage-failed");
          throw new Error("stage failed");
        },
        revokeLegacyGrants: async () => events.push("revoked"),
        activateCanonical: async () => events.push("activated"),
        restoreBefore: async () => events.push("restored"),
      }),
    ).rejects.toThrow("stage failed");
    expect(events).toEqual(["stopped", "snapshotted", "stage-failed", "restored"]);
    expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("stop precedes snapshot and failure restores before the journal", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  try {
    const events: string[] = [];
    await expect(
      runLegacyMigration({
        dataDir: root,
        journal,
        verifyReplacement: async () => true,
        snapshotBefore: async () => events.push("snapshotted"),
        stopAffected: async () => {
          events.push("stopped");
          expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
          throw new Error("crash during stop");
        },
        stageDestination: async () => events.push("staged"),
        revokeLegacyGrants: async () => events.push("revoked"),
        activateCanonical: async () => events.push("activated"),
        restoreBefore: async () => events.push("restored"),
      }),
    ).rejects.toThrow("crash during stop");
    expect(events).toEqual(["stopped", "restored"]);
    expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("private finalization marker makes post-commit-journal failure recoverable", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  try {
    const events: string[] = [];
    await expect(
      runLegacyMigration({
        dataDir: root,
        journal,
        verifyReplacement: async () => true,
        stopAffected: async () => events.push("stopped"),
        snapshotBefore: async () => events.push("snapshotted"),
        stageDestination: async () => events.push("staged"),
        revokeLegacyGrants: async () => events.push("revoked"),
        activateCanonical: async () => events.push("activated"),
        commitLegacyGrants: async () => {
          expect(await readFile(migrationFinalizationPath(root, journal.target_id), "utf8")).toBe(
            "finalizing\n",
          );
          events.push("committed-grants");
        },
        writeJournal: async (value) => {
          if (value.phase === "committed") throw new Error("journal fsync failed");
          await writeMigrationJournal(root, value);
        },
        restoreBefore: async () => events.push("restored"),
      }),
    ).rejects.toThrow("journal fsync failed");
    expect(events).toEqual([
      "stopped",
      "snapshotted",
      "staged",
      "revoked",
      "activated",
      "committed-grants",
      "restored",
    ]);
    expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
