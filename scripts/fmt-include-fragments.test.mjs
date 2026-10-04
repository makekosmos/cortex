import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const script = resolve(import.meta.dirname, "fmt-include-fragments.mjs");

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
  const result = spawnSync("git", args, { cwd, encoding: "utf8", env });
  assert.equal(result.status, 0, result.stderr);
}

function repo() {
  const cwd = mkdtempSync(resolve(tmpdir(), "cortex-fmtinc-"));
  git(cwd, "init", "--quiet");
  return cwd;
}

function run(cwd, ...args) {
  return spawnSync(process.execPath, [script, ...args], { cwd, encoding: "utf8" });
}

test("fmt-include-fragments reports and fixes unformatted fragments", () => {
  const cwd = repo();
  try {
    writeFileSync(resolve(cwd, "lib.rs"), 'mod outer {\n    include!("frag.rs");\n}\n');
    writeFileSync(
      resolve(cwd, "frag.rs"),
      "    pub fn messy() {\nlet  x  =  1;\nassert_eq!(x,1);\n}\n",
    );
    git(cwd, "add", "lib.rs", "frag.rs");

    const check = run(cwd, "--check");
    assert.equal(check.status, 1, "unformatted fragment must fail --check");
    assert.match(check.stdout, /NEEDS FMT.*frag\.rs/);

    const before = readFileSync(resolve(cwd, "frag.rs"), "utf8");
    assert.equal(run(cwd).status, 0, "write mode must succeed");
    const formatted = readFileSync(resolve(cwd, "frag.rs"), "utf8");
    assert.notEqual(formatted, before, "write mode must reformat the fragment");
    assert.match(formatted, /let x = 1;/);
    // fragment content stays at its splice indentation
    assert.match(formatted, /^ {4}pub fn messy\(\)/m);

    assert.equal(run(cwd, "--check").status, 0, "formatted fragment passes --check");
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

test("fmt-include-fragments leaves an already-formatted fragment byte-identical", () => {
  const cwd = repo();
  try {
    const content = "    pub fn tidy() {\n        let x = 1;\n        assert_eq!(x, 1);\n    }\n";
    writeFileSync(resolve(cwd, "lib.rs"), 'mod outer {\n    include!("frag.rs");\n}\n');
    writeFileSync(resolve(cwd, "frag.rs"), content);
    git(cwd, "add", "lib.rs", "frag.rs");

    assert.equal(run(cwd, "--check").status, 0);
    assert.equal(run(cwd).status, 0);
    assert.equal(
      readFileSync(resolve(cwd, "frag.rs"), "utf8"),
      content,
      "a formatted fragment must not be touched",
    );
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

test("fmt-include-fragments discovers relative include! targets and ignores vendor/", () => {
  const cwd = repo();
  try {
    writeFileSync(resolve(cwd, "lib.rs"), 'mod a {\n    include!("sub/part.rs");\n}\n');
    // vendor file carries an include! that must not be followed
    mkdirSync(resolve(cwd, "sub"), { recursive: true });
    mkdirSync(resolve(cwd, "vendor/x"), { recursive: true });
    writeFileSync(resolve(cwd, "sub/part.rs"), "    pub const OK: u8 = 1;\n");
    // vendor file carries an include! that must not be followed
    writeFileSync(
      resolve(cwd, "vendor/x/lib.rs"),
      'mod b {\n    include!("does-not-exist.rs");\n}\n',
    );
    git(cwd, "add", ".");

    const check = run(cwd, "--check");
    assert.equal(
      check.status,
      0,
      `vendor include! of a missing file must be ignored: ${check.stdout} ${check.stderr}`,
    );
    assert.match(check.stdout, /targets=1/);
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

test("fmt-include-fragments skips a tracked file deleted but not yet staged", () => {
  const cwd = repo();
  try {
    writeFileSync(resolve(cwd, "lib.rs"), 'mod a {\n    include!("frag.rs");\n}\n');
    writeFileSync(resolve(cwd, "frag.rs"), "    pub const OK: u8 = 1;\n");
    writeFileSync(resolve(cwd, "gone.rs"), "pub fn gone() {}\n");
    git(cwd, "add", ".");
    // deleted from the worktree but still in the index: `git ls-files` lists it
    rmSync(resolve(cwd, "gone.rs"));

    const check = run(cwd, "--check");
    assert.equal(check.status, 0, `${check.stdout} ${check.stderr}`);
    assert.doesNotMatch(`${check.stdout}${check.stderr}`, /ENOENT/);
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});

test("fmt-include-fragments fails clearly on a missing include target", () => {
  const cwd = repo();
  try {
    writeFileSync(resolve(cwd, "lib.rs"), 'mod a {\n    include!("gone.rs");\n}\n');
    git(cwd, "add", "lib.rs");

    const check = run(cwd, "--check");
    assert.equal(check.status, 1, "missing include target must fail the gate");
    assert.match(check.stderr, /MISSING TARGET.*gone\.rs/);
  } finally {
    rmSync(cwd, { recursive: true, force: true });
  }
});
