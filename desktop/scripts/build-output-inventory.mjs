#!/usr/bin/env node
import {
  appendFileSync,
  existsSync,
  lstatSync,
  opendirSync,
  realpathSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { env } from "./brand.mjs";

const candidates = new Set(["target", "release", ".tmp", "dist", "dist-electron"]);
const ignored = new Set([".git", "node_modules"]);
const roots = process.argv.slice(2).map((root) => path.resolve(root));
const checkpoint = path.resolve(env("INVENTORY_CHECKPOINT") ?? "build-output-inventory.jsonl");
const visited = new Set();
const found = [];

function canonical(directory) {
  try {
    return realpathSync.native(directory).toLowerCase();
  } catch {
    return path.resolve(directory).toLowerCase();
  }
}

function scanBytes(directory) {
  let bytes = 0;
  const stack = [directory];
  while (stack.length) {
    const current = stack.pop();
    let entries;
    try {
      entries = opendirSync(current);
    } catch {
      continue;
    }
    let entry;
    while ((entry = entries.readSync())) {
      const full = path.join(current, entry.name);
      if (entry.name === ".git" || entry.name === "node_modules") continue;
      try {
        const stat = lstatSync(full);
        if (stat.isSymbolicLink()) continue;
        if (stat.isDirectory()) stack.push(full);
        else if (stat.isFile()) bytes += stat.size;
      } catch {}
    }
    entries.closeSync();
  }
  return bytes;
}

function walk(directory) {
  const key = canonical(directory);
  if (!visited.add(key)) return;
  const name = path.basename(directory);
  if (candidates.has(name)) {
    const result = { path: directory, bytes: scanBytes(directory) };
    found.push(result);
    appendFileSync(checkpoint, JSON.stringify(result) + "\n");
    return;
  }
  if (ignored.has(name)) return;
  let entries;
  try {
    entries = opendirSync(directory);
  } catch {
    return;
  }
  let entry;
  while ((entry = entries.readSync())) {
    if (entry.name === ".git" || entry.name === "node_modules") continue;
    const full = path.join(directory, entry.name);
    try {
      if (lstatSync(full).isDirectory()) walk(full);
    } catch {}
  }
  entries.closeSync();
}

if (!roots.length) throw new Error("usage: node build-output-inventory.mjs <root>...");
writeFileSync(
  checkpoint,
  JSON.stringify({ schema_version: 1, roots, started_at: new Date().toISOString() }) + "\n",
);
for (const root of roots) if (existsSync(root)) walk(root);
const total = found.reduce((sum, item) => sum + item.bytes, 0);
appendFileSync(
  checkpoint,
  JSON.stringify({
    complete: true,
    visited: visited.size,
    candidates: found.length,
    bytes: total,
  }) + "\n",
);
console.log(
  JSON.stringify({
    roots,
    visited: visited.size,
    candidates: found.length,
    bytes: total,
    checkpoint,
  }),
);
