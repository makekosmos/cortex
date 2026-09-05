import { mkdir, open, readFile, rename, rm, stat, writeFile } from "node:fs/promises";
import path from "node:path";

export const LEGACY_TO_CANONICAL = {
  arcadia: "com.kosmos.arcadia",
  arrancador: "com.kosmos.arcadia",
  eden: "com.kosmos.memoria",
  delphi: "com.kosmos.agenda",
} as const;

export type CanonicalId = (typeof LEGACY_TO_CANONICAL)[keyof typeof LEGACY_TO_CANONICAL];
export type MigrationPhase = "prepared" | "committed";

export interface MigrationJournal {
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

function fail(message: string): never {
  throw new Error(`[kepler-shell] invalid legacy migration journal: ${message}`);
}

function exactKeys(value: Record<string, unknown>, allowed: Set<string>, label: string): void {
  for (const key of Object.keys(value)) if (!allowed.has(key)) fail(`${label} contains unknown field`);
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function assertCanonical(value: unknown): asserts value is CanonicalId {
  if (typeof value !== "string" || !TARGETS.has(value)) fail("target_id is not allowlisted");
}

export function validateMigrationJournal(value: unknown): MigrationJournal {
  if (!isRecord(value)) fail("journal must be an object");
  exactKeys(value, JOURNAL_KEYS, "journal");
  if (value.schema_version !== 1) fail("unsupported schema_version");
  assertCanonical(value.target_id);
  if (
    !Array.isArray(value.source_ids) ||
    value.source_ids.length === 0 ||
    value.source_ids.some((id) => typeof id !== "string" || !IDS.has(id))
  ) {
    fail("source_ids is invalid");
  }
  const sourceIds = value.source_ids as string[];
  if (new Set(sourceIds).size !== sourceIds.length) fail("source_ids contains duplicates");
  if (sourceIds.some((id) => LEGACY_TO_CANONICAL[id as keyof typeof LEGACY_TO_CANONICAL] !== value.target_id)) {
    fail("source_ids do not match target_id");
  }
  if (!isRecord(value.replacement)) fail("replacement is invalid");
  exactKeys(value.replacement, REPLACEMENT_KEYS, "replacement");
  if (
    typeof value.replacement.version !== "string" ||
    value.replacement.version.length === 0 ||
    !/^[0-9a-f]{64}$/.test(String(value.replacement.sha256)) ||
    !Number.isSafeInteger(value.replacement.catalog_sequence) ||
    value.replacement.catalog_sequence < 0
  ) {
    fail("replacement is invalid");
  }
  if (value.phase !== "prepared" && value.phase !== "committed") fail("phase is invalid");
  if (value.grant_policy !== "reconsent") fail("grant_policy is invalid");
  if (value.records_policy !== "opaque-preserve" && value.records_policy !== "explicit-adapter-v1") {
    fail("records_policy is invalid");
  }
  return value as MigrationJournal;
}

export function migrationJournalPath(dataDir: string, canonicalId: CanonicalId): string {
  if (!path.isAbsolute(dataDir) || !TARGETS.has(canonicalId)) fail("journal path is invalid");
  return path.join(path.resolve(dataDir), "legacy-migrations", "v1", canonicalId, "journal.json");
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

export async function writeMigrationJournal(dataDir: string, journal: MigrationJournal): Promise<void> {
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
    return validateMigrationJournal(JSON.parse(await readFile(filePath, "utf8")) as unknown);
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return null;
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
  return true;
}

export async function migrationJournalExists(dataDir: string, canonicalId: CanonicalId): Promise<boolean> {
  try {
    await stat(migrationJournalPath(dataDir, canonicalId));
    return true;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return false;
    throw error;
  }
}
