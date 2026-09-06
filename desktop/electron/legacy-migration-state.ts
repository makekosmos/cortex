import { mkdir, open, readFile, rename, rm, stat } from "node:fs/promises";
import path from "node:path";
import type { ArkClient } from "@kosmos/ark";
import { isRecord, isString, type JsonValue } from "./extension-permissions";
import type { CanonicalId } from "./legacy-migration-journal";

export type ArkRequest = Parameters<ArkClient["invokeOperation"]>[0];
export type MigrationRequest = (input: ArkRequest) => Promise<JsonValue>;

export interface ReplacementInfo {
  version: string;
  sha256: string;
  catalog_sequence: number;
  wasEnabled: boolean;
}

export function isBoolean(value: JsonValue): value is boolean {
  return value === true || value === false;
}

interface PackageState {
  version: string;
  enabled: boolean;
}

function migrationRoot(dataDir: string, target: CanonicalId): string {
  return path.join(dataDir, "legacy-migrations", "v1", target);
}

function packageStatePath(dataDir: string, target: CanonicalId): string {
  return path.join(migrationRoot(dataDir, target), "before", "package-state.json");
}

function packageStatePendingPath(dataDir: string, target: CanonicalId): string {
  return path.join(migrationRoot(dataDir, target), "before", "package-state.pending");
}

async function writeDurableJson(filePath: string, value: JsonValue): Promise<void> {
  await mkdir(path.dirname(filePath), { recursive: true });
  const temporary = `${filePath}.${process.pid}.tmp`;
  const handle = await open(temporary, "w", 0o600);
  try {
    await handle.writeFile(`${JSON.stringify(value, null, 2)}\n`, "utf8");
    await handle.sync();
  } finally {
    await handle.close();
  }
  await rename(temporary, filePath);
}

export async function packageRows(request: MigrationRequest): Promise<JsonValue[]> {
  const result = await request({ operation: "packages.list", params: { kind: "app" } });
  if (!isRecord(result) || result.truncated === true || !Array.isArray(result.packages))
    throw new Error("package list is invalid or truncated");
  return result.packages;
}

export async function verifiedReplacement(
  request: MigrationRequest,
  rows: readonly JsonValue[],
  target: CanonicalId,
): Promise<ReplacementInfo | null> {
  const value = rows.find((row) => isRecord(row) && row.id === target);
  if (!isRecord(value) || value.kind !== "app" || value.revoked !== false) return null;
  if (!isString(value.version) || !value.version || !isString(value.hash)) return null;
  if (!/^[0-9a-f]{64}$/.test(value.hash) || !isBoolean(value.enabled)) return null;
  const sequence = value.catalog_sequence ?? value.catalogSequence;
  // SAFETY: Number.isSafeInteger rejects non-number JSON values at this boundary.
  const numericSequence = sequence as number;
  if (!Number.isSafeInteger(numericSequence) || numericSequence < 0) return null;
  const verified = await request({
    operation: "packages.verify_replacement",
    params: {
      id: target,
      version: value.version,
      hash: value.hash,
      catalog_sequence: numericSequence,
    },
  });
  if (
    !isRecord(verified) ||
    verified.id !== target ||
    verified.version !== value.version ||
    verified.hash !== value.hash ||
    verified.catalog_sequence !== numericSequence
  )
    return null;
  return {
    version: value.version,
    sha256: value.hash,
    catalog_sequence: numericSequence,
    wasEnabled: value.enabled,
  };
}

function packageState(value: JsonValue, target: CanonicalId): PackageState | null {
  if (
    !isRecord(value) ||
    value.id !== target ||
    !isString(value.version) ||
    !value.version ||
    !isBoolean(value.enabled)
  )
    return null;
  return { version: value.version, enabled: value.enabled };
}

function targetPackageStates(rows: readonly JsonValue[], target: CanonicalId): PackageState[] {
  return rows
    .filter((row) => isRecord(row) && row.id === target)
    .map((row) => {
      const state = packageState(row, target);
      if (!state) throw new Error(`package state response is invalid for '${target}'`);
      return state;
    });
}

export async function snapshotPackageState(
  dataDir: string,
  target: CanonicalId,
  request: MigrationRequest,
): Promise<void> {
  const states = targetPackageStates(await packageRows(request), target);
  if (states.filter((state) => state.enabled).length > 1)
    throw new Error(`more than one enabled package version for '${target}'`);
  await writeDurableJson(
    packageStatePath(dataDir, target),
    states.map((state) => ({ version: state.version, enabled: state.enabled })),
  );
  await writeDurableJson(packageStatePendingPath(dataDir, target), true);
}

export async function packageStateSnapshotPending(
  dataDir: string,
  target: CanonicalId,
): Promise<boolean> {
  try {
    await stat(packageStatePendingPath(dataDir, target));
    return true;
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return false;
    throw error;
  }
}

export async function clearPackageStateSnapshotPending(
  dataDir: string,
  target: CanonicalId,
): Promise<void> {
  await rm(packageStatePendingPath(dataDir, target), { force: true });
}

async function readPackageState(dataDir: string, target: CanonicalId): Promise<PackageState[]> {
  // SAFETY: the file is the host-owned JSON snapshot written by writeDurableJson.
  const value = JSON.parse(await readFile(packageStatePath(dataDir, target), "utf8")) as JsonValue;
  if (!Array.isArray(value)) throw new Error("package state snapshot is invalid");
  const states = value.map((entry) => {
    if (!isRecord(entry) || !isString(entry.version) || !entry.version || !isBoolean(entry.enabled))
      throw new Error("package state snapshot is invalid");
    return { version: entry.version, enabled: entry.enabled };
  });
  if (states.filter((state) => state.enabled).length > 1)
    throw new Error(`more than one enabled package version for '${target}'`);
  return states;
}

export async function restorePackageState(
  dataDir: string,
  target: CanonicalId,
  request: MigrationRequest,
): Promise<void> {
  const states = await readPackageState(dataDir, target);
  for (const state of targetPackageStates(await packageRows(request), target)) {
    if (state.enabled)
      await request({
        operation: "packages.set_enabled",
        params: { id: target, version: state.version, enabled: false },
      });
  }
  for (const state of states.filter((candidate) => candidate.enabled))
    await request({
      operation: "packages.set_enabled",
      params: { id: target, version: state.version, enabled: true },
    });
  const restored = targetPackageStates(await packageRows(request), target)
    .filter((state) => state.enabled)
    .map((state) => state.version);
  const expected = states.filter((state) => state.enabled).map((state) => state.version);
  if (
    restored.length !== expected.length ||
    restored.some((version) => !expected.includes(version))
  )
    throw new Error(`package worker state recovery failed for '${target}'`);
  await clearPackageStateSnapshotPending(dataDir, target);
}
