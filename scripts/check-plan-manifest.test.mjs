import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { DOC_CONTRACTS } from "./check-plan-manifest.mjs";
import { gitEnv } from "./git-env.mjs";

const script = fileURLToPath(new URL("./check-plan.mjs", import.meta.url));
const root = fileURLToPath(new URL("../", import.meta.url));

// gitEnv(): under a hook, GIT_DIR/GIT_INDEX_FILE would point these commands
// (including `git config user.*`) at the real repository.
function git(cwd, ...args) {
  return execFileSync("git", args, { cwd, encoding: "utf8", env: gitEnv() });
}

function planIn(cwd, mode) {
  const result = spawnSync(process.execPath, [script, "--mode", mode], {
    cwd,
    encoding: "utf8",
    env: gitEnv(),
  });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("worktree and pre-commit modes read package.json revisions from git", (t) => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "check-plan-manifest-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const write = (manifest) =>
    writeFileSync(path.join(dir, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  git(dir, "init", "-q");
  git(dir, "config", "user.email", "check-plan@example.invalid");
  git(dir, "config", "user.name", "check-plan");
  write({ scripts: { clippy: "cargo clippy" }, devDependencies: { oxlint: "1.0.0" } });
  git(dir, "add", "package.json");
  git(dir, "commit", "-qm", "base");
  // Every mode plans the diff against merge-base(HEAD, origin/main).
  git(dir, "update-ref", "refs/remotes/origin/main", "HEAD");

  write({ scripts: { clippy: "cargo clippy -D warnings" }, devDependencies: { oxlint: "1.0.0" } });
  assert.deepEqual(planIn(dir, "worktree").checks, ["package-manager", "clippy", "format"]);
  // Nothing is staged yet, so the staged diff is empty.
  assert.deepEqual(planIn(dir, "pre-commit").checks, []);
  git(dir, "add", "package.json");
  // The commit defers clippy to pre-push (check-plan-commit.mjs) and says so.
  const staged = planIn(dir, "pre-commit");
  assert.deepEqual(staged.checks, ["package-manager", "format"]);
  assert.match(staged.reasons.at(-1), /deferred to pre-push: clippy/);

  write({ scripts: { clippy: "cargo clippy -D warnings" }, devDependencies: { oxlint: "2.0.0" } });
  assert.equal(planIn(dir, "worktree").full, true);
  // The staged blob, not the dirtier worktree file, decides pre-commit: a
  // full plan would become the fast gate at commit time.
  assert.deepEqual(planIn(dir, "pre-commit").checks, ["package-manager", "format"]);
});

// Every quoted .md/.mdx/.txt literal in a test that names a tracked document
// makes that document a contract; it must be mapped in DOC_CONTRACTS so an
// edit to it runs the test instead of selecting no checks.
test("documents read by tests are listed as contract docs", () => {
  const tracked = new Set(git(root, "ls-files", "-z").split("\0").filter(Boolean));
  const tests = [...tracked].filter(
    (file) => /\.test\.[cm]?[jt]s$/.test(file) && !/^core\//.test(file) && !/check-plan/.test(file),
  );
  const readers = {};
  for (const file of tests) {
    const source = readFileSync(path.join(root, file), "utf8");
    for (const [, literal] of source.matchAll(/["'`]([\w./-]+\.(?:md|mdx|txt))["'`]/g)) {
      const dir = path.posix.dirname(file);
      for (const candidate of [
        literal,
        `${dir}/${literal}`,
        `${path.posix.dirname(dir)}/${literal}`,
      ]) {
        const doc = path.posix.normalize(candidate);
        if (tracked.has(doc) && existsSync(path.join(root, doc))) (readers[doc] ??= []).push(file);
      }
    }
  }
  for (const [doc, files] of Object.entries(readers))
    assert.ok(Object.hasOwn(DOC_CONTRACTS, doc), `${doc} is read by ${files.join(", ")}`);
  assert.ok(readers["README.md"], "the scan still finds the known README.md reader");
});
