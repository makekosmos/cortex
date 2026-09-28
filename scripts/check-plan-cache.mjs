import {
  copyFileSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import os from "node:os";
import path from "node:path";
import { gitEnv } from "./git-env.mjs";

// Remembers which checks passed for an exact on-disk tree, so the pre-push
// hook does not repeat what the pre-commit hook (or `check:affected`) just
// ran on the same files. The key is the tree hash of every non-ignored file
// on disk — the checks read the disk, not the index — so any edit, staged or
// not, and any new untracked file produces a different key.

function git(cwd, args, extraEnv = {}) {
  const env = { ...gitEnv(), ...extraEnv };
  const result = spawnSync("git", args, { cwd, encoding: "utf8", env });
  return result.error || result.status !== 0 ? null : result.stdout.trim();
}

/** Tree hash of the working tree as the checks see it, or null if unknown. */
export function diskTree(cwd = process.cwd()) {
  const index = git(cwd, ["rev-parse", "--path-format=absolute", "--git-path", "index"]);
  if (!index) return null;
  const temp = mkdtempSync(path.join(os.tmpdir(), "cortex-check-tree-"));
  try {
    const tempIndex = path.join(temp, "index");
    if (existsSync(index)) copyFileSync(index, tempIndex);
    const env = { GIT_INDEX_FILE: tempIndex };
    if (git(cwd, ["add", "--all", "--", "."], env) === null) return null;
    return git(cwd, ["write-tree"], env);
  } finally {
    rmSync(temp, { recursive: true, force: true });
  }
}

function entryPath(cwd, tree) {
  const common = git(cwd, ["rev-parse", "--path-format=absolute", "--git-common-dir"]);
  return common ? path.join(common, "cortex-check-cache", `${tree}.json`) : null;
}

function passedChecks(cwd, tree) {
  const file = entryPath(cwd, tree);
  if (!file) return new Set();
  try {
    return new Set(JSON.parse(readFileSync(file, "utf8")).checks);
  } catch {
    return new Set();
  }
}

/** True when every check in the plan already passed on this exact tree. */
export function coveredByCache(cwd, tree, checks) {
  const passed = passedChecks(cwd, tree);
  return passed.has("full") || checks.every((check) => passed.has(check));
}

export function recordPass(cwd, tree, checks) {
  if (checks.length === 0) return;
  const file = entryPath(cwd, tree);
  if (!file) return;
  const passed = passedChecks(cwd, tree);
  for (const check of checks) passed.add(check);
  mkdirSync(path.dirname(file), { recursive: true });
  writeFileSync(file, `${JSON.stringify({ checks: [...passed].sort() })}\n`);
}
