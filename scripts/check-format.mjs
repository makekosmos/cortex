import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";

const extensions = /\.(c|m)?(js|ts)x?$|\.vue$|\.json$|\.ya?ml$/;
// Text source types that must never carry a UTF-8 BOM — BOMs silently change
// file bytes and have previously snuck into ~80 .rs files at once.
const bomExtensions = /\.(rs|mjs|cjs|ts|tsx|mts|cts|toml|json|md)$/;
// rustfmt silently gives up on compressed one-liners (a >100-char line inside
// an expression leaves the whole enclosing function unformatted); the only
// reliable stable signal is the line length itself.
const RUST_MAX_LINE = 100;
const base = process.env.FORMAT_BASE;
const staged = process.argv.includes("--staged");
const args = base
  ? /^0+$/.test(base)
    ? ["ls-tree", "-r", "--name-only", "HEAD"]
    : ["diff", "--name-only", "--diff-filter=ACMR", `${base}...HEAD`]
  : staged
    ? ["diff", "--cached", "--name-only", "--diff-filter=ACMR"]
    : ["diff", "--name-only", "--diff-filter=ACMR", "HEAD"];
const gitEnv = { ...process.env };
for (const key of [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_COMMON_DIR",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_CEILING_DIRECTORIES",
  "GIT_PREFIX",
])
  delete gitEnv[key];
const changed = spawnSync("git", args, { encoding: "utf8", env: gitEnv });
if (changed.error || changed.status !== 0) process.exit(changed.status ?? 1);

const allChanged = changed.stdout.split(/\r?\n/).filter(Boolean);
const files = allChanged.filter((file) => extensions.test(file));

let failed = false;

// BOM guard: any changed text source file must not start with EF BB BF.
const bomHits = [];
for (const file of allChanged) {
  if (!bomExtensions.test(file) || !existsSync(file)) continue;
  const head = readFileSync(file).subarray(0, 3);
  if (head[0] === 0xef && head[1] === 0xbb && head[2] === 0xbf) bomHits.push(file);
}
if (bomHits.length) {
  failed = true;
  console.error("UTF-8 BOM found in text source files:");
  for (const f of bomHits) console.error(`  ${f}`);
}

// Rust guards: >100-char lines (covers include!() fragments that cargo fmt
// never opens) and cargo fmt itself when a workspace exists.
const changedRs = allChanged.filter((file) => file.endsWith(".rs"));
const longLines = [];
for (const file of changedRs) {
  if (!existsSync(file)) continue;
  readFileSync(file, "utf8")
    .split("\n")
    .forEach((line, i) => {
      if (line.length > RUST_MAX_LINE) longLines.push(`${file}:${i + 1} (${line.length} chars)`);
    });
}
if (longLines.length) {
  failed = true;
  console.error("Rust lines over 100 chars (rustfmt cannot format these regions):");
  for (const l of longLines) console.error(`  ${l}`);
}

// A `\`-continuation inside a string literal whose next line starts at
// column 0 is the signature of a mechanical mid-token split: continuations
// must be indented so the source stays readable. This is a debt guard, so it
// scans every tracked .rs file — a tiny lexer keeps string state correct
// across escapes, raw strings, byte strings, char literals and comments.
function col0Continuations(src) {
  const bad = [];
  const lines = src.split("\n");
  let state = "code"; // code | string | raw | lineComment | blockComment
  let rawHashes = 0;
  let blockDepth = 0;
  for (let li = 0; li < lines.length; li++) {
    const line = lines[li];
    let i = 0;
    while (i < line.length) {
      if (state === "code") {
        if (line.startsWith("//", i)) break;
        if (line.startsWith("/*", i)) {
          state = "blockComment";
          blockDepth = 1;
          i += 2;
          continue;
        }
        const raw = line.slice(i).match(/^b?r(#*)"/);
        if (raw) {
          state = "raw";
          rawHashes = raw[1].length;
          i += raw[0].length;
          continue;
        }
        if (line[i] === '"' || line.startsWith('b"', i)) {
          state = "string";
          i += line[i] === "b" ? 2 : 1;
          continue;
        }
        if (line[i] === "'") {
          // char literal ('a', '\'', '"', '\\') vs lifetime ('a): a char
          // literal is `<quote><escape|char><quote>` — look ahead.
          const m = line.slice(i).match(/^'(\\[\\'"nrt0xu]|[^'\\])'/);
          i += m ? m[0].length : 1;
          continue;
        }
        i++;
        continue;
      }
      if (state === "string") {
        if (line[i] === "\\") {
          i += 2;
          continue;
        }
        if (line[i] === '"') state = "code";
        i++;
        continue;
      }
      if (state === "raw") {
        if (line[i] === '"' && line.startsWith('"'.padEnd(rawHashes + 1, "#"), i)) {
          state = "code";
          i += rawHashes + 1;
          continue;
        }
        i++;
        continue;
      }
      if (state === "blockComment") {
        if (line.startsWith("/*", i)) {
          blockDepth++;
          i += 2;
          continue;
        }
        if (line.startsWith("*/", i)) {
          blockDepth--;
          if (blockDepth === 0) state = "code";
          i += 2;
          continue;
        }
        i++;
        continue;
      }
      i++;
    }
    // a `\`-continued non-raw string literal: the line ends while still in
    // `string` state with an ODD number of trailing backslashes (an even
    // count ends in an escaped `\\`, which is literal content)
    let trailing = 0;
    for (let k = line.length - 1; k >= 0 && line[k] === "\\"; k--) trailing++;
    if (state === "string" && trailing % 2 === 1) {
      const next = lines[li + 1];
      if (next !== undefined && next !== "" && !next.startsWith(" ") && !next.startsWith("\t"))
        bad.push(li + 1);
    }
  }
  return bad;
}

const flatConts = [];
const trackedRs = spawnSync("git", ["ls-files", "*.rs"], {
  encoding: "utf8",
  env: gitEnv,
});
if (!trackedRs.error && trackedRs.status === 0) {
  for (const file of trackedRs.stdout.split(/\r?\n/).filter(Boolean)) {
    if (!existsSync(file) || file.includes("vendor/")) continue;
    for (const ln of col0Continuations(readFileSync(file, "utf8"))) flatConts.push(`${file}:${ln}`);
  }
}
if (flatConts.length) {
  failed = true;
  console.error("Rust string continuations starting at column 0 (mid-token splits):");
  for (const l of flatConts) console.error(`  ${l}`);
}
if (changedRs.length && existsSync("Cargo.toml")) {
  const fmt = spawnSync("cargo", ["fmt", "--all", "--", "--check"], {
    stdio: "inherit",
    shell: process.platform === "win32",
  });
  if (fmt.status !== 0) failed = true;
  const fragments = spawnSync(
    process.execPath,
    [join("scripts", "fmt-include-fragments.mjs"), "--check"],
    { stdio: "inherit" },
  );
  if (fragments.status !== 0) failed = true;
}

if (files.length === 0) process.exit(failed ? 1 : 0);

const result = spawnSync("oxfmt", ["--check", ...files], {
  stdio: "inherit",
  shell: process.platform === "win32",
});
process.exit(result.status !== 0 || failed ? 1 : 0);
