import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const script = resolve(import.meta.dirname, "check-format.mjs");

function git(cwd, ...args) {
  const env = { ...process.env, GIT_CONFIG_NOSYSTEM: "1" };
  for (const key of [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_CEILING_DIRECTORIES",
    "GIT_PREFIX",
  ])
    delete env[key];
  const result = spawnSync("git", args, {
    cwd,
    encoding: "utf8",
    env,
  });
  assert.equal(result.status, 0, result.stderr);
}

test("format discovery ignores unstaged files and fails closed on an invalid base", () => {
  const cwd = mkdtempSync(resolve(tmpdir(), "cortex-format-"));
  const outer = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" });
  assert.equal(outer.status, 0, outer.stderr);
  const outerHead = outer.stdout.trim();
  try {
    git(cwd, "init", "--quiet");
    writeFileSync(resolve(cwd, "sample.ts"), "export const value = 1;\n");
    git(cwd, "add", "sample.ts");
    git(
      cwd,
      "-c",
      "user.name=Test",
      "-c",
      "user.email=test@example.invalid",
      "-c",
      "commit.gpgSign=false",
      "commit",
      "--quiet",
      "-m",
      "fixture",
    );

    writeFileSync(resolve(cwd, "sample.ts"), "export const value = 2;\n");
    const inheritedGitEnv = {
      ...process.env,
      GIT_DIR: resolve(import.meta.dirname, "../.git"),
      GIT_WORK_TREE: resolve(import.meta.dirname, ".."),
    };
    const staged = spawnSync(process.execPath, [script, "--staged"], {
      cwd,
      env: inheritedGitEnv,
    });
    assert.equal(staged.status, 0, "unstaged files must not enter the staged format gate");

    const invalidBase = spawnSync(process.execPath, [script], {
      cwd,
      env: { ...process.env, FORMAT_BASE: "not-a-commit" },
    });
    assert.notEqual(invalidBase.status, 0, "git discovery errors must fail the format gate");

    rmSync(resolve(cwd, "sample.ts"));
    const deleted = spawnSync(process.execPath, [script], { cwd });
    assert.equal(deleted.status, 0, "deleted files must not enter the format gate");
    const after = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" });
    assert.equal(after.status, 0, after.stderr);
    assert.equal(after.stdout.trim(), outerHead, "outer repository HEAD must be unchanged");
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});
