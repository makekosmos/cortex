#!/usr/bin/env node
// KOS-378: Gatekeeper/codesign verdicts for a Mundus DMG, macOS-only.
//
//   node macos-gatekeeper-check.mjs check <dmg>            inspect + fail on a broken seal
//   node macos-gatekeeper-check.mjs resign <dmg> <out.dmg> re-sign the app inside with the
//                                                         release signing plan, rebuild the
//                                                         DMG through createUnsignedDmg, then
//                                                         run `check` on the result
//
// `check` mounts the image read-only, ditto-copies the .app out (preserving
// signatures), stamps a browser-style com.apple.quarantine xattr on the copy
// so spctl/syspolicy see the same condition as a real download, then prints:
//   codesign --verify --deep --strict --verbose=4   ← fail-closed gate
//   codesign -dvvv                                   (identity/flags/team)
//   spctl --assess --type execute -vvv               expected: rejected for
//                                                    unnotarized; fine
//   syspolicy_check distribution                     (macOS 14+, logged)
// Only the codesign verdict gates the exit code — ad-hoc builds are SUPPOSED
// to be rejected by spctl; what must never appear is an invalid seal.
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { MANAGER_MAC_BIN, PRODUCT_NAME } from "./brand.mjs";
import { signMacosApp } from "./macos-codesign.mjs";
import { createUnsignedDmg } from "./package-macos-dmg.mjs";

const APP_NAME = `${PRODUCT_NAME} Manager.app`;

function run(cmd, args, { check = false } = {}) {
  const result = spawnSync(cmd, args, { encoding: "utf8" });
  const out = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
  console.log(`$ ${cmd} ${args.join(" ")}`);
  if (out) console.log(out);
  if (check && (result.status ?? 1) !== 0) {
    throw new Error(`${cmd} ${args[0] ?? ""} failed (exit ${result.status})`);
  }
  return { status: result.status ?? 1, out };
}

function attach(dmgPath, mountPoint) {
  run("hdiutil", ["attach", "-nobrowse", "-readonly", "-mountpoint", mountPoint, dmgPath], {
    check: true,
  });
}

function detach(mountPoint) {
  spawnSync("hdiutil", ["detach", mountPoint, "-quiet"], { stdio: "ignore" });
}

/** Print every verdict for the .app inside `dmgPath`; throw on a broken seal. */
function checkDmg(dmgPath) {
  const work = mkdtempSync(path.join(tmpdir(), "mundus-gk-"));
  const mountPoint = path.join(work, "mnt");
  try {
    attach(dmgPath, mountPoint);
    const staged = path.join(mountPoint, APP_NAME);
    if (!existsSync(staged)) throw new Error(`DMG lacks ${APP_NAME}`);
    const appCopy = path.join(work, APP_NAME);
    run("ditto", [staged, appCopy], { check: true });
    const quarantine = `0081;${Math.floor(Date.now() / 1000).toString(16)};Safari;`;
    run("xattr", ["-w", "com.apple.quarantine", quarantine, "-r", appCopy], { check: true });
    const verify = run("codesign", ["--verify", "--deep", "--strict", "--verbose=4", appCopy]);
    run("codesign", ["-dvvv", appCopy]);
    run("spctl", ["--assess", "--type", "execute", "-vvv", appCopy]);
    run("syspolicy_check", ["distribution", appCopy]);
    if (verify.status !== 0) {
      throw new Error(`codesign --verify --deep --strict failed for ${dmgPath}`);
    }
    console.log(`[gatekeeper-check] PASS: seal valid in ${dmgPath}`);
  } finally {
    detach(mountPoint);
    rmSync(work, { recursive: true, force: true });
  }
}

/** Re-sign the .app inside `inDmg` with the release plan into `outDmg`. */
function resignDmg(inDmg, outDmg) {
  const work = mkdtempSync(path.join(tmpdir(), "mundus-resign-"));
  const mountPoint = path.join(work, "mnt");
  try {
    attach(inDmg, mountPoint);
    const staged = path.join(mountPoint, APP_NAME);
    if (!existsSync(staged)) throw new Error(`DMG lacks ${APP_NAME}`);
    const appCopy = path.join(work, APP_NAME);
    run("ditto", [staged, appCopy], { check: true });
    signMacosApp(appCopy, {
      mainExecutableName: MANAGER_MAC_BIN,
      identity: process.env.CODESIGN_IDENTITY,
    });
    createUnsignedDmg({
      appPath: appCopy,
      dmgPath: outDmg,
      volumeName: path.basename(outDmg, ".dmg"),
    });
  } finally {
    detach(mountPoint);
    rmSync(work, { recursive: true, force: true });
  }
}

const [, , mode, a, b] = process.argv;
if (process.platform !== "darwin") {
  console.log("[gatekeeper-check] non-darwin host; nothing to check");
  process.exit(0);
}
try {
  if (mode === "check" && a) {
    checkDmg(path.resolve(a));
  } else if (mode === "resign" && a && b) {
    resignDmg(path.resolve(a), path.resolve(b));
    checkDmg(path.resolve(b));
  } else {
    console.error("usage: macos-gatekeeper-check.mjs check <dmg> | resign <in.dmg> <out.dmg>");
    process.exitCode = 1;
  }
} catch (error) {
  console.error(`[gatekeeper-check] FATAL: ${error instanceof Error ? error.message : error}`);
  process.exitCode = 1;
}
