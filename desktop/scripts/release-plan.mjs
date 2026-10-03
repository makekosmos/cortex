#!/usr/bin/env node
// Nightly release planner (KOS-293).
//
// Decides whether `main` carries work that has not been released yet, and which
// version the next Windows release gets. The baseline is the latest *published*
// release in makekosmos/cortex — never a bare git tag — so a release whose
// publish step failed stays retryable on the next run.
//
// Cortex has no tags for releases cut before this pipeline existed, so the diff
// baseline is the source commit recorded in the published release's receipt
// (`release-receipt.v2.json`, uploaded as a release asset by
// publish-release.mjs → build-desktop.mjs).
//
// Library API (all pure or dependency-injected for tests):
//   parseStableVersion(v)         → { major, minor, patch } | throws
//   latestPublishedRelease(list)  → { tag, version, receiptAssetId } | null
//   nextReleaseVersion(current, previous, changed) → "X.Y.Z" | null (skip)
//   nextBuildVersion({ run, currentVersion, repo }) → "X.Y.Z" — see below
//   planRelease({ run, currentVersion, repo })     → plan object
//   setWinVersion(version)        → writes the win release version
//
// CLI:
//   node scripts/release-plan.mjs plan
//       Prints `key=value` lines to $GITHUB_OUTPUT (when set) and a human line
//       to stdout. Keys: release, version, sha, previous_tag, previous_commit,
//       bumped, retry, build_version. `sha` is the commit to build — HEAD
//       normally, the existing tag's commit when an earlier run died between
//       tag push and publish.
//   node scripts/release-plan.mjs build-version
//       Prints the version a build right now must carry: the release plan's
//       version when one is planned, else the would-be next release (pin when
//       it is already ahead of the latest published release, else patch+1).
//       KOS-322: the installer smoke builds exactly this version, so an
//       unbumped pin can never make the "new" build equal the installed
//       previous release.
//   node scripts/release-plan.mjs set <version>
//       Idempotently writes <version> as the win release version.

import { appendFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { RELEASE_RECEIPT_FILE } from "./release-receipt.mjs";
import { RELEASE_REPOS } from "./release-repos.mjs";
import { readReleaseVersion, writeReleaseVersion } from "./release-version.mjs";

const REPO_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..");
export const RELEASE_REPO = RELEASE_REPOS.win;

const STABLE_TAG = /^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
const STABLE_VERSION = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;

export function parseStableVersion(value) {
  const match = STABLE_VERSION.exec(String(value ?? "").trim());
  if (!match)
    throw new Error(`expected a stable MAJOR.MINOR.PATCH version, got ${JSON.stringify(value)}`);
  return { major: Number(match[1]), minor: Number(match[2]), patch: Number(match[3]) };
}

function compareVersions(a, b) {
  for (const key of ["major", "minor", "patch"]) if (a[key] !== b[key]) return a[key] - b[key];
  return 0;
}

// The baseline for "has anything changed" is the newest published (not draft,
// not prerelease) stable release, chosen by semver — not by creation date —
// so a re-published older hotfix can never move the baseline backwards.
export function latestPublishedRelease(releases) {
  let best = null;
  for (const release of releases) {
    if (release.draft || release.prerelease) continue;
    const match = STABLE_TAG.exec(String(release.tag_name ?? ""));
    if (!match) continue;
    const candidate = {
      tag: release.tag_name,
      version: `${match[1]}.${match[2]}.${match[3]}`,
      receiptAssetId: (release.assets ?? []).find((asset) => asset.name === RELEASE_RECEIPT_FILE)
        ?.id,
    };
    if (
      !best ||
      compareVersions(parseStableVersion(candidate.version), parseStableVersion(best.version)) > 0
    )
      best = candidate;
  }
  return best;
}

// Same rules as agenda-gpui's release.py: never release a version at or below
// the published one; reuse an already-bumped version; otherwise bump patch.
export function nextReleaseVersion(current, previous, changed) {
  const currentParts = parseStableVersion(current);
  if (previous === null) return current;
  const previousParts = parseStableVersion(previous);
  const order = compareVersions(currentParts, previousParts);
  if (order < 0)
    throw new Error(`the pinned win version ${current} is below the latest published ${previous}`);
  if (!changed) return null;
  if (order > 0) return current;
  return `${currentParts.major}.${currentParts.minor}.${currentParts.patch + 1}`;
}

// run(cmd, args) → { status, stdout, stderr } — injectable so tests never
// touch git or the network. Default implementation spawns in the repo root.
function defaultRun(cmd, args) {
  const result = spawnSync(cmd, args, {
    cwd: REPO_ROOT,
    encoding: "utf8",
    windowsHide: true,
  });
  if (result.error) throw result.error;
  return { status: result.status ?? 1, stdout: result.stdout ?? "", stderr: result.stderr ?? "" };
}

function must(result, description) {
  if (result.status !== 0)
    throw new Error(`${description} failed (${result.status}): ${result.stderr.trim()}`);
  return result.stdout;
}

// Source commit the published release was built from, taken from its uploaded
// verification receipt. A release without a receipt fails loudly — silently
// falling back to "release everything" would turn a corrupt baseline into an
// unwanted publish.
function baselineCommit(run, repo, baseline) {
  if (baseline.receiptAssetId === undefined)
    throw new Error(
      `latest published release ${baseline.tag} has no ${RELEASE_RECEIPT_FILE} asset — cannot determine its source commit`,
    );
  const raw = must(
    run("gh", [
      "api",
      "-H",
      "Accept: application/octet-stream",
      `repos/${repo}/releases/assets/${baseline.receiptAssetId}`,
    ]),
    `download ${RELEASE_RECEIPT_FILE} for ${baseline.tag}`,
  );
  const receipt = JSON.parse(raw);
  const commit = receipt?.inputs?.commit;
  if (!/^[0-9a-f]{40}$/.test(String(commit ?? "")))
    throw new Error(`${RELEASE_RECEIPT_FILE} of ${baseline.tag} has no valid inputs.commit`);
  if (receipt.inputs.version !== baseline.version)
    throw new Error(
      `${RELEASE_RECEIPT_FILE} of ${baseline.tag} records version ${receipt.inputs.version}`,
    );
  return commit;
}

function listReleases(run, repo) {
  const pages = JSON.parse(
    must(
      run("gh", ["api", "--paginate", "--slurp", `repos/${repo}/releases?per_page=100`]),
      `list published ${repo} releases`,
    ),
  );
  // --slurp wraps each page in its own array.
  return pages.flat();
}

// The version any build must carry right now — the same bump rule the
// release planner applies, so a smoke build and a real release never
// disagree about what "next" means. Unlike planRelease this works before the
// first published release: with no baseline the pin wins.
export function nextBuildVersion({ run = defaultRun, currentVersion, repo = RELEASE_REPO } = {}) {
  const baseline = latestPublishedRelease(listReleases(run, repo));
  return nextReleaseVersion(
    currentVersion ?? readReleaseVersion(),
    baseline?.version ?? null,
    true,
  );
}

export function planRelease({ run = defaultRun, currentVersion, repo = RELEASE_REPO } = {}) {
  const baseline = latestPublishedRelease(listReleases(run, repo));
  // No baseline means the repo has never seen a release — the planner must not
  // guess a starting point or fall back to the legacy makekosmos/desktop feed.
  // The first cortex release is the manual KOS-304 bridge publish.
  if (!baseline)
    throw new Error(
      `${repo} has no published stable release to diff against — ` +
        `publish the first one manually (publish-release.mjs --also-bridge-repo makekosmos/desktop)`,
    );
  const head = must(run("git", ["rev-parse", "HEAD"]), "git rev-parse HEAD").trim();
  const current = currentVersion ?? readReleaseVersion();

  const previousCommit = baselineCommit(run, repo, baseline);
  // Missing commits, rewritten history and Git errors must fail — they must
  // never look like "no changes".
  must(run("git", ["merge-base", "--is-ancestor", previousCommit, "HEAD"]), "git merge-base");
  const diff = run("git", ["diff", "--quiet", previousCommit, "HEAD", "--"]);
  if (diff.status !== 0 && diff.status !== 1)
    throw new Error(`git diff failed (${diff.status}): ${diff.stderr.trim()}`);
  const changed = diff.status === 1;

  const version = nextReleaseVersion(current, baseline.version, changed);

  // A tag that exists but has no published release is a partially-failed
  // earlier run: the bump commit and tag were pushed, then publish failed.
  // The tagged commit is the release point — rebuild and publish exactly it,
  // and let newer main commits wait for the next run. A tag outside main's
  // history means rewritten history or a manual tag: fail loudly.
  let sha = head;
  let retry = false;
  if (version !== null) {
    const tag = run("git", ["rev-parse", "-q", "--verify", `refs/tags/v${version}^{commit}`]);
    if (tag.status === 0) {
      const tagged = tag.stdout.trim();
      if (!/^[0-9a-f]{40}$/.test(tagged))
        throw new Error(`git rev-parse returned an unexpected value: ${tagged}`);
      must(
        run("git", ["merge-base", "--is-ancestor", tagged, "HEAD"]),
        `git merge-base --is-ancestor v${version} HEAD`,
      );
      sha = tagged;
      retry = true;
    } else if (tag.status !== 1) {
      throw new Error(`git rev-parse failed (${tag.status}): ${tag.stderr.trim()}`);
    }
  }

  return {
    release: version !== null,
    version: version ?? "",
    // The version a build of this tree must carry (KOS-322): the planned
    // release version when one exists, else the would-be next release, so the
    // installer smoke always tests a build strictly newer than the baseline.
    buildVersion: version ?? nextReleaseVersion(current, baseline.version, true),
    sha,
    previousTag: baseline.tag,
    previousCommit,
    bumped: version !== null && version !== current,
    retry,
  };
}

// The version file is owned by release-version.mjs; `root` exists for tests
// that point at a temp repo.
export function setWinVersion(version, { root } = {}) {
  parseStableVersion(version);
  const current = readReleaseVersion({ root });
  if (current === version) {
    console.log(`[release-plan] win already at ${version}`);
    return false;
  }
  writeReleaseVersion(version, { root });
  console.log(`[release-plan] win: ${current} -> ${version}`);
  return true;
}

function cli() {
  const [command, ...rest] = process.argv.slice(2);
  if (command === "plan") {
    const plan = planRelease();
    const outputs = {
      release: String(plan.release),
      version: plan.version,
      sha: plan.sha,
      previous_tag: plan.previousTag,
      previous_commit: plan.previousCommit,
      bumped: String(plan.bumped),
      retry: String(plan.retry),
      build_version: plan.buildVersion,
    };
    if (process.env.GITHUB_OUTPUT)
      appendFileSync(
        process.env.GITHUB_OUTPUT,
        Object.entries(outputs)
          .map(([key, value]) => `${key}=${value}`)
          .join("\n") + "\n",
      );
    console.log(
      plan.release
        ? `Release v${plan.version} (baseline ${plan.previousTag || "none"}, bump: ${plan.bumped}${plan.retry ? `, retrying tagged commit ${plan.sha}` : ""})`
        : `No changes since the last published release ${plan.previousTag}`,
    );
    return;
  }
  if (command === "build-version") {
    process.stdout.write(`${nextBuildVersion()}\n`);
    return;
  }
  if (command === "set") {
    if (rest.length !== 1) throw new Error("usage: release-plan.mjs set <version>");
    setWinVersion(rest[0]);
    return;
  }
  throw new Error("usage: release-plan.mjs plan | build-version | set <version>");
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    cli();
  } catch (error) {
    console.error(`[release-plan] FATAL: ${error instanceof Error ? error.message : error}`);
    process.exitCode = 1;
  }
}
