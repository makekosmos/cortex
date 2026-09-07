import { mkdir, open, readFile, rename, rm, stat } from "node:fs/promises";
import { readFileSync } from "node:fs";
import path from "node:path";
import { isRecord, isString, type JsonRecord, type JsonValue } from "./extension-permissions";
import { testMigrationBarrier } from "./test-migration-barrier";

export const LEGACY_TO_CANONICAL = {
  arcadia: "com.kosmos.arcadia",
  arrancador: "com.kosmos.arcadia",
  eden: "com.kosmos.memoria",
  delphi: "com.kosmos.agenda",
} as const;

export type CanonicalId = (typeof LEGACY_TO_CANONICAL)[keyof typeof LEGACY_TO_CANONICAL];
export type MigrationPhase = "prepared" | "committed";
export interface MigrationJournal extends JsonRecord {
  schema_version: 1;
  target_id: CanonicalId;
  source_ids: string[];
  replacement: { version: string; sha256: string; catalog_sequence: number };
  phase: MigrationPhase;
  grant_policy: "reconsent";
  records_policy: "opaque-preserve" | "explicit-adapter-v1";
}

const JOURNAL_KEYS = new Set([
  "schema_version",
  "target_id",
  "source_ids",
  "replacement",
  "phase",
  "grant_policy",
  "records_policy",
]);
const REPLACEMENT_KEYS = new Set(["version", "sha256", "catalog_sequence"]);
const IDS = new Set(Object.keys(LEGACY_TO_CANONICAL));
const TARGETS = new Set(Object.values(LEGACY_TO_CANONICAL));
const ACTIVE_MIGRATIONS = new Set<string>();
function fail(message: string): never {
  throw new Error(`[kepler-shell] invalid legacy migration journal: ${message}`);
}

function exactKeys(value: JsonRecord, allowed: Set<string>, label: string): void {
  for (const key of Object.keys(value))
    if (!allowed.has(key)) fail(`${label} contains unknown field`);
}

function assertCanonical(value: JsonValue): asserts value is CanonicalId {
  // SAFETY: isString narrows the value before the allowlist lookup.
  if (!isString(value) || !TARGETS.has(value as CanonicalId)) fail("target_id is not allowlisted");
}

export function validateMigrationJournal(value: JsonValue): MigrationJournal {
  if (!isRecord(value)) fail("journal must be an object");
  exactKeys(value, JOURNAL_KEYS, "journal");
  if (value.schema_version !== 1) fail("unsupported schema_version");
  assertCanonical(value.target_id);
  const targetId = value.target_id;
  if (
    !Array.isArray(value.source_ids) ||
    value.source_ids.length === 0 ||
    value.source_ids.some((id) => !isString(id) || !IDS.has(id))
  ) {
    fail("source_ids is invalid");
  }
  const sourceIds = value.source_ids.filter(isString);
  if (sourceIds.length !== value.source_ids.length) fail("source_ids is invalid");
  if (new Set(sourceIds).size !== sourceIds.length) fail("source_ids contains duplicates");
  if (
    sourceIds.some((id) => {
      // SAFETY: source_ids was checked against the legacy allowlist immediately above.
      return LEGACY_TO_CANONICAL[id as keyof typeof LEGACY_TO_CANONICAL] !== targetId;
    })
  ) {
    fail("source_ids do not match target_id");
  }
  if (!isRecord(value.replacement)) fail("replacement is invalid");
  exactKeys(value.replacement, REPLACEMENT_KEYS, "replacement");
  const version = value.replacement.version;
  const catalogSequence = value.replacement.catalog_sequence;
  if (
    !isString(version) ||
    version.length === 0 ||
    !/^[0-9a-f]{64}$/.test(String(value.replacement.sha256))
  ) {
    fail("replacement is invalid");
  }
  // SAFETY: the journal schema requires catalog_sequence to be a number.
  if (!Number.isSafeInteger(catalogSequence as number) || (catalogSequence as number) < 0)
    fail("replacement is invalid");
  if (value.phase !== "prepared" && value.phase !== "committed") fail("phase is invalid");
  if (value.grant_policy !== "reconsent") fail("grant_policy is invalid");
  if (
    value.records_policy !== "opaque-preserve" &&
    value.records_policy !== "explicit-adapter-v1"
  ) {
    fail("records_policy is invalid");
  }
  const phase = value.phase;
  const recordsPolicy = value.records_policy;
  // SAFETY: the literal checks above establish the domain values used below.
  return {
    schema_version: 1,
    target_id: targetId,
    source_ids: sourceIds,
    replacement: {
      version,
      sha256: String(value.replacement.sha256),
      catalog_sequence: catalogSequence as number,
    },
    phase,
    grant_policy: "reconsent",
    records_policy: recordsPolicy,
  };
}

