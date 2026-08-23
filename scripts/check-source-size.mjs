#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const rootIndex = process.argv.indexOf("--root");
const ROOT = rootIndex === -1
  ? path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
  : path.resolve(process.argv[rootIndex + 1]);
const SOURCE_LIMITS = new Map([
  [".js", 1200],
  [".jsx", 1200],
  [".ts", 1200],
  [".tsx", 1200],
  [".vue", 1200],
  [".rs", 2000],
]);
const IGNORED = new Set([
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
    } else if (entry.isFile() && SOURCE_LIMITS.has(path.extname(entry.name))) {
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
for (const file of await collect(ROOT)) {
  const lines = lineCount(await readFile(file, "utf8"));
  const limit = SOURCE_LIMITS.get(path.extname(file));
  if (lines > limit) {
    violations.push(`${path.relative(ROOT, file).replaceAll("\\", "/")}: ${lines} lines (max ${limit})`);
  }
}

if (violations.length) {
  console.error(`source size check failed (${violations.length} file(s))`);
  console.error(violations.sort().join("\n"));
  process.exit(1);
}

console.log("source size check passed");
