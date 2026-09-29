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
import { loadReleaseBom } from "./release-bom.mjs";
import { verifyLocalReleaseChannel } from "./release-channel-local.mjs";
import { bytes, documentHash, writeAtomic } from "./package-release-utils.mjs";
import { createReceipt, writeReceipt } from "./release-receipt.mjs";
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
  verifyLocalReleaseChannel(outputDir, platform, version);
  const artifacts = collectArtifacts(outputDir, platform, version);
  compareExpectedArtifacts(bom.value.artifacts, artifacts);
  const provenance = {
    schema_version: 1,
    bom_id: bom.value.id,
    bom_digest: bom.digest,
    platform,
    version,
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
  let bomPath = env("RELEASE_BOM") ?? null;
  let dryRun = false;
  let receiptPath = null;
  let skipPreflight = false;
  let packageDir = false;
  let local = env("RELEASE_LOCAL") === "1";

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
    } else if (args[i] === "--package-dir") {
      packageDir = true;
    }
  }

  // Windows is the only supported package target; default to it so
  // `pnpm run build:desktop -- --local` works without extra flags.
  platform ??= "win";
  if (!VALID_PLATFORMS.includes(platform))
    die(`Unknown platform "${platform}". Valid: ${VALID_PLATFORMS.join(", ")}`);
  if (local && bomPath && !existsSync(bomPath)) bomPath = null;

  let version;
  let bom = null;
  if (local && !bomPath) {
    version = JSON.parse(readFileSync(path.join(SHELL_ROOT, "release-versions.json"), "utf8"))[
      platform
    ];
  } else {
    if (!bomPath) die("--bom <path> or MUNDUS_RELEASE_BOM is required for release builds");
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
    version = preflight.version;
    bom = preflight.bom;
  }
  receiptPath ??= path.join(SHELL_ROOT, "release", "release-receipt.v1.json");

  if (!skipPreflight) log("Preflight: source, BOM, and pins");
  log(`Platform: ${platform}`);
  log(`Version:  ${version}`);
  if (bom) log(`BOM:      ${bom.value.id} (${bom.digest})`);
  log("");
  if (dryRun) {
    log("Dry-run plan:");
    log(`  build: NSIS installer via makensis`);
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
