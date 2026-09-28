#!/usr/bin/env node

import { spawnSync } from "node:child_process";
import path from "node:path";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { loadReleaseBom } from "./release-bom.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import { bytes, documentHash, writeAtomic } from "./package-release-utils.mjs";
import { runFirstPartyContracts } from "./first-party-release-contracts.mjs";
import { createReceipt, writeReceipt } from "./release-receipt.mjs";
import {
  currentCommit,
  runReleasePreflight,
  SHELL_ROOT as PREFLIGHT_ROOT,
} from "./release-preflight.mjs";

const SHELL_ROOT = PREFLIGHT_ROOT;
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
  if (process.env.KOSMOS_ELECTRON_BUILDER) return process.env.KOSMOS_ELECTRON_BUILDER;
  const isWin = process.platform === "win32";
  const binDir = path.join(SHELL_ROOT, "node_modules", ".bin");
  // pnpm/npm use .cmd on Windows. Keep the local binary preference before PATH.
  const candidates = isWin
    ? ["electron-builder.cmd", "electron-builder.exe"]
    : ["electron-builder"];
  for (const name of candidates) {
    const p = path.join(binDir, name);
    if (existsSync(p)) return p;
  }
  // Fallback: assume on PATH
  return isWin ? "electron-builder.cmd" : "electron-builder";
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

function collectArtifacts(outputDir, platform, version) {
  const channel = platform === "win" ? "latest.yml" : "latest-mac.yml";
  const names = readdirSync(outputDir).filter((name) => {
    if (name === channel) return true;
    if (!name.includes(version)) return false;
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

async function emitProvenance(outputDir, platform, version, bom) {
  const embedded = verifyEmbeddedBom(outputDir, platform, bom.digest);
  verifyLocalReleaseChannel(outputDir, platform, version);
  const artifacts = collectArtifacts(outputDir, platform, version);
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

async function main() {
  const args = process.argv.slice(2);
  let platform = null;
  let bomPath = process.env.KOSMOS_RELEASE_BOM ?? null;
  let dryRun = false;
  let receiptPath = null;
  let skipPreflight = false;
  let local = process.env.KOSMOS_RELEASE_LOCAL === "1";

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--platform") {
      platform = args[++i];
    } else if (args[i] === "--bom") {
      bomPath = args[++i];
    } else if (args[i] === "--receipt") {
      receiptPath = args[++i];
    } else if (args[i] === "--dry-run") {
      dryRun = true;
    } else if (args[i] === "--skip-preflight") {
      skipPreflight = true;
    } else if (args[i] === "--local") {
      local = true;
    }
  }

  if (!platform) {
    die("--platform <win|mac> is required");
  }
  if (!VALID_PLATFORMS.includes(platform)) {
    die(`Unknown platform "${platform}". Valid: ${VALID_PLATFORMS.join(", ")}`);
  }
  if (!bomPath) die("--bom <path> or KOSMOS_RELEASE_BOM is required for release builds");

  const preflight = skipPreflight
    ? {
        platform,
        version: JSON.parse(readFileSync(path.join(SHELL_ROOT, "release-versions.json"), "utf8"))[
          platform
        ],
        currentCommit: currentCommit(),
        bom: await loadReleaseBom(bomPath, {
          root: path.resolve(SHELL_ROOT, ".."),
          platform,
          currentCommit: currentCommit(),
        }),
      }
    : await runReleasePreflight({ platform, bomPath, local });
  const { version, bom } = preflight;
  receiptPath ??= path.join(SHELL_ROOT, "release", "release-receipt.v1.json");

  // preflight: everything here is cheap and must happen before compilation.
  if (!skipPreflight) log("Preflight: source, BOM, pins, and ARK artifact");
  log(`Platform: ${platform}`);
  log(`Version:  ${version}`);
  log(`BOM:      ${bom.value.id} (${bom.digest})`);
  log("");
  if (dryRun) {
    log("Dry-run plan:");
    log(`  build: electron-builder --${platform} --publish never`);
    log("  verify: local channel, BOM, provenance, first-party contracts, receipt");
    log(
      `  publish: node scripts/publish-release.mjs --platform ${platform} --receipt ${receiptPath}`,
    );
    return;
  }
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
  log("");
  log("electron-builder succeeded. Emitting release provenance...");
  const releaseFiles = await emitProvenance(
    path.join(SHELL_ROOT, "release"),
    platform,
    version,
    bom,
  );
  runFirstPartyContracts(platform);
  const receipt = await createReceipt({
    outputDir: path.join(SHELL_ROOT, "release"),
    platform,
    version,
    currentCommit: currentCommit(),
    bom,
    files: [...releaseFiles.artifactFiles, ...releaseFiles.metadataFiles],
  });
  await writeReceipt(receiptPath, receipt);
  log(`Verification receipt: ${receiptPath}`);
  log(`Build + verify complete for ${platform} v${version}. Run publish-release.mjs explicitly.`);
}
main().catch((error) => die(error instanceof Error ? error.message : String(error)));
