// KOS-350: `manifest.json` is the one release document a cortex release
// carries next to its installers. The Engine updater reads it to find the
// newest version and the installer for its platform; the nightly planner
// reads `source.commit` from it as the diff baseline. Nothing in it is
// hand-written: the build derives it from the release BOM (checkout pins)
// plus the hashes of the installer it just produced.
//
// Schema (schema_version 1, keys serialized sorted via release-utils bytes()):
//   {
//     "schema": "mundus-release-manifest",
//     "schema_version": 1,
//     "product": "mundus",
//     "version": "X.Y.Z",              // product version (== git tag vX.Y.Z)
//     "channel": "production",
//     "source": {                       // what the BOM used to carry
//       "repository": "makekosmos/cortex",
//       "commit": "<40 hex>",           // planner baseline (release commit)
//       "toolchain": { "pnpm", "node", "rust" }
//     },
//     "compatibility": { "engine_api": "X.Y.Z" },
//     "platforms": {                    // win, mac; linux reserved
//       "<platform>": {
//         "file": "Mundus-Setup-X.Y.Z.exe",   // asset name in the release
//         "url": "https://github.com/<repo>/releases/download/vX.Y.Z/<file>",
//         "size": <bytes>,
//         "sha512": "<base64>",               // same digest the updater checks
//         "target": "<rust target triple>",
//         "commit": "<40 hex>",               // commit this platform built
//         "released_at": "<ISO-8601>"
//       }
//     }
//   }
//
// Readers must ignore unknown keys (additive changes keep schema_version 1)
// and reject a schema_version they do not know.
//
// Dual-publish window (KOS-350): while DUAL_PUBLISH_LEGACY_FEEDS is true the
// release also carries latest.yml / latest-mac.yml / release-bom.v2.json for
// clients that predate manifest.json. They are rendered FROM the manifest
// below (legacyChannelYml / legacyBom), never derived separately, so the two
// formats cannot drift. Cutover = flip the flag to false (plus
// LEGACY_FEED_FALLBACK in runtime/src/updater/feed.rs) in a follow-up.

import { readFileSync, statSync } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import { RELEASE_BOM_FILE, RELEASE_BOM_SCHEMA_VERSION } from "./release-bom.mjs";
import { bytes, documentHash, fail, semver } from "./release-utils.mjs";

export const RELEASE_MANIFEST_FILE = "manifest.json";
export const RELEASE_MANIFEST_SCHEMA = "mundus-release-manifest";
export const RELEASE_MANIFEST_SCHEMA_VERSION = 1;
export const DUAL_PUBLISH_LEGACY_FEEDS = true;
// linux is reserved in the schema; nothing publishes it yet.
export const MANIFEST_PLATFORMS = ["win", "mac", "linux"];
const LEGACY_CHANNEL_FILES = { win: "latest.yml", mac: "latest-mac.yml" };
// The release creator (Windows) owns the top-level source/compatibility and
// the single legacy BOM; other platforms only add their platforms entry.
export const MANIFEST_CREATOR_PLATFORM = "win";

const COMMIT = /^[0-9a-f]{40}$/;
const SHA512_BASE64 = /^[A-Za-z0-9+/]{86}==$/;
const SAFE_FILE = /^[A-Za-z0-9._-]+$/;

const isObject = (value) =>
  value !== null && Object.prototype.toString.call(value) === "[object Object]";
const isString = (value) => Object.prototype.toString.call(value) === "[object String]";

export function legacyChannelFile(platform) {
  return LEGACY_CHANNEL_FILES[platform] ?? fail(`no legacy channel file for "${platform}"`);
}

/** Legacy assets a platform still emits next to manifest.json. */
export function legacyFeedFiles(platform, { dual = DUAL_PUBLISH_LEGACY_FEEDS } = {}) {
  if (!dual) return [];
  const files = [legacyChannelFile(platform)];
  if (platform === MANIFEST_CREATOR_PLATFORM) files.push(RELEASE_BOM_FILE);
  return files;
}

export function assetDownloadUrl(repository, version, file) {
  return `https://github.com/${repository}/releases/download/v${version}/${file}`;
}

export function sha512Base64(data) {
  return createHash("sha512").update(data).digest("base64");
}

