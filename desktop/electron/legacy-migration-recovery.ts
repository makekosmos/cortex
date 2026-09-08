import type { ArkClient } from "@kosmos/ark";
import type { JsonValue } from "./extension-permissions";
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
  type MigrationRequest,
} from "./legacy-migration-state";

type NamespaceRestorer = (dataDir: string, target: CanonicalId) => Promise<void>;
type SourceIdsReader = (dataDir: string, target: CanonicalId) => Promise<string[]>;
type MigrationClient = Pick<ArkClient, "invokeOperation">;

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
        await request({
          operation: "packages.rollback_legacy_grants",
          params: journal.grant_transaction_token
            ? { transaction_token: journal.grant_transaction_token }
            : { source_ids: journal.source_ids },
        });
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
        operation: "packages.rollback_legacy_grants",
        params: { source_ids: sourceIds },
      });
      await restoreNamespace(dataDir, target);
      await restorePackageState(dataDir, target, request);
    }
  }
}
