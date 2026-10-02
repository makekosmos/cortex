import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { gitEnv } from "./git-env.mjs";

const script = fileURLToPath(new URL("./check-plan.mjs", import.meta.url));
const zero = "0".repeat(40);

// A scratch repository whose HEAD branch sits one commit ahead of a fabricated
// refs/remotes/origin/main, mirroring the branch layout every mode plans over.
// gitEnv(): under a hook, GIT_DIR/GIT_INDEX_FILE would point these commands at
// the real repository.
function branchRepo(t, files) {
  const dir = mkdtempSync(path.join(os.tmpdir(), "check-plan-branch-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: dir, encoding: "utf8", env: gitEnv() });
    assert.equal(result.status, 0, `${args.join(" ")}: ${result.stderr}`);
    return result.stdout.trim();
  };
  git("init", "-q");
  git("config", "user.email", "check-plan@example.invalid");
  git("config", "user.name", "check-plan");
  writeFileSync(path.join(dir, "a.md"), "base\n");
  git("add", ".");
  git("commit", "-qm", "base");
  git("update-ref", "refs/remotes/origin/main", "HEAD");
  for (const [name, content] of Object.entries(files)) {
    const file = path.join(dir, name);
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, content);
    git("add", name);
  }
  git("commit", "-qm", "branch work");
  const planIn = (mode) => {
    const head = git("rev-parse", "HEAD");
    const result = spawnSync(process.execPath, [script, "--mode", mode], {
      cwd: dir,
      encoding: "utf8",
      env: gitEnv(),
      // Every push in these tests creates the remote ref, like the first
      // `git push -u origin <branch>` of a fresh task branch.
      input: `refs/heads/t ${head} refs/heads/t 0000000000000000000000000000000000000000\n`,
    });
    assert.equal(result.status, 0, result.stderr);
    return JSON.parse(result.stdout);
  };
  return { dir, git, planIn };
}

test("worktree and pre-push compute the same plan for the same tree", (t) => {
  const { planIn } = branchRepo(t, { "desktop/scripts/probe.mjs": "export {};\n" });
  const worktree = planIn("worktree");
  const push = planIn("pre-push");
  for (const plan of [worktree, push]) {
    assert.equal(plan.full, false, plan.reasons.join("; "));
    assert.deepEqual(plan.changed, ["desktop/scripts/probe.mjs"]);
    assert.deepEqual(plan.checks, ["lint", "format"]);
  }
});

test("a new-ref push is covered by a check:affected pass on the same tree", async (t) => {
  const { coveredByCache, diskTree, recordPass } = await import("./check-plan-cache.mjs");
  const { dir, planIn } = branchRepo(t, { "runtime/src/probe.rs": "pub fn p() {}\n" });
  const worktree = planIn("worktree");
  assert.deepEqual(worktree.checks, ["rustfmt", "clippy", "test:rust", "runtime-staging"]);
  const tree = diskTree(dir);
  recordPass(dir, tree, worktree.checks);
  const push = planIn("pre-push");
  assert.deepEqual(push.trees, [tree], "a clean tree is the pushed HEAD tree");
  assert.equal(coveredByCache(dir, tree, push.checks), true);
  writeFileSync(path.join(dir, "runtime/src/probe.rs"), "pub fn p() { 1; }\n");
  assert.equal(
    coveredByCache(dir, diskTree(dir), push.checks),
    false,
    "editing the tree after the pass invalidates the entry",
  );
});

test("a manifest added on the branch still widens a push to the full check", (t) => {
  const { planIn } = branchRepo(t, { "package.json": '{"name":"probe"}\n' });
  const push = planIn("pre-push");
  assert.equal(push.full, true);
  assert.match(push.reasons.join("; "), /package\.json/);
});

function pushPlanIn(dir, input) {
  const result = spawnSync(process.execPath, [script, "--mode", "pre-push"], {
    cwd: dir,
    encoding: "utf8",
    env: gitEnv(),
    input,
  });
  assert.equal(result.status, 0, result.stderr);
  return JSON.parse(result.stdout);
}

