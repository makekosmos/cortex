// Single source of truth for the GitHub repos Mundus releases are published
// to and updated from (KOS-304). The Engine updater feed
// (runtime/src/updater/feed.rs DEFAULT_FEED_BASE) must resolve to
// RELEASE_REPOS.win — check-plan does not verify that across languages, so
// feed.rs pins it with a test.
//
// Releases lived in makekosmos/desktop until 0.10.1; that repo is now only a
// one-time migration bridge (publish-release.mjs --also-bridge-repo) so
// clients still polling its `latest` endpoint find the cortex-feed build.
export const RELEASE_REPOS = {
  win: "makekosmos/cortex",
};
