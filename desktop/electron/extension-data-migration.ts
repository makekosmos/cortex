import {
  copyFile,
  lstat,
  mkdir,
  readdir,
  readFile,
  realpath,
  utimes,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { isRecord, isString, type JsonRecord, type JsonValue } from "./json-types";

export interface LegacyExtensionDataSource {
  id: string;
  root: string;
}

export interface ExtensionDataMergeOptions {
  canonicalId: string;
  destinationRoot: string;
  sources: readonly LegacyExtensionDataSource[];
}

export interface ExtensionDataMergeResult {
  copiedFiles: number;
  mergedJsonFiles: number;
}

const LEGACY_TO_CANONICAL = new Map<string, string>([
  ["arcadia", "com.kosmos.arcadia"],
  ["arrancador", "com.kosmos.arcadia"],
  ["eden", "com.kosmos.memoria"],
  ["delphi", "com.kosmos.agenda"],
]);

const ARCADIA_SOURCE_ORDER = new Map([
  ["arcadia", 0],
  ["arrancador", 1],
]);

type FileStats = Awaited<ReturnType<typeof lstat>>;

function fail(message: string): never {
  throw new Error(`[kepler-shell] unsafe extension data migration: ${message}`);
}

function isWithin(root: string, candidate: string): boolean {
  const relative = path.relative(root, candidate);
  return (
    relative === "" ||
    (relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative))
  );
}

function assertAbsoluteRoot(root: string, label: string): string {
  if (!path.isAbsolute(root) || root.includes("\0")) {
    fail(`${label} must be an absolute path`);
  }
  return path.resolve(root);
}

async function lstatIfPresent(filePath: string): Promise<FileStats | null> {
  try {
    return await lstat(filePath);
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return null;
    throw error;
  }
}

async function assertSafeExistingPath(filePath: string, root: string): Promise<FileStats> {
  if (!isWithin(root, filePath)) fail(`path escapes namespace: ${filePath}`);
  const stats = await lstat(filePath);
  if (stats.isSymbolicLink()) fail(`symlink or junction is not allowed: ${filePath}`);
  if (!stats.isDirectory() && !stats.isFile()) fail(`unsupported filesystem entry: ${filePath}`);
  return stats;
}

async function assertSafeRootPath(root: string, label: string): Promise<void> {
  const parsed = path.parse(root);
  let current = parsed.root;
  const segments = path.relative(parsed.root, root).split(path.sep).filter(Boolean);
  for (const segment of segments) {
    current = path.join(current, segment);
    const stats = await lstatIfPresent(current);
    if (!stats) continue;
    if (stats.isSymbolicLink()) fail(`${label} contains a symlink or junction: ${current}`);
    if (!stats.isDirectory()) fail(`${label} is not a directory: ${current}`);
  }

  const existing = await lstatIfPresent(root);
  if (existing?.isSymbolicLink()) fail(`${label} is a symlink or junction`);
  if (existing && !existing.isDirectory()) fail(`${label} is not a directory`);
  if (existing) {
    const resolved = await realpath(root);
    if (!isWithin(root, resolved) || !isWithin(resolved, root)) {
      fail(`${label} resolves outside its declared root`);
    }
  }
}

async function validateTree(root: string): Promise<void> {
  const rootStats = await lstatIfPresent(root);
  if (!rootStats) return;
  await assertSafeExistingPath(root, root);
  for (const entry of await readdir(root)) {
    const entryPath = path.resolve(root, entry);
    const stats = await assertSafeExistingPath(entryPath, root);
    if (stats.isDirectory()) await validateTree(entryPath);
  }
}

export async function validateLegacyExtensionDataRoot(root: string): Promise<void> {
  const absolute = assertAbsoluteRoot(root, "extension data root");
  await assertSafeRootPath(absolute, "extension data root");
  await validateTree(absolute);
}

async function ensureDirectory(root: string, relative: string): Promise<void> {
  const directory = path.resolve(root, relative);
  if (!isWithin(root, directory)) fail(`path escapes namespace: ${directory}`);
  const parts = path.relative(root, directory).split(path.sep).filter(Boolean);
  let current = root;
  for (const part of parts) {
    current = path.join(current, part);
    const existing = await lstatIfPresent(current);
    if (!existing) {
      await mkdir(current);
      continue;
    }
    if (existing.isSymbolicLink() || !existing.isDirectory()) {
      fail(`destination path is unsafe: ${current}`);
    }
  }
}

function cloneJson(value: JsonValue): JsonValue {
  if (Array.isArray(value)) return value.map(cloneJson);
  if (!isRecord(value)) return value;
  // SAFETY: Object.create(null) is used as a JSON object with no inherited keys.
  const clone = Object.create(null) as JsonRecord;
  for (const [key, child] of Object.entries(value)) {
    Object.defineProperty(clone, key, {
      configurable: true,
      enumerable: true,
      value: cloneJson(child),
      writable: true,
    });
  }
  return clone;
}

