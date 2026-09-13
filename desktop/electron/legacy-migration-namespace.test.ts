import { expect, test } from "../test-support/node-test.mjs";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { runLegacyMigration } from "./legacy-migration-journal";
import { restoreNamespace } from "./legacy-migration-runtime";

const journal = {
  schema_version: 1 as const,
  target_id: "com.kosmos.arcadia" as const,
  source_ids: ["arcadia", "arrancador"],
  replacement: { version: "1.2.3", sha256: "a".repeat(64), catalog_sequence: 13 },
  phase: "prepared" as const,
  grant_policy: "reconsent" as const,
  records_policy: "opaque-preserve" as const,
  grant_transaction_token: null,
};

test("stop failure before a completed namespace snapshot preserves canonical data", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  const destination = path.join(root, "extensions-data", journal.target_id, "state.json");
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, '{"preserve":true}\n');
    await expect(
      runLegacyMigration({
        dataDir: root,
        journal,
        verifyReplacement: async () => true,
        stopAffected: async () => {
          throw new Error("crash during stop");
        },
        snapshotBefore: async () => {
          throw new Error("snapshot not reached");
        },
        stageDestination: async () => {},
        revokeLegacyGrants: async () => {},
        activateCanonical: async () => {},
        restoreBefore: () => restoreNamespace(root, journal.target_id),
      }),
    ).rejects.toThrow("crash during stop");
    expect(await readFile(destination, "utf8")).toBe('{"preserve":true}\n');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("a stale snapshot marker cannot remove data from a new migration attempt", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  const before = path.join(root, "legacy-migrations", "v1", journal.target_id, "before");
  const destination = path.join(root, "extensions-data", journal.target_id, "state.json");
  try {
    await mkdir(path.join(before, "snapshot.complete"), { recursive: true });
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, '{"new-attempt":true}\n');
    await expect(
      runLegacyMigration({
        dataDir: root,
        journal,
        verifyReplacement: async () => true,
        stopAffected: async () => {
          await rm(path.join(before, "snapshot.complete"), { recursive: true, force: true });
          throw new Error("crash during stop");
        },
        snapshotBefore: async () => {},
        stageDestination: async () => {},
        revokeLegacyGrants: async () => {},
        activateCanonical: async () => {},
        restoreBefore: () => restoreNamespace(root, journal.target_id),
      }),
    ).rejects.toThrow("crash during stop");
    expect(await readFile(destination, "utf8")).toBe('{"new-attempt":true}\n');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("completed snapshot removes a newly-created canonical destination on rollback", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-journal-"));
  const before = path.join(root, "legacy-migrations", "v1", journal.target_id, "before");
  const destination = path.join(root, "extensions-data", journal.target_id, "state.json");
  try {
    await mkdir(before, { recursive: true });
    await mkdir(path.join(before, "snapshot.complete"));
    await mkdir(path.dirname(destination), { recursive: true });
    await writeFile(destination, '{"new":true}\n');
    await restoreNamespace(root, journal.target_id);
    await expect(readFile(destination, "utf8")).rejects.toMatchObject({ code: "ENOENT" });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
