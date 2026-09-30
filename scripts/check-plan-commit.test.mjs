import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { createPlan, executePlan } from "./check-plan.mjs";
import { commitPlan, PUSH_ONLY_CHECKS } from "./check-plan-commit.mjs";
import { coveredByCache, recordPass } from "./check-plan-cache.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const names = (plan) => {
  const seen = [];
  executePlan(plan, (command) => (seen.push(command.name), 0));
  return seen;
};

test("a commit defers the Rust compile and test checks to pre-push", () => {
  const plan = commitPlan(createPlan({ mode: "pre-commit", files: ["runtime/src/lib.rs"] }));
  assert.deepEqual(plan.checks, ["rustfmt"]);
  assert.match(plan.reasons.at(-1), /deferred to pre-push: clippy, test:rust, runtime-staging/);
  assert.deepEqual(names(plan), ["brand", "source-size", "rustfmt"]);
  const manager = commitPlan(createPlan({ mode: "pre-commit", files: ["manager-gpui/src/a.rs"] }));
  assert.deepEqual(manager.checks, []);
});

test("pre-push keeps every check the planner selected", () => {
  const plan = createPlan({ mode: "worktree", files: ["runtime/src/lib.rs"] });
  const push = { ...plan, mode: "pre-push" };
  assert.equal(commitPlan(push), push);
  for (const check of ["clippy", "test:rust", "runtime-staging"])
    assert.ok(PUSH_ONLY_CHECKS.includes(check) && push.checks.includes(check), check);
});

test("a full plan at commit time runs the fast gate, never the full check", () => {
  const full = createPlan({ mode: "pre-commit", files: ["Cargo.lock"] });
  assert.equal(full.full, true);
  const plan = commitPlan(full);
  assert.equal(plan.full, false);
  assert.deepEqual(plan.checks, ["fast"]);
  assert.deepEqual(names(plan), ["fast"]);
});

test("a passed fast gate does not satisfy the full push gate", (t) => {
  const repo = mkdtempSync(path.join(os.tmpdir(), "check-plan-commit-"));
  t.after(() => rmSync(repo, { recursive: true, force: true }));
  assert.equal(spawnSync("git", ["init", "-q", repo]).status, 0);
  recordPass(repo, "tree", ["fast", "rustfmt"]);
  assert.equal(coveredByCache(repo, "tree", ["full"]), false);
  assert.equal(coveredByCache(repo, "tree", ["rustfmt", "clippy"]), false);
  assert.equal(coveredByCache(repo, "tree", ["rustfmt"]), true);
});

test("the pre-commit hook applies the commit plan unless --full is given", () => {
  const run = (args) =>
    JSON.parse(
      spawnSync(process.execPath, ["scripts/check-plan.mjs", ...args], {
        cwd: root,
        encoding: "utf8",
      }).stdout,
    );
  const files = ["--files", "runtime/src/lib.rs"];
  assert.deepEqual(run(["--mode", "pre-commit", ...files]).checks, ["rustfmt"]);
  assert.deepEqual(run(["--mode", "pre-commit", "--full"]).checks, ["full"]);
});