/** Installer facts for a platforms entry, read from the built file. */
export function describeInstaller(file) {
  return {
    file: path.basename(file),
    size: statSync(file).size,
    sha512: sha512Base64(readFileSync(file)),
  };
}

/**
 * Platform-scoped manifest for one build: top-level fields and the single
 * platforms entry all come from the derived BOM + the installer bytes.
 */
export function createReleaseManifest({ bom, installer, releasedAt = new Date().toISOString() }) {
  const { release, source, compatibility } = bom.value;
  const { target, ...toolchain } = source.toolchain;
  return validateReleaseManifest({
    schema: RELEASE_MANIFEST_SCHEMA,
    schema_version: RELEASE_MANIFEST_SCHEMA_VERSION,
    product: "mundus",
    version: release.version,
    channel: release.channel,
    source: { repository: source.repository, commit: source.commit, toolchain },
    compatibility: structuredClone(compatibility),
    platforms: {
      [release.platform]: {
        file: installer.file,
        url: assetDownloadUrl(source.repository, release.version, installer.file),
        size: installer.size,
        sha512: installer.sha512,
        target,
        commit: source.commit,
        released_at: releasedAt,
      },
    },
  });
}

function validatePlatformEntry(manifest, platform, entry) {
  const where = `manifest platforms.${platform}`;
  if (!MANIFEST_PLATFORMS.includes(platform)) fail(`${where}: unknown platform`);
  if (!isObject(entry)) fail(`${where} must be an object`);
  if (!isString(entry.file) || !SAFE_FILE.test(entry.file) || entry.file.startsWith("."))
    fail(`${where}.file is not a safe asset name`);
  if (entry.url !== assetDownloadUrl(manifest.source.repository, manifest.version, entry.file))
    fail(`${where}.url does not point at the v${manifest.version} release asset`);
  if (!Number.isSafeInteger(entry.size) || entry.size <= 0)
    fail(`${where}.size must be a positive integer`);
  if (!isString(entry.sha512) || !SHA512_BASE64.test(entry.sha512))
    fail(`${where}.sha512 must be a base64 SHA-512`);
  if (!isString(entry.target) || !entry.target) fail(`${where}.target is required`);
  if (!COMMIT.test(String(entry.commit ?? ""))) fail(`${where}.commit must be a 40-hex commit`);
  if (!isString(entry.released_at) || Number.isNaN(Date.parse(entry.released_at)))
    fail(`${where}.released_at must be an ISO timestamp`);
}

export function validateReleaseManifest(manifest) {
  if (!isObject(manifest)) fail("manifest must be a JSON object");
  if (manifest.schema !== RELEASE_MANIFEST_SCHEMA)
    fail(`manifest schema must be "${RELEASE_MANIFEST_SCHEMA}"`);
  if (manifest.schema_version !== RELEASE_MANIFEST_SCHEMA_VERSION)
    fail(`manifest schema_version must be ${RELEASE_MANIFEST_SCHEMA_VERSION}`);
  semver(manifest.version, "manifest version");
  if (!isString(manifest.channel) || !manifest.channel) fail("manifest channel required");
  if (!isObject(manifest.source) || !/^[\w.-]+\/[\w.-]+$/.test(String(manifest.source.repository)))
    fail("manifest source.repository must be owner/name");
  if (!COMMIT.test(String(manifest.source.commit ?? "")))
    fail("manifest source.commit must be a 40-hex commit");
  if (!isObject(manifest.source.toolchain)) fail("manifest source.toolchain required");
  for (const tool of ["pnpm", "node", "rust"])
    semver(manifest.source.toolchain[tool], `manifest source.toolchain.${tool}`);
  if (!isObject(manifest.compatibility)) fail("manifest compatibility required");
  semver(manifest.compatibility.engine_api, "manifest compatibility.engine_api");
  if (!isObject(manifest.platforms) || Object.keys(manifest.platforms).length === 0)
    fail("manifest platforms must list at least one platform");
  for (const [platform, entry] of Object.entries(manifest.platforms))
    validatePlatformEntry(manifest, platform, entry);
  return manifest;
}

