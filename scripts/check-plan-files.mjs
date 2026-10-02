import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { gitEnv } from "./git-env.mjs";
import { parseNameStatus } from "./check-plan-git.mjs";

// Discovery for the planner: which files a mode must classify, and against
// which revisions. "What this branch changes" is defined once for every mode:
// the diff of the revision under test against merge-base(HEAD, origin/main).
// It never depends on the remote ref named in the push, so a brand-new remote
// branch, a release push carrying a tag, or a rebase across main's own
// commits all plan the same way.

const ZERO_SHA = /^0{40}$/;

export function git(args) {
  return spawnSync("git", args, { cwd: process.cwd(), encoding: "utf8", env: gitEnv() });
}

function diffFiles(rangeArgs) {
  const result = git(["diff", "--name-status", "-z", ...rangeArgs]);
  if (result.error || result.status !== 0) return null;
  return parseNameStatus(result.stdout);
}

function untrackedFiles() {
  const result = git(["ls-files", "--others", "--exclude-standard", "-z"]);
  if (result.error || result.status !== 0) return null;
  return result.stdout
    .split("\0")
    .filter(Boolean)
    .map((path) => ({ path: path.replaceAll("\\", "/"), status: "A" }));
}

function isShallow() {
  const result = git(["rev-parse", "--is-shallow-repository"]);
  return result.error || result.status !== 0 || result.stdout.trim() === "true";
}

function revParse(spec) {
  const result = git(["rev-parse", "--verify", spec]);
  return result.error || result.status !== 0 ? null : result.stdout.trim();
}

function oneMergeBase(base, head) {
  const result = git(["merge-base", "--all", base, head]);
  if (result.error || result.status !== 0) return null;
  const bases = result.stdout.trim().split(/\r?\n/).filter(Boolean);
  return bases.length === 1 ? bases[0] : null;
}

// A missing merge base fails closed; in a shallow clone the merge base may be
// a grafted boundary, so that fails closed too.
function branchBase(revision) {
  if (isShallow()) return null;
  return oneMergeBase("origin/main", revision);
}

// Parses the whole pre-push ref list: one record per line, or null when any
// line is malformed. An empty list means the input had no records at all.
export function parsePushInput(input) {
  const records = [];
  for (const line of input.split(/\r?\n/)) {
    if (!line.trim()) continue;
    const parts = line.trim().split(/\s+/);
    if (parts.length !== 4) return null;
    const [localRef, localSha, remoteRef] = parts;
    records.push({ localRef, localSha, remoteRef });
  }
  return records;
}

// Every pushed ref gets its own diff against its own merge base with
// origin/main; the plan is the union over all of them. The dedupe key is the
// (path, before, after) triple, not the path alone: a manifest change is
// classified by content, and two pushed commits can disagree about the same
// path — a scripts-only edit is narrow while a dependency edit on the same
// manifest is full, and keeping both revision pairs means both verdicts
// apply regardless of record order.
// `trees` lists each pushed commit's tree: the disk-keyed cache may only hit
// when every one of them is the tree the checks ran on.
function pushFiles(records) {
  const commits = [];
  for (const record of records ?? []) {
    if (ZERO_SHA.test(record.localSha)) continue; // a ref deletion pushes no code
    const commit = revParse(`${record.localSha}^{commit}`);
    if (!commit) return { full: `${record.localRef} pushes a non-commit object` };
    if (!commits.includes(commit)) commits.push(commit);
  }
  if (records === null) commits.push("HEAD");
  if (commits.length === 0) return { files: [], trees: [] };
  const seen = new Set();
  const files = [];
  const trees = [];
  for (const commit of commits) {
    const base = branchBase(commit);
    if (!base) return { full: `no merge base with origin/main for ${commit.slice(0, 12)}` };
    const diff = diffFiles([base, commit]);
    if (!diff) return { full: "unable to inspect the push diff" };
    const tree = revParse(`${commit}^{tree}`);
    if (!tree) return { full: "unable to read the pushed tree" };
    if (!trees.includes(tree)) trees.push(tree);
    for (const file of diff) {
      const key = `${file.path}${base}${commit}`;
      if (seen.has(key)) continue;
      seen.add(key);
      files.push({ ...file, revisions: { before: base, after: commit } });
    }
  }
  return { files, trees };
}

export function filesForMode(mode) {
  if (mode === "pre-commit" || mode === "worktree") {
    const base = branchBase("HEAD");
    if (!base) return { full: "no merge base with origin/main" };
    // The commit plan describes the index, because that is the tree the commit
    // will contain; the worktree plan describes the disk, which is what the
    // checks and the cache key see. On a clean tree both equal HEAD.
    if (mode === "pre-commit") {
      const files = diffFiles(["--cached", base]);
      return files
        ? { files, revisions: { before: base, after: "index" } }
        : { full: "unable to inspect staged changes" };
    }
    const files = diffFiles([base]);
    const untracked = untrackedFiles();
    return files && untracked
      ? { files: [...files, ...untracked], revisions: { before: base, after: "worktree" } }
      : { full: "unable to inspect worktree changes" };
  }
  if (mode === "pre-push") {
    let records;
    if (process.stdin.isTTY) {
      // A manual run has no pipe; planning HEAD answers "what would I push".
      // Reading a TTY would also block on input, so don't try.
      records = null;
    } else {
      let input = "";
      try {
        input = readFileSync(0, "utf8");
      } catch {
        return { full: "pre-push input is unavailable" };
      }
      records = parsePushInput(input);
      // Under the hook git always sends at least one ref record; a pipe that
      // is empty or malformed is not a "check HEAD" signal.
      if (records === null) return { full: "pre-push input is malformed" };
      if (records.length === 0) return { full: "pre-push input is empty" };
    }
    return pushFiles(records);
  }
  return { full: `unknown planner mode: ${mode}` };
}
