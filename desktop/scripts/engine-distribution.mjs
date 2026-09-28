import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { extractZip, readZip, safeEntryName, writeZip } from "./zip-utils.mjs";

export const ENGINE_FILES = [
  "kepler-backend.exe",
  "kepler-focus-helper.exe",
  "kepler-focus-svc.exe",
  "tray.ico",
];

const sha256 = (value) => crypto.createHash("sha256").update(value).digest("hex");
const ENGINE_VERSION = /^\d+\.\d+\.\d+$/;
const ENGINE_FILE = /^[A-Za-z0-9._-]+$/;
const SOURCE_COMMIT = /^[0-9a-f]{40}$/;

function validateEngineManifest(manifest) {
  if (!manifest || manifest.schema_version !== 1 || manifest.product !== "kosmos-engine")
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

export function buildEngineArchive(releaseDir, archive, { version, sourceCommit }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("engine version must be semver");
  if (!SOURCE_COMMIT.test(sourceCommit))
    throw new Error("engine sourceCommit must be a 40-character lowercase commit");
  const files = ENGINE_FILES.map((name) => {
    const data = fs.readFileSync(path.join(releaseDir, name));
    return { name, data, sha256: sha256(data), size: data.length };
  });
  const manifest = {
    schema_version: 1,
    product: "kosmos-engine",
    version,
    source_commit: sourceCommit,
    files: files.map(({ name, sha256: digest, size }) => ({ name, sha256: digest, size })),
  };
  fs.mkdirSync(path.dirname(archive), { recursive: true });
  writeZip(archive, [
    { name: "engine-manifest.json", data: JSON.stringify(manifest) },
    ...files.map(({ name, data }) => ({ name, data })),
  ]);
  return {
    ...manifest,
    archive_sha256: sha256(fs.readFileSync(archive)),
    archive_size: fs.statSync(archive).size,
  };
}

export function verifyEngineArchive(archive, manifest) {
  validateEngineManifest(manifest);
  const entries = new Map(readZip(archive).map((entry) => [safeEntryName(entry.name), entry]));
  if (!entries.has("engine-manifest.json")) throw new Error("engine manifest missing");
  for (const file of manifest.files ?? []) {
    const entry = entries.get(file.name);
    if (
      !entry ||
      entry.isDir ||
      entry.data.length !== file.size ||
      sha256(entry.data) !== file.sha256
    )
      throw new Error(`engine artifact mismatch: ${file.name}`);
  }
  return true;
}

export function installEngineArchive(archive, manifest, engineRoot) {
  validateEngineManifest(manifest);
  if (manifest.archive_sha256 && sha256(fs.readFileSync(archive)) !== manifest.archive_sha256)
    throw new Error("engine archive hash mismatch");
  verifyEngineArchive(archive, manifest);
  const versionRoot = path.join(engineRoot, "versions", manifest.version);
  const currentPath = path.join(engineRoot, "current.json");
  if (fs.existsSync(currentPath)) {
    try {
      const current = JSON.parse(fs.readFileSync(currentPath, "utf8"));
      const currentRoot = path.join(engineRoot, "versions", current.version);
      if (
        current.schema_version === 1 &&
        current.version === manifest.version &&
        verifyInstalledEngine(currentRoot, manifest)
      )
        return currentRoot;
    } catch {
      // A malformed pointer is repaired by the verified archive below.
    }
  }
  const tempRoot = `${versionRoot}.${process.pid}.tmp`;
  fs.rmSync(tempRoot, { recursive: true, force: true });
  extractZip(archive, tempRoot);
  fs.mkdirSync(path.dirname(versionRoot), { recursive: true });
  fs.rmSync(versionRoot, { recursive: true, force: true });
  fs.renameSync(tempRoot, versionRoot);
  fs.writeFileSync(
    currentPath,
    JSON.stringify({ schema_version: 1, version: manifest.version }),
    "utf8",
  );
  return versionRoot;
}

export function resolveInstalledEngine(engineRoot) {
  const root = fs.realpathSync(engineRoot);
  const pointer = JSON.parse(fs.readFileSync(path.join(root, "current.json"), "utf8"));
  if (pointer?.schema_version !== 1 || !ENGINE_VERSION.test(pointer.version ?? ""))
    throw new Error("invalid installed Engine pointer");

  const versionsRoot = fs.realpathSync(path.join(root, "versions"));
  const versionRoot = path.resolve(versionsRoot, pointer.version);
  if (!isWithinRoot(versionRoot, versionsRoot)) throw new Error("invalid installed Engine path");
  const canonicalVersionRoot = fs.realpathSync(versionRoot);
  if (!isWithinRoot(canonicalVersionRoot, versionsRoot))
    throw new Error("installed Engine path escapes versions root");

  const manifest = JSON.parse(
    fs.readFileSync(path.join(canonicalVersionRoot, "engine-manifest.json"), "utf8"),
  );
  validateEngineManifest(manifest);
  if (manifest.version !== pointer.version) throw new Error("installed Engine version mismatch");
  const files = new Map(manifest.files.map((file) => [file.name, file]));
  if (files.size !== ENGINE_FILES.length || ENGINE_FILES.some((name) => !files.has(name)))
    throw new Error("installed Engine inputs are incomplete");
  for (const name of ENGINE_FILES) {
    const file = files.get(name);
    const candidate = path.join(canonicalVersionRoot, name);
    const canonical = fs.realpathSync(candidate);
    if (!isWithinRoot(canonical, canonicalVersionRoot))
      throw new Error(`installed Engine path escapes version root: ${name}`);
    const data = fs.readFileSync(canonical);
    if (data.length !== file.size || sha256(data) !== file.sha256)
      throw new Error(`engine artifact mismatch: ${name}`);
  }
  return {
    root,
    version: pointer.version,
    versionRoot: canonicalVersionRoot,
    backend: fs.realpathSync(path.join(canonicalVersionRoot, "kepler-backend.exe")),
    tray: fs.realpathSync(path.join(canonicalVersionRoot, "tray.ico")),
  };
}

function isWithinRoot(candidate, root) {
  const relative = path.relative(root, candidate);
  return (
    relative !== "" &&
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
}

function verifyInstalledEngine(root, manifest) {
  return (manifest.files ?? []).every((file) => {
    try {
      const data = fs.readFileSync(path.join(root, file.name));
      return data.length === file.size && sha256(data) === file.sha256;
    } catch {
      return false;
    }
  });
}
