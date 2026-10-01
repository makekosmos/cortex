#!/usr/bin/env node
// Nightly release planner (KOS-293).
//
// Decides whether `main` carries work that has not been released yet, and which
// version the next Windows release gets. The baseline is the latest *published*
// release in makekosmos/desktop — never a bare git tag — so a release whose
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
//   planRelease({ run, currentVersion, repo })     → plan object
//   setWinVersion(version)        → writes release-versions.json
//
// CLI:
//   node scripts/release-plan.mjs plan
//       Prints `key=value` lines to $GITHUB_OUTPUT (when set) and a human line
//       to stdout. Keys: release, version, sha, previous_tag, previous_commit,
//       bumped.
//   node scripts/release-plan.mjs set <version>
//       Idempotently writes <version> as release-versions.json["win"].

import { appendFileSync, readFileSync, writeFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { RELEASE_RECEIPT_FILE } from "./release-receipt.mjs";
import { readVersions } from "./release-version.mjs";

const SHELL_ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const REPO_ROOT = path.resolve(SHELL_ROOT, "..");
const VERSIONS_FILE = path.join(SHELL_ROOT, "release-versions.json");
export const RELEASE_REPO = "makekosmos/desktop";

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
    throw new Error(
      `release-versions.json win ${current} is below the latest published ${previous}`,
    );
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

export function planRelease({ run = defaultRun, currentVersion, repo = RELEASE_REPO } = {}) {
  const pages = JSON.parse(
    must(
      run("gh", ["api", "--paginate", "--slurp", `repos/${repo}/releases?per_page=100`]),
      `list published ${repo} releases`,
    ),
  );
  // --slurp wraps each page in its own array.
  const releases = pages.flat();
  const baseline = latestPublishedRelease(releases);
  const head = must(run("git", ["rev-parse", "HEAD"]), "git rev-parse HEAD").trim();
  const current = currentVersion ?? readVersions().win;

  let previousCommit = "";
  let changed = true;
  if (baseline) {
    previousCommit = baselineCommit(run, repo, baseline);
    // Missing commits, rewritten history and Git errors must fail — they must
    // never look like "no changes".
    must(run("git", ["merge-base", "--is-ancestor", previousCommit, "HEAD"]), "git merge-base");
    const diff = run("git", ["diff", "--quiet", previousCommit, "HEAD", "--"]);
    if (diff.status !== 0 && diff.status !== 1)
      throw new Error(`git diff failed (${diff.status}): ${diff.stderr.trim()}`);
    changed = diff.status === 1;
  }

  const version = nextReleaseVersion(current, baseline?.version ?? null, changed);
  return {
    release: version !== null,
    version: version ?? "",
    sha: head,
    previousTag: baseline?.tag ?? "",
    previousCommit,
    bumped: version !== null && version !== current,
  };
}

// writeVersions/readVersions read desktop/release-versions.json; tests pass a
// temp file instead.
export function setWinVersion(version, versionsFile) {
  parseStableVersion(version);
  const versions = JSON.parse(readFileSync(versionsFile ?? VERSIONS_FILE, "utf8"));
  if (versions.win === version) {
    console.log(`[release-plan] win already at ${version}`);
    return false;
  }
  const updated = { ...versions, win: version };
  writeFileSync(versionsFile ?? VERSIONS_FILE, JSON.stringify(updated, null, 2) + "\n", "utf8");
  console.log(`[release-plan] win: ${versions.win} -> ${version}`);
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
        ? `Release v${plan.version} (baseline ${plan.previousTag || "none"}, bump: ${plan.bumped})`
        : `No changes since the last published release ${plan.previousTag}`,
    );
    return;
  }
  if (command === "set") {
    if (rest.length !== 1) throw new Error("usage: release-plan.mjs set <version>");
    setWinVersion(rest[0]);
    return;
  }
  throw new Error("usage: release-plan.mjs plan | set <version>");
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    cli();
  } catch (error) {
    console.error(`[release-plan] FATAL: ${error instanceof Error ? error.message : error}`);
    process.exitCode = 1;
  }
}
