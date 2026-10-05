// KOS-350: writes the release feed files for one built installer —
// manifest.json always, plus the legacy latest*.yml / release-bom.v2.json
// while DUAL_PUBLISH_LEGACY_FEEDS is on. The legacy files are rendered from
// the manifest (never derived on their own), and the rendered BOM must
// byte-match the BOM derived from HEAD, so the formats cannot drift.

import path from "node:path";
import { RELEASE_BOM_FILE } from "./release-bom.mjs";
import { verifyLocalReleaseManifest } from "./release-channel-local.mjs";
import {
  createReleaseManifest,
  describeInstaller,
  DUAL_PUBLISH_LEGACY_FEEDS,
  legacyBom,
  legacyChannelYml,
  legacyFeedFiles,
  manifestBytes,
  RELEASE_MANIFEST_FILE,
} from "./release-manifest.mjs";
import { writeAtomic } from "./release-utils.mjs";

/**
 * Returns { manifest, files } — `files` is installer + manifest + every
 * legacy feed written, i.e. the exact local receipt / publish set. A local
 * build (`local: true`) gets no release BOM file.
 */
export async function emitReleaseFeeds({
  outputDir,
  platform,
  version,
  bom,
  installerFile,
  local = false,
  dual = DUAL_PUBLISH_LEGACY_FEEDS,
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
  const written = [];
  for (const name of legacyFeedFiles(platform, { dual })) {
    const file = path.join(outputDir, name);
    if (name === RELEASE_BOM_FILE) {
      if (local) continue;
      const rendered = legacyBom(manifest, platform);
      if (rendered.digest !== bom.digest)
        throw new Error(
          "release-bom rendered from manifest.json does not match the BOM derived from HEAD",
        );
      await writeAtomic(file, rendered.bytes);
    } else {
      await writeAtomic(file, legacyChannelYml(manifest, platform));
    }
    log(`Legacy feed (dual-publish, rendered from manifest): ${file}`);
    written.push(file);
  }
  verifyLocalReleaseManifest(outputDir, version, platform, { dual });
  return { manifest, files: [installerFile, manifestFile, ...written] };
}
