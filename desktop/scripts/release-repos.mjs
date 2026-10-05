// Single source of truth for the GitHub repos Mundus releases are published
// to and updated from (KOS-304 / KOS-349). The Engine updater feed
// (runtime/src/updater/feed.rs DEFAULT_FEED_BASE) must resolve to
// RELEASE_REPOS.win — check-plan does not verify that across languages, so
// feed.rs pins it with a test.
//
// Both Windows and macOS publish to makekosmos/cortex (same product version).
// Windows owns latest.yml + the NSIS installer; macOS owns latest-mac.yml +
// the .dmg. A failure on one platform must not gate the other: each
// preflight and verifier looks up only its own channel file here. The
// The former separate Mac publish repo is deleted (Jack) and must not
// reappear as a publish target.
export const RELEASE_REPOS = {
  win: "makekosmos/cortex",
  mac: "makekosmos/cortex",
};

const CHANNEL_FILES = {
  win: "latest.yml",
  mac: "latest-mac.yml",
};

/** Repo and channel file for one platform. Throws on anything else. */
export function releaseTarget(platform) {
  const repo = RELEASE_REPOS[platform];
  const channelFile = CHANNEL_FILES[platform];
  if (!repo || !channelFile) throw new Error(`Unknown platform "${platform ?? ""}"`);
  return { platform, repo, channelFile };
}
