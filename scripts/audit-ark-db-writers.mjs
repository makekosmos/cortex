#!/usr/bin/env node

import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const sourceRoot = path.join(repoRoot, "core", "crates", "ark-core", "src");
const patterns = [
  ["service transaction boundary", /\bwith_write_tx\s*\(|\bwith_conn_mut\s*\(/],
  [
    "shared connection escape",
    /\bget_shared_conn\s*\(|\.conn\.lock\s*\(|Arc\s*<\s*Mutex\s*<\s*rusqlite::Connection/,
  ],
  [
    "SQLite mutation API",
    /\.(?:execute|execute_batch|execute_cached|unchecked_transaction|transaction_with_behavior)\s*\(/,
  ],
  ["sync blocking adapter", /\bspawn_blocking\s*\(/],
  ["connection lifecycle", /\b(?:open_db|init_schema|restore|compact|backup)\s*\(/],
];

async function collectRustFiles(directory, result = []) {
  const entries = await readdir(directory, { withFileTypes: true });
  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      await collectRustFiles(entryPath, result);
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      result.push(entryPath);
    }
  }
  return result;
}

const files = (await collectRustFiles(sourceRoot)).sort();
const findings = patterns.map(([label]) => ({ label, entries: [] }));
for (const file of files) {
  const source = await readFile(file, "utf8");
  const relative = path.relative(repoRoot, file);
  for (const [lineIndex, line] of source.split(/\r?\n/).entries()) {
    for (const [patternIndex, [, matcher]] of patterns.entries()) {
      if (matcher.test(line)) {
        findings[patternIndex].entries.push({
          path: relative,
          line: lineIndex + 1,
          test: /(?:^|\/)(?:tests?(?:_[^/]+)?)(?:\/|$)|(?:^|\/)[^/]+_tests?\.rs$/.test(relative),
          source: line.trim(),
        });
      }
    }
  }
}

const nonTestPathMatches = findings.flatMap(({ entries }) =>
  entries.filter(({ test }) => !test),
).length;
process.stdout.write(`# ARK SQLite writer census\n\n`);
process.stdout.write(
  `Generated from ${path.relative(repoRoot, sourceRoot)}; ${nonTestPathMatches} matches outside test-named files, ${files.length} Rust files scanned. Test-path matches are marked [test]; inline cfg(test) blocks are not distinguished. This is a source inventory, not proof of runtime reachability or writer ownership.\n`,
);
for (const { label, entries } of findings) {
  process.stdout.write(`\n## ${label} (${entries.length})\n`);
  for (const entry of entries) {
    process.stdout.write(
      `- ${entry.path}:${entry.line}${entry.test ? " [test]" : ""}: \`${entry.source.replaceAll("`", "\\`")}\`\n`,
    );
  }
}
