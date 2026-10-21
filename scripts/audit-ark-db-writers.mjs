#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import {
  collectRustFiles,
  debugOnlyFunctionRanges,
  inlineTestRanges,
  maskRustTrivia,
} from "./rust-source-scan.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const arkSourceRoots = [
  path.join(repoRoot, "core", "crates", "ark-core", "src"),
  path.join(repoRoot, "core", "crates", "ark-store", "src"),
];
const allowlistPath = path.join(repoRoot, "scripts", "ark-db-writers.allow.json");
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
  [
    "connection lifecycle",
    /\b(?:open_db|init_schema|restore_snapshot|compact_db|compact_usage|backup_to_file|backup_to_file_chunked)\s*\(|\.(?:restore|backup)\s*\(/,
  ],
];

const isTestFile = (relative) =>
  /(?:^|\/)(?:tests?(?:_[^/]+)?|[^/]+_tests)(?:\.rs|\/)|(?:^|\/)[^/]+_tests?\.rs$/.test(relative);

function lineStarts(source) {
  const starts = [0];
  for (let index = 0; index < source.length; index++)
    if (source[index] === "\n") starts.push(index + 1);
  return starts;
}

export async function collectWriterFindings(sourceRoots = arkSourceRoots, root = repoRoot) {
  const roots = Array.isArray(sourceRoots) ? sourceRoots : [sourceRoots];
  const files = (await Promise.all(roots.map((sourceRoot) => collectRustFiles(sourceRoot))))
    .flat()
    .sort();
  const findings = [];
  for (const file of files) {
    const source = await readFile(file, "utf8");
    const masked = maskRustTrivia(source);
    const ranges = inlineTestRanges(source, masked);
    const debugOnlyRanges = debugOnlyFunctionRanges(source, masked);
    const starts = lineStarts(source);
    const relative = path.relative(root, file).split(path.sep).join("/");
    const testFile = isTestFile(relative);
    for (let lineIndex = 0; lineIndex < starts.length; lineIndex++) {
      const start = starts[lineIndex];
      const end = lineIndex + 1 < starts.length ? starts[lineIndex + 1] - 1 : source.length;
      const codeLine = masked.slice(start, end);
      if (!codeLine.trim()) continue;
      for (const [category, matcher] of patterns) {
        const match = matcher.exec(codeLine);
        if (!match) continue;
        const occurrence = start + match.index;
        if (
          category === "SQLite mutation API" &&
          match[0].startsWith(".execute_batch") &&
          /^\.execute_batch\s*\(\s*"(?:SAVEPOINT|RELEASE|ROLLBACK\s+TO)\b/i.test(
            source.slice(occurrence),
          )
        )
          continue;
        const scope = testFile
          ? "test_file"
          : ranges.some(
                ({ start: rangeStart, end: rangeEnd }) =>
                  occurrence >= rangeStart && occurrence < rangeEnd,
              )
            ? "inline_test"
            : debugOnlyRanges.some(
                  ({ start: rangeStart, end: rangeEnd }) =>
                    occurrence >= rangeStart && occurrence < rangeEnd,
                )
              ? "debug_only"
              : "production";
        findings.push({
          path: relative,
          line: lineIndex + 1,
          category,
          scope,
          source: source.slice(start, end).trim(),
        });
      }
    }
  }
  return findings;
}

const findingKey = ({ path: filePath, category }) => `${filePath}\0${category}`;

export function productionCounts(findings) {
  const counts = new Map();
  for (const finding of findings) {
    if (finding.scope !== "production") continue;
    const key = findingKey(finding);
    counts.set(key, (counts.get(key) ?? 0) + 1);
  }
  return counts;
}

export function compareAllowlist(findings, entries) {
  const actual = productionCounts(findings);
  const expected = new Map(entries.map((entry) => [findingKey(entry), entry.count]));
  const keys = new Set([...actual.keys(), ...expected.keys()]);
  const differences = [];
  for (const key of [...keys].sort()) {
    const [filePath, category] = key.split("\0");
    const found = actual.get(key) ?? 0;
    const allowed = expected.get(key);
    if (allowed === undefined)
      differences.push(`${filePath} [${category}]: found ${found}, no allowlist entry`);
    else if (found !== allowed)
      differences.push(`${filePath} [${category}]: allowlist ${allowed}, found ${found}`);
  }
  return differences;
}

async function loadAllowlist(file) {
  const value = JSON.parse(await readFile(file, "utf8"));
  if (value.version !== 1 || !Array.isArray(value.entries))
    throw new Error("writer allowlist must have version 1 and an entries array");
  const keys = new Set();
  for (const entry of value.entries) {
    if (entry === null || Object.getPrototypeOf(entry) !== Object.prototype)
      throw new Error(`invalid writer allowlist entry: ${JSON.stringify(entry)}`);
    if (
      !arkSourceRoots.some((root) =>
        String(entry.path ?? "").startsWith(`${path.relative(repoRoot, root)}/`),
      ) ||
      !patterns.some(([category]) => category === entry.category) ||
      !Number.isSafeInteger(entry.count) ||
      entry.count < 1
    )
      throw new Error(`invalid writer allowlist entry: ${JSON.stringify(entry)}`);
    const key = findingKey(entry);
    if (keys.has(key))
      throw new Error(`duplicate writer allowlist entry: ${entry.path} [${entry.category}]`);
    keys.add(key);
  }
  return value.entries;
}

function formatInventory(findings, fileCount) {
  const production = findings.filter(({ scope }) => scope === "production");
  const sourceRoots = arkSourceRoots
    .map((root) => path.relative(repoRoot, root).split(path.sep).join("/"))
    .join(" and ");
  const output = [
    "# ARK SQLite writer census",
    "",
    `Generated from ${sourceRoots}; ${production.length} production matches, ${fileCount} Rust files scanned. Findings in test files, inline #[cfg(test)] modules, and #[cfg(debug_assertions)] adapters are shown separately. This is a source inventory, not proof of runtime reachability or writer ownership.`,
  ];
  for (const [category] of patterns) {
    const entries = findings.filter((finding) => finding.category === category);
    output.push("", `## ${category} (${entries.length})`);
    for (const entry of entries)
      output.push(
        `- ${entry.path}:${entry.line} [${entry.scope}]: \`${entry.source.replaceAll("`", "\\`")}\``,
      );
  }
  return `${output.join("\n")}\n`;
}

export async function runAudit(args = process.argv.slice(2)) {
  const unknown = args.filter((arg) => arg !== "--check");
  if (unknown.length) throw new Error(`unknown argument: ${unknown[0]}`);
  const findings = await collectWriterFindings();
  if (args.includes("--check")) {
    const entries = await loadAllowlist(allowlistPath);
    const differences = compareAllowlist(findings, entries);
    if (differences.length) {
      process.stderr.write(
        `ARK SQLite writer census changed (${differences.length} differences):\n`,
      );
      for (const difference of differences) process.stderr.write(`- ${difference}\n`);
      process.stderr.write(
        "Reduce allowlisted counts as writer paths are removed; review any increase explicitly.\n",
      );
      return 1;
    }
    process.stdout.write(
      `ARK SQLite writer census passed: ${entries.length} path/category counts match.\n`,
    );
    return 0;
  }
  const files = (await Promise.all(arkSourceRoots.map((root) => collectRustFiles(root)))).flat();
  process.stdout.write(formatInventory(findings, files.length));
  return 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  runAudit().then(
    (status) => {
      process.exitCode = status;
    },
    (error) => {
      process.stderr.write(`ARK SQLite writer census failed: ${error.message}\n`);
      process.exitCode = 1;
    },
  );
}
