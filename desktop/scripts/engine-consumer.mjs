import crypto from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { verifyEngineArchive } from "./engine-distribution.mjs";

const VERSION = /^\d+\.\d+\.\d+$/;
const HASH = /^[a-f0-9]{64}$/i;

export function trustedEngineUrls(version) {
  if (!VERSION.test(version)) throw new Error("engine version must be semver");
  const base = `https://github.com/makekosmos/desktop/releases/download/v${version}/`;
  return {
    manifest: `${base}Kosmos-Engine-manifest.json`,
    archive: `${base}Kosmos-Engine-${version}.zip`,
    installer: `${base}Kosmos-Engine-Setup-${version}.exe`,
  };
}

const sha256 = (value) => crypto.createHash("sha256").update(value).digest("hex");

function validateManifest(manifest, version) {
  const urls = trustedEngineUrls(version);
  if (
    manifest?.schema_version !== 1 ||
    manifest.product !== "kosmos-engine" ||
    manifest.version !== version ||
    manifest.url !== urls.archive ||
    manifest.installer_url !== urls.installer ||
    !HASH.test(manifest.archive_sha256) ||
    !Number.isSafeInteger(manifest.archive_size) ||
    manifest.archive_size < 1 ||
    !HASH.test(manifest.installer_sha256) ||
    !Number.isSafeInteger(manifest.installer_size) ||
    manifest.installer_size < 1
  )
    throw new Error("invalid pinned Engine manifest");
  return manifest;
}

async function bytesFrom(fetchImpl, url) {
  const response = await fetchImpl(url);
  if (!response.ok) throw new Error(`Engine artifact download failed: ${url}`);
  return Buffer.from(await response.arrayBuffer());
}

function assertArtifact(bytes, expectedHash, expectedSize, label) {
  if (bytes.length !== expectedSize || sha256(bytes) !== expectedHash.toLowerCase())
    throw new Error(`Engine ${label} hash or size mismatch`);
}

export async function consumeEngineArtifacts({ version, targetDir, reuse, fetchImpl = fetch }) {
  let manifest;
  let archive;
  let installer;
  if (reuse) {
    if (!reuse.manifest || !reuse.archive || !reuse.installer)
      throw new Error("Engine reuse requires manifest, archive, and installer");
    manifest = JSON.parse(fs.readFileSync(reuse.manifest, "utf8"));
  } else {
    const urls = trustedEngineUrls(version);
    manifest = JSON.parse((await bytesFrom(fetchImpl, urls.manifest)).toString("utf8"));
  }
  validateManifest(manifest, version);
  if (reuse) {
    archive = fs.readFileSync(reuse.archive);
    installer = fs.readFileSync(reuse.installer);
  } else {
    archive = await bytesFrom(fetchImpl, manifest.url);
    installer = await bytesFrom(fetchImpl, manifest.installer_url);
  }
  assertArtifact(archive, manifest.archive_sha256, manifest.archive_size, "archive");
  assertArtifact(installer, manifest.installer_sha256, manifest.installer_size, "installer");
  verifyEngineArchiveBytes(archive, manifest);
  fs.mkdirSync(targetDir, { recursive: true });
  fs.writeFileSync(path.join(targetDir, "Kosmos-Engine.zip"), archive);
  fs.writeFileSync(path.join(targetDir, `Kosmos-Engine-Setup-${version}.exe`), installer);
  fs.writeFileSync(
    path.join(targetDir, "engine-manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
  return manifest;
}

function verifyEngineArchiveBytes(bytes, manifest) {
  const archive = path.join(
    fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-engine-consumer-")),
    "engine.zip",
  );
  try {
    fs.writeFileSync(archive, bytes);
    verifyEngineArchive(archive, manifest);
  } finally {
    fs.rmSync(path.dirname(archive), { recursive: true, force: true });
  }
}
