#!/usr/bin/env node

import { execFileSync, spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { existsSync, readdirSync, readFileSync, rmSync, statSync } from "node:fs";
import { getVersion } from "./release-version.mjs";
import { loadReleaseBom } from "./release-bom.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import { bytes, documentHash, writeAtomic } from "./package-release-utils.mjs";
import { runFirstPartyContracts } from "./first-party-release-contracts.mjs";
import { copyEngineManifest, copyEngineRelease } from "./engine-distribution.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const SHELL_ROOT = path.resolve(__dirname, "..");
const ENGINE_VERSION = "0.1.3";
const VALID_PLATFORMS = ["win", "mac"];
function die(msg) {
  console.error(`[build-desktop] FATAL: ${msg}`);
  process.exit(1);
}

function log(...args) {
  console.log("[build-desktop]", ...args);
}

/**
 * Resolve the electron-builder binary.
 * Prefers the local node_modules/.bin/electron-builder(.cmd on Windows).
 * Falls back to globally available "electron-builder".
 */
function resolveElectronBuilder() {
  const isWin = process.platform === "win32";
  const binDir = path.join(SHELL_ROOT, "node_modules", ".bin");
  // bun installs Windows shims as .exe (+ .bunx); npm/pnpm use .cmd. Probe in
  // order and take the first that exists. In this bun workspace the binary is
  // node_modules/.bin/electron-builder.exe.
  const candidates = isWin
    ? ["electron-builder.exe", "electron-builder.cmd", "electron-builder.bunx"]
    : ["electron-builder"];
  for (const name of candidates) {
    const p = path.join(binDir, name);
    if (existsSync(p)) return p;
  }
  // Fallback: assume on PATH
  return isWin ? "electron-builder.cmd" : "electron-builder";
}

function currentCommit() {
  try {
    return execFileSync("git", ["rev-parse", "HEAD"], { cwd: SHELL_ROOT, encoding: "utf8" }).trim();
  } catch {
    die("unable to resolve the Cortex HEAD commit");
  }
}

function ensureCleanSource() {
  const tracked = execFileSync("git", ["status", "--porcelain", "--untracked-files=no"], {
    cwd: SHELL_ROOT,
    encoding: "utf8",
  }).trim();
  const untracked = execFileSync(
    "git",
    [
      "ls-files",
      "--others",
      "--exclude-standard",
      "--",
      "src",
      "electron",
      "scripts",
      "build",
      "shared",
      "../host/src",
      "../host/electron",
      "../manager/src",
      "../manager/electron",
      "../runtime/src",
      "../native-services",
      "../packages",
    ],
    { cwd: SHELL_ROOT, encoding: "utf8" },
  ).trim();
  if (tracked || untracked) die("release builds require a clean tracked and source worktree");
}

function verifyEmbeddedBom(outputDir, platform, digest) {
  const candidates = [path.join(outputDir, "win-unpacked", "resources", "release-bom.json")];
  if (platform === "mac") {
    for (const directory of readdirSync(outputDir, { withFileTypes: true })) {
      if (!directory.isDirectory() || !directory.name.startsWith("mac")) continue;
      const root = path.join(outputDir, directory.name);
      for (const app of readdirSync(root, { withFileTypes: true })) {
        if (app.isDirectory() && app.name.endsWith(".app"))
          candidates.push(path.join(root, app.name, "Contents", "Resources", "release-bom.json"));
      }
    }
  }
  const embedded = candidates.find(existsSync);
  if (!embedded) die(`embedded release BOM not found in ${outputDir}`);
  if (documentHash(readFileSync(embedded)) !== digest)
    die(`embedded release BOM digest mismatch: ${embedded}`);
  return embedded;
}

function collectArtifacts(outputDir, platform, version, engineVersion) {
  const channel = platform === "win" ? "latest.yml" : "latest-mac.yml";
  const versions = [version, engineVersion].filter(Boolean);
  const names = readdirSync(outputDir).filter((name) => {
    if (name === channel) return true;
    if (name !== "Kosmos-Engine-manifest.json" && !versions.some((value) => name.includes(value)))
      return false;
    return /\.(?:exe|dmg|zip|blockmap|json)$/i.test(name);
  });
  const artifacts = names.map((name) => {
    const file = path.join(outputDir, name);
    return { name, sha256: documentHash(readFileSync(file)), size: statSync(file).size };
  });
  if (!artifacts.some(({ name }) => /\.(?:exe|dmg)$/i.test(name)))
    die(`no installer artifact found in ${outputDir}`);
  if (!artifacts.some(({ name }) => name === channel)) die(`missing ${channel} in ${outputDir}`);
  return artifacts;
}

function compareExpectedArtifacts(expected, actual) {
  if (!expected?.length) return;
  const actualByName = new Map(actual.map((artifact) => [artifact.name, artifact]));
  for (const artifact of expected) {
    const found = actualByName.get(artifact.name);
    if (!found || found.sha256 !== artifact.sha256 || found.size !== artifact.size)
      die(`final artifact does not match BOM: ${artifact.name}`);
  }
}

function verifyArkArtifact(bom) {
  const expected = bom.value.source.core.ark_artifact;
  const file = path.join(SHELL_ROOT, ".tmp", "runtime.next", expected.name);
  if (!existsSync(file) || statSync(file).size !== expected.size)
    die(`ARK artifact is missing or has the wrong size: ${expected.name}`);
  if (documentHash(readFileSync(file)) !== expected.sha256)
    die(`ARK artifact hash does not match BOM: ${expected.name}`);
}

async function emitProvenance(outputDir, platform, version, bom, engineVersion) {
  const embedded = verifyEmbeddedBom(outputDir, platform, bom.digest);
  verifyLocalReleaseChannel(outputDir, platform, version);
  const artifacts = collectArtifacts(outputDir, platform, version, engineVersion);
  compareExpectedArtifacts(bom.value.artifacts, artifacts);
  const provenance = {
    schema_version: 1,
    bom_id: bom.value.id,
    bom_digest: bom.digest,
    platform,
    version,
    embedded_bom: path.relative(outputDir, embedded),
    artifacts,
  };
  const file = path.join(outputDir, "release-provenance.json");
  const bomFile = path.join(outputDir, "release-bom.v1.json");
  await writeAtomic(bomFile, bom.bytes);
  await writeAtomic(file, bytes(provenance));
  log(`Release provenance: ${file}`);
  return {
    metadataFiles: [bomFile, file],
    artifactFiles: artifacts.map(({ name }) => path.join(outputDir, name)),
  };
}

function publishRelease(platform, version, files) {
  const repository = platform === "win" ? "makekosmos/desktop" : "makekosmos/desktop-mac";
  if (!process.env.GH_TOKEN) die("GH_TOKEN is required to publish a verified release");
  const existing = spawnSync("gh", ["release", "view", `v${version}`, "--repo", repository], {
    cwd: SHELL_ROOT,
    stdio: "ignore",
    windowsHide: true,
    env: process.env,
  });
  if (existing.status === 0) die(`immutable release already exists: ${repository} v${version}`);
  const result = spawnSync(
    "gh",
    [
      "release",
      "create",
      `v${version}`,
      ...files,
      "--repo",
      repository,
      "--title",
      `Kosmos ${version}`,
      "--notes",
      "Immutable release assembled from the attached release BOM.",
    ],
    { cwd: SHELL_ROOT, stdio: "inherit", windowsHide: true, env: process.env },
  );
  if (result.status !== 0) die(`failed to publish verified release to ${repository}`);
}

async function main() {
  const args = process.argv.slice(2);
  let platform = null;
  let bomPath = process.env.KOSMOS_RELEASE_BOM ?? null;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--platform") {
      platform = args[++i];
    } else if (args[i] === "--bom") {
      bomPath = args[++i];
    }
  }

  if (!platform) {
    die("--platform <win|mac> is required");
  }
  if (!VALID_PLATFORMS.includes(platform)) {
    die(`Unknown platform "${platform}". Valid: ${VALID_PLATFORMS.join(", ")}`);
  }
  if (!bomPath) die("--bom <path> or KOSMOS_RELEASE_BOM is required for publish builds");

  const version = getVersion(platform),
    engineVersion =
      platform === "win" ? (process.env.KOSMOS_ENGINE_VERSION ?? ENGINE_VERSION) : null;
  ensureCleanSource();
  const bom = await loadReleaseBom(bomPath, {
    root: path.resolve(SHELL_ROOT, ".."),
    platform,
    currentCommit: currentCommit(),
  });
  verifyArkArtifact(bom);
  log(`Platform: ${platform}`);
  log(`Version:  ${version}`);
  log(`BOM:      ${bom.value.id} (${bom.digest})`);
  log("");
  if (platform === "win" && existsSync(path.join(SHELL_ROOT, "release")))
    readdirSync(path.join(SHELL_ROOT, "release"))
      .filter((name) => /^Kosmos-Engine-\d+\.\d+\.\d+\.(?:zip|json)$/.test(name))
      .forEach((name) => rmSync(path.join(SHELL_ROOT, "release", name)));
  const eb = resolveElectronBuilder();
  log(`electron-builder: ${eb}`);
  let ebArgs;
  if (platform === "win") {
    ebArgs = ["--win", "nsis", "--publish", "never", `-c.extraMetadata.version=${version}`];
  } else {
    ebArgs = ["--mac", "dmg", "--publish", "never", `-c.extraMetadata.version=${version}`];
  }
  log(`Running: ${eb} ${ebArgs.join(" ")}`);
  log("");

  const ebResult = spawnSync(eb, ebArgs, {
    cwd: SHELL_ROOT,
    stdio: "inherit",
    shell: true,
    windowsHide: true,
    env: { ...process.env, KOSMOS_RELEASE_BOM_PATH: bom.path },
  });
  if (ebResult.status !== 0) {
    console.error("");
    console.error(
      `[build-desktop] electron-builder exited with code ${ebResult.status ?? "(signal)"}`,
    );
    process.exit(ebResult.status ?? 1);
  }
  if (platform === "win") {
    copyEngineRelease(SHELL_ROOT, engineVersion);
    copyEngineManifest(SHELL_ROOT, engineVersion);
  }
  log("");
  log("electron-builder succeeded. Emitting release provenance...");
  const releaseFiles = await emitProvenance(
    path.join(SHELL_ROOT, "release"),
    platform,
    version,
    bom,
    engineVersion,
  );
  runFirstPartyContracts(platform);
  publishRelease(platform, version, [...releaseFiles.artifactFiles, ...releaseFiles.metadataFiles]);
  log("Running verify guard...");
  log("");
  const verifyScript = path.join(__dirname, "verify-release-channel.mjs");
  const verifyResult = spawnSync(
    process.execPath, // node
    [verifyScript, "--platform", platform, "--version", version],
    {
      cwd: SHELL_ROOT,
      stdio: "inherit",
      windowsHide: true,
    },
  );
  if (verifyResult.status !== 0) {
    console.error("");
    console.error(`[build-desktop] verify-release-channel FAILED for ${platform} v${version}.`);
    console.error(
      `[build-desktop] The verified release was published but channel integrity failed.`,
    );
    console.error(`[build-desktop] Review the output above and follow the fix instructions.`);
    process.exit(verifyResult.status ?? 1);
  }
  log(`Build + verify complete for ${platform} v${version}. Release is consistent.`);
}
main().catch((error) => die(error instanceof Error ? error.message : String(error)));
