// The single owner of desktop/release-versions.json on the script side: every
// Node script reads and writes the release version through this module, and a
// contract test fails on any other mention of the file under scripts/ or in
// Rust outside runtime/crates/pe-version-info.
//
// Shape: `{ "win": "x.y.z", "mac": "x.y.z" }`. The two pins are independent
// channels. Windows nightly (`release-plan.mjs set`) updates only `win` and
// must leave `mac` untouched — there is no shared parity bump, because a
// mac pin must never block a Windows-only release and a Windows release
// publishes no mac artifact. `mac` is the version on makekosmos/desktop-mac.
// The "win" key is what the nightly workflow, `runtime/crates/pe-version-info`
// (product_version, used by runtime/build.rs and manager-gpui/build.rs) and
// older checkouts read by name.
//
// Library API:
//   readReleaseVersion({ root, platform = "win" } = {})
//       → "x.y.z" for that channel (semver-validated)
//   writeReleaseVersion(version, { root, platform = "win" } = {})
//       → updates that one key and preserves the other
//
// `root` is the repository root; it defaults to this checkout's root and
// exists for tests that work on a temp repo.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const STABLE_VERSION = /^\d+\.\d+\.\d+$/;

// Key order is contractual for diffs: win, then mac.
export const RELEASE_PLATFORMS = ["win", "mac"];

function versionsPath(root) {
  return path.join(root ?? REPO_ROOT, "desktop", "release-versions.json");
}

// The file is machine-written, so surrounding whitespace is rejected rather
// than trimmed — a tag or installer name must never carry it silently.
function assertStableVersion(version, context) {
  if (
    Object.prototype.toString.call(version) !== "[object String]" ||
    !STABLE_VERSION.test(version)
  )
    throw new Error(
      `${context} must be a MAJOR.MINOR.PATCH version, got ${JSON.stringify(version)}`,
    );
  return version;
}

function assertPlatform(platform) {
  if (!RELEASE_PLATFORMS.includes(platform))
    throw new Error(`Unknown platform "${platform ?? ""}"`);
  return platform;
}

function readParsed(file) {
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(`cannot read ${file}: ${error.message}`);
  }
  // JSON.parse accepts arrays and null; only a plain object is a version map.
  if (Object.prototype.toString.call(parsed) !== "[object Object]")
    throw new Error(`cannot read ${file}: expected an object`);
  return parsed;
}

/** Read one channel from desktop/release-versions.json. Defaults to win. */
export function readReleaseVersion({ root, platform = "win" } = {}) {
  assertPlatform(platform);
  // KOS-322: MUNDUS_SMOKE_VERSION overrides the win pin for the installer
  // smoke only — its build must be strictly newer than the latest published
  // release without committing a version bump. The release pipeline never
  // sets it (release-plan.mjs owns the pin there), so it can never leak into
  // a real release. The installer smoke sets it to `release-plan.mjs
  // build-version` output — see the "Read toolchain pins and resolve the
  // smoke version" step in .github/workflows/installer-smoke.yml.
  const smoke = process.env.MUNDUS_SMOKE_VERSION;
  if (platform === "win" && smoke !== undefined)
    return assertStableVersion(smoke, "MUNDUS_SMOKE_VERSION");
  const file = versionsPath(root);
  const parsed = readParsed(file);
  return assertStableVersion(parsed[platform], `release-versions.json ${platform}`);
}

/**
 * Update one channel. A missing file gains only that key — a Windows write
 * must not invent a mac version. An existing sibling key is preserved as-is
 * after validation, so the nightly win bump cannot drop or rewrite mac.
 */
export function writeReleaseVersion(version, { root, platform = "win" } = {}) {
  assertPlatform(platform);
  assertStableVersion(version, "release version");
  const file = versionsPath(root);
  const parsed = existsSync(file) ? readParsed(file) : {};
  for (const key of Object.keys(parsed)) {
    if (!RELEASE_PLATFORMS.includes(key))
      throw new Error(`release-versions.json has unknown key ${JSON.stringify(key)}`);
    if (key !== platform) assertStableVersion(parsed[key], `release-versions.json ${key}`);
  }
  parsed[platform] = version;
  const ordered = {};
  for (const key of RELEASE_PLATFORMS) {
    if (Object.hasOwn(parsed, key)) ordered[key] = parsed[key];
  }
  writeFileSync(file, JSON.stringify(ordered, null, 2) + "\n", "utf8");
}
