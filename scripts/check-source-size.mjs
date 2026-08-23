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
const SOFT_LIMIT = 300;
const HARD_LIMIT = 500;
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

const warnings = [];
const violations = [];
for (const file of await collect(ROOT)) {
  const lines = lineCount(await readFile(file, "utf8"));
  const label = path.relative(ROOT, file).replaceAll("\\", "/");
  if (lines > HARD_LIMIT) {
    violations.push(`${label}: ${lines} lines (hard max ${HARD_LIMIT})`);
  } else if (lines > SOFT_LIMIT) {
    warnings.push(`${label}: ${lines} lines (target ${SOFT_LIMIT})`);
  }
}

if (warnings.length) {
  console.warn(`source size warning (${warnings.length} file(s))`);
  console.warn(warnings.sort().join("\n"));
}

if (violations.length) {
  console.error(`source size check failed (${violations.length} file(s))`);
  console.error(violations.sort().join("\n"));
  process.exit(1);
}

console.log("source size check passed");
