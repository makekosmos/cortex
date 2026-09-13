import { createHash, randomUUID } from "node:crypto";
import {
  copyFileSync,
  closeSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  openSync,
  readFileSync,
  renameSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

export const ARK_CORE_REPOSITORY = "https://github.com/makekosmos/core.git";
// Keep the Rust API and sidecar binary on the same immutable Core revision.
export const ARK_CORE_REVISION = "169c1967a074ae6658e81d59892247b24332ce29";

const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const defaultCacheRoot = path.join(shellRoot, ".tmp", "ark-core-rpc");
const LOCK_WAIT_MS = 10 * 60 * 1000;
const STALE_LOCK_MS = 15 * 60 * 1000;
const waitBuffer = new Int32Array(new SharedArrayBuffer(4));

function featureKey(features) {
  return features.length === 0 ? "default" : [...new Set(features)].sort().join("+");
}

export function installRoot(debug, features, cacheRoot = defaultCacheRoot) {
  return path.join(cacheRoot, ARK_CORE_REVISION, debug ? "debug" : "release", featureKey(features));
}

function isProcessAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

function readLock(lockPath) {
  try {
    const lock = JSON.parse(readFileSync(lockPath, "utf8"));
    return { pid: Number(lock.pid), token: lock.token };
  } catch {
    return null;
  }
}

export function acquireCacheLock(
  lockPath,
  { waitMs = LOCK_WAIT_MS, staleMs = STALE_LOCK_MS } = {},
) {
  mkdirSync(path.dirname(lockPath), { recursive: true });
  const token = randomUUID();
  const deadline = Date.now() + waitMs;
  while (true) {
    try {
      const fd = openSync(lockPath, "wx");
      try {
        writeFileSync(fd, JSON.stringify({ pid: process.pid, token, startedAt: Date.now() }));
      } finally {
        closeSync(fd);
      }
      return () => {
        if (readLock(lockPath)?.token === token) rmSync(lockPath, { force: true });
      };
    } catch (error) {
      if (error?.code !== "EEXIST") throw error;
      let stale = false;
      try {
        const lock = readLock(lockPath);
        const age = Date.now() - statSync(lockPath).mtimeMs;
        stale = lock ? !isProcessAlive(lock.pid) : age > staleMs;
      } catch {
        stale = true;
      }
      if (stale) {
        rmSync(lockPath, { force: true });
        continue;
      }
      if (Date.now() >= deadline) throw new Error(`timed out waiting for cache lock: ${lockPath}`);
      Atomics.wait(waitBuffer, 0, 0, 50);
    }
  }
}

function sidecarName() {
  return `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`;
}

function cachedBinary(root) {
  return path.join(root, "bin", sidecarName());
}

function isCompleteCache(root) {
  try {
    return (
      existsSync(cachedBinary(root)) &&
      readFileSync(path.join(root, ".complete"), "utf8").trim() === ARK_CORE_REVISION
    );
  } catch {
    return false;
  }
}

function hashFile(filePath) {
  return createHash("sha256").update(readFileSync(filePath)).digest("hex");
}

export function validatePrebuiltManifest(manifestPath) {
  let manifest;
  try {
    manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
  } catch (error) {
    throw new Error(`prebuilt manifest cannot be read: ${error.message}`);
  }
  if (manifest.platform !== process.platform)
    throw new Error(
      `prebuilt platform ${manifest.platform ?? "missing"} does not match ${process.platform}`,
    );
  if (manifest.arch !== process.arch)
    throw new Error(`prebuilt arch ${manifest.arch ?? "missing"} does not match ${process.arch}`);
  if (manifest.coreRevision !== ARK_CORE_REVISION)
    throw new Error("prebuilt Core revision does not match the pinned revision");
  if (
    !manifest.binary ||
    manifest.binary.constructor !== String ||
    path.basename(manifest.binary) !== manifest.binary
  )
    throw new Error("prebuilt binary must be a relative filename");
  if (manifest.binary !== sidecarName())
    throw new Error(`prebuilt binary must be ${sidecarName()}`);
  if (!/^[0-9a-f]{64}$/i.test(manifest.sha256 ?? ""))
    throw new Error("prebuilt SHA-256 is missing or invalid");
  const binary = path.resolve(path.dirname(manifestPath), manifest.binary);
  if (!existsSync(binary)) throw new Error(`prebuilt binary is missing: ${binary}`);
  if (hashFile(binary).toLowerCase() !== manifest.sha256.toLowerCase())
    throw new Error("prebuilt SHA-256 does not match");
  return binary;
}

function copyToTarget(source, targetDir) {
  mkdirSync(targetDir, { recursive: true });
  const target = path.join(targetDir, sidecarName());
  copyFileSync(source, target);
  return target;
}

function buildCachedSidecar(root, { debug, features, cargoCommand, cargoArgsPrefix }) {
  const parent = path.dirname(root);
  mkdirSync(parent, { recursive: true });
  const staging = mkdtempSync(path.join(parent, `.${path.basename(root)}.tmp-`));
  try {
    const args = [
      ...cargoArgsPrefix,
      "install",
      "--git",
      ARK_CORE_REPOSITORY,
      "--rev",
      ARK_CORE_REVISION,
      "ark-core",
      "--bin",
      "ark-core-rpc",
      "--root",
      staging,
      "--locked",
    ];
    if (debug) args.push("--debug");
    if (features.length > 0) args.push("--features", features.join(","));
    const result = spawnSync(cargoCommand, args, {
      cwd: shellRoot,
      env: { ...process.env, CARGO_TARGET_DIR: path.join(staging, "target") },
      stdio: "inherit",
      windowsHide: true,
    });
    if ((result.status ?? 1) !== 0)
      throw new Error(`cargo install failed with status ${result.status ?? 1}`);
    if (!existsSync(cachedBinary(staging)))
      throw new Error(`cargo install did not produce ${cachedBinary(staging)}`);
    writeFileSync(path.join(staging, ".complete"), `${ARK_CORE_REVISION}\n`);
    rmSync(root, { recursive: true, force: true });
    renameSync(staging, root);
  } catch (error) {
    rmSync(staging, { recursive: true, force: true });
    throw error;
  }
}

export function ensureArkCoreRpc({
  debug = false,
  features = [],
  targetDir,
  cacheRoot = defaultCacheRoot,
  cargoCommand = "cargo",
  cargoArgsPrefix = [],
  prebuiltManifest,
} = {}) {
  if (prebuiltManifest) {
    try {
      const prebuilt = validatePrebuiltManifest(prebuiltManifest);
      return targetDir ? copyToTarget(prebuilt, targetDir) : prebuilt;
    } catch (error) {
      console.error(
        `[ark-core-rpc] prebuilt rejected: ${error.message}; falling back to source build`,
      );
    }
  }
  const root = installRoot(debug, features, cacheRoot);
  const releaseLock = acquireCacheLock(`${root}.lock`);
  try {
    if (!isCompleteCache(root))
      buildCachedSidecar(root, { debug, features, cargoCommand, cargoArgsPrefix });
    const installed = cachedBinary(root);
    return targetDir ? copyToTarget(installed, targetDir) : installed;
  } finally {
    releaseLock();
  }
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))
) {
  const debug = process.argv.includes("--debug");
  const targetIndex = process.argv.indexOf("--target-dir");
  const targetDir = targetIndex === -1 ? undefined : process.argv[targetIndex + 1];
  console.log(ensureArkCoreRpc({ debug, targetDir }));
}
