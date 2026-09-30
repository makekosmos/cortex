#!/usr/bin/env node

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
} from "node:fs";
import path from "node:path";
import { deriveReleaseBom, RELEASE_BOM_FILE } from "./release-bom.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import { bytes, documentHash, writeAtomic } from "./release-utils.mjs";
import { createReceipt, RELEASE_RECEIPT_FILE, writeReceipt } from "./release-receipt.mjs";
import { getVersion } from "./release-version.mjs";
import { ensureNsis } from "./ensure-nsis.mjs";
import { env, MANAGER_EXE } from "./brand.mjs";
import {
  currentCommit,
  runReleasePreflight,
  SHELL_ROOT as PREFLIGHT_ROOT,
} from "./release-preflight.mjs";

const SHELL_ROOT = PREFLIGHT_ROOT;
const VALID_PLATFORMS = ["win"];
function die(msg) {
  console.error(`[build-desktop] FATAL: ${msg}`);
  process.exit(1);
}

function log(...args) {
  console.log("[build-desktop]", ...args);
}

function collectArtifacts(outputDir, platform, version) {
  const channel = "latest.yml";
  const names = readdirSync(outputDir).filter((name) => {
    if (name === channel) return true;
    if (!name.includes(version)) return false;
    return /\.(?:exe|json)$/i.test(name);
  });
  const artifacts = names.map((name) => {
    const file = path.join(outputDir, name);
    return { name, sha256: documentHash(readFileSync(file)), size: statSync(file).size };
  });
  if (!artifacts.some(({ name }) => name.endsWith(".exe")))
    die(`no installer artifact found in ${outputDir}`);
  if (!artifacts.some(({ name }) => name === channel)) die(`missing ${channel} in ${outputDir}`);
  return artifacts;
}

async function emitProvenance(outputDir, platform, version, bom) {
  verifyLocalReleaseChannel(outputDir, platform, version);
  const artifacts = collectArtifacts(outputDir, platform, version);
  const provenance = {
    schema_version: 1,
    bom_id: bom.value.id,
    bom_digest: bom.digest,
    platform,
    version,
    artifacts,
  };
  const file = path.join(outputDir, "release-provenance.json");
  const bomFile = path.join(outputDir, RELEASE_BOM_FILE);
  await writeAtomic(bomFile, bom.bytes);
  await writeAtomic(file, bytes(provenance));
  log(`Release provenance: ${file}`);
  return {
    metadataFiles: [bomFile, file],
    artifactFiles: artifacts.map(({ name }) => path.join(outputDir, name)),
  };
}

function stageInstaller() {
  const stage = path.join(SHELL_ROOT, ".tmp", "installer-stage");
  rmSync(stage, { recursive: true, force: true });
  const resources = path.join(stage, "resources");
  mkdirSync(resources, { recursive: true });

  const engineDir = path.join(SHELL_ROOT, ".tmp", "engine.next");
  for (const [sourceName, targetName] of [
    ["Mundus-Engine.zip", "Mundus Engine.zip"],
    ["engine-manifest.json", "engine-manifest.json"],
  ]) {
    const source = path.join(engineDir, sourceName);
    if (!existsSync(source)) die(`missing engine artifact: ${source}`);
    copyFileSync(source, path.join(resources, targetName));
  }
  for (const script of ["install-engine.ps1", "engine-post-install.ps1"]) {
    copyFileSync(path.join(SHELL_ROOT, "build", script), path.join(resources, script));
  }
  copyFileSync(path.join(SHELL_ROOT, "build", "icon.ico"), path.join(resources, "icon.ico"));
  copyFileSync(path.join(SHELL_ROOT, "build", "tray.ico"), path.join(resources, "tray.ico"));
  cpSync(path.join(SHELL_ROOT, "build", "installer-assets"), path.join(stage, "installer-assets"), {
    recursive: true,
  });

  const componentsDir = path.join(resources, "components");
  // Manager is the only bundled component — a missing stage is a build
  // failure, never a silent skip.
  const managerSource = path.join(SHELL_ROOT, ".tmp", "components", "manager", "win-unpacked");
  if (!existsSync(managerSource)) die(`missing manager component: ${managerSource}`);
  cpSync(managerSource, path.join(componentsDir, "manager"), { recursive: true });

  // Fail closed: the only bundled component must be present under exactly the
  // packaged executable name, and no stale sibling exe can be left behind after
  // the rmSync above.
  const managerDir = path.join(componentsDir, "manager");
  const exeFiles = readdirSync(managerDir).filter((name) => name.toLowerCase().endsWith(".exe"));
  if (exeFiles.length !== 1 || exeFiles[0].toLowerCase() !== MANAGER_EXE.toLowerCase()) {
    die(
      `staged manager payload must contain exactly ${MANAGER_EXE}; found: ${exeFiles.join(", ")}`,
    );
  }
  return stage;
}

function sha512Base64(file) {
  const hash = createHash("sha512");
  hash.update(readFileSync(file));
  return hash.digest("base64");
}

