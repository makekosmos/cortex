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

// KOS-306: every exe the installer ships (or the Engine installs) must carry
// a VERSIONINFO resource — bare exes are a Defender first-sight ML feature.
// Read on the build machine via Get-Item; fails the build on any gap.
function assertVersionInfo(file, expectedVersion) {
  const probe = spawnSync(
    "powershell",
    [
      "-NoProfile",
      "-Command",
      `(Get-Item -LiteralPath '${file.replaceAll("'", "''")}').VersionInfo | ConvertTo-Json`,
    ],
    { encoding: "utf8" },
  );
  if (probe.status !== 0) die(`cannot read VERSIONINFO of ${file}: ${probe.stderr}`);
  const info = JSON.parse(probe.stdout);
  for (const key of ["CompanyName", "ProductName", "FileDescription"]) {
    if (!info[key]) die(`${file}: VERSIONINFO ${key} is empty`);
  }
  if (info.ProductName !== "Mundus") die(`${file}: ProductName ${info.ProductName} != Mundus`);
  if (info.LegalCopyright !== "Copyright (C) Kazui")
    die(`${file}: LegalCopyright ${info.LegalCopyright} != Copyright (C) Kazui`);
  if (String(info.FileVersion).trim() !== expectedVersion)
    die(`${file}: FileVersion ${info.FileVersion} != ${expectedVersion}`);
}

// KOS-306 round 2: every shipped exe must carry exactly one application
// manifest with requestedExecutionLevel asInvoker — the Engine's comes from
// pe-version-info, the Manager's from gpui-pre's windows-manifest feature.
function assertApplicationManifest(file) {
  const data = readFileSync(file);
  const marker = Buffer.from("urn:schemas-microsoft-com:asm.v1");
  let hits = 0;
  for (let i = data.indexOf(marker); i !== -1; i = data.indexOf(marker, i + 1)) hits++;
  if (hits !== 1) die(`${file}: expected exactly one application manifest, found ${hits}`);
  if (!data.includes('requestedExecutionLevel level="asInvoker"'))
    die(`${file}: application manifest lacks requestedExecutionLevel asInvoker`);
}

function assertNoPowerShellPayload(stage) {
  const staged = readdirSync(stage, { recursive: true }).map(String);
  const offenders = staged.filter((name) => name.toLowerCase().endsWith(".ps1"));
  if (offenders.length) die(`staged payload must not contain .ps1: ${offenders.join(", ")}`);
}

function stageInstaller(version) {
  const stage = path.join(SHELL_ROOT, ".tmp", "installer-stage");
  rmSync(stage, { recursive: true, force: true });
  const resources = path.join(stage, "resources");
  mkdirSync(resources, { recursive: true });

  // KOS-306: the Engine payload is staged unpacked — the installer runs the
  // staged exe's `install` subcommand in place of the removed PowerShell
  // scripts.
  const engineDir = path.join(SHELL_ROOT, ".tmp", "engine.next");
  for (const name of ["mundus-engine.exe", "tray.ico", "engine-manifest.json"]) {
    const source = path.join(engineDir, name);
    if (!existsSync(source)) die(`missing engine artifact: ${source}`);
  }
  cpSync(engineDir, path.join(resources, "engine"), { recursive: true });
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

  // KOS-306: fail the build when a shipped exe lacks VERSIONINFO, and never
  // let a .ps1 back into the payload.
  assertNoPowerShellPayload(stage);
  for (const exe of [
    path.join(resources, "engine", "mundus-engine.exe"),
    path.join(managerDir, MANAGER_EXE),
  ]) {
    assertVersionInfo(exe, version);
    assertApplicationManifest(exe);
  }
  return stage;
}

// KOS-306: static Defender gate. `-ScanType 3 -File` is a read-only custom
// scan — it catches static signatures and the local ML model, but NOT the
// cloud first-sight verdict that flagged 0.10.1 (an unknown hash has no
// reputation yet). The unpacked-payload + VERSIONINFO work is what fixes the
// cloud verdict; this gate just keeps the local model clean.
function defenderGate(outFile) {
  const mpCmdRun = path.join(
    process.env["ProgramFiles"] ?? "C:\\Program Files",
    "Windows Defender",
    "MpCmdRun.exe",
  );
  if (!existsSync(mpCmdRun)) {
    log(`Defender gate skipped: ${mpCmdRun} not found (non-Windows host?)`);
    return;
  }
  log(`Defender gate: ${mpCmdRun} -Scan -ScanType 3 -File ${outFile}`);
  const scan = spawnSync(
    mpCmdRun,
    ["-Scan", "-ScanType", "3", "-File", outFile, "-DisableRemediation"],
    { stdio: "inherit", windowsHide: true },
  );
  if ((scan.status ?? 1) !== 0)
    die(`Defender scan reported a detection in ${outFile} (exit ${scan.status})`);
}

function sha512Base64(file) {
  const hash = createHash("sha512");
  hash.update(readFileSync(file));
  return hash.digest("base64");
}

async function buildWindows(version) {
  const stage = stageInstaller(version);
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

  // KOS-306: the installer itself ships with VERSIONINFO too, then the
  // static/local-ML Defender scan gates the artifact.
  assertVersionInfo(outFile, version);
  assertApplicationManifest(outFile);
  defenderGate(outFile);

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
    const stage = stageInstaller(version);
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
