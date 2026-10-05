// Resolves which release a verify-release-channel run talks about.
// Split out of verify-release-channel.mjs so the channel checker can stay
// a downloader and the platform choice is testable without hitting GitHub.
//
// Default platform is win: publish-release.mjs invokes the checker with no
// --platform, and that must keep verifying platforms.win of manifest.json
// (+ legacy latest.yml during dual-publish) on makekosmos/cortex.

import { releaseTarget } from "./release-repos.mjs";

/**
 * Parse CLI args.
 * Supports:
 *   --platform <win|mac>  (default win)
 *   --version <x.y.z>
 *   --repo <owner/name>  (bridge publishes pass the second repo explicitly)
 *   <x.y.z>  (positional, backward-compat)
 */
export function parseArgs(argv) {
  const result = { platform: null, version: null, repo: null, positional: null };
  const args = argv.slice(2);

  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--platform") {
      result.platform = args[++i];
    } else if (args[i] === "--version") {
      result.version = args[++i];
    } else if (args[i] === "--repo") {
      result.repo = args[++i];
    } else if (!args[i].startsWith("--")) {
      result.positional = args[i];
    }
  }

  return result;
}

/** Platform, repo, manifest and legacy channel file. Default platform is win. */
export function resolveVerifyTarget(argv) {
  const parsed = parseArgs(argv);
  const platform = parsed.platform ?? "win";
  const target = releaseTarget(platform);
  return {
    ...target,
    repo: parsed.repo ?? target.repo,
    version: parsed.version ?? parsed.positional ?? null,
  };
}
