import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { collectStorageViolations } from "./check-ark-core-storage-boundary.mjs";

async function fixture(t, { manifest, source, files = {} }) {
  const root = await mkdtemp(path.join(os.tmpdir(), "ark-core-storage-boundary-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const sourceRoot = path.join(root, "core", "crates", "ark-core", "src");
  const sourcePath = path.join(sourceRoot, "lib.rs");
  const manifestPath = path.join(root, "core", "crates", "ark-core", "Cargo.toml");
  await mkdir(sourceRoot, { recursive: true });
  await writeFile(manifestPath, manifest);
  await writeFile(sourcePath, source);
  for (const [relativePath, contents] of Object.entries(files)) {
    const file = path.join(sourceRoot, relativePath);
    await mkdir(path.dirname(file), { recursive: true });
    await writeFile(file, contents);
  }
  return { root, sourceRoot, manifestPath };
}

test("the boundary checker ignores comments but finds SQLite code and SQL literals", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "// rusqlite::Connection and SQL `SELECT 1` are examples only",
      "fn query() {",
      '  let statement = r#"SELECT id FROM objects"#;',
      '  let _ = rusqlite::Connection::open("ark.db");',
      "}",
    ].join("\n"),
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(
    violations.map(({ line, category }) => ({ line, category })),
    [
      { line: 3, category: "SQL literal" },
      { line: 4, category: "SQLite lifecycle" },
      { line: 4, category: "SQLite type or import" },
    ],
  );
});

test("the boundary checker rejects rusqlite in dev dependencies", async (t) => {
  const paths = await fixture(t, {
    manifest: "[dev-dependencies]\nrusqlite.workspace = true\n",
    source: "pub fn ready() {}\n",
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(violations, [
    {
      path: "core/crates/ark-core/Cargo.toml",
      line: 2,
      category: "rusqlite dependency",
      scope: "production",
    },
  ]);
});

test("the baseline inventory distinguishes inline tests from production code", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "pub fn ready() {}",
      "#[cfg(test)] mod tests {",
      '  const QUERY: &str = "SELECT id FROM objects";',
      "}",
    ].join("\n"),
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(
    violations.map(({ category, line, scope }) => ({ category, line, scope })),
    [{ category: "SQL literal", line: 3, scope: "inline_test" }],
  );
});

test("the boundary checker scopes externally declared cfg(test) modules as tests", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: ["#[cfg(test)] mod fixtures;", "mod production;", "pub fn ready() {}"].join("\n"),
    files: {
      "fixtures.rs": "mod nested;\n",
      "fixtures/nested.rs": 'const QUERY: &str = "SELECT id FROM objects";\n',
      "production.rs": 'const QUERY: &str = "SELECT id FROM objects";\n',
    },
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(
    violations.map(({ path: file, line, category, scope }) => ({
      path: file.split("/").at(-1),
      line,
      category,
      scope,
    })),
    [
      { path: "nested.rs", line: 1, category: "SQL literal", scope: "test_file" },
      { path: "production.rs", line: 1, category: "SQL literal", scope: "production" },
    ],
  );
});

test("the boundary checker scopes cfg(test) imports and functions as tests", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "#[cfg(test)] use rusqlite::Connection;",
      "#[cfg(test)] fn fixture(connection: &Connection) {",
      '  connection.execute_batch("CREATE TABLE probe(value TEXT)").unwrap();',
      "}",
      'pub fn production(connection: &rusqlite::Connection) { connection.execute("SELECT 1", []).unwrap(); }',
    ].join("\n"),
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(
    violations.map(({ line, category, scope }) => ({ line, category, scope })),
    [
      { line: 1, category: "SQLite type or import", scope: "inline_test" },
      { line: 3, category: "SQL literal", scope: "inline_test" },
      { line: 3, category: "SQLite operation", scope: "inline_test" },
      { line: 5, category: "SQL literal", scope: "production" },
      { line: 5, category: "SQLite operation", scope: "production" },
      { line: 5, category: "SQLite type or import", scope: "production" },
    ],
  );
});

test("a clean Core source tree passes the ownership gate", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nark-store = { path = "../ark-store" }\n',
    source: "pub fn network_connection(value: Connection) { value.close(); executor.execute(); }\n",
  });
  assert.deepEqual(await collectStorageViolations(paths), []);
});
