#!/usr/bin/env node
// Build wrapper for per-platform desktop releases.
//
// Reads the platform version from release-versions.json, runs electron-builder
// with the correct platform target and publish config, injects the version via
// -c.extraMetadata.version, then runs the verify guard on success.
//
// Usage:
//   node scripts/build-desktop.mjs --platform <win|mac>
//
// Environment:
//   GH_TOKEN must be set for --publish always to succeed (electron-builder
//   GitHubPublisher requires it). The script does NOT check for it — if it is
//   absent, electron-builder will fail with a clear message.
//
// Do NOT execute this script directly (it would trigger a live publish).
// It is invoked by: bun run build / bun run build:mac

import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { existsSync } from "node:fs";
import { getVersion } from "./release-version.mjs";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const SHELL_ROOT = path.resolve(__dirname, "..");

const VALID_PLATFORMS = ["win", "mac"];

// ─── Helpers ───────────────────────────────────────────────────────────────────

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

// ─── Main ──────────────────────────────────────────────────────────────────────

function main() {
  // ── 1. Parse --platform flag ─────────────────────────────────────────────
  const args = process.argv.slice(2);
  let platform = null;

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--platform") {
      platform = args[++i];
    }
  }

  if (!platform) {
    die("--platform <win|mac> is required");
  }
  if (!VALID_PLATFORMS.includes(platform)) {
    die(`Unknown platform "${platform}". Valid: ${VALID_PLATFORMS.join(", ")}`);
  }

  // ── 2. Read version from release-versions.json ───────────────────────────
  const version = getVersion(platform);
  log(`Platform: ${platform}`);
  log(`Version:  ${version}`);
  log("");

  // ── 3. Resolve electron-builder binary ───────────────────────────────────
  const eb = resolveElectronBuilder();
  log(`electron-builder: ${eb}`);

  // ── 4. Build the electron-builder command ────────────────────────────────
  let ebArgs;
  if (platform === "win") {
    ebArgs = ["--win", "nsis", "--publish", "always", `-c.extraMetadata.version=${version}`];
  } else {
    ebArgs = ["--mac", "dmg", "--publish", "always", `-c.extraMetadata.version=${version}`];
  }

  log(`Running: ${eb} ${ebArgs.join(" ")}`);
  log("");

  // ── 5. Spawn electron-builder ─────────────────────────────────────────────
  const ebResult = spawnSync(eb, ebArgs, {
    cwd: SHELL_ROOT,
    stdio: "inherit",
    shell: true,
  });

  if (ebResult.status !== 0) {
    console.error("");
    console.error(
      `[build-desktop] electron-builder exited with code ${ebResult.status ?? "(signal)"}`,
    );
    process.exit(ebResult.status ?? 1);
  }

  log("");
  log("electron-builder succeeded. Running verify guard...");
  log("");

  // ── 6. Run verify guard ───────────────────────────────────────────────────
  const verifyScript = path.join(__dirname, "verify-release-channel.mjs");
  const verifyResult = spawnSync(
    process.execPath, // node
    [verifyScript, "--platform", platform, "--version", version],
    {
      cwd: SHELL_ROOT,
      stdio: "inherit",
    },
  );

  if (verifyResult.status !== 0) {
    console.error("");
    console.error(`[build-desktop] verify-release-channel FAILED for ${platform} v${version}.`);
    console.error(`[build-desktop] The release was published but channel integrity check failed.`);
    console.error(`[build-desktop] Review the output above and follow the fix instructions.`);
    process.exit(verifyResult.status ?? 1);
  }

  log("");
  log(`Build + verify complete for ${platform} v${version}. Release is consistent.`);
}

main();