async function buildWindows(version) {
  const stage = stageInstaller();
  const releaseDir = path.join(SHELL_ROOT, "release");
  mkdirSync(releaseDir, { recursive: true });
  const outFile = path.join(releaseDir, `Mundus-Setup-${version}.exe`);

  const { makensis, env } = await ensureNsis();
  const args = [
    // installer.nsi is UTF-8 without a BOM (it carries Russian strings).
    "/INPUTCHARSET",
    "UTF8",
    `/DVERSION=${version}`,
    `/DSTAGE_DIR=${stage}`,
    `/DOUT_FILE=${outFile}`,
    `/DMANAGER_EXE=${MANAGER_EXE}`,
    path.join(SHELL_ROOT, "build", "installer.nsi"),
  ];
  log(`makensis: ${makensis}`);
  log(`Running: ${makensis} ${args.join(" ")}`);
  const result = spawnSync(makensis, args, {
    cwd: SHELL_ROOT,
    stdio: "inherit",
    windowsHide: true,
    env: { ...process.env, ...env },
  });
  if (result.status !== 0) {
    console.error("");
    console.error(`[build-desktop] makensis exited with code ${result.status ?? "(signal)"}`);
    process.exit(result.status ?? 1);
  }

  // electron-updater / Engine updater channel file.
  const installerSha512 = sha512Base64(outFile);
  const installerSize = statSync(outFile).size;
  const latest = [
    `version: ${version}`,
    "files:",
    `  - url: Mundus-Setup-${version}.exe`,
    `    sha512: ${installerSha512}`,
    `    size: ${installerSize}`,
    `path: Mundus-Setup-${version}.exe`,
    `sha512: ${installerSha512}`,
    `releaseDate: '${new Date().toISOString()}'`,
    "",
  ].join("\n");
  await writeAtomic(path.join(releaseDir, "latest.yml"), latest);
  log(`Installer: ${outFile} (${installerSize} bytes)`);
  return { releaseDir, outFile };
}

async function main() {
  const args = process.argv.slice(2);
  let platform = null;
  let dryRun = false;
  let receiptPath = null;
  let skipPreflight = false;
  let packageDir = false;
  let local = env("RELEASE_LOCAL") === "1";

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--platform") {
      platform = args[++i];
    } else if (args[i] === "--receipt") {
      receiptPath = args[++i];
    } else if (args[i] === "--dry-run") {
      dryRun = true;
    } else if (args[i] === "--skip-preflight") {
      skipPreflight = true;
    } else if (args[i] === "--local") {
      local = true;
    } else if (args[i] === "--package-dir") {
      packageDir = true;
    } else {
      die(`Unknown argument "${args[i]}"`);
    }
  }

  // Windows is the only supported package target; default to it so
  // `pnpm run build:desktop -- --local` works without extra flags.
  platform ??= "win";
  if (!VALID_PLATFORMS.includes(platform))
    die(`Unknown platform "${platform}". Valid: ${VALID_PLATFORMS.join(", ")}`);

  // A local build is a throwaway installer off any ref: no BOM, no receipt.
  // A release build derives its BOM from HEAD — through the full preflight, or
  // directly when the caller (`pnpm run build`) has just run the preflight.
  let version;
  let bom = null;
  if (local) {
    version = getVersion(platform);
  } else if (skipPreflight) {
    version = getVersion(platform);
    bom = await deriveReleaseBom(path.resolve(SHELL_ROOT, ".."), platform, currentCommit());
  } else {
    ({ version, bom } = await runReleasePreflight({ platform }));
    log("Preflight: source, BOM, and pins");
  }
  receiptPath ??= path.join(SHELL_ROOT, "release", RELEASE_RECEIPT_FILE);

  log(`Platform: ${platform}`);
  log(`Version:  ${version}`);
  if (bom) log(`BOM:      ${bom.value.id} (${bom.digest})`);
  log("");
  if (dryRun) {
    log("Dry-run plan:");
    log(`  build: NSIS installer via makensis`);
    if (!bom) {
      log("  local build: no BOM, receipt, or publish");
      return;
    }
    log("  verify: local channel, BOM, provenance, receipt");
    log(
      `  publish: node scripts/publish-release.mjs --platform ${platform} --receipt ${receiptPath}`,
    );
    return;
  }
  if (packageDir) {
    const stage = stageInstaller();
    log(`Staged installer payload at ${stage}`);
    return;
  }

  const { releaseDir, outFile } = await buildWindows(version);
  log("");
  if (bom) {
    log("NSIS installer built. Emitting release provenance...");
    const releaseFiles = await emitProvenance(releaseDir, platform, version, bom);
    const receipt = await createReceipt({
      outputDir: releaseDir,
      platform,
      version,
      currentCommit: currentCommit(),
      bom,
      files: [...releaseFiles.artifactFiles, ...releaseFiles.metadataFiles],
    });
    await writeReceipt(receiptPath, receipt);
    log(`Verification receipt: ${receiptPath}`);
    log(`Build + verify complete for ${platform} v${version}. Run publish-release.mjs explicitly.`);
  } else {
    log(`Local build complete: ${outFile}`);
  }
}
main().catch((error) => die(error instanceof Error ? error.message : String(error)));
