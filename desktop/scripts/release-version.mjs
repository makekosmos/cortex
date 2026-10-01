#!/usr/bin/env node
// Source-of-truth CLI + library for per-platform desktop version management.
//
// Each platform (win / mac) has its own independent PATCH version, but shares
// a MAJOR.MINOR "parity line" that is bumped together when a feature ships on
// both platforms.
//
// Library API:
//   readVersions()           → { win: "0.5.3", mac: "0.5.1" }
//   getVersion(platform)     → "0.5.3"
//   writeVersions(obj)       → void  (2-space indent + trailing newline)
//
// CLI usage:
//   node scripts/release-version.mjs get <win|mac>
//       Prints just the version string to stdout. Used by build wrapper.
//
//   node scripts/release-version.mjs bump --platform <win|mac>
//       PATCH +1 for that platform. Writes file, prints "old -> new".
//
//   node scripts/release-version.mjs bump --platform <win|mac> --minor
//       MINOR +1, PATCH reset to 0 for that platform.
//
//   node scripts/release-version.mjs bump --platform <win|mac> --major
//       MAJOR +1, MINOR 0, PATCH 0 for that platform.
//
//   node scripts/release-version.mjs bump --minor
//       (NO --platform) BOTH platforms: MINOR +1, PATCH reset to 0.
//       This is the shared parity-line bump.
//
//   node scripts/release-version.mjs bump --major
//       (NO --platform) BOTH platforms: MAJOR +1, MINOR 0, PATCH 0.
//
// Default bump = PATCH. Minor/major ONLY via explicit --minor / --major flag.
// Does NOT touch git, does NOT publish, does NOT edit package.json.

import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const VERSIONS_PATH = path.resolve(__dirname, "..", "release-versions.json");

const VALID_PLATFORMS = ["win", "mac"];

// ─── Semver helpers ────────────────────────────────────────────────────────────

/**
 * Parse "X.Y.Z" into { major, minor, patch } (all integers).
 * Returns null if the string is not a valid X.Y.Z semver.
 */
function parseSemver(v) {
  const m = /^(\d+)\.(\d+)\.(\d+)$/.exec(String(v ?? "").trim());
  if (!m) return null;
  return { major: parseInt(m[1], 10), minor: parseInt(m[2], 10), patch: parseInt(m[3], 10) };
}

function formatSemver({ major, minor, patch }) {
  return `${major}.${minor}.${patch}`;
}

function validateVersionString(v, context) {
  if (!parseSemver(v)) {
    console.error(
      `[release-version] Invalid semver "${v}" in ${context} — expected X.Y.Z (e.g. 0.5.3)`,
    );
    process.exit(1);
  }
}

// ─── Library exports ───────────────────────────────────────────────────────────

/** Read release-versions.json and return { win, mac }. Validates semver on read. */
export function readVersions() {
  let raw;
  try {
    raw = readFileSync(VERSIONS_PATH, "utf8");
  } catch (err) {
    console.error(`[release-version] Cannot read ${VERSIONS_PATH}: ${err.message}`);
    process.exit(1);
  }
  let obj;
  try {
    obj = JSON.parse(raw);
  } catch (err) {
    console.error(`[release-version] Invalid JSON in release-versions.json: ${err.message}`);
    process.exit(1);
  }
  for (const platform of VALID_PLATFORMS) {
    validateVersionString(obj[platform], `release-versions.json["${platform}"]`);
  }
  return obj;
}

/** Get the version for a single platform ("win" or "mac"). */
export function getVersion(platform) {
  if (!VALID_PLATFORMS.includes(platform)) {
    console.error(
      `[release-version] Unknown platform "${platform}". Valid values: ${VALID_PLATFORMS.join(", ")}`,
    );
    process.exit(1);
  }
  return readVersions()[platform];
}

/** Write { win, mac } back to release-versions.json (2-space indent + trailing newline). */
export function writeVersions(obj) {
  for (const platform of VALID_PLATFORMS) {
    validateVersionString(obj[platform], `writeVersions["${platform}"]`);
  }
  writeFileSync(VERSIONS_PATH, JSON.stringify(obj, null, 2) + "\n", "utf8");
}

// ─── Bump logic ────────────────────────────────────────────────────────────────

/**
 * Bump a single version string according to the bump type.
 * bumpType: "patch" | "minor" | "major"
 */
function bumpVersion(versionStr, bumpType) {
  const parsed = parseSemver(versionStr);
  if (!parsed) {
    console.error(`[release-version] Cannot bump invalid version "${versionStr}"`);
    process.exit(1);
  }
  let { major, minor, patch } = parsed;
  if (bumpType === "major") {
    major += 1;
    minor = 0;
    patch = 0;
  } else if (bumpType === "minor") {
    minor += 1;
    patch = 0;
  } else {
    // patch (default)
    patch += 1;
  }
  return formatSemver({ major, minor, patch });
}

// ─── CLI ───────────────────────────────────────────────────────────────────────

function cli() {
  const args = process.argv.slice(2);

  if (args.length === 0 || args[0] === "--help" || args[0] === "-h") {
    console.log(
      [
        "Usage:",
        "  node scripts/release-version.mjs get <win|mac>",
        "  node scripts/release-version.mjs bump --platform <win|mac> [--minor|--major]",
        "  node scripts/release-version.mjs bump [--minor|--major]   (both platforms)",
      ].join("\n"),
    );
    process.exit(0);
  }

  const command = args[0];

  if (command === "get") {
    const platform = args[1];
    if (!platform || !VALID_PLATFORMS.includes(platform)) {
      console.error(
        `[release-version] get requires a platform argument: ${VALID_PLATFORMS.join("|")}`,
      );
      process.exit(1);
    }
    process.stdout.write(getVersion(platform) + "\n");
    return;
  }

  if (command === "bump") {
    // Parse flags
    let platform = null;
    let bumpType = "patch"; // default

    for (let i = 1; i < args.length; i++) {
      if (args[i] === "--platform") {
        platform = args[++i];
        if (!platform || !VALID_PLATFORMS.includes(platform)) {
          console.error(
            `[release-version] --platform requires one of: ${VALID_PLATFORMS.join(", ")}`,
          );
          process.exit(1);
        }
      } else if (args[i] === "--minor") {
        bumpType = "minor";
      } else if (args[i] === "--major") {
        bumpType = "major";
      } else {
        console.error(`[release-version] Unknown argument: ${args[i]}`);
        process.exit(1);
      }
    }

    const versions = readVersions();

    if (platform) {
      // Single-platform bump
      const oldVersion = versions[platform];
      const newVersion = bumpVersion(oldVersion, bumpType);
      versions[platform] = newVersion;
      writeVersions(versions);
      console.log(`${platform}: ${oldVersion} -> ${newVersion}`);
    } else {
      // Both-platforms bump (parity line)
      const oldWin = versions.win;
      const oldMac = versions.mac;
      const newWin = bumpVersion(oldWin, bumpType);
      const newMac = bumpVersion(oldMac, bumpType);
      versions.win = newWin;
      versions.mac = newMac;
      writeVersions(versions);
      console.log(`win: ${oldWin} -> ${newWin}`);
      console.log(`mac: ${oldMac} -> ${newMac}`);
    }
    return;
  }

  console.error(`[release-version] Unknown command: ${command}. Try --help.`);
  process.exit(1);
}

// Run CLI if executed directly (not imported)
// ESM: check if this module is the entry point
const isMain =
  process.argv[1] &&
  (process.argv[1] === fileURLToPath(import.meta.url) ||
    // On Windows the path may use backslashes from the resolver
    path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url)));

if (isMain) {
  cli();
}