test("a release push of branch plus tag on one commit dedupes to one plan", (t) => {
  const { dir, git } = branchRepo(t, { "docs/probe.md": "x\n" });
  const head = git("rev-parse", "HEAD");
  const single = pushPlanIn(dir, `refs/heads/t ${head} refs/heads/t ${zero}\n`);
  const release = pushPlanIn(
    dir,
    `refs/heads/t ${head} refs/heads/main ${zero}\nrefs/tags/v9 ${head} refs/tags/v9 ${zero}\n`,
  );
  for (const plan of [single, release]) {
    assert.equal(plan.full, false, plan.reasons.join("; "));
    assert.deepEqual(plan.changed, ["docs/probe.md"]);
    assert.equal(plan.trees.length, 1);
  }
});

test("a push of two different commits plans the union of both diffs", (t) => {
  const { dir, git } = branchRepo(t, { "desktop/scripts/probe.mjs": "export {};\n" });
  const first = git("rev-parse", "HEAD");
  // A second ref on an independent commit that also touches the tree.
  git("checkout", "-q", "-b", "other", "origin/main");
  mkdirSync(path.join(dir, "docs"), { recursive: true });
  writeFileSync(path.join(dir, "docs", "other.md"), "y\n");
  git("add", "docs/other.md");
  git("commit", "-qm", "other branch work");
  const second = git("rev-parse", "HEAD");
  const plan = pushPlanIn(
    dir,
    `refs/heads/a ${first} refs/heads/a ${zero}\nrefs/heads/b ${second} refs/heads/b ${zero}\n`,
  );
  assert.equal(plan.full, false, plan.reasons.join("; "));
  assert.deepEqual(plan.changed.sort(), ["desktop/scripts/probe.mjs", "docs/other.md"]);
  assert.equal(plan.trees.length, 2, "each pushed commit certifies its own tree");
  assert.deepEqual(plan.checks, ["lint", "format"]);
});

test("empty or malformed piped stdin under the hook fails closed", (t) => {
  const { dir } = branchRepo(t, { "docs/probe.md": "x\n" });
  for (const [input, reason] of [
    ["", /input is empty/],
    ["too few fields\n", /input is malformed/],
  ]) {
    const plan = pushPlanIn(dir, input);
    assert.equal(plan.full, true, JSON.stringify(input));
    assert.match(plan.reasons.join("; "), reason);
  }
});

test("a passing full run records full only when the tree did not change", async (t) => {
  const { runPlan } = await import("./check-plan.mjs");
  const { coveredByCache, diskTree } = await import("./check-plan-cache.mjs");
  const { dir } = branchRepo(t, { "docs/probe.md": "x\n" });
  const plan = { mode: "worktree", full: true, checks: ["full"], changed: [], trees: [] };
  const clean = await new Promise((resolve) => {
    const cwd = process.cwd();
    process.chdir(dir);
    try {
      resolve(runPlan(plan, {}, () => 0));
    } finally {
      process.chdir(cwd);
    }
  });
  assert.equal(clean, 0);
  const tree = diskTree(dir);
  assert.equal(coveredByCache(dir, tree, ["full"]), true, "unchanged tree records the pass");

  const { dir: other } = branchRepo(t, { "docs/probe.md": "x\n" });
  const before = diskTree(other);
  const dirty = await new Promise((resolve) => {
    const cwd = process.cwd();
    process.chdir(other);
    try {
      resolve(
        runPlan(plan, {}, () => {
          writeFileSync(path.join(other, "docs", "late.md"), "changed mid-run\n");
          return 0;
        }),
      );
    } finally {
      process.chdir(cwd);
    }
  });
  assert.equal(dirty, 0);
  assert.notEqual(diskTree(other), before);
  assert.equal(
    coveredByCache(other, diskTree(other), ["full"]),
    false,
    "a tree that changed during the run records nothing",
  );
});

