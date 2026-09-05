import { mkdir, open, readFile, rename } from "node:fs/promises";
import path from "node:path";
import type { ArkClient } from "@kosmos/ark";
import { isRecord, isString, type JsonValue } from "./extension-permissions";
import type { CanonicalId } from "./legacy-migration-journal";

export type ArkRequest = Parameters<ArkClient["invokeOperation"]>[0];
export type MigrationRequest = (input: ArkRequest) => Promise<JsonValue>;

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
}
