#!/usr/bin/env node
// KOS-349: stage Engine + Manager (+ Swift helpers) into Mundus.app
// (ad-hoc signed; icon + Info.plist wired) and wrap it in Mundus-<ver>.dmg
// via dmgbuild with a drag-to-Applications layout over a hidpi background.
//
// Signing / notarization: not wired yet. When Apple secrets land in the
// macOS nightly job, replace the ad-hoc codesign below with a Developer ID
// signature + notarytool (see TODO below). The DMG is still a publishable
// CI artifact for the cortex channel.
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { homedir } from "node:os";
import path, { join } from "node:path";
import { fileURLToPath } from "node:url";
import {
  dmgName,
  ENGINE_BINARY_UNIX,
  LOCAL_STT_BINARY,
  MANAGER_MAC_BIN,
  PRODUCT_NAME,
} from "./brand.mjs";
import {
  BUNDLE_ID,
  logGatekeeperAssessment,
  signMacosApp,
  verifyMacosAppSignature,
} from "./macos-codesign.mjs";
import { bytes, writeAtomic } from "./release-utils.mjs";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const desktopRoot = path.resolve(__dirname, "..");
const cortexRoot = path.resolve(desktopRoot, "..");

function die(message) {
  throw new Error(message);
}

function findBinary(candidates, label) {
  for (const candidate of candidates) {
    if (existsSync(candidate) && statSync(candidate).isFile()) return candidate;
  }
  die(`missing ${label}; looked in:\n  ${candidates.join("\n  ")}`);
}

function engineBinaryCandidates() {
  const targetDir = process.env.CARGO_TARGET_DIR
    ? path.resolve(process.env.CARGO_TARGET_DIR)
    : path.join(cortexRoot, "target");
  return [
    path.join(targetDir, "release", ENGINE_BINARY_UNIX),
    path.join(cortexRoot, "target", "release", ENGINE_BINARY_UNIX),
  ];
}

/** The local speech-to-text sidecar the Engine spawns; its absence is the
 * difference between a transcript and silence. */
function localSttCandidates() {
  const targetDir = process.env.CARGO_TARGET_DIR
    ? path.resolve(process.env.CARGO_TARGET_DIR)
    : path.join(cortexRoot, "target");
  return [
    path.join(targetDir, "release", LOCAL_STT_BINARY),
    path.join(cortexRoot, "target", "release", LOCAL_STT_BINARY),
  ];
}

function managerBinaryCandidates() {
  const targetDir = process.env.CARGO_TARGET_DIR
    ? path.resolve(process.env.CARGO_TARGET_DIR)
    : path.join(cortexRoot, "manager", "target");
  return [
    path.join(targetDir, "release", "manager"),
    path.join(cortexRoot, "manager", "target", "release", "manager"),
  ];
}

function run(cmd, args, opts = {}) {
  const result = spawnSync(cmd, args, { stdio: "inherit", ...opts });
  if ((result.status ?? 1) !== 0) {
    die(`${cmd} ${args[0] ?? ""} failed (exit ${result.status})`);
  }
}

/**
 * Build Contents/Resources/mundus.icns from the pre-masked macOS icon
 * (squircle + margins + shadow baked into build/macos/icon-1024.png —
 * sips can't alpha-mask; regenerate it with scripts/macos-icon.py).
 */
function buildIcns(appContents, workDir) {
  const source = path.join(desktopRoot, "build", "macos", "icon-1024.png");
  if (!existsSync(source)) die(`missing ${source}; run scripts/macos-icon.py`);
  const iconset = path.join(workDir, "mundus.iconset");
  rmSync(iconset, { recursive: true, force: true });
  mkdirSync(iconset, { recursive: true });
  for (const size of [16, 32, 128, 256, 512]) {
    run("sips", [
      "-z",
      String(size),
      String(size),
      source,
      "--out",
      path.join(iconset, `icon_${size}x${size}.png`),
    ]);
    run("sips", [
      "-z",
      String(size * 2),
      String(size * 2),
      source,
      "--out",
      path.join(iconset, `icon_${size}x${size}@2x.png`),
    ]);
  }
  const resources = path.join(appContents, "Resources");
  mkdirSync(resources, { recursive: true });
  run("iconutil", ["-c", "icns", iconset, "-o", path.join(resources, "mundus.icns")]);
  rmSync(iconset, { recursive: true, force: true });
}