test("a push of a commit that is not the disk tree cannot hit the disk cache", async (t) => {
  const { runPlan } = await import("./check-plan.mjs");
  const { coveredByCache, diskTree, recordPass } = await import("./check-plan-cache.mjs");
  const { dir, git } = branchRepo(t, { "desktop/scripts/probe.mjs": "export {};\n" });
  const tree = diskTree(dir);
  recordPass(dir, tree, ["lint", "format"]);
  const head = git("rev-parse", "HEAD");
  // Another ref pointing at a different commit; its tree is not the disk tree.
  git("checkout", "-q", "-b", "other", "origin/main");
  mkdirSync(path.join(dir, "docs"), { recursive: true });
  writeFileSync(path.join(dir, "docs", "other.md"), "y\n");
  git("add", "docs/other.md");
  git("commit", "-qm", "other");
  const second = git("rev-parse", "HEAD");
  const plan = pushPlanIn(
    dir,
    `refs/heads/a ${head} refs/heads/a ${zero}\nrefs/heads/b ${second} refs/heads/b ${zero}\n`,
  );
  const ran = [];
  const cwd = process.cwd();
  process.chdir(dir);
  try {
    assert.equal(
      runPlan(plan, { run: true }, (command) => (ran.push(command.name), 0)),
      0,
    );
  } finally {
    process.chdir(cwd);
  }
  assert.ok(ran.length > 0, "a pushed tree other than the disk tree must not hit the cache");
  assert.equal(
    coveredByCache(dir, tree, plan.checks),
    true,
    "the earlier pass is still recorded for the disk tree itself",
  );
});

test("the same manifest in two pushed commits is classified by content, not order", (t) => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "check-plan-push-"));
  t.after(() => rmSync(dir, { recursive: true, force: true }));
  const git = (...args) => {
    const result = spawnSync("git", args, { cwd: dir, encoding: "utf8", env: gitEnv() });
    assert.equal(result.status, 0, `${args.join(" ")}: ${result.stderr}`);
    return result.stdout.trim();
  };
  git("init", "-q");
  git("config", "user.email", "check-plan@example.invalid");
  git("config", "user.name", "check-plan");
  const manifest = (clippy, oxlint) =>
    `${JSON.stringify({ scripts: { clippy }, devDependencies: { oxlint } }, null, 2)}\n`;
  writeFileSync(path.join(dir, "package.json"), manifest("cargo clippy", "1.0.0"));
  git("add", ".");
  git("commit", "-qm", "base");
  git("update-ref", "refs/remotes/origin/main", "HEAD");

  // Branch a: scripts-only edit — narrow. Branch b: a dependency edit — full.
  writeFileSync(path.join(dir, "package.json"), manifest("cargo clippy -D warnings", "1.0.0"));
  git("add", "package.json");
  git("commit", "-qm", "scripts only");
  const a = git("rev-parse", "HEAD");
  git("checkout", "-q", "-b", "b", "origin/main");
  writeFileSync(path.join(dir, "package.json"), manifest("cargo clippy", "2.0.0"));
  git("add", "package.json");
  git("commit", "-qm", "dependency bump");
  const b = git("rev-parse", "HEAD");

  for (const input of [
    `refs/heads/a ${a} refs/heads/a ${zero}\nrefs/heads/b ${b} refs/heads/b ${zero}\n`,
    `refs/heads/b ${b} refs/heads/b ${zero}\nrefs/heads/a ${a} refs/heads/a ${zero}\n`,
  ]) {
    const plan = pushPlanIn(dir, input);
    assert.equal(plan.full, true, input);
    assert.deepEqual(plan.changed, ["package.json"], "the union still lists the path once");
  }
});

function runPlanStderr(dir, plan) {
  const writes = [];
  const original = process.stderr.write;
  process.stderr.write = (chunk) => (writes.push(String(chunk)), true);
  const cwd = process.cwd();
  process.chdir(dir);
  try {
    return import("./check-plan.mjs").then(({ runPlan }) => {
      try {
        return { status: runPlan(plan, {}, () => 0), stderr: writes.join("") };
      } finally {
        process.chdir(cwd);
        process.stderr.write = original;
      }
    });
  } catch (error) {
    process.chdir(cwd);
    process.stderr.write = original;
    throw error;
  }
}

test("a mismatch between pushed and on-disk trees says the checks ran on disk", async (t) => {
  const { diskTree } = await import("./check-plan-cache.mjs");
  const { dir } = branchRepo(t, { "docs/probe.md": "x\n" });
  const plan = { mode: "pre-push", full: false, checks: ["lint"], changed: [] };

  const same = await runPlanStderr(dir, { ...plan, trees: [diskTree(dir)] });
  assert.equal(same.status, 0);
  assert.doesNotMatch(same.stderr, /working tree/);

  const other = "f".repeat(40);
  const diff = await runPlanStderr(dir, { ...plan, trees: [other] });
  assert.equal(diff.status, 0);
  assert.match(diff.stderr, /checks run on the working tree/);
});
