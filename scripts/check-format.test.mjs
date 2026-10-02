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

    writeFileSync(resolve(cwd, "sample.ts"), "export const value = 1;\n");

    writeFileSync(
      resolve(cwd, "bom.rs"),
      Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("fn main() {}\n")]),
    );
    git(cwd, "add", "bom.rs");
    const bommed = spawnSync(process.execPath, [script], { cwd });
    assert.equal(bommed.status, 1, "BOM in a changed .rs file must fail the gate");
    assert.match(String(bommed.stderr), /UTF-8 BOM/);

    writeFileSync(resolve(cwd, "bom.rs"), "fn main() {}\n");
    const clean = spawnSync(process.execPath, [script], { cwd });
    assert.equal(clean.status, 0, "plain .rs file must pass the BOM guard");

    writeFileSync(resolve(cwd, "long.rs"), `fn x() {\n    let s = "${"a".repeat(110)}";\n}\n`);
    git(cwd, "add", "long.rs");
    const long = spawnSync(process.execPath, [script], { cwd });
    assert.equal(long.status, 1, "over-100-char .rs line must fail the gate");
    assert.match(String(long.stderr), /over 100 chars/);
    rmSync(resolve(cwd, "long.rs"));

    writeFileSync(
      resolve(cwd, "flat.rs"),
      'fn x() {\n    let s = "INSERT INTO t(a,b) VALUES(\\\nVALUES(1,2)";\n}\n',
    );
    git(cwd, "add", "flat.rs");
    const flat = spawnSync(process.execPath, [script], { cwd });
    assert.equal(flat.status, 1, "column-0 string continuation must fail the gate");
    assert.match(String(flat.stderr), /column 0/);

    // a `\`-line in the middle of a multi-line literal carries no `"`, so a
    // line-local check misses it — the lexer must still catch it
    writeFileSync(
      resolve(cwd, "flat.rs"),
      'fn x() {\n    let s = "CREATE TABLE t (a TEXT,\n         b TEXT \\\nc TEXT)";\n}\n',
    );
    const middle = spawnSync(process.execPath, [script], { cwd });
    assert.equal(middle.status, 1, "quote-less continuation line must fail the gate");
    assert.match(String(middle.stderr), /column 0/);

    writeFileSync(
      resolve(cwd, "flat.rs"),
      'fn x() {\n    let s = "INSERT INTO t(a,b) \\\n         VALUES(1,2)";\n}\n',
    );
    const indented = spawnSync(process.execPath, [script], { cwd });
    assert.equal(indented.status, 0, "indented string continuation must pass");

    // raw strings, char literals and `"` inside comments must not confuse the
    // lexer: none of these opens a string continuation
    writeFileSync(
      resolve(cwd, "flat.rs"),
      [
        "fn x() {",
        '    let r = r#"raw content',
        'still inside the raw string at column zero"#;',
        "    let c1: char = '\"';",
        "    let c2: char = '\\'';",
        '    // a comment ending in \\ and a "quote" inside it',
        "next line at column zero inside nothing",
        '    let s = "one \\\n        two";',
        "}",
      ].join("\n") + "\n",
    );
    const lexerSafe = spawnSync(process.execPath, [script], { cwd });
    assert.equal(lexerSafe.status, 0, "raw strings, char literals and comments must pass");

    rmSync(resolve(cwd, "sample.ts"));
    rmSync(resolve(cwd, "bom.rs"));
    rmSync(resolve(cwd, "flat.rs"));
    const deleted = spawnSync(process.execPath, [script], { cwd });
    assert.equal(deleted.status, 0, "deleted files must not enter the format gate");
    const after = spawnSync("git", ["rev-parse", "HEAD"], { encoding: "utf8" });
    assert.equal(after.status, 0, after.stderr);
    assert.equal(after.stdout.trim(), outerHead, "outer repository HEAD must be unchanged");
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});
