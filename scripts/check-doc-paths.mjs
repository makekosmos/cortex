#!/usr/bin/env node

// Documents are read by people who cannot tell a stale path from a live one.
// This scan keeps the file paths that docs and AGENTS.md cite honest: every
// repository path written in a code span or linked relatively must exist.

import { readdir, readFile, stat } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const rootIndex = process.argv.indexOf("--root");
const ROOT =
  rootIndex === -1
    ? path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..")
    : path.resolve(process.argv[rootIndex + 1]);

// Only paths with a directory component are checked: a bare file name is a
// reference to code, not a claim about where it lives.
const PATH_LIKE =
  /^[\w@.-]+(?:\/[\w@.-]+)+\.(?:md|rs|mjs|cjs|js|jsx|ts|tsx|vue|json|jsonl|toml|ya?ml|sh|ps1|swift)$/;
// Generated or ignored output trees are not part of the checkout.
const GENERATED = /^(?:\.(?:audit|tmp|dev-logs|dev-data)|release|target|node_modules)\//;
// docs/experiments/** records dated measurements; its paths describe the tree
// as it was on that date and are deliberately not kept current.
const FROZEN = [/^docs\/experiments\//];
const CODE_SPAN = /`([^`\n]+)`/g;
const LINK = /\[[^\]\n]*\]\(([^)\s]+)\)/g;

async function markdownFiles(root) {
  const files = [path.join(root, "AGENTS.md"), path.join(root, "README.md")];
  async function collect(directory) {
    let entries;
    try {
      entries = await readdir(directory, { withFileTypes: true });
    } catch {
      return;
    }
    for (const entry of entries) {
      const absolute = path.join(directory, entry.name);
      if (entry.isDirectory() && entry.name !== "node_modules" && entry.name !== "target") {
        await collect(absolute);
      } else if (entry.isFile() && entry.name.endsWith(".md")) {
        files.push(absolute);
      }
    }
  }
  await collect(path.join(root, "docs"));
  for (const directory of ["core", "runtime", "desktop", "manager"]) {
    async function collectAgents(dir, depth) {
      if (depth > 3) return;
      let entries;
      try {
        entries = await readdir(dir, { withFileTypes: true });
      } catch {
        return;
      }
      for (const entry of entries) {
        const absolute = path.join(dir, entry.name);
        if (entry.isDirectory() && entry.name !== "node_modules" && entry.name !== "target") {
          await collectAgents(absolute, depth + 1);
        } else if (entry.isFile() && entry.name === "AGENTS.md") {
          files.push(absolute);
        }
      }
    }
    await collectAgents(path.join(root, directory), 0);
  }
  return [...new Set(files)];
}

async function exists(candidate) {
  try {
    await stat(candidate);
    return true;
  } catch {
    return false;
  }
}

function looksLikeRepoPath(text) {
  if (!PATH_LIKE.test(text)) return false;
  if (/[<>*{}\s]/.test(text)) return false;
  if (text.startsWith("/") || text.startsWith("~")) return false;
  return true;
}

export async function missingDocPaths(root) {
  const missing = [];
  for (const file of await markdownFiles(root)) {
    let source;
    try {
      source = await readFile(file, "utf8");
    } catch {
      continue;
    }
    const relative = path.relative(root, file).replaceAll("\\", "/");
    if (FROZEN.some((pattern) => pattern.test(relative))) continue;
    const candidates = new Set();
    for (const match of source.matchAll(CODE_SPAN)) candidates.add(match[1].trim());
    for (const match of source.matchAll(LINK)) {
      const target = match[1];
      if (target.startsWith("http") || target.startsWith("#")) continue;
      candidates.add(target);
    }
    for (const candidate of candidates) {
      if (!looksLikeRepoPath(candidate)) continue;
      if (GENERATED.test(candidate)) continue;
      const fromRoot = path.join(root, candidate);
      const fromDocument = path.join(path.dirname(file), candidate);
      if ((await exists(fromRoot)) || (await exists(fromDocument))) continue;
      missing.push(`${relative}: ${candidate}`);
    }
  }
  return missing.sort();
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const missing = await missingDocPaths(ROOT);
  if (missing.length) {
    console.error(`doc path check failed (${missing.length} reference(s))`);
    console.error(missing.join("\n"));
    process.exit(1);
  }
  console.log("doc path check passed");
}
