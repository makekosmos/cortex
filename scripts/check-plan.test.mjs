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

test("desktop UI selects desktop typecheck, changed lint, and format", () => {
  const result = plan("--mode", "worktree", "--files", "desktop/src/App.vue");
  assert.deepEqual(result.json.checks, ["desktop-typecheck", "lint", "format"]);
  assert.equal(result.json.full, false);
  assert.match(result.stderr, /desktop\/src\/App\.vue/);
});

test("manager, host contracts, Rust, native, and first-party files expand conservatively", () => {
  assert.deepEqual(plan("--files", "manager/src/App.vue").json.checks, [
    "manager-typecheck",
    "lint",
    "format",
  ]);
  assert.deepEqual(plan("--files", "host/electron/main.ts").json.checks, [
    "host-typecheck",
    "host-contracts",
  ]);
  assert.deepEqual(plan("--files", "runtime/src/lib.rs").json.checks, [
    "rustfmt",
    "clippy",
    "test:rust",
    "runtime-staging",
  ]);
  assert.deepEqual(plan("--files", "native-services/src/main.rs").json.checks, ["native-services"]);
  assert.equal(plan("--files", "host/e2e/first-party-foo.spec.ts").json.full, true);
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
  for (const file of ["README.md", "CHANGELOG.md", "docs/checks.md", "desktop/assets/icon.png"]) {
    const result = plan("--files", file).json;
    assert.equal(result.full, false, file);
    assert.deepEqual(result.checks, [], file);
  }
  for (const file of [
    "desktop/README.md",
    "host/notes.txt",
    "runtime/help.md",
    "packages/readme.md",
  ]) {
    assert.equal(plan("--files", file).json.full, true, file);
  }
  for (const file of ["shared/icon.svg", "packages/foo/logo.png"]) {
    assert.equal(plan("--files", file).json.full, true, file);
  }
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
    { full: false, checks: ["desktop-typecheck", "manager-typecheck"], changed: [], reasons: [] },
    (command) => {
      seen.push(command.name);
      return command.name === "desktop-typecheck" ? 1 : 0;
    },
  );
  assert.equal(status, 1);
  assert.deepEqual(seen, ["desktop-typecheck", "manager-typecheck"]);
});

test("full gate runs the root check plus host and first-party contract suites", async () => {
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
  assert.deepEqual(seen, [
    "pnpm run check",
    "pnpm run test:host-contracts",
    "pnpm run test:first-party-contracts",
  ]);
});

test("full gate still fails when a contract suite fails and runs every command", async () => {
  const { executePlan } = await import("./check-plan.mjs");
  const seen = [];
  const status = executePlan(
    { mode: "worktree", full: true, checks: ["full"], changed: [], reasons: [] },
    (command) => {
      seen.push(command.name);
      return command.name === "host-contracts" ? 1 : 0;
    },
  );
  assert.equal(status, 1);
  assert.deepEqual(seen, ["full", "host-contracts", "first-party-contracts"]);
});

test("full gate runs the same contract commands an affected plan selects", async () => {
  const { createPlan, executePlan } = await import("./check-plan.mjs");
  const run = (plan) => {
    const seen = [];
    executePlan(plan, (command) => {
      seen.push(`${command.command} ${command.args.join(" ")}`);
      return 0;
    });
    return seen;
  };
  const affected = run(createPlan({ mode: "worktree", files: ["host/electron/main.ts"] }));
  assert.ok(affected.includes("pnpm run test:host-contracts"));
  const full = run(createPlan({ mode: "worktree", full: true }));
  assert.ok(full.includes("pnpm run test:host-contracts"));
  assert.ok(full.includes("pnpm run test:first-party-contracts"));
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
  assert.deepEqual(seen, ["source-size"]);
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
