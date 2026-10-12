// KOS-350: writes the release feed for one built installer: manifest.json.

import path from "node:path";
import { verifyLocalReleaseManifest } from "./release-channel-local.mjs";
import {
  createReleaseManifest,
  describeInstaller,
  manifestBytes,
  RELEASE_MANIFEST_FILE,
} from "./release-manifest.mjs";
import { writeAtomic } from "./release-utils.mjs";

/**
 * Returns { manifest, files } — `files` is installer + manifest, i.e. the
 * exact local receipt / publish set.
 */
export async function emitReleaseFeeds({
  outputDir,
  platform,
  version,
  bom,
  installerFile,
  releasedAt,
  log = () => {},
}) {
  const manifest = createReleaseManifest({
    bom,
    installer: describeInstaller(installerFile),
    releasedAt: releasedAt ?? new Date().toISOString(),
  });
  const manifestFile = path.join(outputDir, RELEASE_MANIFEST_FILE);
  await writeAtomic(manifestFile, manifestBytes(manifest));
  log(`Release manifest: ${manifestFile}`);
  verifyLocalReleaseManifest(outputDir, version, platform);
  return { manifest, files: [installerFile, manifestFile] };
}
