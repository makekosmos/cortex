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
  rustStringLiterals,
} from "./rust-source-scan.mjs";

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const CORE = path.join(ROOT, "core", "crates", "ark-core");
const CODE_PATTERNS = [
  ["SQLite type or import", /\b(?:rusqlite|Transaction|Statement|Row)\b/],
  [
    "SQLite operation",
    /\b(?:conn|connection|transaction|tx)\s*\.\s*(?:execute|execute_batch|execute_cached|query_row|query_map|prepare|prepare_cached|unchecked_transaction|transaction_with_behavior)\s*\(/,
  ],
  [
    "SQLite lifecycle",
    /\b(?:Connection::open|open_db|open_with_flags|init_schema|recover_interrupted_restore|restore_snapshot|backup_to_file|backup_to_file_chunked)\s*\(/,
  ],
];
const SQL_LITERAL =
  /\b(?:SELECT|INSERT\s+INTO|UPDATE\s+[\w"]+|DELETE\s+FROM|CREATE\s+(?:TABLE|INDEX|TRIGGER)|ALTER\s+TABLE|DROP\s+(?:TABLE|INDEX)|PRAGMA|SAVEPOINT|RELEASE\s+\w+)\b/i;

function lineAt(source, offset) {
  let line = 1;
  for (let index = 0; index < offset; index++) if (source[index] === "\n") line++;
  return line;
}

function isTestFile(relativePath) {
  return /(?:^|\/)tests?(?:_[^/]+)?(?:\.rs|\/)|(?:^|\/)[^/]+_tests?\.rs$/.test(relativePath);
}

function testOnlyModuleDeclarations(source) {
  const masked = maskRustTrivia(source);
  return [
    ...masked.matchAll(/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/g),
  ].map((match) => match[1]);
}

function moduleDeclarations(source) {
  const masked = maskRustTrivia(source);
  return [...masked.matchAll(/\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/g)].map((match) => match[1]);
}

function childModuleFile(file, name, files) {
  const fileName = path.basename(file);
  const moduleDirectory = ["lib.rs", "main.rs", "mod.rs"].includes(fileName)
    ? path.dirname(file)
    : path.join(path.dirname(file), path.basename(file, ".rs"));
  for (const candidate of [
    path.join(moduleDirectory, `${name}.rs`),
    path.join(moduleDirectory, name, "mod.rs"),
  ]) {
    if (files.has(candidate)) return candidate;
  }
  return undefined;
}

async function testOnlyModulePaths(sourceRoot) {
  const files = new Set(await collectRustFiles(sourceRoot));
  const testOnly = new Set();
  let changed = true;
  while (changed) {
    changed = false;
    for (const parent of files) {
      const source = await readFile(parent, "utf8");
      const gated = new Set(testOnlyModuleDeclarations(source));
      for (const name of moduleDeclarations(source)) {
        const child = childModuleFile(parent, name, files);
        if (!child) continue;
        if ((testOnly.has(parent) || gated.has(name)) && !testOnly.has(child)) {
          testOnly.add(child);
          changed = true;
        }
      }
    }
  }
  return testOnly;
}

function sourceViolations(source, relativePath, testOnly = false) {
  const masked = maskRustTrivia(source);
  const violations = [];
  const found = new Set();
  const testRanges = inlineTestRanges(source, masked);
  const debugRanges = debugOnlyFunctionRanges(source, masked);
  for (const [category, pattern] of CODE_PATTERNS) {
    for (const match of masked.matchAll(new RegExp(pattern.source, "g")))
      addViolation(lineAt(source, match.index), category, match.index);
  }
  for (const literal of rustStringLiterals(source)) {
    if (SQL_LITERAL.test(literal.content))
      addViolation(lineAt(source, literal.start), "SQL literal", literal.start);
  }
  return violations;

  function addViolation(line, category, offset) {
    const scope =
      isTestFile(relativePath) || testOnly
        ? "test_file"
        : testRanges.some(({ start, end }) => start <= offset && offset < end)
          ? "inline_test"
          : debugRanges.some(({ start, end }) => start <= offset && offset < end)
            ? "debug_only"
            : "production";
    const key = `${line}\0${category}\0${scope}`;
    if (found.has(key)) return;
    found.add(key);
    violations.push({ path: relativePath, line, category, scope });
  }
}

export async function collectStorageViolations({
  sourceRoot = path.join(CORE, "src"),
  manifestPath = path.join(CORE, "Cargo.toml"),
  root = ROOT,
} = {}) {
  const violations = [];
  const manifest = await readFile(manifestPath, "utf8");
  const testOnlyPaths = new Set(
    [...(await testOnlyModulePaths(sourceRoot))].map((file) =>
      path.relative(root, file).split(path.sep).join("/"),
    ),
  );
  const dependency = /^\s*rusqlite\b(?:\s*=|\s*\{|\s*\.)/m.exec(manifest);
  if (dependency)
    violations.push({
      path: path.relative(root, manifestPath).split(path.sep).join("/"),
      line: lineAt(manifest, dependency.index),
      category: "rusqlite dependency",
      scope: "production",
    });

  for (const file of (await collectRustFiles(sourceRoot)).sort()) {
    const source = await readFile(file, "utf8");
    const relativePath = path.relative(root, file).split(path.sep).join("/");
    violations.push(...sourceViolations(source, relativePath, testOnlyPaths.has(relativePath)));
  }
  return violations.sort(
    (left, right) =>
      left.path.localeCompare(right.path) ||
      left.line - right.line ||
      left.category.localeCompare(right.category),
  );
}

export async function runCheck(args = process.argv.slice(2)) {
  if (args.length) throw new Error(`unknown argument: ${args[0]}`);
  const violations = await collectStorageViolations();
  if (violations.length) {
    process.stderr.write(`ark-core storage boundary has ${violations.length} violations:\n`);
    for (const violation of violations)
      process.stderr.write(
        `- ${violation.path}:${violation.line} [${violation.scope}, ${violation.category}]\n`,
      );
    return 1;
  }
  process.stdout.write(
    "ark-core storage boundary passed: Core has no SQLite dependency or SQL source.\n",
  );
  return 0;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === path.resolve(process.argv[1])) {
  runCheck().then(
    (status) => {
      process.exitCode = status;
    },
    (error) => {
      process.stderr.write(`ark-core storage boundary check failed: ${error.message}\n`);
      process.exitCode = 1;
    },
  );
}
