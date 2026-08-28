import assert from "node:assert/strict";
import { mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const script = resolve(import.meta.dirname, "check-format.mjs");

function git(cwd, ...args) {
  const result = spawnSync("git", args, { cwd, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
}

test("format discovery ignores unstaged files and fails closed on an invalid base", () => {
  const cwd = mkdtempSync(resolve(tmpdir(), "cortex-format-"));
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
    const staged = spawnSync(process.execPath, [script, "--staged"], { cwd });
    assert.equal(staged.status, 0, "unstaged files must not enter the staged format gate");

    const invalidBase = spawnSync(process.execPath, [script], {
      cwd,
      env: { ...process.env, FORMAT_BASE: "not-a-commit" },
    });
    assert.notEqual(invalidBase.status, 0, "git discovery errors must fail the format gate");

    rmSync(resolve(cwd, "sample.ts"));
    const deleted = spawnSync(process.execPath, [script], { cwd });
    assert.equal(deleted.status, 0, "deleted files must not enter the format gate");
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});
