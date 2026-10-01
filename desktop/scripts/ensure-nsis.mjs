#!/usr/bin/env node
import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  createReadStream,
  createWriteStream,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  renameSync,
  rmSync,
} from "node:fs";
import { get } from "node:https";
import path from "node:path";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";
import { env } from "./brand.mjs";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const NSIS_URL =
  "https://github.com/electron-userland/electron-builder-binaries/releases/download/nsis%401.2.1/nsis-bundle-3.12.tar.gz";
const NSIS_SHA256 = "56997fdefe25e7928a1a68b4583d08b240b66cf660234053b20131a74cc082f4";
const cacheDir = path.join(root, ".tmp", "nsis");
const bundleDir = path.join(cacheDir, "nsis-bundle");

function sha256(file) {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(file);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
    stream.on("error", reject);
  });
}

function download(url, dest) {
  return new Promise((resolve, reject) => {
    const file = createWriteStream(dest);
    get(url, (response) => {
      if (response.statusCode === 301 || response.statusCode === 302) {
        file.close();
        rmSync(dest, { force: true });
        download(response.headers.location, dest).then(resolve, reject);
        return;
      }
      if (response.statusCode !== 200) {
        file.close();
        rmSync(dest, { force: true });
        reject(new Error(`download failed: HTTP ${response.statusCode}`));
        return;
      }
      pipeline(response, file).then(resolve, reject);
    }).on("error", reject);
  });
}

function findMakensis(dir) {
  if (!existsSync(dir)) return null;
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isFile() && entry.name === "makensis.exe") return full;
    if (entry.isDirectory()) {
      const found = findMakensis(full);
      if (found) return found;
    }
  }
  return null;
}

// makensis resolves Stubs/Plugins/Include relative to NSISDIR, which is the
// bundle root above the Bin/ directory holding makensis.exe.
function nsisEnvFor(makensis) {
  const binDir = path.dirname(makensis);
  return {
    NSISDIR: path.basename(binDir).toLowerCase() === "bin" ? path.dirname(binDir) : binDir,
  };
}

export async function ensureNsis() {
  const nsisDir = env("NSIS_DIR");
  if (nsisDir) {
    const dir = path.resolve(nsisDir);
    const makensis = findMakensis(dir);
    if (makensis) return { makensis, env: nsisEnvFor(makensis) };
    throw new Error(`MUNDUS_NSIS_DIR does not contain makensis.exe: ${dir}`);
  }

  const existing = findMakensis(bundleDir);
  if (existing) return { makensis: existing, env: nsisEnvFor(existing) };

  mkdirSync(cacheDir, { recursive: true });
  const archive = path.join(cacheDir, "nsis-bundle-3.12.tar.gz");
  console.log("[ensure-nsis] downloading NSIS bundle...");
  await download(NSIS_URL, archive);
  const hash = await sha256(archive);
  if (hash !== NSIS_SHA256) {
    rmSync(archive, { force: true });
    throw new Error(`NSIS bundle sha256 mismatch: expected ${NSIS_SHA256}, got ${hash}`);
  }

  // Extract next to the final location: the rename below fails with EXDEV
  // when os.tmpdir() and the worktree sit on different volumes (CI runners).
  const extractDir = mkdtempSync(path.join(cacheDir, "extract-"));
  const result = spawnSync("tar", ["-xzf", path.basename(archive), "-C", extractDir], {
    cwd: cacheDir,
    stdio: "inherit",
    windowsHide: true,
  });
  if (result.status !== 0) throw new Error("failed to extract NSIS bundle");

  rmSync(bundleDir, { recursive: true, force: true });
  mkdirSync(cacheDir, { recursive: true });
  const extracted = path.join(extractDir, "nsis-bundle");
  if (!existsSync(extracted)) throw new Error("NSIS bundle extraction missing nsis-bundle/");
  renameSync(extracted, bundleDir);
  rmSync(extractDir, { recursive: true, force: true });
  rmSync(archive, { force: true });

  const makensis = findMakensis(bundleDir);
  if (!makensis) throw new Error("makensis.exe not found in extracted NSIS bundle");
  return { makensis, env: nsisEnvFor(makensis) };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  ensureNsis()
    .then(({ makensis }) => console.log(makensis))
    .catch((error) => {
      console.error(`[ensure-nsis] FATAL: ${error instanceof Error ? error.message : error}`);
      process.exitCode = 1;
    });
}
