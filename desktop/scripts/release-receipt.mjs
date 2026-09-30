import { lstat, readFile, stat } from "node:fs/promises";
import path from "node:path";
import { bytes, canonical, documentHash, integer, sha256, writeAtomic } from "./release-utils.mjs";

export const RELEASE_RECEIPT_SCHEMA_VERSION = 2;
export const RELEASE_RECEIPT_FILE = `release-receipt.v${RELEASE_RECEIPT_SCHEMA_VERSION}.json`;

// Everything the receipt pins comes from the derived BOM, so a receipt built
// from a different commit, version, or toolchain can never pass as current.
export function receiptInputs({ platform, version, currentCommit, bom }) {
  return {
    platform,
    version,
    commit: currentCommit,
    pins: structuredClone({
      bom_id: bom.value.id,
      toolchain: bom.value.source.toolchain,
      compatibility: bom.value.compatibility,
    }),
    bom: { sha256: bom.digest },
  };
}

function relativeArtifact(outputDir, file) {
  const relative = path.relative(outputDir, file);
  if (
    !relative ||
    path.isAbsolute(relative) ||
    relative === ".." ||
    relative.startsWith(`..${path.sep}`)
  )
    throw new Error(`artifact must be inside release output: ${file}`);
  return normalizeArtifactPath(relative.split(path.sep).join("/"));
}

export function normalizeArtifactPath(value) {
  if (
    Object.prototype.toString.call(value) !== "[object String]" ||
    !value ||
    value.includes("\\") ||
    path.posix.isAbsolute(value) ||
    path.posix.normalize(value) !== value ||
    value.includes("/")
  )
    throw new Error(`artifact path is not canonical: ${value}`);
  return value;
}

export async function createReceipt({ outputDir, platform, version, currentCommit, bom, files }) {
  const artifacts = [];
  for (const file of files) {
    const info = await stat(file);
    if (!info.isFile()) throw new Error(`artifact is not a file: ${file}`);
    const data = await readFile(file);
    artifacts.push({
      path: relativeArtifact(outputDir, file),
      size: info.size,
      sha256: documentHash(data),
    });
  }
  if (artifacts.length === 0) throw new Error("verification receipt needs artifacts");
  return {
    schema_version: RELEASE_RECEIPT_SCHEMA_VERSION,
    inputs: receiptInputs({ platform, version, currentCommit, bom }),
    artifacts,
  };
}

export async function writeReceipt(file, receipt) {
  await writeAtomic(file, bytes(receipt));
  return receipt;
}

export async function readReceipt(file) {
  const value = JSON.parse(await readFile(file, "utf8"));
  if (value?.schema_version !== RELEASE_RECEIPT_SCHEMA_VERSION)
    throw new Error(`receipt schema_version must be ${RELEASE_RECEIPT_SCHEMA_VERSION}`);
  if (!value.inputs || !Array.isArray(value.artifacts) || value.artifacts.length === 0)
    throw new Error("receipt is incomplete");
  return value;
}

export function assertReceiptMatchesBom(receipt, expected) {
  const actual = receiptInputs(expected);
  if (JSON.stringify(canonical(receipt.inputs)) !== JSON.stringify(canonical(actual)))
    throw new Error("receipt commit, pins, or BOM identity are stale");
  return receipt;
}

export function assertExactArtifactSet(receipt, expectedPaths) {
  const actual = receipt.artifacts.map(({ path: file }) => normalizeArtifactPath(file)).sort();
  const expected = expectedPaths.map(normalizeArtifactPath).sort();
  if (actual.length !== expected.length || actual.some((file, index) => file !== expected[index]))
    throw new Error("receipt artifact set is incomplete or contains unexpected files");
}

export async function verifyReceiptArtifacts(receipt, outputDir) {
  const root = path.resolve(outputDir);
  const seen = new Set();
  const verified = [];
  for (const [index, artifact] of receipt.artifacts.entries()) {
    if (!artifact || Object.prototype.toString.call(artifact.path) !== "[object String]")
      throw new Error(`receipt artifact ${index} has an unsafe path`);
    const normalizedPath = normalizeArtifactPath(artifact.path);
    const file = path.resolve(root, normalizedPath);
    if (path.relative(root, file).startsWith(`..${path.sep}`) || seen.has(normalizedPath))
      throw new Error(`receipt artifact ${index} has an unsafe or duplicate path`);
    seen.add(normalizedPath);
    sha256(artifact.sha256, `receipt.artifacts[${index}].sha256`);
    integer(artifact.size, `receipt.artifacts[${index}].size`);
    let info;
    try {
      info = await lstat(file);
    } catch {
      throw new Error(`receipt artifact missing: ${normalizedPath}`);
    }
    if (!info.isFile() || info.isSymbolicLink() || info.size !== artifact.size)
      throw new Error(`receipt artifact size/mutation mismatch: ${normalizedPath}`);
    if (documentHash(await readFile(file)) !== artifact.sha256)
      throw new Error(`receipt artifact hash/mutation mismatch: ${normalizedPath}`);
    verified.push({ ...artifact, path: normalizedPath, file });
  }
  return verified;
}
