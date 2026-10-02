import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { ENGINE_BINARY, ENGINE_MANIFEST_PRODUCT } from "./brand.mjs";

// Engine privileged ops live inside mundus-engine.exe itself
// (`mundus-engine privileged install|uninstall|run-service`) — no separate
// helper exes ship in the distribution.
export const ENGINE_FILES = [ENGINE_BINARY, "tray.ico"];

// MIGRATION(KOS-267): 'kosmos-engine' manifests exist in installed Engine
// roots written by 0.9.x installers; the Engine's `install` subcommand
// accepts both until cleanup.
const ENGINE_MANIFEST_PRODUCTS = new Set([ENGINE_MANIFEST_PRODUCT, "kosmos-engine"]);

const sha256 = (value) => crypto.createHash("sha256").update(value).digest("hex");
const ENGINE_VERSION = /^\d+\.\d+\.\d+$/;
const ENGINE_FILE = /^[A-Za-z0-9._-]+$/;
const SOURCE_COMMIT = /^[0-9a-f]{40}$/;

export function validateEngineManifest(manifest) {
  if (!manifest || manifest.schema_version !== 1 || !ENGINE_MANIFEST_PRODUCTS.has(manifest.product))
    throw new Error("invalid engine manifest");
  if (
    Object.prototype.toString.call(manifest.version) !== "[object String]" ||
    !ENGINE_VERSION.test(manifest.version)
  )
    throw new Error("engine version must be semver");
  if (
    Object.prototype.toString.call(manifest.source_commit) !== "[object String]" ||
    !SOURCE_COMMIT.test(manifest.source_commit)
  )
    throw new Error("engine source_commit must be a 40-character lowercase commit");
  if (!Array.isArray(manifest.files) || manifest.files.length === 0)
    throw new Error("engine manifest files are required");
  for (const file of manifest.files) {
    if (
      !file ||
      Object.prototype.toString.call(file.name) !== "[object String]" ||
      file.name === "." ||
      file.name === ".." ||
      !ENGINE_FILE.test(file.name) ||
      !Number.isSafeInteger(file.size) ||
      file.size < 0 ||
      Object.prototype.toString.call(file.sha256) !== "[object String]" ||
      !/^[a-f0-9]{64}$/i.test(file.sha256)
    )
      throw new Error("invalid engine manifest file");
  }
}

// KOS-306: the payload ships unpacked — the NSIS installer runs the staged
// mundus-engine.exe directly (`install --manifest …`), so there is no zip
// and no PowerShell Expand-Archive step. `payloadDir` receives exactly the
// manifest-listed files plus engine-manifest.json; the subcommand verifies
// each file's sha256 before copying it into versions/<v>.
export function buildEnginePayload(releaseDir, payloadDir, { version, sourceCommit }) {
  if (!ENGINE_VERSION.test(version)) throw new Error("engine version must be semver");
  if (!SOURCE_COMMIT.test(sourceCommit))
    throw new Error("engine sourceCommit must be a 40-character lowercase commit");
  const files = ENGINE_FILES.map((name) => {
    const data = fs.readFileSync(path.join(releaseDir, name));
    return { name, data, sha256: sha256(data), size: data.length };
  });
  const manifest = {
    schema_version: 1,
    product: "mundus-engine",
    version,
    source_commit: sourceCommit,
    files: files.map(({ name, sha256: digest, size }) => ({ name, sha256: digest, size })),
  };
  fs.rmSync(payloadDir, { recursive: true, force: true });
  fs.mkdirSync(payloadDir, { recursive: true });
  for (const { name, data } of files) {
    fs.writeFileSync(path.join(payloadDir, name), data);
  }
  fs.writeFileSync(
    path.join(payloadDir, "engine-manifest.json"),
    JSON.stringify(manifest, null, 2) + "\n",
  );
  return manifest;
}
