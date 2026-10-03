// The single owner of desktop/release-versions.json on the script side: every
// Node script reads and writes the release version through this module, and a
// contract test fails on any other mention of the file under scripts/ or in
// Rust outside runtime/crates/pe-version-info.
//
// The file shape `{ "win": "x.y.z" }` is contractual — the "win" key is read
// by name by the nightly workflow, `runtime/crates/pe-version-info`
// (product_version, used by runtime/build.rs and manager-gpui/build.rs) and
// older checkouts. Windows is the only released platform.
//
// Library API:
//   readReleaseVersion({ root } = {})            → "x.y.z" (semver-validated)
//   writeReleaseVersion(version, { root } = {})   → writes { "win": version }
//
// `root` is the repository root; it defaults to this checkout's root and
// exists for tests that work on a temp repo.

import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
const STABLE_VERSION = /^\d+\.\d+\.\d+$/;

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

/** Read desktop/release-versions.json and return the validated win version. */
export function readReleaseVersion({ root } = {}) {
  // KOS-322: MUNDUS_SMOKE_VERSION overrides the pin for the installer smoke
  // only — its build must be strictly newer than the latest published release
  // without committing a version bump. The release pipeline never sets it
  // (release-plan.mjs owns the pin there), so it can never leak into a real
  // release. The installer smoke sets it to `release-plan.mjs build-version`
  // output — see the "Read toolchain pins and resolve the smoke version"
  // step in .github/workflows/installer-smoke.yml.
  const smoke = process.env.MUNDUS_SMOKE_VERSION;
  if (smoke !== undefined) return assertStableVersion(smoke, "MUNDUS_SMOKE_VERSION");
  const file = versionsPath(root);
  let parsed;
  try {
    parsed = JSON.parse(readFileSync(file, "utf8"));
  } catch (error) {
    throw new Error(`cannot read ${file}: ${error.message}`);
  }
  return assertStableVersion(parsed?.win, `release-versions.json win`);
}

/** Write { "win": version } back to release-versions.json (2-space + newline). */
export function writeReleaseVersion(version, { root } = {}) {
  assertStableVersion(version, "release version");
  writeFileSync(versionsPath(root), JSON.stringify({ win: version }, null, 2) + "\n", "utf8");
}
