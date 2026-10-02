import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { gitEnv } from "./git-env.mjs";

const script = fileURLToPath(new URL("./check-plan.mjs", import.meta.url));
const root = fileURLToPath(new URL("../", import.meta.url));

function invoke(args, options = {}) {
  return spawnSync(process.execPath, [script, ...args], {
    encoding: "utf8",
    ...options,
  });
}

function plan(...args) {
  const result = invoke(args);
  assert.equal(result.status, 0, result.stderr);
  return { json: JSON.parse(result.stdout), stderr: result.stderr };
}

test("desktop scripts select lint and format", () => {
  const result = plan("--mode", "worktree", "--files", "desktop/scripts/engine-distribution.mjs");
  assert.deepEqual(result.json.checks, ["lint", "format"]);
  assert.equal(result.json.full, false);
  assert.match(result.stderr, /desktop\/scripts\/engine-distribution\.mjs/);
});

test("Rust, native, and packaging files expand conservatively", () => {
  assert.deepEqual(plan("--files", "runtime/src/lib.rs").json.checks, [
    "rustfmt",
    "clippy",
    "test:rust",
    "runtime-staging",
  ]);
  assert.deepEqual(plan("--files", "manager-gpui/src/app.rs").json.checks, ["manager-gpui"]);
});

test("core subtree maps crates to rust checks and inert paths to none", () => {
  assert.deepEqual(plan("--files", "core/crates/ark-core/src/lib.rs").json.checks, [
    "rustfmt",
    "clippy",
    "test:rust",
  ]);
  assert.deepEqual(plan("--files", "core/docs/plan.md").json.checks, []);
  assert.equal(plan("--files", "core/Cargo.toml").json.full, true);
  assert.equal(plan("--files", "core/ark/packages/ark/package.json").json.full, true);
});

test("manager-gpui maps to its standalone crate gate", () => {
  assert.deepEqual(plan("--files", "manager-gpui/src/app.rs").json.checks, ["manager-gpui"]);
  assert.equal(plan("--files", "manager-gpui/Cargo.toml").json.full, true);
});

test("shared, lockfile, build, workflow, and unknown files fail closed", () => {
  for (const file of [
    "shared/ipc.ts",
    "bun.lock",
    "runtime/Cargo.toml",
    "desktop/package.json",
    "packages/nested/pnpm-lock.yaml",
    "runtime/rust-toolchain.toml",
    "desktop/scripts/build.mjs",
    ".github/workflows/ci.yml",
    "scripts/new-tool.mjs",
    "packages/new-contract.ts",
  ]) {
    const result = plan("--files", file).json;
    assert.equal(result.full, true, file);
    assert.deepEqual(result.checks, ["full"], file);
  }
});

test("deleted shared or build files still fail closed", async () => {
  const { createPlan } = await import("./check-plan.mjs");
  for (const path of ["shared/ipc.ts", "desktop/scripts/build.mjs"]) {
    const result = createPlan({ mode: "worktree", files: [{ path, status: "D" }] });
    assert.equal(result.full, true, path);
  }
});

test("docs and isolated assets are a safe no-op", () => {
  for (const file of [
    "CHANGELOG.md",
    "docs/checks.md",
    "desktop/assets/icon.png",
    "desktop/DEV.md",
    "desktop/README.md",
    "runtime/help.md",
    "runtime/notes.txt",
    "packages/readme.mdx",
  ]) {
    const result = plan("--files", file).json;
    assert.equal(result.full, false, file);
    assert.deepEqual(result.checks, [], file);
  }
  for (const file of ["host/notes.txt", "runtime/native/CMakeLists.txt", "shared/requirements.txt"])
    assert.equal(plan("--files", file).json.full, true, file);
  for (const file of ["shared/icon.svg", "packages/foo/logo.png"]) {
    assert.equal(plan("--files", file).json.full, true, file);
  }
});

test("README.md is a contract doc read by the package-manager test", () => {
  assert.deepEqual(plan("--files", "README.md").json.checks, ["package-manager"]);
});

const manifest = (scripts, extra = {}) =>
  JSON.stringify({ name: "cortex", scripts, devDependencies: { oxlint: "1.0.0" }, ...extra });
const edit = (path, before, after) => ({ path, status: "M", before, after });

