#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { collectRustFiles, inlineTestRanges, maskRustTrivia } from "./rust-source-scan.mjs";
import { collectStorageViolations } from "./check-ark-core-storage-boundary.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const CORE = path.join(ROOT, "core", "crates", "ark-core");
const IMPL =
  /\bimpl\s*(?:<[^>]*>\s*)?(DbCommand|DbQuery|DbMaintenanceCommand)\s+for\s+([A-Za-z_]\w*)/g;
const EXECUTION_BOUNDARY = {
  DbCommand: "immediate write transaction",
  DbQuery: "deferred read transaction",
  DbMaintenanceCommand: "exclusive maintenance connection",
};

function isTestFile(relativePath) {
  return /(?:^|\/)tests?(?:_[^/]+)?(?:\.rs|\/)|(?:^|\/)[^/]+_tests?\.rs$/.test(relativePath);
}

export async function collectCommandInventory(sourceRoot = path.join(CORE, "src"), root = ROOT) {
  const commands = [];
  for (const file of (await collectRustFiles(sourceRoot)).sort()) {
    const source = await readFile(file, "utf8");
    const masked = maskRustTrivia(source);
    const testRanges = inlineTestRanges(source, masked);
    const relativePath = path.relative(root, file).split(path.sep).join("/");
    if (isTestFile(relativePath)) continue;
    for (const match of masked.matchAll(IMPL)) {
      if (testRanges.some(({ start, end }) => start <= match.index && match.index < end)) continue;
      const line = source.slice(0, match.index).split("\n").length;
      commands.push({
        kind: match[1],
        name: match[2],
        boundary: EXECUTION_BOUNDARY[match[1]],
        path: relativePath,
        line,
      });
    }
  }
  return commands.sort(
    (left, right) =>
      left.kind.localeCompare(right.kind) ||
      left.path.localeCompare(right.path) ||
      left.line - right.line,
  );
}

export async function renderInventory() {
  const commands = await collectCommandInventory();
  const violations = await collectStorageViolations();
  const byScopeAndCategory = new Map();
  for (const { scope, category } of violations) {
    const key = `${scope}\0${category}`;
    byScopeAndCategory.set(key, (byScopeAndCategory.get(key) ?? 0) + 1);
  }
  const output = [
    "# ARK Core storage census",
    "",
    `Generated from the current Core source. The scanner found ${commands.length} typed operations and ${violations.length} SQLite ownership references. It scans all Rust sources and labels standalone tests, inline tests, debug-only code, and production separately. It does not infer policy ownership or discover indirect SQL hidden behind helper calls.`,
    "",
    "## Transaction contracts",
    "",
    "| Command contract | Count | Execution boundary |",
    "| --- | ---: | --- |",
  ];
  for (const kind of Object.keys(EXECUTION_BOUNDARY)) {
    const count = commands.filter((command) => command.kind === kind).length;
    output.push(`| \`${kind}\` | ${count} | ${EXECUTION_BOUNDARY[kind]} |`);
  }
  output.push(
    "",
    "## Typed operation inventory",
    "",
    "| Kind | Operation | Source |",
    "| --- | --- | --- |",
  );
  for (const command of commands)
    output.push(
      `| \`${command.kind}\` | \`${command.name}\` | [${command.path}:${command.line}](../${command.path}:${command.line}) |`,
    );
  output.push(
    "",
    "## SQLite references by scope",
    "",
    "| Scope | Category | Matches |",
    "| --- | --- | ---: |",
  );
  for (const [key, count] of [...byScopeAndCategory].sort(([left], [right]) =>
    left.localeCompare(right),
  )) {
    const [scope, category] = key.split("\0");
    output.push(`| ${scope} | ${category} | ${count} |`);
  }
  return `${output.join("\n")}\n`;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  renderInventory().then(
    (inventory) => process.stdout.write(inventory),
    (error) => {
      process.stderr.write(`ARK Core storage census failed: ${error.message}\n`);
      process.exitCode = 1;
    },
  );
}
