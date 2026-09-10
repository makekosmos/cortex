import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { extractZip, readZip, safeEntryName, writeZip } from "./zip-utils.mjs";

export const ENGINE_FILES = [
  "kepler-backend.exe",
  "ark-core-rpc.exe",
  "kepler-focus-helper.exe",
  "kepler-focus-svc.exe",
  "tray.ico",
];

const sha256 = (value) => crypto.createHash("sha256").update(value).digest("hex");
const ENGINE_VERSION = /^\d+\.\d+\.\d+$/;
const ENGINE_FILE = /^[A-Za-z0-9._-]+$/;

function validateEngineManifest(manifest) {
  if (!manifest || manifest.schema_version !== 1 || manifest.product !== "kosmos-engine")
    throw new Error("invalid engine manifest");
  if (
    Object.prototype.toString.call(manifest.version) !== "[object String]" ||
    !ENGINE_VERSION.test(manifest.version)
  )
    throw new Error("engine version must be semver");
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

export function buildEngineArchive(releaseDir, archive, { version, url }) {
  if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("engine version must be semver");
  if (!/^https:\/\//.test(url)) throw new Error("engine url must use HTTPS");
  const files = ENGINE_FILES.map((name) => {
    const data = fs.readFileSync(path.join(releaseDir, name));
    return { name, data, sha256: sha256(data), size: data.length };
  });
  const manifest = {
    schema_version: 1,
    product: "kosmos-engine",
    version,
    url,
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

export function copyStandaloneEngineArtifact(source, target) {
  if (!fs.existsSync(source)) throw new Error(`standalone engine archive missing: ${source}`);
  fs.copyFileSync(source, target);
}

export function copyEngineRelease(shellRoot, version) {
  fs.mkdirSync(path.join(shellRoot, "release"), { recursive: true });
  copyStandaloneEngineArtifact(
    path.join(shellRoot, ".tmp", "engine.next", "Kosmos-Engine.zip"),
    path.join(shellRoot, "release", `Kosmos-Engine-${version}.zip`),
  );
  const installer = path.join(
    shellRoot,
    ".tmp",
    "engine.next",
    `Kosmos-Engine-Setup-${version}.exe`,
  );
  if (fs.existsSync(installer))
    copyStandaloneEngineArtifact(
      installer,
      path.join(shellRoot, "release", path.basename(installer)),
    );
}

export function copyEngineManifest(shellRoot, version) {
  const source = path.join(shellRoot, ".tmp", "engine.next", "engine-manifest.json");
  const target = path.join(shellRoot, "release", `Kosmos-Engine-${version}.json`);
  if (!fs.existsSync(source)) throw new Error(`standalone engine manifest missing: ${source}`);
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.copyFileSync(source, target);
  fs.copyFileSync(source, path.join(shellRoot, "release", "Kosmos-Engine-manifest.json"));
}
