import { createHash, randomUUID } from "node:crypto";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { acquireCacheLock } from "./ark-core-rpc-lock.mjs";
import { arkCoreSourceId, ARK_CORE_SOURCE_DIR } from "./ark-core-source.mjs";
import { defaultArkCoreTargetDir } from "./runtime-staging.mjs";
export { acquireCacheLock } from "./ark-core-rpc-lock.mjs";
export { arkCoreSourceId, ARK_CORE_SOURCE, ARK_CORE_SOURCE_DIR } from "./ark-core-source.mjs";
const shellRoot = fileURLToPath(new URL("..", import.meta.url));
const defaultCacheRoot = path.join(shellRoot, ".tmp", "ark-core-rpc");
const featureKey = (features) =>
  features.length === 0 ? "default" : [...new Set(features)].sort().join("+");
export function installRoot(
  debug,
  features,
  cacheRoot = defaultCacheRoot,
  platform = process.platform,
  arch = process.arch,
  sourceId = arkCoreSourceId(),
) {
  return path.join(
    cacheRoot,
    platform,
    arch,
    sourceId.replace(":", "-"),
    debug ? "debug" : "release",
    featureKey(features),
  );
}
const sidecarName = () => `ark-core-rpc${process.platform === "win32" ? ".exe" : ""}`;
const cachedBinary = (root) => path.join(root, "bin", sidecarName());
const hashFile = (filePath) => createHash("sha256").update(readFileSync(filePath)).digest("hex");
function backupPaths(target) {
  const prefix = `${path.basename(target)}.`;
  return readdirSync(path.dirname(target))
    .filter((name) => name.startsWith(prefix) && name.endsWith(".old-"))
    .map((name) => path.join(path.dirname(target), name))
    .sort();
}
function isCompleteCache(root, sourceId) {
  try {
    return (
      existsSync(cachedBinary(root)) &&
      readFileSync(path.join(root, ".complete"), "utf8").trim() === sourceId
    );
  } catch {
    return false;
  }
}
function recoverCacheEntry(root, sourceId) {
  const backups = backupPaths(root);
  if (!existsSync(root)) {
    for (const backup of backups) {
      if (!isCompleteCache(backup, sourceId)) {
        rmSync(backup, { recursive: true, force: true });
        continue;
      }
      try {
        renameSync(backup, root);
        break;
      } catch {}
    }
  } else if (!isCompleteCache(root, sourceId)) {
    for (const backup of backups) {
      if (!isCompleteCache(backup, sourceId)) {
        rmSync(backup, { recursive: true, force: true });
        continue;
      }
      rmSync(root, { recursive: true, force: true });
      try {
        renameSync(backup, root);
      } catch {}
      break;
    }
  }
  if (isCompleteCache(root, sourceId))
    for (const backup of backupPaths(root)) rmSync(backup, { recursive: true, force: true });
}
function recoverTargetBackup(target, expectedSha256) {
  if (!existsSync(target))
    for (const backup of backupPaths(target)) {
      let valid = true;
      try {
        valid = !expectedSha256 || hashFile(backup) === expectedSha256;
      } catch {
        valid = false;
      }
      if (!valid) {
        rmSync(backup, { force: true });
        continue;
      }
      try {
        renameSync(backup, target);
        break;
      } catch {}
    }
  if (existsSync(target)) for (const backup of backupPaths(target)) rmSync(backup, { force: true });
}
function readValidatedPrebuiltManifest(manifestPath) {
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
  if (manifest.coreRevision !== arkCoreSourceId())
    throw new Error("prebuilt Core revision does not match the in-tree ark-core source");
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
  return { binary, sha256: manifest.sha256.toLowerCase() };
}
export function validatePrebuiltManifest(manifestPath) {
  return readValidatedPrebuiltManifest(manifestPath).binary;
}
function materializePrebuilt(prebuilt, cacheRoot, copyFile, renamePath) {
  const root = path.join(
    cacheRoot,
    "prebuilt",
    process.platform,
    process.arch,
    arkCoreSourceId().replace(":", "-"),
    prebuilt.sha256,
  );
  const binary = path.join(root, sidecarName());
  try {
    if (
      readFileSync(path.join(root, ".complete"), "utf8").trim() === prebuilt.sha256 &&
      hashFile(binary) === prebuilt.sha256
    )
      return binary;
  } catch {}
  const result = publishToTarget(prebuilt.binary, root, prebuilt.sha256, copyFile, renamePath);
  writeFileSync(path.join(root, ".complete"), `${prebuilt.sha256}\n`);
  return result;
}
export function publishToTarget(
  source,
  targetDir,
  expectedSha256,
  copyFile = copyFileSync,
  renamePath = renameSync,
) {
  mkdirSync(targetDir, { recursive: true });
  const target = path.join(targetDir, sidecarName());
  recoverTargetBackup(target, expectedSha256);
  const temporary = path.join(targetDir, `.${sidecarName()}.${randomUUID()}.tmp-`);
  try {
    copyFile(source, temporary);
    if (expectedSha256 && hashFile(temporary) !== expectedSha256)
      throw new Error("prebuilt SHA-256 changed during copy");
    if (existsSync(target)) {
      const backup = `${target}.${randomUUID()}.old-`;
      renamePath(target, backup);
      try {
        renamePath(temporary, target);
      } catch (error) {
        renamePath(backup, target);
        throw error;
      }
      rmSync(backup, { force: true });
    } else {
      renamePath(temporary, target);
    }
    return target;
  } catch (error) {
    rmSync(temporary, { force: true });
    throw error;
  }
}
function buildCachedSidecar(root, { debug, features, cargoCommand, cargoArgsPrefix, renamePath }) {
  const parent = path.dirname(root);
  mkdirSync(parent, { recursive: true });
  const staging = mkdtempSync(path.join(parent, `.${path.basename(root)}.tmp-`));
  let backup;
  try {
    const args = [
      ...cargoArgsPrefix,
      "install",
      "--path",
      ARK_CORE_SOURCE_DIR,
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
      env: {
        ...process.env,
        CARGO_TARGET_DIR: process.env.KOSMOS_ARK_TARGET_DIR ?? defaultArkCoreTargetDir(),
      },
      stdio: "inherit",
      windowsHide: true,
    });
    if ((result.status ?? 1) !== 0)
      throw new Error(`cargo install failed with status ${result.status ?? 1}`);
    if (!existsSync(cachedBinary(staging)))
      throw new Error(`cargo install did not produce ${cachedBinary(staging)}`);
    writeFileSync(path.join(staging, ".complete"), `${arkCoreSourceId()}\n`);
    backup = `${root}.${randomUUID()}.old-`;
    if (existsSync(root)) renamePath(root, backup);
    try {
      renamePath(staging, root);
    } catch (error) {
      if (existsSync(backup)) renamePath(backup, root);
      backup = undefined;
      throw error;
    }
    if (backup) rmSync(backup, { recursive: true, force: true });
    backup = undefined;
  } catch (error) {
    if (backup && existsSync(backup) && !existsSync(root)) renameSync(backup, root);
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
  copyFile = copyFileSync,
  renamePath = renameSync,
} = {}) {
  if (prebuiltManifest) {
    try {
      const prebuilt = readValidatedPrebuiltManifest(prebuiltManifest);
      return targetDir
        ? publishToTarget(prebuilt.binary, targetDir, prebuilt.sha256, copyFile, renamePath)
        : materializePrebuilt(prebuilt, cacheRoot, copyFile, renamePath);
    } catch (error) {
      console.error(
        `[ark-core-rpc] prebuilt rejected: ${error.message}; falling back to source build`,
      );
    }
  }
  const sourceId = arkCoreSourceId();
  const root = installRoot(debug, features, cacheRoot, undefined, undefined, sourceId);
  const releaseLock = acquireCacheLock(`${root}.lock`);
  try {
    recoverCacheEntry(root, sourceId);
    const marker = path.join(root, ".complete");
    if (
      !existsSync(cachedBinary(root)) ||
      !existsSync(marker) ||
      readFileSync(marker, "utf8").trim() !== sourceId
    )
      buildCachedSidecar(root, { debug, features, cargoCommand, cargoArgsPrefix, renamePath });
    const installed = cachedBinary(root);
    return targetDir ? publishToTarget(installed, targetDir) : installed;
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
