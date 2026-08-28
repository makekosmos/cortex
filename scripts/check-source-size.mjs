#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const rootIndex = process.argv.indexOf("--root");
const ROOT =
  rootIndex === -1
    ? path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
    : path.resolve(process.argv[rootIndex + 1]);
const SOURCE_LIMIT = 300;
// Existing debt is explicit and finite. New files must meet the limit; removing
// an entry is the only way to retire debt, so the check never quietly regresses.
const GRANDFATHERED = new Set([
  "crates/ark-core/src/ffi_start.rs",
  "crates/ark-core/src/main/definitions.rs",
  "crates/ark-core/src/main/runtime.rs",
  "crates/ark-core/src/main/sync.rs",
  "crates/ark-core/src/main/sync/start.rs",
  "crates/ark-core/src/main/tests.rs",
  "crates/ark-core/src/sync_client_start.rs",
  "crates/ark-core/src/sync_server_core.rs",
  "crates/ark-core/src/sync_server_messages.rs",
]);
const SOURCE_EXTENSIONS = new Set([
  ".js",
  ".jsx",
  ".mjs",
  ".cjs",
  ".ts",
  ".tsx",
  ".vue",
  ".rs",
  ".inc",
]);
const IGNORED = new Set([
  ".agent",
  ".agents",
  ".git",
  ".tmp",
  "build",
  "coverage",
  "dist",
  "dist-electron",
  "graphify-out",
  "node_modules",
  "release",
  "target",
]);

async function collect(dir, files = []) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    if (entry.isDirectory() && !IGNORED.has(entry.name)) {
      await collect(path.join(dir, entry.name), files);
    } else if (entry.isFile() && SOURCE_EXTENSIONS.has(path.extname(entry.name))) {
      files.push(path.join(dir, entry.name));
    }
  }
  return files;
}

function lineCount(text) {
  const lines = text.split(/\r?\n/);
  return lines.at(-1) === "" ? lines.length - 1 : lines.length;
}

const violations = [];
const debt = [];
for (const file of await collect(ROOT)) {
  const lines = lineCount(await readFile(file, "utf8"));
  const label = path.relative(ROOT, file).replaceAll("\\", "/");
  if (lines > SOURCE_LIMIT) {
    if (GRANDFATHERED.has(label)) debt.push(`${label}: ${lines} lines`);
    else violations.push(`${label}: ${lines} lines (max ${SOURCE_LIMIT})`);
  }
}

if (violations.length) {
  console.error(`source size check failed (${violations.length} file(s))`);
  console.error(violations.sort().join("\n"));
  process.exit(1);
}

console.log(
  `source size check passed (${debt.length} grandfathered file(s) over ${SOURCE_LIMIT} lines)`,
);
if (debt.length) console.log(`baseline debt:\n${debt.sort().join("\n")}`);
