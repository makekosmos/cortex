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

test("the boundary checker rejects connection callbacks that leak SQLite into Core", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "#[cfg(test)] fn inspect(handle: &DbHandle) {",
      '  handle.with_connection(|connection| connection.query_row("SELECT 1", [], |row| row.get::<_, i64>(0)));',
      "}",
    ].join("\n"),
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(
    violations.map(({ line, category, scope }) => ({ line, category, scope })),
    [
      { line: 2, category: "SQL literal", scope: "inline_test" },
      { line: 2, category: "SQLite connection escape hatch", scope: "inline_test" },
      { line: 2, category: "SQLite operation", scope: "inline_test" },
    ],
  );
});

test("the boundary checker rejects rusqlite in every Cargo dependency scope", async (t) => {
  const paths = await fixture(t, {
    manifest: "[dependencies]\nrusqlite = { workspace = true }\n",
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

test("the boundary checker rejects optional and dev-only rusqlite dependencies", async (t) => {
  const optional = await fixture(t, {
    manifest: "[dependencies]\nrusqlite = { workspace = true, optional = true }\n",
    source: "pub fn ready() {}\n",
  });
  const devOnly = await fixture(t, {
    manifest: "[dev-dependencies]\nrusqlite.workspace = true\n",
    source: "pub fn ready() {}\n",
  });
  for (const paths of [optional, devOnly]) {
    assert.deepEqual(
      (await collectStorageViolations(paths)).map(({ category, scope }) => ({ category, scope })),
      [{ category: "rusqlite dependency", scope: "production" }],
    );
  }
});

test("the baseline inventory distinguishes inline tests from production code", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "pub fn ready() {}",
      "#[cfg(test)] mod tests {",
      '  const QUERY: &str = "SELECT id FROM objects";',
      '  const DIAGNOSTIC: &str = "sync profile savepoint release failed";',
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

test("the boundary checker follows paths on externally declared cfg(test) modules", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "#[cfg(test)]",
      '#[path = "tests/transport.rs"]',
      "mod transport_tests;",
      "pub fn ready() {}",
    ].join("\n"),
    files: {
      "tests/transport.rs": 'include!("query.rs");\n',
      "tests/query.rs": 'const QUERY: &str = "SELECT id FROM objects";\n',
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
    [{ path: "query.rs", line: 1, category: "SQL literal", scope: "test_file" }],
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

test("the boundary checker scopes debug-only imports and async functions", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "#[cfg(debug_assertions)] use rusqlite::Connection;",
      "#[cfg(debug_assertions)] pub(crate) async fn fixture(connection: &Connection) {",
      '  connection.execute("SELECT 1", []).unwrap();',
      "}",
      "pub fn production(writer: &mut StoreWriter) { writer.restore_snapshot(); }",
    ].join("\n"),
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(
    violations.map(({ line, category, scope }) => ({ line, category, scope })),
    [
      { line: 1, category: "SQLite type or import", scope: "debug_only" },
      { line: 3, category: "SQL literal", scope: "debug_only" },
      { line: 3, category: "SQLite operation", scope: "debug_only" },
    ],
  );
});

test("the boundary checker scopes opt-in test-support SQLite code", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      '#[cfg(any(test, feature = "test-support"))] use rusqlite::Connection;',
      '#[cfg(any(test, feature = "test-support"))] fn fixture(connection: &Connection) {',
      '  connection.execute("SELECT 1", []).unwrap();',
      "}",
      "pub fn production(writer: &mut StoreWriter) { writer.restore_snapshot(); }",
    ].join("\n"),
  });
  const violations = await collectStorageViolations(paths);
  assert.deepEqual(
    violations.map(({ line, category, scope }) => ({ line, category, scope })),
    [
      { line: 1, category: "SQLite type or import", scope: "debug_only" },
      { line: 3, category: "SQL literal", scope: "debug_only" },
      { line: 3, category: "SQLite operation", scope: "debug_only" },
    ],
  );
});

test("the boundary checker scopes cfg(test) traits and impl blocks as tests", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nserde = "1"\n',
    source: [
      "#[cfg(test)] trait Probe {",
      "  fn read(connection: &rusqlite::Connection);",
      "}",
      "#[cfg(test)] impl Probe for Fixture {",
      '  fn read(connection: &rusqlite::Connection) { connection.execute("SELECT 1", []).unwrap(); }',
      "}",
    ].join("\n"),
  });
  const violations = await collectStorageViolations(paths);
  assert.ok(violations.length > 0);
  assert.ok(violations.every(({ scope }) => scope === "inline_test"));
});

test("a clean Core source tree passes the ownership gate", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nark-store = { path = "../ark-store" }\n',
    source: "pub fn network_connection(value: Connection) { value.close(); executor.execute(); }\n",
  });
  assert.deepEqual(await collectStorageViolations(paths), []);
});

test("the checked-out Core tree has no production SQLite code references", async () => {
  const production = (await collectStorageViolations()).filter(
    ({ scope }) => scope === "production",
  );
  assert.deepEqual(
    production.map(({ path: file, category }) => ({ path: file, category })),
    [{ path: "core/crates/ark-core/Cargo.toml", category: "rusqlite dependency" }],
    "Core production code and dependencies must not depend on SQLite",
  );
});