function writeInfoPlist(appContents, version) {
  const plist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${PRODUCT_NAME}</string>
  <key>CFBundleDisplayName</key>
  <string>${PRODUCT_NAME}</string>
  <key>CFBundleIdentifier</key>
  <string>computer.mundus.app</string>
  <key>CFBundleVersion</key>
  <string>${version}</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>${MANAGER_MAC_BIN}</string>
  <key>CFBundleIconFile</key>
  <string>mundus</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
  <!-- TCC refuses microphone access to a process whose bundle carries no
       usage string, so without this the Engine's capture helper is denied
       silently and dictation never records (KOS-MAC-MIC). -->
  <key>NSMicrophoneUsageDescription</key>
  <string>${PRODUCT_NAME} записывает голос, чтобы превратить его в текст.</string>
  <key>NSSpeechRecognitionUsageDescription</key>
  <string>${PRODUCT_NAME} распознаёт записанную речь.</string>
</dict>
</plist>
`;
  writeFileSync(path.join(appContents, "Info.plist"), plist);
}

/**
 * Stage Mundus.app under `stageDir` and return the .app path.
 * Layout matches Manager's engine_candidates + Engine's native helper lookup:
 *   Contents/MacOS/Mundus
 *   Contents/MacOS/mundus-engine
 *   Contents/MacOS/mundus-local-stt
 *   Contents/MacOS/native/macos/<helpers>
 */
export function stageMacosApp({ version, stageDir }) {
  rmSync(stageDir, { recursive: true, force: true });
  const appName = `${PRODUCT_NAME}.app`;
  const appRoot = path.join(stageDir, appName);
  const contents = path.join(appRoot, "Contents");
  const macos = path.join(contents, "MacOS");
  const nativeDir = path.join(macos, "native", "macos");
  mkdirSync(macos, { recursive: true });
  mkdirSync(nativeDir, { recursive: true });

  const engineSrc = findBinary(engineBinaryCandidates(), ENGINE_BINARY_UNIX);
  const managerSrc = findBinary(managerBinaryCandidates(), "manager");
  const engineDst = path.join(macos, ENGINE_BINARY_UNIX);
  const managerDst = path.join(macos, MANAGER_MAC_BIN);
  copyFileSync(engineSrc, engineDst);
  copyFileSync(managerSrc, managerDst);
  chmodSync(engineDst, 0o755);
  chmodSync(managerDst, 0o755);

  // Local dictation: without this file the Engine transcribes into nothing.
  const sttSrc = findBinary(localSttCandidates(), LOCAL_STT_BINARY);
  const sttDst = path.join(macos, LOCAL_STT_BINARY);
  copyFileSync(sttSrc, sttDst);
  chmodSync(sttDst, 0o755);

  const helpersSrc = path.join(desktopRoot, ".tmp", "native", "macos");
  if (existsSync(helpersSrc)) {
    for (const name of readdirSync(helpersSrc)) {
      const src = path.join(helpersSrc, name);
      if (!statSync(src).isFile()) continue;
      const dst = path.join(nativeDir, name);
      copyFileSync(src, dst);
      chmodSync(dst, 0o755);
    }
  }

  writeInfoPlist(contents, version);

  // Icon + signature are macOS-only steps; staging on other hosts still
  // produces the lookup layout for tests.
  if (process.platform === "darwin") {
    buildIcns(contents, stageDir);
    // TODO(KOS-349): entitlements + notarytool once the Apple secrets land in
    // the macOS nightly job. CODESIGN_IDENTITY already switches the plan to
    // hardened runtime + secure timestamp inside-out signing.
    const local = localIdentity();
    signMacosApp(appRoot, {
      mainExecutableName: MANAGER_MAC_BIN,
      identity: process.env.CODESIGN_IDENTITY ?? local.identity,
      keychain: process.env.CODESIGN_KEYCHAIN ?? local.keychain,
      // A local self-signed identity has no Apple timestamp server: the secure
      // timestamp stays for a Developer ID and is skipped otherwise.
      timestamp: process.env.CODESIGN_TIMESTAMP ?? local.timestamp,
      // No entitlements file yet, and the hardened runtime would refuse the
      // microphone without one.
      hardened: Boolean(process.env.CODESIGN_IDENTITY) && !local.identity,
      requirement: local.requirement,
    });
  }
  return appRoot;
}

/**
 * The developer's own signing identity, when one was made on this Mac:
 * `~/Library/Keychains/mundus-signing.keychain-db`. macOS remembers an Input
 * Monitoring, Microphone or Accessibility grant by the certificate, so a
 * rebuild keeps them — an ad-hoc signature changes with every build and loses
 * all three. CI has no such keychain and stays on the ad-hoc path.
 */
function localIdentity() {
  const keychain = join(homedir(), "Library", "Keychains", "mundus-signing.keychain-db");
  const passwordFile = join(homedir(), ".mundus-signing.keychain-password");
  if (!existsSync(keychain) || !existsSync(passwordFile)) return {};
  const password = readFileSync(passwordFile, "utf8").trim();
  // codesign cannot read a locked keychain, and a fresh login locks it.
  spawnSync("security", ["unlock-keychain", "-p", password, keychain], { stdio: "ignore" });
  // A self-signed certificate is trusted only for this user, and macOS checks
  // privacy grants against the system trust store: a requirement that names
  // the certificate leaf verifies without it, where the default `certificate
  // root` one does not. tccd logs the mismatch as «Failed to match existing
  // code requirement».
  const found = spawnSync(
    "security",
    ["find-certificate", "-c", "Mundus Local Signing", "-Z", keychain],
    { encoding: "utf8" },
  );
  const sha1 = /SHA-1 hash: ([0-9A-F]+)/.exec(found.stdout ?? "")?.[1];
  const requirement = sha1
    ? `designated => identifier "${BUNDLE_ID}" and certificate leaf = H"${sha1}"`
    : undefined;
  return {
    identity: "Mundus Local Signing",
    keychain,
    timestamp: "--timestamp=none",
    requirement,
  };
}

