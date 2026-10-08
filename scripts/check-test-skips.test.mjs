import assert from "node:assert/strict";
import test from "node:test";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { rustFiles, scanSource } from "./check-test-skips.mjs";

test("does not scan nested repositories or worktrees", () => {
  const root = mkdtempSync(join(tmpdir(), "test-skips-"));
  try {
    writeFileSync(join(root, "owned.rs"), "");
    for (const name of ["repository", "worktree"]) {
      const child = join(root, name);
      mkdirSync(child);
      if (name === "repository") mkdirSync(join(child, ".git"));
      else writeFileSync(join(child, ".git"), "gitdir: elsewhere");
      writeFileSync(join(child, "foreign.rs"), "#[ignore]");
    }
    assert.deepEqual(rustFiles(root), [join(root, "owned.rs")]);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("flags a test that returns early when link setup fails", () => {
  const violations = scanSource(
    "fixture.rs",
    `#[test]
fn linked_root() {
    if !link_dir(outside.path(), &linked) {
        return;
    }
}`,
  );
  assert.equal(violations.length, 1);
  assert.match(violations[0], /link\/symlink setup/);
});

test("flags a printed NOT_RUN/skip notice followed by return", () => {
  const violations = scanSource(
    "fixture.rs",
    `#[tokio::test]
async fn contract() {
    let Some(worker) = std::env::var_os("WORKER_EXE").map(PathBuf::from) else {
        eprintln!("NOT_RUN: set WORKER_EXE");
        return;
    };
}`,
  );
  assert.equal(violations.length, 1);
});

test("flags a bare #[ignore] but accepts a reasoned one", () => {
  assert.equal(scanSource("a.rs", "#[ignore]\nfn t() {}").length, 1);
  assert.equal(scanSource("b.rs", '#[ignore = "needs internet"]\nfn t() {}').length, 0);
});

test("accepts junction setup that expects instead of skipping", () => {
  const violations = scanSource(
    "fixture.rs",
    `#[test]
fn linked_root() {
    crate::test_links::link_dir(outside.path(), &linked).expect("junction");
    assert!(roots.open(&linked).is_err());
}
#[cfg_attr(unix, ignore = "unix only")]
fn other() {}
`,
  );
  assert.deepEqual(violations, []);
});
