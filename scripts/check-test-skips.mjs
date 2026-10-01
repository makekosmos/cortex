// Source lint: a test must never pass because its setup failed. Fails when a
// .rs file contains
//   * `#[ignore]` (or `cfg_attr(..., ignore)`) with no `= "reason"`;
//   * inside a `#[test]`/`#[tokio::test]`/… body, an early bare `return;`
//     after link/symlink setup or a printed skip/NOT_RUN notice — the classic
//     "test passed without testing" shape audited in KOS-282.
// Runs on every hook invocation like check:brand — it is a fast text scan.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const SKIP_DIRS = new Set([".git", "node_modules", "target", "dist"]);

// `#[ignore]` or `#[cfg_attr(..., ignore)]` — but never `ignore = "..."`.
const BARE_IGNORE = /#\s*\[[^\]\n]*\bignore\b(?!\s*=)[^\]\n]*\]/g;

const TEST_ATTR = /#\s*\[\s*(?:[\w:]+::)?(?:test|rstest|test_case)\b[^\]]*\]/g;

// `if !linked { return; }` / `if !link_dir(..) { return; }`: a setup result
// consulted to abort the test early.
const LINK_EARLY_RETURN = /if\s+!\s*[^;{}]*\b\w*(?:link|symlink)\w*\b[^;{}]*\{[^{}]*\breturn\s*;/gs;

// `{ eprintln!("skip…" | "NOT_RUN…"); return; }`: a notice printed where a
// failure belongs, followed by a bare return, in the same block.
const PRINTED_SKIP_RETURN =
  /\{[^{}]*(?:e?println!)\s*\([^;{}]*?(?:skip|NOT_RUN)[^;{}]*?\)\s*;[^{}]*\breturn\s*;/gs;

const BODY_PATTERNS = [
  ["early return on link/symlink setup failure", LINK_EARLY_RETURN],
  ["printed skip notice followed by a bare return", PRINTED_SKIP_RETURN],
];

const COMMENTS = /\/\/[^\n]*|\/\*[\s\S]*?\*\//g;
const COMMENTS_AND_STRINGS =
  /r#+"(?:[\s\S]*?)"#+|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])+'|\/\/[^\n]*|\/\*[\s\S]*?\*\//g;

const blank = (m) => " ".repeat(m.length);

// String literals stay readable here — the printed-notice pattern must see
// the `skip`/`NOT_RUN` text inside `eprintln!(…)`.
function maskedComments(source) {
  return source.replace(COMMENTS, blank);
}

// String/char literals are blanked as well so a `}` inside them cannot
// unbalance the brace walk that delimits a test body.
function maskedAll(source) {
  return source.replace(COMMENTS_AND_STRINGS, blank);
}

// Spans [start, end) of every `fn` body directly preceded by a test
// attribute, in the comment-and-string-masked source.
export function testBodies(source) {
  const clean = maskedAll(source);
  const bodies = [];
  for (const attr of clean.matchAll(TEST_ATTR)) {
    const fn = /\bfn\b/g;
    fn.lastIndex = attr.index + attr[0].length;
    const head = fn.exec(clean);
    if (!head) continue;
    const open = clean.indexOf("{", head.index);
    if (open === -1) continue;
    let depth = 0;
    let end = open;
    for (; end < clean.length; end++) {
      if (clean[end] === "{") depth++;
      else if (clean[end] === "}" && --depth === 0) break;
    }
    bodies.push([attr.index, end]);
  }
  return bodies;
}

export function scanSource(path, source) {
  const violations = [];
  const bodies = testBodies(source);
  const visible = maskedComments(source);
  for (const [label, pattern] of BODY_PATTERNS) {
    for (const match of visible.matchAll(pattern)) {
      const inside = bodies.some(([start, end]) => match.index > start && match.index < end);
      if (!inside) continue;
      const line = source.slice(0, match.index).split("\n").length;
      violations.push(`${path}:${line}: ${label}`);
    }
  }
  for (const match of visible.matchAll(BARE_IGNORE)) {
    const line = source.slice(0, match.index).split("\n").length;
    violations.push(`${path}:${line}: bare #[ignore] without a reason`);
  }
  return violations;
}

export function rustFiles(root) {
  const files = [];
  const walk = (dir) => {
    for (const entry of readdirSync(dir, { withFileTypes: true })) {
      if (entry.isDirectory()) {
        if (!SKIP_DIRS.has(entry.name)) walk(join(dir, entry.name));
      } else if (entry.name.endsWith(".rs")) files.push(join(dir, entry.name));
    }
  };
  walk(root);
  return files;
}

export function scanTree(root) {
  const violations = [];
  for (const file of rustFiles(root))
    violations.push(...scanSource(file, readFileSync(file, "utf8")));
  return violations;
}

function main() {
  const violations = scanTree(process.cwd());
  for (const violation of violations) console.error(`test-skips: ${violation}`);
  if (violations.length) {
    console.error(
      `test-skips: ${violations.length} test(s) can pass without testing — fix or #[ignore = "reason"]`,
    );
    process.exitCode = 1;
  }
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