function ensureDmgbuild() {
  const probe = spawnSync("python3", ["-c", "import dmgbuild"], { stdio: "ignore" });
  if ((probe.status ?? 1) === 0) return;
  const install = spawnSync("python3", ["-m", "pip", "install", "--quiet", "--user", "dmgbuild"], {
    stdio: "ignore",
  });
  if ((install.status ?? 1) === 0) return;
  run("python3", [
    "-m",
    "pip",
    "install",
    "--quiet",
    "--user",
    "--break-system-packages",
    "dmgbuild",
  ]);
}

// dmgbuild writes the .DS_Store (background, icon view, icon positions)
// directly — no Finder scripting, so it also works on headless CI runners.
const DMGBUILD_PY = `
import os
import dmgbuild

app = os.environ["MUNDUS_DMG_APP"]
app_name = os.path.basename(app)
dmgbuild.build_dmg(
    filename=os.environ["MUNDUS_DMG_OUT"],
    volume_name=os.environ["MUNDUS_DMG_VOLUME"],
    settings={
        "format": "UDZO",
        "files": [app],
        "symlinks": {"Applications": "/Applications"},
        "icon": os.path.join(app, "Contents/Resources/mundus.icns"),
        "background": os.environ["MUNDUS_DMG_BG_TIFF"],
        "show_status_bar": False,
        "show_tab_view": False,
        "show_toolbar": False,
        "show_pathbar": False,
        "show_sidebar": False,
        "default_view": "icon-view",
        # Window and icon geometry must match scripts/dmg-background.py.
        "window_rect": ((200, 120), (660, 400)),
        "icon_size": 104,
        "text_size": 12,
        "icon_locations": {app_name: (165, 195), "Applications": (495, 195)},
    },
)
`;

/**
 * Build an unsigned UDIF DMG containing the staged .app with the classic
 * drag-into-Applications layout. dmgbuild installs via pip --user when
 * missing, same as zeron's scripts/package-macos.sh.
 * TODO(KOS-349): when APPLE_DEVELOPER_ID_CERT / NOTARY_* secrets exist in CI,
 * sign the DMG and submit it with notarytool before returning.
 */
