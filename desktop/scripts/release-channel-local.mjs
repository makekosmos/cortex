import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync } from "node:fs";
import path from "node:path";
import { parseReleaseManifest, RELEASE_MANIFEST_FILE } from "./release-manifest.mjs";

function sha512(file) {
  return createHash("sha512").update(readFileSync(file)).digest("base64");
}

// KOS-350: manifest.json is the release source of truth. Checks the local
// manifest's entry for `platform` against the installer bytes. Returns the
// parsed manifest.
export function verifyLocalReleaseManifest(outputDir, expectedVersion, platform = "win") {
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
  return manifest;
}
