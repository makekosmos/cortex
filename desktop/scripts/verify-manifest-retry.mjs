// Bounded manifest-readiness polling for verify-release-channel.mjs
// (KOS-377). Right after a platform publishes, GitHub can keep serving a
// manifest.json that predates that platform's asset merge for minutes —
// the mac nightly hit exactly this: fetch 2 s after upload returned a
// win-only manifest and the run failed with no retry. Poll with a
// cache-busting query param until the manifest parses AND carries the
// expected platform entry AND version, or the deadline passes. A real
// mismatch still fails — just after the bound, not instantly.

import { parseReleaseManifest } from "./release-manifest.mjs";

export const MANIFEST_READY_DEADLINE_MS = 4 * 60 * 1000;
export const MANIFEST_RETRY_STEP_MS = 5_000;

/** Why a parsed manifest is not yet the one we published, or null when it is. */
export function manifestReadinessProblem(manifest, platform, version) {
  if (!manifest?.platforms?.[platform]) {
    return `no "${platform}" entry in platforms`;
  }
  if (version && manifest.version !== version) {
    return `version ${manifest.version} != expected ${version}`;
  }
  return null;
}

/**
 * Fetch `url` until it parses as a release manifest that carries `platform`
 * and `version`, or `deadlineMs` elapses. Every attempt appends a unique
 * `cb` query param so a stale cached copy is not re-served. Throws with the
 * last observed problem once the bound is reached.
 */
export async function fetchManifestUntilReady({
  url,
  platform,
  version,
  deadlineMs = MANIFEST_READY_DEADLINE_MS,
  fetchImpl = fetch,
  sleepImpl = (ms) => new Promise((resolve) => setTimeout(resolve, ms)),
  now = () => Date.now(),
  log = () => {},
}) {
  const deadline = now() + deadlineMs;
  let lastProblem = "no fetch attempted";
  for (let attempt = 1; ; attempt++) {
    const busted = `${url}${url.includes("?") ? "&" : "?"}cb=${attempt}-${now()}`;
    try {
      const res = await fetchImpl(busted);
      if (!res.ok) throw new Error(`HTTP ${res.status} ${res.statusText ?? ""}`.trim());
      const manifest = parseReleaseManifest(await res.text());
      const problem = manifestReadinessProblem(manifest, platform, version);
      if (!problem) {
        if (attempt > 1) log(`manifest ready on attempt ${attempt}`);
        return { manifest, attempt };
      }
      lastProblem = problem;
    } catch (error) {
      lastProblem = error instanceof Error ? error.message : String(error);
    }
    const remaining = deadline - now();
    if (remaining <= 0) break;
    const wait = Math.min(MANIFEST_RETRY_STEP_MS * Math.min(attempt, 4), remaining);
    log(`manifest not ready (${lastProblem}); retrying in ${Math.round(wait / 1000)}s...`);
    await sleepImpl(wait);
  }
  throw new Error(
    `${url}: still not ready after ${Math.round(deadlineMs / 1000)}s — last: ${lastProblem}`,
  );
}
