import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { releaseTarget } from "./release-repos.mjs";
import {
  DUAL_PUBLISH_LEGACY_FEEDS,
  legacyChannelProblems,
  parseReleaseManifest,
  RELEASE_MANIFEST_FILE,
} from "./release-manifest.mjs";

function scalar(value) {
  return value.trim().replace(/^(?:"([^"]*)"|'([^']*)')$/, "$1$2");
}

function sha512(file) {
  return createHash("sha512").update(readFileSync(file)).digest("base64");
}

// KOS-350: manifest.json is the release source of truth. Checks the local
// manifest's entry for `platform` against the installer bytes and, during the
// dual-publish window, that the legacy channel yml says exactly the same.
// Returns the parsed manifest.
export function verifyLocalReleaseManifest(
  outputDir,
  expectedVersion,
  platform = "win",
  { dual = DUAL_PUBLISH_LEGACY_FEEDS } = {},
) {
  const manifest = parseReleaseManifest(
    readFileSync(path.join(outputDir, RELEASE_MANIFEST_FILE), "utf8"),
  );
  if (manifest.version !== expectedVersion)
    throw new Error(`${RELEASE_MANIFEST_FILE} version ${manifest.version} != ${expectedVersion}`);
  const entry = manifest.platforms[platform];
  if (!entry) throw new Error(`${RELEASE_MANIFEST_FILE} has no ${platform} entry`);
  const artifact = path.join(outputDir, entry.file);
  if (
    !existsSync(artifact) ||
    statSync(artifact).size !== entry.size ||
    sha512(artifact) !== entry.sha512
  )
    throw new Error(`${RELEASE_MANIFEST_FILE} artifact mismatch: ${entry.file}`);
  if (dual) {
    const channel = verifyLocalReleaseChannel(outputDir, expectedVersion, platform);
    const problems = legacyChannelProblems(manifest, platform, channel);
    if (problems.length > 0)
      throw new Error(
        `${releaseTarget(platform).channelFile} drifted from ${RELEASE_MANIFEST_FILE}: ${problems.join("; ")}`,
      );
  }
  return manifest;
}

// Legacy channel yml check (dual-publish window). `platform` defaults to win
// (latest.yml); mac checks latest-mac.yml in its own output directory; this
// function never looks at the other platform's file. Returns the parsed
// { version, files } so callers can compare it with manifest.json.
export function verifyLocalReleaseChannel(outputDir, expectedVersion, platform = "win") {
  const { channelFile: channelName } = releaseTarget(platform);
  const document = readFileSync(path.join(outputDir, channelName), "utf8");
  const version = /^version:\s*(.+?)\s*$/m.exec(document)?.[1];
  const primaryName = /^path:\s*(.+?)\s*$/m.exec(document)?.[1];
  const primaryHash = /^sha512:\s*(\S+)\s*$/m.exec(document)?.[1];
  if (!version || scalar(version) !== expectedVersion || !primaryName || !primaryHash)
    throw new Error(`${channelName} has invalid version or primary artifact metadata`);

  const files = [];
  let current;
  let inFiles = false;
  for (const line of document.split(/\r?\n/)) {
    if (/^files:\s*$/.test(line)) {
      inFiles = true;
      continue;
    }
    if (inFiles && /^\S/.test(line)) {
      if (current) files.push(current);
      current = undefined;
      inFiles = false;
    }
    if (!inFiles) continue;
    const url = /^\s*-\s+url:\s*(.+?)\s*$/.exec(line)?.[1];
    if (url) {
      if (current) files.push(current);
      current = { name: scalar(url) };
      continue;
    }
    const hash = /^\s+sha512:\s*(\S+)\s*$/.exec(line)?.[1];
    if (hash && current) current.sha512 = hash;
    const size = /^\s+size:\s*(\d+)\s*$/.exec(line)?.[1];
    if (size && current) current.size = Number(size);
  }
  if (current) files.push(current);
  if (files.length === 0) throw new Error(`${channelName} has no files`);

  const seen = new Set();
  for (const file of files) {
    if (
      !file.name ||
      path.basename(file.name) !== file.name ||
      seen.has(file.name) ||
      !file.sha512 ||
      !Number.isSafeInteger(file.size) ||
      file.size <= 0
    )
      throw new Error(`${channelName} has invalid file metadata`);
    seen.add(file.name);
    const artifact = path.join(outputDir, file.name);
    if (
      !existsSync(artifact) ||
      statSync(artifact).size !== file.size ||
      sha512(artifact) !== file.sha512
    )
      throw new Error(`${channelName} artifact mismatch: ${file.name}`);
  }

  const primary = scalar(primaryName);
  if (!seen.has(primary) || sha512(path.join(outputDir, primary)) !== primaryHash)
    throw new Error(`${channelName} primary artifact mismatch: ${primary}`);
  return {
    version: scalar(version),
    files: files.map(({ name, sha512: hash, size }) => ({ url: name, sha512: hash, size })),
  };
}
