import assert from "node:assert/strict";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";

import { collectStorageViolations } from "./check-ark-core-storage-boundary.mjs";

test("the default scan covers integration tests and examples across the whole Core crate", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "ark-core-storage-targets-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const crateRoot = path.join(root, "core", "crates", "ark-core");
  const sourceRoot = path.join(crateRoot, "src");
  const tests = path.join(crateRoot, "tests", "it", "direct_sql.rs");
  const example = path.join(crateRoot, "examples", "direct_sql.rs");
  const manifestPath = path.join(crateRoot, "Cargo.toml");
  await mkdir(sourceRoot, { recursive: true });
  await mkdir(path.dirname(tests), { recursive: true });
  await mkdir(path.dirname(example), { recursive: true });
  await writeFile(manifestPath, '[dependencies]\nserde = "1"\n');
  await writeFile(path.join(sourceRoot, "lib.rs"), "pub fn ready() {}\n");
  await writeFile(
    tests,
    'fn fixture(conn: &rusqlite::Connection) { conn.execute("SELECT 1", []).unwrap(); }\n',
  );
  await writeFile(example, 'fn main() { let _ = rusqlite::Connection::open("ark.db"); }\n');

  const violations = await collectStorageViolations({ sourceRoot: crateRoot, manifestPath, root });
  assert.ok(
    violations.some(
      ({ path: file, scope }) => file.endsWith("tests/it/direct_sql.rs") && scope === "test_file",
    ),
  );
  assert.ok(
    violations.some(
      ({ path: file, scope }) => file.endsWith("examples/direct_sql.rs") && scope === "production",
    ),
  );
});
