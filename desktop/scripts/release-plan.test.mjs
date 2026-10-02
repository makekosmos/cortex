import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import {
  latestPublishedRelease,
  nextReleaseVersion,
  parseStableVersion,
  planRelease,
  RELEASE_REPO,
  setWinVersion,
} from "./release-plan.mjs";
import { RELEASE_RECEIPT_FILE } from "./release-receipt.mjs";

const HEAD = "a".repeat(40);

const BASE_COMMIT = "b".repeat(40);
const TAGGED_COMMIT = "c".repeat(40);
const CURRENT = "0.10.0";

test("the release baseline repo is makekosmos/cortex", () => {
  assert.equal(RELEASE_REPO, "makekosmos/cortex");
});

function release(tag, { draft = false, prerelease = false, receipt = true } = {}) {
  return {
    tag_name: tag,
    draft,
    prerelease,
    assets: receipt ? [{ name: RELEASE_RECEIPT_FILE, id: 12345 }] : [],
  };
}

// Fake command runner: matches on the joined command line, in order of
// specificity. Unmatched commands fail loudly like real spawn errors.
function fakeRun(handlers) {
  const calls = [];
  const run = (cmd, args) => {
    const line = [cmd, ...args].join(" ");
    calls.push(line);
    for (const [prefix, result] of handlers) {
      if (line.startsWith(prefix)) return { status: 0, stdout: "", stderr: "", ...result };
    }
    return { status: 127, stdout: "", stderr: `unexpected command: ${line}` };
  };
  return { run, calls };
}

const receiptJson = JSON.stringify({ inputs: { commit: BASE_COMMIT, version: "0.10.0" } });

function baseHandlers({ diffStatus = 1 } = {}) {
  return [
    [
      `gh api --paginate --slurp repos/${RELEASE_REPO}/releases`,
      { stdout: JSON.stringify([[release("v0.10.0")]]) },
    ],
    ["gh api -H Accept: application/octet-stream", { stdout: receiptJson }],
    ["git rev-parse HEAD", { stdout: HEAD }],
    [`git merge-base --is-ancestor ${BASE_COMMIT} HEAD`, {}],
    [`git diff --quiet ${BASE_COMMIT} HEAD --`, { status: diffStatus }],
    ["git rev-parse -q --verify refs/tags/", { status: 1 }],
  ];
}

test("no changes since the published release skips with a clear plan", () => {
  const { run } = fakeRun(baseHandlers({ diffStatus: 0 }));
  const plan = planRelease({ run, currentVersion: CURRENT });
  assert.equal(plan.release, false);
  assert.equal(plan.version, "");
  assert.equal(plan.previousTag, "v0.10.0");
  assert.equal(plan.previousCommit, BASE_COMMIT);
});

test("changes with an unbumped version bump the patch", () => {
  const { run } = fakeRun(baseHandlers());
  const plan = planRelease({ run, currentVersion: CURRENT });
  assert.deepEqual(plan, {
    release: true,
    version: "0.10.1",
    sha: HEAD,
    previousTag: "v0.10.0",
    previousCommit: BASE_COMMIT,
    bumped: true,
    retry: false,
  });
});

test("an already-bumped version is used as-is", () => {
  const { run } = fakeRun(baseHandlers());
  const plan = planRelease({ run, currentVersion: "0.11.0" });
  assert.equal(plan.release, true);
  assert.equal(plan.version, "0.11.0");
  assert.equal(plan.bumped, false);
});

test("a repo version below the published one fails", () => {
  const { run } = fakeRun(baseHandlers());
  assert.throws(
    () => planRelease({ run, currentVersion: "0.9.9" }),
    /below the latest published 0\.10\.0/,
  );
});

test("git errors fail loudly instead of looking like no changes", () => {
  const { run } = fakeRun([
    [
      `gh api --paginate --slurp repos/${RELEASE_REPO}/releases`,
      { stdout: JSON.stringify([[release("v0.10.0")]]) },
    ],
    ["gh api -H Accept: application/octet-stream", { stdout: receiptJson }],
    ["git rev-parse HEAD", { stdout: HEAD }],
    [`git merge-base --is-ancestor ${BASE_COMMIT} HEAD`, { status: 1, stderr: "not an ancestor" }],
  ]);
  assert.throws(() => planRelease({ run, currentVersion: CURRENT }), /git merge-base failed/);

  const { run: runDiff } = fakeRun(baseHandlers({ diffStatus: 128 }));
  assert.throws(() => planRelease({ run: runDiff, currentVersion: CURRENT }), /git diff failed/);
});

// KOS-304: with no published release in cortex there is no receipt to diff
// against. The planner must fail closed and point at the one-time bridge
// publish — never fall back to makekosmos/desktop or guess a version.
test("with no published releases the plan fails closed", () => {
  const { run } = fakeRun([
    [`gh api --paginate --slurp repos/${RELEASE_REPO}/releases`, { stdout: "[[]]" }],
  ]);
  assert.throws(
    () => planRelease({ run, currentVersion: CURRENT }),
    /no published stable release.*--also-bridge-repo/s,
  );
});

