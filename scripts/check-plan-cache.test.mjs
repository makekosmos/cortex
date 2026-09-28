import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { coveredByCache, diskTree, recordPass } from "./check-plan-cache.mjs";
import { gitEnv } from "./git-env.mjs";

function repo() {
  const dir = mkdtempSync(path.join(os.tmpdir(), "cortex-check-cache-"));
  // gitEnv(): under a hook, GIT_DIR/GIT_INDEX_FILE would point these commands
  // (including `git config user.*`) at the real repository.
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: dir, encoding: "utf8", env: gitEnv() });
    assert.equal(result.status, 0, result.stderr);
  };
  git("init", "-q");
  git("config", "user.email", "test@example.invalid");
  git("config", "user.name", "test");
  writeFileSync(path.join(dir, ".gitignore"), "target/\n");
  writeFileSync(path.join(dir, "a.rs"), "fn a() {}\n");
  git("add", ".");
  git("commit", "-q", "-m", "init");
  return { dir, git, cleanup: () => rmSync(dir, { recursive: true, force: true }) };
}

test("disk tree follows unstaged edits and untracked files", (t) => {
  const { dir, git, cleanup } = repo();
  t.after(cleanup);
  const clean = diskTree(dir);
  assert.match(clean, /^[0-9a-f]{40}$/);

  writeFileSync(path.join(dir, "new.rs"), "fn n() {}\n");
  assert.notEqual(diskTree(dir), clean);
  rmSync(path.join(dir, "new.rs"));
  assert.equal(diskTree(dir), clean);

  writeFileSync(path.join(dir, "a.rs"), "fn a() { 1; }\n");
  const edited = diskTree(dir);
  assert.notEqual(edited, clean);
  git("add", "a.rs");
  assert.equal(diskTree(dir), edited, "staging does not change what is on disk");
});

test("ignored build output does not change the disk tree", (t) => {
  const { dir, cleanup } = repo();
  t.after(cleanup);
  const clean = diskTree(dir);
  mkdirSync(path.join(dir, "target"));
  writeFileSync(path.join(dir, "target", "out.bin"), "binary");
  assert.equal(diskTree(dir), clean);
});

test("a recorded pass covers the same or a smaller set of checks", (t) => {
  const { dir, cleanup } = repo();
  t.after(cleanup);
  const tree = diskTree(dir);
  assert.equal(coveredByCache(dir, tree, ["rustfmt"]), false);

  recordPass(dir, tree, ["rustfmt", "clippy"]);
  assert.equal(coveredByCache(dir, tree, ["rustfmt"]), true);
  assert.equal(coveredByCache(dir, tree, ["clippy", "rustfmt"]), true);
  assert.equal(coveredByCache(dir, tree, ["rustfmt", "test:rust"]), false);

  recordPass(dir, tree, ["full"]);
  assert.equal(coveredByCache(dir, tree, ["manager-gpui", "lint"]), true);

  writeFileSync(path.join(dir, "a.rs"), "fn b() {}\n");
  assert.equal(coveredByCache(dir, diskTree(dir), ["rustfmt"]), false);
});
