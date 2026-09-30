import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";

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
    manifest(scripts, { mundus: { workspace: {} } }),
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

test("pre-push input parses one update and fails closed for new or ambiguous pushes", async () => {
  const { parseNameStatus, parsePushInput } = await import("./check-plan.mjs");
  assert.deepEqual(parsePushInput("refs/heads/feature abc refs/heads/main def\n"), {
    localRef: "refs/heads/feature",
    localSha: "abc",
    remoteRef: "refs/heads/main",
    remoteSha: "def",
  });
  assert.equal(
    parsePushInput(
      "refs/heads/feature abc refs/heads/main 0000000000000000000000000000000000000000\n",
    ),
    null,
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

test("pre-push mode uses the pushed ref range from raw stdin", () => {
  const sha = "4c1f2a139fcae92ac0cf995b40ea63a7713c3b63";
  const result = invoke(["--mode", "pre-push"], {
    input: `refs/heads/kos-15 ${sha} refs/heads/main ${sha}\n`,
  });
  assert.equal(result.status, 0, result.stderr);
  assert.equal(JSON.parse(result.stdout).full, false);
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
  assert.deepEqual(seen, ["brand", "desktop-contracts", "manager-gpui"]);
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
  assert.deepEqual(seen, ["brand", "source-size"]);
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
