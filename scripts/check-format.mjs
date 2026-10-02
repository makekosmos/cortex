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