test("scripts-only package.json edits select the checks those scripts belong to", async () => {
  const { createPlan } = await import("./check-plan.mjs");
  const checks = (...files) => createPlan({ files }).checks;
  const base = { "test:desktop-contracts": "node a.mjs", rustfmt: "cargo fmt", check: "x" };
  const root = (scripts) => edit("package.json", manifest(base), manifest(scripts));
  assert.deepEqual(checks(root({ ...base, "test:desktop-contracts": "node b.mjs" })), [
    "package-manager",
    "desktop-contracts",
    "format",
  ]);
  const withoutRustfmt = { "test:desktop-contracts": "node a.mjs", check: "x" };
  assert.deepEqual(checks(root(withoutRustfmt)), ["package-manager", "rustfmt", "format"]);
  const desktop = edit("desktop/package.json", manifest({ build: "a" }), manifest({ build: "b" }));
  assert.deepEqual(checks(desktop), ["package-manager", "release-bom", "format"]);
  // Whitespace-only reformatting is not a semantic change.
  const reformatted = JSON.stringify(JSON.parse(manifest(base)), null, 2);
  assert.deepEqual(checks(edit("package.json", manifest(base), reformatted)), [
    "package-manager",
    "format",
  ]);
});

test("dependency, tooling, unmapped, and unreadable manifest edits fail closed", async () => {
  const { createPlan } = await import("./check-plan.mjs");
  const scripts = { rustfmt: "cargo fmt", check: "x" };
  const before = manifest(scripts);
  for (const after of [
    manifest(scripts, { devDependencies: { oxlint: "2.0.0" } }),
    manifest({ ...scripts, rustfmt: "cargo fmt --all" }, { devDependencies: {} }),
    manifest(scripts, { engines: { node: ">=24" } }),
    manifest(scripts, { packageManager: "pnpm@13.0.0" }),
    manifest(scripts, { "custom-section": { pins: {} } }),
    manifest({ ...scripts, check: "y" }),
    manifest({ ...scripts, "brand-new": "node x.mjs" }),
    manifest("not-an-object"),
    "{ not json",
  ]) {
    const result = createPlan({ files: [edit("package.json", before, after)] });
    assert.equal(result.full, true, after);
    assert.match(result.reasons[0], /package\.json requires the full check \(/, after);
  }
  for (const file of [
    { path: "package.json", status: "M" },
    { path: "package.json", status: "A", before, after: before },
    { path: "desktop/package.json", status: "D", before, after: before },
    edit("core/ark/packages/ark/package.json", before, before),
  ])
    assert.equal(createPlan({ files: [file] }).full, true, JSON.stringify(file));
});

test("explicit full always selects every check", () => {
  const result = plan("--full").json;
  assert.equal(result.full, true);
  assert.deepEqual(result.checks, ["full"]);
});

test("unknown planner modes fail closed", async () => {
  const { createPlan } = await import("./check-plan.mjs");
  for (const mode of ["ci", "nonexistent"]) {
    assert.equal(createPlan({ mode }).full, true, mode);
  }
});

test("pre-push input parses one update; zero and ambiguous records defer to HEAD", async () => {
  const { parseNameStatus, parsePushInput } = await import("./check-plan.mjs");
  assert.deepEqual(parsePushInput("refs/heads/feature abc refs/heads/main def\n"), {
    localRef: "refs/heads/feature",
    localSha: "abc",
    remoteRef: "refs/heads/main",
  });
  // A new remote ref is a normal record: the remote sha is simply all zeros.
  assert.deepEqual(
    parsePushInput(
      "refs/heads/feature abc refs/heads/feature 0000000000000000000000000000000000000000\n",
    )?.localSha,
    "abc",
  );
  // A deletion has a zero local sha; several or malformed records parse to null.
  assert.equal(
    parsePushInput(
      "refs/heads/feature 0000000000000000000000000000000000000000 refs/heads/feature abc\n",
    )?.localSha,
    "0000000000000000000000000000000000000000",
  );
  assert.equal(parsePushInput("one\ntwo\n"), null);
  assert.deepEqual(parseNameStatus("R100\0old/path.ts\0new/path.ts\0"), [
    { path: "old/path.ts", status: "R" },
    { path: "new/path.ts", status: "R" },
  ]);
  assert.deepEqual(parseNameStatus("C100\0old/path.ts\0copy/path.ts\0"), [
    { path: "old/path.ts", status: "C" },
    { path: "copy/path.ts", status: "C" },
  ]);
  for (const status of ["T", "U", "X", "Z"])
    assert.equal(parseNameStatus(`${status}\0file\0`), null);
});

test("command failures aggregate instead of stopping after the first selected group", async () => {
  const { executePlan } = await import("./check-plan.mjs");
  const seen = [];
  const status = executePlan(
    { full: false, checks: ["desktop-contracts", "manager-gpui"], changed: [], reasons: [] },
    (command) => {
      seen.push(command.name);
      return command.name === "desktop-contracts" ? 1 : 0;
    },
  );
  assert.equal(status, 1);
  assert.deepEqual(seen, ["brand", "test-skips", "desktop-contracts", "manager-gpui"]);
});

test("full gate runs the root check", async () => {
  const { executePlan } = await import("./check-plan.mjs");
  const seen = [];
  const status = executePlan(
    { mode: "worktree", full: true, checks: ["full"], changed: [], reasons: [] },
    (command) => {
      seen.push(`${command.command} ${command.args.join(" ")}`);
      return 0;
    },
  );
  assert.equal(status, 0);
  assert.deepEqual(seen, ["pnpm run check"]);
});

test("full gate still fails when the root check fails", async () => {
  const { executePlan } = await import("./check-plan.mjs");
  const seen = [];
  const status = executePlan(
    { mode: "worktree", full: true, checks: ["full"], changed: [], reasons: [] },
    (command) => {
      seen.push(command.name);
      return command.name === "full" ? 1 : 0;
    },
  );
  assert.equal(status, 1);
  assert.deepEqual(seen, ["full"]);
});

test("pre-commit retains the existing source-size safeguard through the planner", async () => {
  const { executePlan } = await import("./check-plan.mjs");
  const seen = [];
  assert.equal(
    executePlan(
      { mode: "pre-commit", full: false, checks: [], changed: [], reasons: [] },
      (command) => {
        seen.push(command.name);
        return 0;
      },
    ),
    0,
  );
  assert.deepEqual(seen, ["brand", "test-skips", "source-size"]);
});

test("hook entrypoints keep the planner gate", () => {
  const hook = readFileSync(`${root}lefthook.yml`, "utf8");
  assert.match(hook, /check:plan --mode pre-commit --run/);
  assert.match(hook, /check:plan --mode pre-push --run/);
  assert.match(hook, /use_stdin: true/);
});

test("checkEnv strips git hook variables from spawned check commands", async () => {
  const { checkEnv } = await import("./check-plan-commands.mjs");
  const saved = process.env.GIT_DIR;
  process.env.GIT_DIR = "leaked-by-git";
  try {
    const env = checkEnv();
    assert.equal(env.GIT_DIR, undefined);
    assert.equal(env.GIT_WORK_TREE, undefined);
    assert.equal(env.PATH, process.env.PATH);
  } finally {
    if (saved === undefined) delete process.env.GIT_DIR;
    else process.env.GIT_DIR = saved;
  }
});

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
  assert.equal(push.tree, tree, "a clean tree is the pushed HEAD tree");
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

test("a multi-ref push like a release plans HEAD, not an ambiguous failure", (t) => {
  const { dir, git } = branchRepo(t, { "docs/probe.md": "x\n" });
  const head = git("rev-parse", "HEAD");
  const zero = "0".repeat(40);
  const result = spawnSync(process.execPath, [script, "--mode", "pre-push"], {
    cwd: dir,
    encoding: "utf8",
    env: gitEnv(),
    input: `refs/heads/t ${head} refs/heads/main ${zero}\nrefs/tags/v9 ${head} refs/tags/v9 ${zero}\n`,
  });
  assert.equal(result.status, 0, result.stderr);
  const plan = JSON.parse(result.stdout);
  assert.equal(plan.full, false, plan.reasons.join("; "));
  assert.deepEqual(plan.changed, ["docs/probe.md"]);
});

test("check records its pass as a full cache entry", async (t) => {
  const { coveredByCache, diskTree } = await import("./check-plan-cache.mjs");
  const { dir } = branchRepo(t, { "docs/probe.md": "x\n" });
  const result = spawnSync(process.execPath, [script, "--record-full"], {
    cwd: dir,
    encoding: "utf8",
    env: gitEnv(),
  });
  assert.equal(result.status, 0, result.stderr);
  const tree = diskTree(dir);
  assert.equal(coveredByCache(dir, tree, ["full"]), true);
  assert.equal(coveredByCache(dir, tree, ["clippy", "lint"]), true);
});
