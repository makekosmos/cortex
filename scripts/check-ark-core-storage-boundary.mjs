#!/usr/bin/env node

import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import {
  bracePairs,
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
  ["SQLite connection escape hatch", /\.\s*with_connection\s*\(/],
  [
    "SQLite operation",
    /\b(?:conn|connection|transaction|tx)\s*\.\s*(?:execute|execute_batch|execute_cached|query_row|query_map|prepare|prepare_cached|unchecked_transaction|transaction_with_behavior)\s*\(/,
  ],
  [
    "SQLite lifecycle",
    /\b(?:Connection::open|open_db|open_with_flags|init_schema|recover_interrupted_restore|backup_to_file|backup_to_file_chunked)\s*\(/,
  ],
];
const SQL_LITERAL =
  /^\s*(?:SELECT|INSERT\s+INTO|UPDATE\s+[\w"]+|DELETE\s+FROM|CREATE\s+(?:TABLE|INDEX|TRIGGER)|ALTER\s+TABLE|DROP\s+(?:TABLE|INDEX)|PRAGMA|SAVEPOINT|RELEASE\s+\w+)\b/i;

function lineAt(source, offset) {
  let line = 1;
  for (let index = 0; index < offset; index++) if (source[index] === "\n") line++;
  return line;
}

function cargoDependencyScope(manifest, offset) {
  const sections = [...manifest.slice(0, offset).matchAll(/^\s*\[([^\]]+)\]\s*$/gm)];
  const section = sections.at(-1)?.[1] ?? "";
  return /(?:^|\.)dev-dependencies$/.test(section) ? "test_file" : "production";
}

function isTestFile(relativePath) {
  return /(?:^|\/)tests?(?:_[^/]+)?(?:\.rs|\/)|(?:^|\/)[^/]+_tests?\.rs$/.test(relativePath);
}

function testOnlyModuleDeclarations(source) {
  const masked = maskRustTrivia(source);
  const declarations = [];
  const pattern = /((?:#\s*\[[^\]]+\]\s*)*)mod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/g;
  for (const match of masked.matchAll(pattern)) {
    if (!/#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]/.test(match[1])) continue;
    const attributes = source.slice(match.index, match.index + match[0].length);
    const explicitPath = /#\s*\[\s*path\s*=\s*"([^"]+)"\s*\]/.exec(attributes)?.[1];
    declarations.push({ name: match[2], path: explicitPath });
  }
  return declarations;
}

function testOnlyItemRanges(source, masked = maskRustTrivia(source)) {
  const ranges = [];
  const functionPattern =
    /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:(?:pub(?:\s*\([^)]*\))?|async|unsafe|extern(?:\s*"[^"]+")?)\s+)*fn\s+[A-Za-z_][A-Za-z0-9_]*(?:\s*<[^>]*>)?\s*\([^)]*\)[^{]*\{/g;
  const pairs = bracePairs(masked);
  for (const match of masked.matchAll(functionPattern)) {
    const open = match.index + match[0].lastIndexOf("{");
    const close = pairs.get(open);
    if (close === undefined) throw new Error(`unclosed cfg(test) function at byte ${open}`);
    ranges.push({ start: match.index, end: close + 1 });
  }
  const usePattern = /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*use\b[^;]*;/g;
  for (const match of masked.matchAll(usePattern))
    ranges.push({ start: match.index, end: match.index + match[0].length });
  const blockItemPattern =
    /#\s*\[\s*cfg\s*\(\s*test\s*\)\s*\]\s*(?:(?:pub(?:\s*\([^)]*\))?)\s+)?(?:unsafe\s+)?(?:impl|trait)\b[^{}]*\{/g;
  for (const match of masked.matchAll(blockItemPattern)) {
    const open = match.index + match[0].lastIndexOf("{");
    const close = pairs.get(open);
    if (close === undefined) throw new Error(`unclosed cfg(test) item at byte ${open}`);
    ranges.push({ start: match.index, end: close + 1 });
  }
  return ranges;
}

function moduleDeclarations(source) {
  const masked = maskRustTrivia(source);
  return [...masked.matchAll(/\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*;/g)].map((match) => match[1]);
}

function childModuleFile(file, name, files, explicitPath) {
  if (explicitPath) {
    const candidate = path.resolve(path.dirname(file), explicitPath);
    return files.has(candidate) ? candidate : undefined;
  }
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
      const gated = new Map(
        testOnlyModuleDeclarations(source).map(({ name, path: modulePath }) => [name, modulePath]),
      );
      if (testOnly.has(parent)) {
        for (const match of source.matchAll(/\binclude!\s*\(\s*"([^"]+)"\s*\)/g)) {
          const included = path.resolve(path.dirname(parent), match[1]);
          if (files.has(included) && !testOnly.has(included)) {
            testOnly.add(included);
            changed = true;
          }
        }
      }
      for (const name of moduleDeclarations(source)) {
        const child = childModuleFile(parent, name, files, gated.get(name));
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
  const testOnlyRanges = testOnlyItemRanges(source, masked);
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
          : testOnlyRanges.some(({ start, end }) => start <= offset && offset < end)
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
  sourceRoot = CORE,
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
  const dependency = /^\s*rusqlite\b(?:\s*=|\s*\{|\s*\.)[^\n]*/m.exec(manifest);
  if (dependency)
    violations.push({
      path: path.relative(root, manifestPath).split(path.sep).join("/"),
      line: lineAt(manifest, dependency.index),
      category: "rusqlite dependency",
      scope: cargoDependencyScope(manifest, dependency.index),
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
