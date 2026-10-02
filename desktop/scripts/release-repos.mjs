// Single source of truth for the GitHub repos Mundus releases are published
// to and updated from (KOS-304). The Engine updater feed
// (runtime/src/updater/feed.rs DEFAULT_FEED_BASE) must resolve to
// RELEASE_REPOS.win — check-plan does not verify that across languages, so
// feed.rs pins it with a test.
//
// The channels are independent. Windows publishes to this repo and never
// uploads a mac artifact. Mac publishes to makekosmos/desktop-mac, whose
// electron-updater channel file is latest-mac.yml. A failure on one repo
// cannot gate the other: each preflight and verifier looks up only its own
// entry here.
//
// Releases lived in makekosmos/desktop until 0.10.1; that repo is now only a
// one-time migration bridge (publish-release.mjs --also-bridge-repo) so
// clients still polling its `latest` endpoint find the cortex-feed build.
export const RELEASE_REPOS = {
  win: "makekosmos/cortex",
  mac: "makekosmos/desktop-mac",
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