export function migrationJournalPath(dataDir: string, canonicalId: CanonicalId): string {
  if (!path.isAbsolute(dataDir) || !TARGETS.has(canonicalId)) fail("journal path is invalid");
  return path.join(path.resolve(dataDir), "legacy-migrations", "v1", canonicalId, "journal.json");
}
export function isLegacyLaunchBlocked(dataDir: string, id: string): boolean {
  // SAFETY: the lookup is constrained to the literal legacy allowlist.
  const target = LEGACY_TO_CANONICAL[id as keyof typeof LEGACY_TO_CANONICAL];
  if (!target) return false;
  try {
    // SAFETY: JSON.parse is validated by the committed-phase checks below before it affects launch policy.
    const value = JSON.parse(
      readFileSync(migrationJournalPath(dataDir, target), "utf8"),
    ) as JsonValue;
    const journal = validateMigrationJournal(value);
    return (
      (journal.phase === "prepared" || journal.phase === "committed") &&
      journal.target_id === target
    );
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return false;
    return true;
  }
}
export function assertLegacyLaunchAllowed(dataDir: string, id: string): void {
  if (isLegacyMigrationActive(dataDir, id))
    throw new Error("[kepler-shell] legacy package is being migrated");
  if (isLegacyLaunchBlocked(dataDir, id))
    throw new Error(`[kepler-shell] legacy package is disabled after migration: ${id}`);
}

export function isLegacyMigrationActive(dataDir: string, id: string): boolean {
  // SAFETY: the lookup is constrained to the literal legacy allowlist.
  const target = LEGACY_TO_CANONICAL[id as keyof typeof LEGACY_TO_CANONICAL];
  if (!target) return false;
  return ACTIVE_MIGRATIONS.has(migrationJournalPath(dataDir, target));
}

export function legacyMigrationTarget(id: string): CanonicalId | null {
  // SAFETY: the lookup is constrained to the literal legacy allowlist.
  const target = LEGACY_TO_CANONICAL[id as keyof typeof LEGACY_TO_CANONICAL];
  return target ?? null;
}

export async function withLegacyMigrationLock<T>(
  dataDir: string,
  canonicalId: CanonicalId,
  operation: () => Promise<T>,
): Promise<T> {
  const key = migrationJournalPath(dataDir, canonicalId);
  if (ACTIVE_MIGRATIONS.has(key)) throw new Error("[kepler-shell] legacy migration already active");
  ACTIVE_MIGRATIONS.add(key);
  try {
    return await operation();
  } finally {
    ACTIVE_MIGRATIONS.delete(key);
  }
}

async function durableWrite(filePath: string, bytes: Buffer): Promise<void> {
  await mkdir(path.dirname(filePath), { recursive: true });
  const temp = `${filePath}.${process.pid}.tmp`;
  const handle = await open(temp, "w", 0o600);
  try {
    await handle.writeFile(bytes);
    await handle.sync();
  } finally {
    await handle.close();
  }
  await rename(temp, filePath);
}

