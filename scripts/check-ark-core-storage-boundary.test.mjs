import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { collectStorageViolations } from "./check-ark-core-storage-boundary.mjs";

async function fixture(t, { manifest, source }) {
  const root = await mkdtemp(path.join(os.tmpdir(), "ark-core-storage-boundary-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const sourceRoot = path.join(root, "core", "crates", "ark-core", "src");
  const sourcePath = path.join(sourceRoot, "lib.rs");
  const manifestPath = path.join(root, "core", "crates", "ark-core", "Cargo.toml");
  await mkdir(sourceRoot, { recursive: true });
  await writeFile(manifestPath, manifest);
  await writeFile(sourcePath, source);
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
    },
  ]);
});

test("a clean Core source tree passes the ownership gate", async (t) => {
  const paths = await fixture(t, {
    manifest: '[dependencies]\nark-store = { path = "../ark-store" }\n',
    source: "pub fn storage_is_owned_by_store() { executor.execute(); }\n",
  });
  assert.deepEqual(await collectStorageViolations(paths), []);
});