export function createUnsignedDmg({ appPath, dmgPath, volumeName }) {
  if (process.platform !== "darwin") {
    die("dmgbuild DMG packaging requires macOS (darwin)");
  }
  rmSync(dmgPath, { force: true });
  ensureDmgbuild();

  // Pair the 1x/2x background renders into a hidpi tiff so the artwork
  // stays crisp on retina displays.
  const macosAssets = path.join(desktopRoot, "build", "macos");
  const bgTiff = `${dmgPath}.background.tiff`;
  rmSync(bgTiff, { force: true });
  run("tiffutil", [
    "-cathidpicheck",
    path.join(macosAssets, "dmg-background.png"),
    path.join(macosAssets, "dmg-background@2x.png"),
    "-out",
    bgTiff,
  ]);

  try {
    run("python3", ["-c", DMGBUILD_PY], {
      env: {
        ...process.env,
        MUNDUS_DMG_APP: appPath,
        MUNDUS_DMG_OUT: dmgPath,
        MUNDUS_DMG_VOLUME: volumeName,
        MUNDUS_DMG_BG_TIFF: bgTiff,
      },
    });
  } finally {
    rmSync(bgTiff, { force: true });
  }
  if (!existsSync(dmgPath)) die(`DMG was not created: ${dmgPath}`);
  return dmgPath;
}

/**
 * Prove the signature survives the DMG round-trip: mount the finished image
 * read-only and re-verify the sealed .app inside. hdiutil output goes to the
 * build log; the codesign verdict is the fail-closed gate.
 */
export function verifyDmgSignature(dmgPath, appName = `${PRODUCT_NAME}.app`) {
  if (process.platform !== "darwin") {
    die("DMG signature verification requires macOS (darwin)");
  }
  const mountPoint = `${dmgPath}.mnt`;
  rmSync(mountPoint, { recursive: true, force: true });
  mkdirSync(mountPoint, { recursive: true });
  try {
    run("hdiutil", ["attach", "-nobrowse", "-readonly", "-mountpoint", mountPoint, dmgPath]);
    const staged = path.join(mountPoint, appName);
    if (!existsSync(staged)) die(`DMG does not contain ${appName}: ${dmgPath}`);
    verifyMacosAppSignature(staged);
    logGatekeeperAssessment(staged);
  } finally {
    spawnSync("hdiutil", ["detach", mountPoint, "-quiet"], { stdio: "ignore" });
    rmSync(mountPoint, { recursive: true, force: true });
  }
}

/**
 * Full mac packaging: stage .app → DMG under desktop/release. manifest.json
 * is written by build-desktop.mjs from the DMG bytes (KOS-350).
 */
export async function packageMacosDmg(
  version,
  { releaseDir = path.join(desktopRoot, "release") } = {},
) {
  mkdirSync(releaseDir, { recursive: true });
  const stageDir = path.join(desktopRoot, ".tmp", "macos-dmg-stage");
  const appPath = stageMacosApp({ version, stageDir });
  const fileName = dmgName(version);
  const dmgPath = path.join(releaseDir, fileName);
  createUnsignedDmg({
    appPath,
    dmgPath,
    volumeName: `${PRODUCT_NAME} ${version}`,
  });
  verifyDmgSignature(dmgPath);
  // Record that this artifact is unsigned so operators do not mistake it for
  // a notarized build. Not uploaded to the GitHub release.
  await writeAtomic(
    path.join(releaseDir, "macos-signing-status.json"),
    bytes({
      signed: false,
      notarized: false,
      // TODO(KOS-349): set true once Apple Developer ID + notary secrets are
      // wired into the macOS nightly job and createUnsignedDmg gains a signed path.
      todo: "Wire codesign + notarytool when Apple secrets are available; do not invent certs.",
    }),
  );
  return { releaseDir, dmgPath, fileName, appPath };
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const version = process.argv[2];
  if (!version) {
    console.error("usage: package-macos-dmg.mjs <version>");
    process.exit(1);
  }
  packageMacosDmg(version)
    .then(({ dmgPath }) => {
      console.log(`[package-macos-dmg] ${dmgPath}`);
    })
    .catch((error) => {
      console.error(`[package-macos-dmg] FATAL: ${error instanceof Error ? error.message : error}`);
      process.exitCode = 1;
    });
}
