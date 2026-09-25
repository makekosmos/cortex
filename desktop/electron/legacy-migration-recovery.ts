import type { ArkClient } from "@kosmos/ark";
import type { JsonValue } from "./json-types";
import {
  LEGACY_TO_CANONICAL,
  type CanonicalId,
  readMigrationJournal,
  recoverPreparedMigration,
  writeMigrationJournal,
} from "./legacy-migration-journal";
import {
  clearPackageStateSnapshotPending,
  packageStateSnapshotPending,
  restorePackageState,
  type ArkRequest,
  type MigrationRequest,
} from "./legacy-migration-state";

type NamespaceRestorer = (dataDir: string, target: CanonicalId) => Promise<void>;
type SourceIdsReader = (dataDir: string, target: CanonicalId) => Promise<string[]>;
type MigrationClient = Pick<ArkClient, "invokeOperation">;

function migrationSnapshotRequest(journal: {
  target_id: CanonicalId;
  source_ids: string[];
  grant_transaction_token: string | null;
}): ArkRequest {
  return {
    operation: "packages.restore_migration_snapshot",
    params: {
      schema_version: 1,
      target_id: journal.target_id,
      source_ids: journal.source_ids,
      transaction_token: journal.grant_transaction_token,
    },
  };
}

export async function recoverLegacyMigrations(
  dataDir: string,
  client: MigrationClient | undefined,
  restoreNamespace: NamespaceRestorer,
  sourceIdsWithData: SourceIdsReader,
): Promise<void> {
  // SAFETY: Object.values is sourced exclusively from the canonical allowlist.
  const targets = [...new Set(Object.values(LEGACY_TO_CANONICAL))] as CanonicalId[];
  for (const target of targets) {
    const journal = await readMigrationJournal(dataDir, target);
    if (journal?.phase === "finalizing") {
      if (!client || !journal.grant_transaction_token)
        throw new Error("finalizing migration requires grant transaction token and PackageService");
      const request: MigrationRequest = (input) => client.invokeOperation<JsonValue>(input);
      await request({
        operation: "packages.commit_legacy_grants",
        params: { transaction_token: journal.grant_transaction_token },
      });
      await writeMigrationJournal(dataDir, { ...journal, phase: "committed" });
      await clearPackageStateSnapshotPending(dataDir, target);
    } else if (journal?.phase === "prepared") {
      if (!client) {
        await restoreNamespace(dataDir, target);
        continue;
      }
      const request: MigrationRequest = (input) => client.invokeOperation<JsonValue>(input);
      await recoverPreparedMigration(dataDir, target, async () => {
        await request(migrationSnapshotRequest(journal));
        await restoreNamespace(dataDir, target);
        await restorePackageState(dataDir, target, request);
      });
    } else if (journal?.phase === "committed") {
      await clearPackageStateSnapshotPending(dataDir, target);
    } else if (await packageStateSnapshotPending(dataDir, target)) {
      if (!client) throw new Error("package state recovery requires PackageService");
      const request: MigrationRequest = (input) => client.invokeOperation<JsonValue>(input);
      const sourceIds = await sourceIdsWithData(dataDir, target);
      await request({
        operation: "packages.restore_migration_snapshot",
        params: {
          schema_version: 1,
          target_id: target,
          source_ids: sourceIds,
          transaction_token: null,
        },
      });
      await restoreNamespace(dataDir, target);
      await restorePackageState(dataDir, target, request);
    }
  }
}
