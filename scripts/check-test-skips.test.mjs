import assert from "node:assert/strict";
import test from "node:test";
import { scanSource } from "./check-test-skips.mjs";

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
