// Single source of truth for the GitHub repos Mundus releases are published
// to and updated from (KOS-304 / KOS-349 / KOS-350). The Engine updater feed
// (runtime/src/updater/feed.rs DEFAULT_FEED_BASE) must resolve to
// RELEASE_REPOS.win — check-plan does not verify that across languages, so
// feed.rs pins it with a test.
//
// Both Windows and macOS publish to makekosmos/cortex (same product version).
// KOS-350: every release carries ONE `manifest.json` (release-manifest.mjs)
// with a platforms entry per installer — Windows creates it with the NSIS
// installer, macOS merges its .dmg entry in. During the dual-publish window
// each platform also uploads its legacy channel file (`channelFile` below:
// latest.yml / latest-mac.yml), rendered from that same manifest. A failure
// on one platform must not gate the other: each preflight and verifier looks
// up only its own platform here. The former separate Mac publish repo is
// deleted (Jack) and must not reappear as a publish target.
import { legacyChannelFile, RELEASE_MANIFEST_FILE } from "./release-manifest.mjs";

export const RELEASE_REPOS = {
  win: "makekosmos/cortex",
  mac: "makekosmos/cortex",
};

/** Repo, manifest and legacy channel file for one platform. Throws on anything else. */
export function releaseTarget(platform) {
  const repo = RELEASE_REPOS[platform];
  if (!repo) throw new Error(`Unknown platform "${platform ?? ""}"`);
  // Legacy electron-builder channel file (latest.yml / latest-mac.yml) —
  // uploaded only during the dual-publish window.
  const channelFile = legacyChannelFile(platform);
  return { platform, repo, channelFile, manifestFile: RELEASE_MANIFEST_FILE };
}
