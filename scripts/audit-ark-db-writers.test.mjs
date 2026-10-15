import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { collectWriterFindings, compareAllowlist } from "./audit-ark-db-writers.mjs";
import { inlineTestRanges, maskRustTrivia } from "./rust-source-scan.mjs";

test("Rust trivia masking ignores nested comments and string and character contents", () => {
  const source = String.raw`let raw = r###"#[cfg(test)] mod fake { conn.execute(); }"###;
let escaped = "brace: } and quote: \"";
let byte = b"{ bytes }";
let character = '}';
let unicode = '\u{7d}';
// #[cfg(test)] mod comment { conn.execute(); }
/* outer /* nested } */ still comment */
`;
  const masked = maskRustTrivia(source);
  assert.equal(masked.length, source.length);
  assert.equal(masked.split("\n").length, source.split("\n").length);
  assert.doesNotMatch(masked, /conn\.execute/);
  assert.doesNotMatch(masked, /mod fake|mod comment/);
});

test("only the exact inline cfg(test) module body is classified as test code", () => {
  const source = `// #[cfg(test)] mod comment { conn.execute(); }
fn production() { conn.execute("write", []); }
#[cfg(test)]
pub(crate) mod tests {
  const RUST: &str = r#"} #[cfg(test)] mod decoy { } "#;
  /* nested comment { /* } */ still comment */
  fn nested() { conn.execute("test", []); }
}
`;
  const ranges = inlineTestRanges(source);
  const testWrite = source.indexOf('conn.execute("test"');
  const productionWrite = source.indexOf('conn.execute("write"');
  assert.equal(ranges.length, 1);
  assert.ok(ranges[0].start <= testWrite && testWrite < ranges[0].end);
  assert.ok(!(ranges[0].start <= productionWrite && productionWrite < ranges[0].end));
});

test("writer census excludes inline and path-based tests but retains production calls", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "ark-writer-census-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const sourceRoot = path.join(root, "core", "crates", "ark-core", "src");
  const productionPath = path.join(sourceRoot, "service", "writer.rs");
  const testPath = path.join(sourceRoot, "db", "tests", "writer.rs");
  await mkdir(path.dirname(productionPath), { recursive: true });
  await mkdir(path.dirname(testPath), { recursive: true });
  await writeFile(
    productionPath,
    [
      'fn write(conn: &Connection) { conn.execute("INSERT", []); }',
      '// conn.execute("comment", []);',
      '#[cfg(test)] mod tests { fn test(conn: &Connection) { conn.execute("test", []); } }',
    ].join("\n"),
  );
  await writeFile(testPath, 'fn setup(conn: &Connection) { conn.execute("fixture", []); }\n');

  const findings = await collectWriterFindings(sourceRoot, root);
  assert.deepEqual(
    findings.map(({ path: file, line, scope }) => ({ file, line, scope })),
    [
      { file: "core/crates/ark-core/src/db/tests/writer.rs", line: 1, scope: "test_file" },
      { file: "core/crates/ark-core/src/service/writer.rs", line: 1, scope: "production" },
      { file: "core/crates/ark-core/src/service/writer.rs", line: 3, scope: "inline_test" },
    ],
  );
});

test("allowlist detects both new writer findings and stale counts", () => {
  const finding = {
    path: "core/crates/ark-core/src/service/writer.rs",
    category: "shared connection escape",
    scope: "production",
  };
  const current = [finding];
  assert.deepEqual(compareAllowlist(current, [{ ...finding, count: 1 }]), []);
  assert.match(compareAllowlist(current, [])[0], /no allowlist entry/);
  assert.match(compareAllowlist(current, [{ ...finding, count: 2 }])[0], /allowlist 2, found 1/);
  assert.match(
    compareAllowlist([...current, finding], [{ ...finding, count: 1 }])[0],
    /allowlist 1, found 2/,
  );
});

test("an unallowlisted production get_shared_conn fails the census", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "ark-writer-census-new-escape-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const sourceRoot = path.join(root, "core", "crates", "ark-core", "src");
  const writer = path.join(sourceRoot, "service", "new_writer.rs");
  await mkdir(path.dirname(writer), { recursive: true });
  await writeFile(writer, "fn write() { get_shared_conn(&state); }\n");

  const findings = await collectWriterFindings(sourceRoot, root);
  const differences = compareAllowlist(findings, []);
  assert.equal(
    differences[0],
    "core/crates/ark-core/src/service/new_writer.rs [shared connection escape]: found 1, no allowlist entry",
  );
});