function fillMissingObjectKeys(destination: JsonRecord, source: JsonRecord): boolean {
  let changed = false;
  for (const [key, sourceValue] of Object.entries(source)) {
    if (!Object.prototype.hasOwnProperty.call(destination, key)) {
      Object.defineProperty(destination, key, {
        configurable: true,
        enumerable: true,
        value: cloneJson(sourceValue),
        writable: true,
      });
      changed = true;
      continue;
    }

    const destinationValue = destination[key];
    if (isRecord(destinationValue) && isRecord(sourceValue)) {
      changed = fillMissingObjectKeys(destinationValue, sourceValue) || changed;
    }
  }
  return changed;
}

async function mergeJsonFile(
  sourcePath: string,
  destinationPath: string,
  result: ExtensionDataMergeResult,
): Promise<void> {
  let source: JsonValue;
  let destination: JsonValue;
  try {
    // SAFETY: JSON.parse values are narrowed immediately by isRecord before use.
    source = JSON.parse(await readFile(sourcePath, "utf8")) as JsonValue;
    // SAFETY: JSON.parse values are narrowed immediately by isRecord before use.
    destination = JSON.parse(await readFile(destinationPath, "utf8")) as JsonValue;
  } catch {
    return;
  }
  if (!isRecord(source) || !isRecord(destination)) return;
  if (!fillMissingObjectKeys(destination, source)) return;
  await writeFile(destinationPath, `${JSON.stringify(destination, null, 2)}\n`, "utf8");
  result.mergedJsonFiles += 1;
}

async function mergeTree(
  sourceRoot: string,
  destinationRoot: string,
  relative: string,
  result: ExtensionDataMergeResult,
): Promise<void> {
  const sourceDirectory = path.resolve(sourceRoot, relative);
  const destinationDirectory = path.resolve(destinationRoot, relative);
  await ensureDirectory(destinationRoot, relative);

  for (const entry of await readdir(sourceDirectory)) {
    const sourcePath = path.resolve(sourceDirectory, entry);
    const destinationPath = path.resolve(destinationDirectory, entry);
    const sourceStats = await assertSafeExistingPath(sourcePath, sourceRoot);
    const destinationStats = await lstatIfPresent(destinationPath);

    if (sourceStats.isDirectory()) {
      if (
        destinationStats &&
        (!destinationStats.isDirectory() || destinationStats.isSymbolicLink())
      )
        continue;
      await mergeTree(sourceRoot, destinationRoot, path.join(relative, entry), result);
      continue;
    }

    if (destinationStats) {
      if (destinationStats.isSymbolicLink())
        fail(`destination contains a symlink or junction: ${destinationPath}`);
      if (!destinationStats.isFile()) continue;
      if (path.extname(entry).toLowerCase() === ".json") {
        await mergeJsonFile(sourcePath, destinationPath, result);
      }
      continue;
    }

    await ensureDirectory(destinationRoot, relative);
    await copyFile(sourcePath, destinationPath, 1);
    await utimes(destinationPath, sourceStats.atime, sourceStats.mtime);
    result.copiedFiles += 1;
  }
}

function validateSources(canonicalId: string, sources: readonly LegacyExtensionDataSource[]): void {
  if (!LEGACY_TO_CANONICAL.has("arcadia") && canonicalId === "com.kosmos.arcadia") {
    fail("internal allowlist is incomplete");
  }
  if (!sources.length) fail("at least one legacy source is required");
  const seen = new Set<string>();
  for (const source of sources) {
    if (!source || !isString(source.id) || !isString(source.root)) {
      fail("source must contain an id and root");
    }
    if (seen.has(source.id)) fail(`duplicate source id: ${source.id}`);
    seen.add(source.id);
    if (LEGACY_TO_CANONICAL.get(source.id) !== canonicalId) {
      fail(`source is not allowlisted for ${canonicalId}: ${source.id}`);
    }
  }
}

export async function mergeLegacyExtensionData(
  options: ExtensionDataMergeOptions,
): Promise<ExtensionDataMergeResult> {
  validateSources(options.canonicalId, options.sources);
  const destinationRoot = assertAbsoluteRoot(options.destinationRoot, "destination root");
  const sources = options.sources
    .map((source) => ({
      ...source,
      root: assertAbsoluteRoot(source.root, `source root ${source.id}`),
    }))
    .sort((left, right) => {
      if (options.canonicalId !== "com.kosmos.arcadia") return 0;
      return (ARCADIA_SOURCE_ORDER.get(left.id) ?? 0) - (ARCADIA_SOURCE_ORDER.get(right.id) ?? 0);
    });

  await assertSafeRootPath(destinationRoot, "destination root");
  for (const source of sources) {
    await assertSafeRootPath(source.root, `source root ${source.id}`);
    if (isWithin(destinationRoot, source.root) || isWithin(source.root, destinationRoot)) {
      fail("source and destination namespaces overlap");
    }
  }

  await validateTree(destinationRoot);
  for (const source of sources) await validateTree(source.root);

  if (!(await lstatIfPresent(destinationRoot))) await ensureDirectory(destinationRoot, "");
  const result: ExtensionDataMergeResult = { copiedFiles: 0, mergedJsonFiles: 0 };
  for (const source of sources) {
    if (await lstatIfPresent(source.root))
      await mergeTree(source.root, destinationRoot, "", result);
  }
  return result;
}