// A run can die between "tag pushed" and "release published": the next plan
// must then pin the tagged commit as the release point, not HEAD — otherwise
// every later run trips on the existing tag forever.
test("an unpublished tag on an older commit becomes the release point", () => {
  const { run } = fakeRun([
    ...baseHandlers().slice(0, -1),
    ["git rev-parse -q --verify refs/tags/v0.10.1^{commit}", { stdout: TAGGED_COMMIT }],
    [`git merge-base --is-ancestor ${TAGGED_COMMIT} HEAD`, {}],
  ]);
  const plan = planRelease({ run, currentVersion: CURRENT });
  assert.equal(plan.release, true);
  assert.equal(plan.version, "0.10.1");
  assert.equal(plan.sha, TAGGED_COMMIT);
  assert.equal(plan.retry, true);
});

test("an unpublished tag outside main history fails loudly", () => {
  const { run } = fakeRun([
    ...baseHandlers().slice(0, -1),
    ["git rev-parse -q --verify refs/tags/v0.10.1^{commit}", { stdout: TAGGED_COMMIT }],
    [
      `git merge-base --is-ancestor ${TAGGED_COMMIT} HEAD`,
      { status: 1, stderr: "not an ancestor" },
    ],
  ]);
  assert.throws(() => planRelease({ run, currentVersion: CURRENT }), /merge-base.*v0\.10\.1/);
});

test("a git error while resolving the tag fails loudly", () => {
  const { run } = fakeRun([
    ...baseHandlers().slice(0, -1),
    [
      "git rev-parse -q --verify refs/tags/v0.10.1^{commit}",
      { status: 128, stderr: "not a git repository" },
    ],
  ]);
  assert.throws(() => planRelease({ run, currentVersion: CURRENT }), /git rev-parse failed/);
});

test("drafts, prereleases and non-stable tags never become the baseline", () => {
  const releases = [
    release("v0.9.0"),
    release("v0.11.0", { draft: true }),
    release("v0.12.0", { prerelease: true }),
    release("nightly-2026"),
    release("v0.9.9-beta.1"),
  ];
  const best = latestPublishedRelease(releases);
  assert.equal(best.tag, "v0.9.0");
  assert.equal(best.version, "0.9.0");
});

test("a published release without a verification receipt fails loudly", () => {
  const { run } = fakeRun([
    [
      `gh api --paginate --slurp repos/${RELEASE_REPO}/releases`,
      { stdout: JSON.stringify([[release("v0.10.0", { receipt: false })]]) },
    ],
    ["git rev-parse HEAD", { stdout: HEAD }],
  ]);
  assert.throws(() => planRelease({ run, currentVersion: CURRENT }), /no .* asset/);
});

test("a receipt naming another commit tree fails the baseline check", () => {
  const { run } = fakeRun([
    [
      `gh api --paginate --slurp repos/${RELEASE_REPO}/releases`,
      { stdout: JSON.stringify([[release("v0.10.0")]]) },
    ],
    [
      "gh api -H Accept: application/octet-stream",
      { stdout: JSON.stringify({ inputs: { commit: "not-a-sha", version: "0.10.0" } }) },
    ],
    ["git rev-parse HEAD", { stdout: HEAD }],
  ]);
  assert.throws(() => planRelease({ run, currentVersion: CURRENT }), /inputs\.commit/);
});

test("parseStableVersion rejects non-stable input", () => {
  assert.deepEqual(parseStableVersion("1.2.3"), { major: 1, minor: 2, patch: 3 });
  for (const bad of ["1.2", "1.2.3.4", "v1.2.3", "1.2.3-rc1", "01.2.3", ""])
    assert.throws(() => parseStableVersion(bad));
});

test("setWinVersion writes only the win entry and stays idempotent", async () => {
  const dir = await mkdtemp(path.join(os.tmpdir(), "mundus-release-plan-"));
  const file = path.join(dir, "desktop", "release-versions.json");
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, JSON.stringify({ win: "0.10.0" }, null, 2) + "\n");
  assert.equal(setWinVersion("0.10.1", { root: dir }), true);
  assert.deepEqual(JSON.parse(await readFile(file, "utf8")), { win: "0.10.1" });
  assert.equal(setWinVersion("0.10.1", { root: dir }), false);
  assert.throws(() => setWinVersion("0.10.x", { root: dir }), /MAJOR\.MINOR\.PATCH/);
});

test("nextReleaseVersion mirrors the manual release bump rules", () => {
  assert.equal(nextReleaseVersion("0.10.0", null, true), "0.10.0");
  assert.equal(nextReleaseVersion("0.10.0", "0.10.0", false), null);
  assert.equal(nextReleaseVersion("0.10.0", "0.10.0", true), "0.10.1");
  assert.equal(nextReleaseVersion("0.11.2", "0.10.0", true), "0.11.2");
  assert.equal(nextReleaseVersion("0.11.2", "0.10.0", false), null);
  assert.throws(() => nextReleaseVersion("0.9.0", "0.10.0", true), /below/);
});
