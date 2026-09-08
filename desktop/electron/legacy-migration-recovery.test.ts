import { expect, test } from "bun:test";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import type { ArkClient } from "@kosmos/ark";
import type { ArkRequest } from "./legacy-migration-state";
import { readMigrationJournal, writeMigrationJournal } from "./legacy-migration-journal";
import { recoverLegacyMigrationsBeforeLaunch } from "./legacy-migration-runtime";

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

async function recoveryClient(requests: ArkRequest[]) {
  return {
    async invokeOperation<T>(input: ArkRequest): Promise<T> {
      requests.push(input);
      if (input.operation === "packages.list") {
        // SAFETY: the isolated fixture intentionally has no package rows.
        return { packages: [] } as T;
      }
      // SAFETY: recovery ignores mutation response bodies in this fixture.
      return {} as T;
    },
  } satisfies Pick<ArkClient, "invokeOperation">;
}

async function preparePackageState(root: string, sourceIds: readonly string[] = []) {
  const migrationRoot = path.join(root, "legacy-migrations", "v1", "com.kosmos.arcadia", "before");
  await Promise.all(
    sourceIds.map((id) => mkdir(path.join(root, "extensions-data", id), { recursive: true })),
  );
  await mkdir(migrationRoot, { recursive: true });
  await writeFile(path.join(migrationRoot, "package-state.json"), "[]\n");
  await writeFile(path.join(migrationRoot, "package-state.pending"), "true\n");
}

test("prepared recovery sends a typed snapshot request when its token is null", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-recovery-"));
  try {
    await preparePackageState(root);
    await writeMigrationJournal(root, journal);
    const requests: ArkRequest[] = [];
    await recoverLegacyMigrationsBeforeLaunch(root, await recoveryClient(requests));
    expect(requests[0]).toEqual({
      operation: "packages.restore_migration_snapshot",
      params: {
        schema_version: 1,
        target_id: "com.kosmos.arcadia",
        source_ids: ["arcadia", "arrancador"],
        transaction_token: null,
      },
    });
    expect(await readMigrationJournal(root, journal.target_id)).toBeNull();
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("pending package recovery resolves an active snapshot by source ids", async () => {
  const root = await mkdtemp(path.join(os.tmpdir(), "cortex-recovery-"));
  try {
    await preparePackageState(root, ["arcadia"]);
    const requests: ArkRequest[] = [];
    await recoverLegacyMigrationsBeforeLaunch(root, await recoveryClient(requests));
    expect(requests[0]).toEqual({
      operation: "packages.restore_migration_snapshot",
      params: {
        schema_version: 1,
        target_id: "com.kosmos.arcadia",
        source_ids: ["arcadia"],
        transaction_token: null,
      },
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
