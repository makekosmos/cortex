import { cp, lstat, mkdir, rename, rm } from "node:fs/promises";
import path from "node:path";
import type { ArkClient } from "@kosmos/ark";
import { isRecord, isString, type JsonValue } from "./extension-permissions";
import { extensionUserDataDir } from "./extension-roots";
import { stopExtension } from "./extension-host";
import {
  mergeLegacyExtensionData,
  validateLegacyExtensionDataRoot,
} from "./extension-data-migration";
import {
  LEGACY_TO_CANONICAL,
  type CanonicalId,
  type LegacyMigrationHost,
  readMigrationJournal,
  recoverPreparedMigration,
  runLegacyMigration,
  withLegacyMigrationLock,
} from "./legacy-migration-journal";

interface PackageList {
  packages?: JsonValue;
  truncated?: boolean;
}
interface ReplacementInfo {
  version: string;
  sha256: string;
  catalog_sequence: number;
  wasEnabled: boolean;
}

const LEGACY_IDS = Object.keys(LEGACY_TO_CANONICAL);

function canonicalIds(target: CanonicalId): string[] {
  // SAFETY: Object.keys is sourced exclusively from the literal legacy allowlist.
  return LEGACY_IDS.filter(
    (id) => LEGACY_TO_CANONICAL[id as keyof typeof LEGACY_TO_CANONICAL] === target,
  );
}

function packageRow(value: JsonValue, target: CanonicalId): ReplacementInfo | null {
  if (!isRecord(value) || value.id !== target || !isString(value.version) || !value.version)
    return null;
  if (!isString(value.hash) || !/^[0-9a-f]{64}$/.test(value.hash)) return null;
  if (value.revoked === true || value.compatible === false) return null;
  const sequence = value.catalog_sequence ?? value.catalogSequence;
  // SAFETY: catalog_sequence is a numeric package-summary field at this boundary.
  const numericSequence = sequence as number;
  if (!Number.isSafeInteger(numericSequence) || numericSequence < 0) return null;
  return {
    version: value.version,
    sha256: value.hash,
    catalog_sequence: numericSequence,
    wasEnabled: value.enabled === true,
  };
}

async function existsDirectory(root: string): Promise<boolean> {
  try {
    return (await lstat(root)).isDirectory();
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return false;
    throw error;
  }
}

async function copyNamespace(source: string, destination: string): Promise<void> {
  await mkdir(path.dirname(destination), { recursive: true });
  await cp(source, destination, {
    recursive: true,
    force: true,
    errorOnExist: false,
    preserveTimestamps: true,
  });
}

function migrationRoot(dataDir: string, target: CanonicalId): string {
  return path.join(dataDir, "legacy-migrations", "v1", target);
}

async function sourceIdsWithData(target: CanonicalId): Promise<string[]> {
  const ids: string[] = [];
  for (const id of canonicalIds(target)) {
    const dataRoot = extensionUserDataDir(id);
    const extensionRoot = path.join(path.dirname(path.dirname(dataRoot)), "extensions", id);
    if ((await existsDirectory(dataRoot)) || (await existsDirectory(extensionRoot))) ids.push(id);
  }
  return ids;
}

async function snapshotNamespaces(
  dataDir: string,
  target: CanonicalId,
  sourceIds: readonly string[],
): Promise<void> {
  const root = migrationRoot(dataDir, target);
  await rm(path.join(root, "before"), { recursive: true, force: true });
  await mkdir(path.join(root, "before"), { recursive: true });
  const destination = extensionUserDataDir(target);
  if (await existsDirectory(destination))
    await copyNamespace(destination, path.join(root, "before", "destination"));
  for (const id of sourceIds) {
    const source = extensionUserDataDir(id);
    if (await existsDirectory(source)) {
      await validateLegacyExtensionDataRoot(source);
      await copyNamespace(source, path.join(root, "before", id));
    }
  }
}

async function stageNamespace(
  dataDir: string,
  target: CanonicalId,
  sourceIds: readonly string[],
): Promise<void> {
  const root = migrationRoot(dataDir, target);
  const staged = path.join(root, "staged", "destination");
  await rm(path.join(root, "staged"), { recursive: true, force: true });
  await mkdir(path.dirname(staged), { recursive: true });
  const destination = extensionUserDataDir(target);
  await validateLegacyExtensionDataRoot(destination);
  if (await existsDirectory(destination)) await copyNamespace(destination, staged);
  else await mkdir(staged, { recursive: true });
  await mergeLegacyExtensionData({
    canonicalId: target,
    destinationRoot: staged,
    sources: sourceIds.map((id) => ({ id, root: extensionUserDataDir(id) })),
  });
}

async function activateNamespace(dataDir: string, target: CanonicalId): Promise<void> {
  const staged = path.join(migrationRoot(dataDir, target), "staged", "destination");
  const destination = extensionUserDataDir(target);
  const old = `${destination}.migration-old-${process.pid}`;
  await rm(old, { recursive: true, force: true });
  if (await existsDirectory(destination)) await rename(destination, old);
  try {
    await mkdir(path.dirname(destination), { recursive: true });
    await rename(staged, destination);
  } catch (error) {
    if (!(await existsDirectory(destination)) && (await existsDirectory(old)))
      await rename(old, destination);
    throw error;
  }
  await rm(old, { recursive: true, force: true });
}

