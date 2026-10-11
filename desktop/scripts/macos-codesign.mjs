// KOS-378: inside-out signing for Mundus Manager.app. `codesign --deep` is
// the deprecated one-shot form; signing every nested Mach-O explicitly and
// sealing the bundle last is the order Apple's tooling expects, and it is the
// only arrangement where a post-sign edit is impossible to miss.
//
// Ad-hoc (`--sign -`) is the unsigned-release path: valid seal, no identity.
// Gatekeeper on macOS <=15 then offers the "unverified developer / Open
// Anyway" flow; on macOS 26 quarantined unnotarized apps are blocked either
// way, so first-install guidance lives in docs + the release notes, not in
// the signature. When CODESIGN_IDENTITY lands (KOS-349) the same plan gains
// hardened runtime + secure timestamp for notarization.
import { spawnSync } from "node:child_process";
import { openSync, readSync, closeSync, readdirSync, statSync } from "node:fs";
import path from "node:path";

export const BUNDLE_ID = "com.kazui.mundus.manager";

const MACHO_MAGICS = new Set([0xfeedface, 0xfeedfacf, 0xcafebabe, 0xcafebabf]);

export function isMachO(file) {
  const fd = openSync(file, "r");
  try {
    const buf = Buffer.alloc(4);
    if (readSync(fd, buf, 0, 4, 0) < 4) return false;
    // Thin magics are little-endian on disk (cf fa ed fe); fat magics are
    // big-endian (ca fe ba be). Check both interpretations.
    return MACHO_MAGICS.has(buf.readUInt32BE(0)) || MACHO_MAGICS.has(buf.readUInt32LE(0));
  } finally {
    closeSync(fd);
  }
}

/** Every Mach-O inside the bundle, deepest paths first (inside-out order). */
export function listMachOFiles(appRoot) {
  const found = [];
  const walk = (dir, depth) => {
    for (const name of readdirSync(dir)) {
      const file = path.join(dir, name);
      const stat = statSync(file);
      if (stat.isDirectory()) walk(file, depth + 1);
      else if (stat.isFile() && isMachO(file)) found.push({ file, depth });
    }
  };
  walk(appRoot, 0);
  return found.sort((a, b) => b.depth - a.depth).map(({ file }) => file);
}

/**
 * The ordered codesign invocations for `appRoot`. Every Mach-O except the
 * main executable is signed first; the bundle seal (which re-signs the main
 * executable and writes CodeResources) is always last so no file can change
 * after the seal exists. `identity` undefined selects the ad-hoc path.
 */
export function signingCommands({ appRoot, machos, mainExecutableName, identity }) {
  const mainExe = path.join(appRoot, "Contents", "MacOS", mainExecutableName);
  const flags = identity ? ["--options", "runtime", "--timestamp"] : ["--timestamp=none"];
  const sign = identity ?? "-";
  const commands = machos
    .filter((file) => file !== mainExe)
    .map((target) => {
      const name = path.basename(target).replace(/[^A-Za-z0-9.-]/g, "-");
      return {
        target,
        args: ["--force", ...flags, "--sign", sign, "--identifier", `${BUNDLE_ID}.${name}`, target],
      };
    });
  commands.push({
    target: appRoot,
    args: ["--force", ...flags, "--sign", sign, "--identifier", BUNDLE_ID, appRoot],
  });
  return commands;
}

function run(cmd, args) {
  const result = spawnSync(cmd, args, { stdio: "inherit" });
  if ((result.status ?? 1) !== 0) {
    throw new Error(`${cmd} ${args[0] ?? ""} failed (exit ${result.status})`);
  }
}

/**
 * Sign `appRoot` inside-out and prove the seal before returning. Detritus
 * xattrs are stripped first: FinderInfo/resource-fork leftovers make
 * `codesign --strict` fail with "detritus not allowed".
 */
export function signMacosApp(appRoot, { mainExecutableName, identity } = {}) {
  const probe = spawnSync("xattr", ["-cr", appRoot], { stdio: "ignore" });
  if ((probe.status ?? 1) !== 0) throw new Error(`xattr -cr failed on ${appRoot}`);
  for (const { args } of signingCommands({
    appRoot,
    machos: listMachOFiles(appRoot),
    mainExecutableName,
    identity,
  })) {
    run("codesign", args);
  }
  verifyMacosAppSignature(appRoot);
}

/**
 * Log the Gatekeeper assessment for `appRoot`. An unnotarized ad-hoc app is
 * expected to be rejected by spctl — that is the "unverified developer"
 * path, not a build failure. The verdict text is logged so nightly output
 * shows which Gatekeeper bucket the artifact lands in; codesign --verify is
 * the fail-closed check.
 */
export function logGatekeeperAssessment(appRoot) {
  for (const args of [
    ["spctl", "--assess", "--type", "execute", "-vvv", appRoot],
    ["syspolicy_check", "distribution", appRoot],
  ]) {
    const result = spawnSync(args[0], args.slice(1), { encoding: "utf8" });
    const out = `${result.stdout ?? ""}${result.stderr ?? ""}`.trim();
    console.log(`[macos-sign] $ ${args.join(" ")}\n${out || "(no output)"}`);
  }
}

/** Fail the build unless the deep+strict seal verifies end to end. */
export function verifyMacosAppSignature(appRoot) {
  const result = spawnSync("codesign", ["--verify", "--deep", "--strict", "--verbose=2", appRoot], {
    encoding: "utf8",
  });
  if ((result.status ?? 1) !== 0) {
    throw new Error(
      `codesign --verify --deep --strict failed for ${appRoot}:\n${result.stderr ?? ""}`,
    );
  }
}