export async function writeMigrationJournal(
  dataDir: string,
  journal: MigrationJournal,
): Promise<void> {
  const valid = validateMigrationJournal(journal);
  const filePath = migrationJournalPath(dataDir, valid.target_id);
  await durableWrite(filePath, Buffer.from(`${JSON.stringify(valid, null, 2)}\n`, "utf8"));
}

export async function readMigrationJournal(
  dataDir: string,
  canonicalId: CanonicalId,
): Promise<MigrationJournal | null> {
  const filePath = migrationJournalPath(dataDir, canonicalId);
  try {
    // SAFETY: validateMigrationJournal performs the complete journal schema validation.
    return validateMigrationJournal(JSON.parse(await readFile(filePath, "utf8")) as JsonValue);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
    throw error;
  }
}

export async function recoverPreparedMigration(
  dataDir: string,
  canonicalId: CanonicalId,
  restoreBefore: () => Promise<void>,
): Promise<boolean> {
  const journal = await readMigrationJournal(dataDir, canonicalId);
  if (!journal || journal.phase !== "prepared") return false;
  await restoreBefore();
  const journalPath = migrationJournalPath(dataDir, canonicalId);
  await rm(journalPath, { force: true });
  await rm(migrationFinalizationPath(dataDir, canonicalId), { force: true });
  return true;
}

export function migrationFinalizationPath(dataDir: string, canonicalId: CanonicalId): string {
  return `${migrationJournalPath(dataDir, canonicalId)}.finalizing`;
}

export async function migrationJournalExists(
  dataDir: string,
  canonicalId: CanonicalId,
): Promise<boolean> {
  try {
    await stat(migrationJournalPath(dataDir, canonicalId));
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return false;
    throw error;
  }
}

export interface LegacyMigrationHost {
  dataDir: string;
  journal: MigrationJournal;
  verifyReplacement: () => Promise<boolean>;
  stopAffected: () => Promise<void>;
  snapshotBefore: () => Promise<void>;
  stageDestination: () => Promise<void>;
  revokeLegacyGrants: () => Promise<void>;
  commitLegacyGrants?: () => Promise<void>;
  activateCanonical: () => Promise<void>;
  restoreBefore: () => Promise<void>;
  writeJournal?: (journal: MigrationJournal) => Promise<void>;
}

export async function runLegacyMigration(
  host: LegacyMigrationHost,
): Promise<"committed" | "pending" | "recovered"> {
  const current = await readMigrationJournal(host.dataDir, host.journal.target_id);
  if (current?.phase === "committed") return "committed";
  if (current?.phase === "prepared") {
    await recoverPreparedMigration(host.dataDir, host.journal.target_id, host.restoreBefore);
    return "recovered";
  }
  if (!(await host.verifyReplacement())) return "pending";

  let recoveryRequired = false;
  const write =
    host.writeJournal ??
    ((journal: MigrationJournal) => writeMigrationJournal(host.dataDir, journal));
  try {
    recoveryRequired = true;
    await host.stopAffected();
    await host.snapshotBefore();
    await host.stageDestination();
    await write({ ...host.journal, phase: "prepared" });
    await host.revokeLegacyGrants();
    await host.activateCanonical();
    await testMigrationBarrier("prepared", host.journal.target_id);
    await durableWrite(
      migrationFinalizationPath(host.dataDir, host.journal.target_id),
      Buffer.from("finalizing\n", "utf8"),
    );
    await host.commitLegacyGrants?.();
    await write({ ...host.journal, phase: "committed" });
    await testMigrationBarrier("committed", host.journal.target_id);
    await rm(migrationFinalizationPath(host.dataDir, host.journal.target_id), { force: true });
    return "committed";
  } catch (error) {
    if (recoveryRequired) {
      const prepared = await readMigrationJournal(host.dataDir, host.journal.target_id);
      if (prepared?.phase === "prepared") {
        await recoverPreparedMigration(host.dataDir, host.journal.target_id, host.restoreBefore);
      } else if (!prepared) {
        await host.restoreBefore();
      }
    }
    throw error;
  }
}