async function restoreNamespace(dataDir: string, target: CanonicalId): Promise<void> {
  const before = path.join(migrationRoot(dataDir, target), "before", "destination");
  const destination = extensionUserDataDir(target);
  await rm(destination, { recursive: true, force: true });
  if (await existsDirectory(before)) await copyNamespace(before, destination);
  await rm(path.join(migrationRoot(dataDir, target), "staged"), { recursive: true, force: true });
}

interface LegacyMigrationRunner {
  recoverBeforeLaunch(): Promise<void>;
  run(): Promise<void>;
}

export function createLegacyMigrationRunner(
  dataDir: string,
  client: ArkClient,
): LegacyMigrationRunner {
  const request = client.invokeOperation.bind(client);
  // SAFETY: Object.values is sourced exclusively from the canonical allowlist.
  const targets = [...new Set(Object.values(LEGACY_TO_CANONICAL))] as CanonicalId[];

  async function replacement(target: CanonicalId): Promise<ReturnType<typeof packageRow>> {
    const result = await request({ operation: "packages.list", params: { kind: "app" } });
    // SAFETY: invokeOperation returns a JSON-compatible operation payload.
    if (!isRecord(result as JsonValue)) return null;
    // SAFETY: the operation response is the package-list wire object after the record check.
    // SAFETY: the operation response is the package-list wire object after the record check.
    const packageList = result as PackageList;
    if (packageList.truncated === true) return null;
    const rows = Array.isArray(packageList.packages) ? packageList.packages : [];
    return packageRow(rows.find((row) => isRecord(row) && row.id === target) ?? null, target);
  }

  async function host(
    target: CanonicalId,
    ids: string[],
    replacementInfo: ReplacementInfo,
  ): Promise<LegacyMigrationHost> {
    return {
      dataDir,
      journal: {
        schema_version: 1,
        target_id: target,
        source_ids: ids,
        replacement: {
          version: replacementInfo.version,
          sha256: replacementInfo.sha256,
          catalog_sequence: replacementInfo.catalog_sequence,
        },
        phase: "prepared",
        grant_policy: "reconsent",
        records_policy: "opaque-preserve",
      },
      verifyReplacement: async () => (await replacement(target)) !== null,
      stopAffected: async () => {
        const result = await request({ operation: "packages.list", params: { kind: "app" } });
        // SAFETY: invokeOperation returns a JSON-compatible operation payload.
        if (isRecord(result as JsonValue)) {
          // SAFETY: the operation response is the package-list wire object after the record check.
          const packageList = result as PackageList;
          if (packageList.truncated === true) {
            throw new Error(`package list truncated while stopping '${target}'`);
          }
          if (Array.isArray(packageList.packages)) {
            for (const row of packageList.packages) {
              if (
                isRecord(row) &&
                row.id === target &&
                isString(row.version) &&
                row.version !== replacementInfo.version &&
                row.enabled === true
              ) {
                await request({
                  operation: "packages.set_enabled",
                  params: { id: target, version: row.version, enabled: false },
                });
              }
            }
          }
        }
        await request({
          operation: "packages.set_enabled",
          params: { id: target, version: replacementInfo.version, enabled: false },
        });
      },
      snapshotBefore: () => snapshotNamespaces(dataDir, target, ids),
      stageDestination: () => stageNamespace(dataDir, target, ids),
      revokeLegacyGrants: async () => {
        await request({ operation: "packages.revoke_legacy_grants", params: { source_ids: ids } });
      },
      activateCanonical: async () => {
        await activateNamespace(dataDir, target);
        await request({
          operation: "packages.set_enabled",
          params: { id: target, version: replacementInfo.version, enabled: true },
        });
      },
      restoreBefore: async () => {
        await restoreNamespace(dataDir, target);
        await request({
          operation: "packages.set_enabled",
          params: {
            id: target,
            version: replacementInfo.version,
            enabled: replacementInfo.wasEnabled,
          },
        });
      },
    };
  }

  async function recoverBeforeLaunch(): Promise<void> {
    await recoverLegacyMigrationsBeforeLaunch(dataDir);
  }

  async function run(): Promise<void> {
    await recoverBeforeLaunch();
    for (const target of targets) {
      const ids = await sourceIdsWithData(target);
      if (!ids.length) continue;
      const info = await replacement(target);
      if (!info) continue;
      await request({
        operation: "packages.validate_legacy_grants",
        params: { source_ids: ids },
      });
      const outcome = await withLegacyMigrationLock(dataDir, target, async () => {
        for (const id of ids) await stopExtension(id);
        return runLegacyMigration(await host(target, ids, info));
      });
      if (outcome === "pending") continue;
    }
  }

  return { recoverBeforeLaunch, run };
}

export async function recoverLegacyMigrationsBeforeLaunch(dataDir: string): Promise<void> {
  // SAFETY: Object.values is sourced exclusively from the canonical allowlist above.
  const targets = [...new Set(Object.values(LEGACY_TO_CANONICAL))] as CanonicalId[];
  for (const target of targets) {
    const journal = await readMigrationJournal(dataDir, target);
    if (journal?.phase === "prepared") {
      await recoverPreparedMigration(dataDir, target, () => restoreNamespace(dataDir, target));
    }
  }
}
