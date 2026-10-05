#!/usr/bin/env node

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
import { deriveReleaseBom } from "./release-bom.mjs";
import { emitReleaseFeeds } from "./release-feeds.mjs";
import { createReceipt, RELEASE_RECEIPT_FILE, writeReceipt } from "./release-receipt.mjs";
import { readReleaseVersion } from "./release-version.mjs";
import { ensureNsis } from "./ensure-nsis.mjs";
import { env, MANAGER_EXE } from "./brand.mjs";
import { packageMacosDmg } from "./package-macos-dmg.mjs";
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

// KOS-350: release assets are installer + manifest.json (+ legacy feeds
// during dual-publish) — see release-feeds.mjs. Provenance and receipt stay
// local for the build→publish handoff and are not uploaded to GitHub.
function emitReleaseMetadata(outputDir, platform, version, bom, installerFile, { local }) {
  return emitReleaseFeeds({ outputDir, platform, version, bom, installerFile, local, log });
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
// manifest with requestedExecutionLevel asInvoker — both the Engine's and
// the Manager's come from pe-version-info (KOS-347: the imago gpui pin no
// longer ships a `windows-manifest` feature).
function assertApplicationManifest(file, { commonControls = false } = {}) {
  const data = readFileSync(file);
  const marker = Buffer.from("urn:schemas-microsoft-com:asm.v1");
  let hits = 0;
  for (let i = data.indexOf(marker); i !== -1; i = data.indexOf(marker, i + 1)) hits++;
  if (hits !== 1) die(`${file}: expected exactly one application manifest, found ${hits}`);
  if (!data.includes('requestedExecutionLevel level="asInvoker"'))
    die(`${file}: application manifest lacks requestedExecutionLevel asInvoker`);
  // gpui imports TaskDialogIndirect, exported only by comctl32 v6: a GUI exe
  // without the Common-Controls v6 dependency fails to load on Windows.
  if (
    commonControls &&
    !data.includes('name="Microsoft.Windows.Common-Controls" version="6.0.0.0"')
  )
    die(`${file}: GUI manifest lacks the Common-Controls v6 dependency`);
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
  for (const [exe, gui] of [
    [path.join(resources, "engine", "mundus-engine.exe"), false],
    [path.join(managerDir, MANAGER_EXE), true],
  ]) {
    assertVersionInfo(exe, version);
    assertApplicationManifest(exe, { commonControls: gui });
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

  const installerSize = statSync(outFile).size;
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

  // Default to Windows so `pnpm run build -- --local` keeps working.
  // Pass `--platform mac` on a macOS host / nightly mac job (KOS-349).
  platform ??= "win";
  if (!VALID_PLATFORMS.includes(platform))
    die(`Unknown platform "${platform}". Valid: ${VALID_PLATFORMS.join(", ")}`);

  // A local build is a throwaway installer off any ref: no BOM, no receipt.
  // A release build derives its BOM from HEAD — through the full preflight, or
  // directly when the caller (`pnpm run build`) has just run the preflight.
  let version;
  let bom = null;
  // --skip-preflight wins over --local: CI may set MUNDUS_RELEASE_LOCAL while
  // still wanting BOM/receipt for publish (mac nightly on a dirty pin bump).
  if (skipPreflight) {
    version = readReleaseVersion({ platform });
    bom = await deriveReleaseBom(path.resolve(SHELL_ROOT, ".."), platform, currentCommit());
  } else if (local) {
    version = readReleaseVersion({ platform });
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
    log(
      `  build: ${platform === "mac" ? "unsigned Mundus-<ver>.dmg via hdiutil" : "NSIS installer via makensis"}`,
    );
    if (!bom) {
      log("  local build: manifest.json (+ legacy yml); no BOM, receipt, or publish");
      return;
    }
    log("  verify: manifest.json (+ legacy feeds), BOM, receipt (local handoff only)");
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

  let releaseDir;
  let outFile;
  if (platform === "mac") {
    if (process.platform !== "darwin")
      die("mac packaging requires macOS (darwin) — run the nightly mac job or a Mac host");
    log(
      "Packaging unsigned Mundus-<ver>.dmg (signing/notarization TODO when Apple secrets exist)...",
    );
    ({ releaseDir, dmgPath: outFile } = await packageMacosDmg(version));
  } else {
    ({ releaseDir, outFile } = await buildWindows(version));
  }
  log("");
  if (bom) {
    log("Installer built. Emitting manifest.json (+ legacy feeds) + local receipt...");
    const { files } = await emitReleaseMetadata(releaseDir, platform, version, bom, outFile, {
      local: false,
    });
    const receipt = await createReceipt({
      outputDir: releaseDir,
      platform,
      version,
      currentCommit: currentCommit(),
      bom,
      files,
    });
    await writeReceipt(receiptPath, receipt);
    log(`Verification receipt (local only): ${receiptPath}`);
    log(`Build + verify complete for ${platform} v${version}. Run publish-release.mjs explicitly.`);
  } else {
    // Local builds still get manifest.json (+ legacy yml) so the Engine
    // updater can be pointed at a local feed; no BOM file, no receipt.
    try {
      const localBom = await deriveReleaseBom(
        path.resolve(SHELL_ROOT, ".."),
        platform,
        currentCommit(),
      );
      await emitReleaseMetadata(releaseDir, platform, version, localBom, outFile, { local: true });
    } catch (error) {
      log(
        `warning: skipped local manifest.json (${error instanceof Error ? error.message : error})`,
      );
    }
    log(`Local build complete: ${outFile}`);
  }
}
main().catch((error) => die(error instanceof Error ? error.message : String(error)));
