import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("./check-plan.mjs", import.meta.url));

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
  const result = plan("--mode", "ci", "--files", "desktop/src/App.vue");
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
    assert.deepEqual(result.jobs, [
      "actionlint",
      "portable",
      "windows-runtime",
      "first-party-contracts",
    ]);
  }
});

test("deleted shared or build files still fail closed", async () => {
  const { createPlan } = await import("./check-plan.mjs");
  for (const path of ["shared/ipc.ts", "desktop/scripts/build.mjs"]) {
    const result = createPlan({ mode: "ci", files: [{ path, status: "D" }] });
    assert.equal(result.full, true, path);
  }
});

test("docs and isolated assets are a safe no-op", () => {
  for (const file of ["README.md", "CHANGELOG.md", "docs/checks.md", "desktop/assets/icon.png"]) {
    const result = plan("--files", file).json;
    assert.equal(result.full, false, file);
    assert.deepEqual(result.checks, [], file);
    assert.deepEqual(result.jobs, [], file);
  }
  for (const file of [
    "desktop/README.md",
    "host/notes.txt",
    "runtime/help.md",
    "packages/readme.md",
  ]) {
    assert.equal(plan("--files", file).json.full, true, file);
  }
});

test("explicit full always selects every check and job", () => {
  const result = plan("--full").json;
  assert.equal(result.full, true);
  assert.deepEqual(result.checks, ["full"]);
  assert.deepEqual(result.jobs, [
    "actionlint",
    "portable",
    "windows-runtime",
    "first-party-contracts",
  ]);
});

test("invalid, missing, zero, and shallow bases fail closed", () => {
  for (const args of [
    ["--mode", "ci", "--base", "not-a-commit", "--head", "HEAD"],
    ["--mode", "ci", "--base", "0000000000000000000000000000000000000000", "--head", "HEAD"],
    ["--mode", "ci", "--base", "HEAD", "--head", "missing-head"],
  ]) {
    assert.equal(plan(...args).json.full, true);
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

test("hook and CI entrypoints keep the planner and stable quality gate", () => {
  const root = fileURLToPath(new URL("../", import.meta.url));
  const hook = readFileSync(`${root}lefthook.yml`, "utf8");
  const workflow = readFileSync(`${root}.github/workflows/ci.yml`, "utf8");
  assert.match(hook, /check:plan --mode pre-commit --run/);
  assert.match(hook, /check:plan --mode pre-push --run/);
  assert.match(hook, /use_stdin: true/);
  assert.match(workflow, /id: plan/);
  assert.match(workflow, /cortex-quality-gate:/);
  assert.match(workflow, /github\.event\.pull_request\.number \|\| github\.ref/);
  assert.match(workflow, /branches:\n\s+- main/);
});