export function parseReleaseManifest(text) {
  let value;
  try {
    value = JSON.parse(text);
  } catch (error) {
    fail(`manifest is not valid JSON: ${error.message}`);
  }
  return validateReleaseManifest(value);
}

export function manifestBytes(manifest) {
  return bytes(validateReleaseManifest(manifest));
}

/**
 * Combine the manifest already attached to the GitHub release (remote, or
 * null when the release has none yet) with this platform's local build.
 * Windows creates the release and owns every top-level field; a Windows
 * retry keeps any platform entry another job already attached. macOS only
 * adds `platforms.mac` and never rewrites the Windows-owned fields.
 */
export function mergeReleaseManifest(remote, local, platform) {
  validateReleaseManifest(local);
  const entry = local.platforms[platform] ?? fail(`local manifest has no ${platform} entry`);
  if (remote === null || remote === undefined) {
    if (platform !== MANIFEST_CREATOR_PLATFORM)
      fail(
        `release v${local.version} has no ${RELEASE_MANIFEST_FILE} yet — the ${MANIFEST_CREATOR_PLATFORM} publish creates it first`,
      );
    return structuredClone(local);
  }
  validateReleaseManifest(remote);
  if (remote.version !== local.version)
    fail(`release manifest version ${remote.version} does not match ${local.version}`);
  if (platform === MANIFEST_CREATOR_PLATFORM) {
    if (remote.source.commit !== local.source.commit)
      fail(
        `release v${local.version} manifest was built from ${remote.source.commit}, not ${local.source.commit}`,
      );
    return validateReleaseManifest({
      ...structuredClone(local),
      platforms: { ...structuredClone(remote.platforms), [platform]: structuredClone(entry) },
    });
  }
  return validateReleaseManifest({
    ...structuredClone(remote),
    platforms: { ...structuredClone(remote.platforms), [platform]: structuredClone(entry) },
  });
}

function platformEntry(manifest, platform) {
  return (
    validateReleaseManifest(manifest).platforms[platform] ??
    fail(`manifest has no ${platform} entry`)
  );
}

/** electron-builder style latest.yml / latest-mac.yml rendered from the manifest. */
export function legacyChannelYml(manifest, platform) {
  const entry = platformEntry(manifest, platform);
  return [
    `version: ${manifest.version}`,
    "files:",
    `  - url: ${entry.file}`,
    `    sha512: ${entry.sha512}`,
    `    size: ${entry.size}`,
    `path: ${entry.file}`,
    `sha512: ${entry.sha512}`,
    `releaseDate: '${entry.released_at}'`,
    "",
  ].join("\n");
}

/**
 * release-bom.v2.json rendered from the manifest. Byte-identical to
 * deriveReleaseBom() for the same checkout, which publish asserts — so the
 * legacy BOM and the manifest always describe the same build.
 */
export function legacyBom(manifest, platform) {
  const entry = platformEntry(manifest, platform);
  const value = {
    schema_version: RELEASE_BOM_SCHEMA_VERSION,
    id: `mundus-desktop-${manifest.version}-${platform}`,
    release: { version: manifest.version, channel: manifest.channel, platform },
    source: {
      repository: manifest.source.repository,
      commit: entry.commit,
      toolchain: { ...manifest.source.toolchain, target: entry.target },
    },
    compatibility: structuredClone(manifest.compatibility),
  };
  const raw = bytes(value);
  return { value, bytes: raw, digest: documentHash(raw) };
}

/**
 * Parsed legacy channel ({ version, files: [{ url, sha512, size }] }) must
 * describe exactly the manifest's entry for the platform. Returns problems.
 */
export function legacyChannelProblems(manifest, platform, channel) {
  const entry = platformEntry(manifest, platform);
  const problems = [];
  if (channel.version !== manifest.version)
    problems.push(`version ${channel.version} != manifest ${manifest.version}`);
  const first = channel.files?.[0];
  if (!first) problems.push("no files[] entry");
  else {
    if (first.url !== entry.file) problems.push(`file ${first.url} != manifest ${entry.file}`);
    if (first.sha512 !== entry.sha512) problems.push("sha512 differs from manifest");
    if (Number(first.size) !== entry.size) problems.push("size differs from manifest");
  }
  return problems;
}
