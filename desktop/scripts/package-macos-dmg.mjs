#!/usr/bin/env node
// KOS-349: stage Engine + Manager (+ Swift helpers) into an unsigned
// Mundus Manager.app and wrap it in Mundus-<ver>.dmg via hdiutil.
//
// Signing / notarization: not wired yet. When Apple secrets land in the
// macOS nightly job, add codesign + notarytool here (see TODO below). An
// unsigned DMG is still a publishable CI artifact for the cortex channel.
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { dmgName, ENGINE_BINARY_UNIX, MANAGER_MAC_BIN, PRODUCT_NAME } from "./brand.mjs";
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

function managerBinaryCandidates() {
  const targetDir = process.env.CARGO_TARGET_DIR
    ? path.resolve(process.env.CARGO_TARGET_DIR)
    : path.join(cortexRoot, "manager-gpui", "target");
  return [
    path.join(targetDir, "release", "manager-gpui"),
    path.join(cortexRoot, "manager-gpui", "target", "release", "manager-gpui"),
  ];
}

function writeInfoPlist(appContents, version) {
  const plist = `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>
  <string>${PRODUCT_NAME} Manager</string>
  <key>CFBundleDisplayName</key>
  <string>${PRODUCT_NAME} Manager</string>
  <key>CFBundleIdentifier</key>
  <string>com.kazui.mundus.manager</string>
  <key>CFBundleVersion</key>
  <string>${version}</string>
  <key>CFBundleShortVersionString</key>
  <string>${version}</string>
  <key>CFBundlePackageType</key>
  <string>APPL</string>
  <key>CFBundleExecutable</key>
  <string>${MANAGER_MAC_BIN}</string>
  <key>LSMinimumSystemVersion</key>
  <string>13.0</string>
  <key>NSHighResolutionCapable</key>
  <true/>
</dict>
</plist>
`;
  writeFileSync(path.join(appContents, "Info.plist"), plist);
}

/**
 * Stage Mundus Manager.app under `stageDir` and return the .app path.
 * Layout matches Manager's engine_candidates + Engine's native helper lookup:
 *   Contents/MacOS/Mundus Manager
 *   Contents/MacOS/mundus-engine
 *   Contents/MacOS/native/macos/<helpers>
 */
export function stageMacosApp({ version, stageDir }) {
  rmSync(stageDir, { recursive: true, force: true });
  const appName = `${PRODUCT_NAME} Manager.app`;
  const appRoot = path.join(stageDir, appName);
  const contents = path.join(appRoot, "Contents");
  const macos = path.join(contents, "MacOS");
  const nativeDir = path.join(macos, "native", "macos");
  mkdirSync(macos, { recursive: true });
  mkdirSync(nativeDir, { recursive: true });

  const engineSrc = findBinary(engineBinaryCandidates(), ENGINE_BINARY_UNIX);
  const managerSrc = findBinary(managerBinaryCandidates(), "manager-gpui");
  const engineDst = path.join(macos, ENGINE_BINARY_UNIX);
  const managerDst = path.join(macos, MANAGER_MAC_BIN);
  copyFileSync(engineSrc, engineDst);
  copyFileSync(managerSrc, managerDst);
  chmodSync(engineDst, 0o755);
  chmodSync(managerDst, 0o755);

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
  return appRoot;
}

/**
 * Build an unsigned UDIF DMG containing the staged .app.
 * TODO(KOS-349): when APPLE_DEVELOPER_ID_CERT / NOTARY_* secrets exist in CI,
 * codesign the .app + DMG and submit with notarytool before returning.
 */
export function createUnsignedDmg({ appPath, dmgPath, volumeName }) {
  if (process.platform !== "darwin") {
    die("hdiutil DMG packaging requires macOS (darwin)");
  }
  rmSync(dmgPath, { force: true });
  const staging = `${dmgPath}.stage`;
  rmSync(staging, { recursive: true, force: true });
  mkdirSync(staging, { recursive: true });
  cpSync(appPath, path.join(staging, path.basename(appPath)), { recursive: true });

  // Unsigned publishable artifact. Do not invent Apple certs here.
  const result = spawnSync(
    "hdiutil",
    ["create", "-volname", volumeName, "-srcfolder", staging, "-ov", "-format", "UDZO", dmgPath],
    { stdio: "inherit" },
  );
  rmSync(staging, { recursive: true, force: true });
  if ((result.status ?? 1) !== 0) die(`hdiutil create failed (exit ${result.status})`);
  if (!existsSync(dmgPath)) die(`DMG was not created: ${dmgPath}`);
  return dmgPath;
}

/**
 * Full mac packaging: stage .app → DMG under desktop/release. manifest.json
 * (and the legacy latest-mac.yml during dual-publish) are written by
 * build-desktop.mjs from the DMG bytes (KOS-350).
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
