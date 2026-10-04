#!/usr/bin/env node
// KOS-332: narrow agent iteration. Maps changed files to cargo crates and runs
// only `cargo check -p <crate>` plus `cargo nextest run -p <crate> [filter]`
// instead of the workspace-wide gate. The full gate (`pnpm run check` /
// pre-push) stays the source of truth — clippy --workspace, the fixture
// integration tests and staging run only there (KOS-270).
//
// Usage:
//   node scripts/quick-check.mjs                # files changed vs HEAD
//   node scripts/quick-check.mjs <filter>...    # extra args go to nextest
//   node scripts/quick-check.mjs --check-only   # cargo check, no nextest
//   node scripts/quick-check.mjs --files <path>...  # explicit file list
import { spawnSync } from "node:child_process";
import { resolve } from "node:path";

const cortex = resolve(import.meta.dirname, "..");

// Longest prefix first: runtime/crates/* must win over runtime/.
const CRATE_MAP = [
  { prefix: "runtime/crates/package-protocol/", packages: ["package-protocol"] },
  { prefix: "runtime/crates/pe-version-info/", packages: ["pe-version-info"] },
  { prefix: "core/crates/ark-core/", packages: ["ark-core"] },
  { prefix: "runtime/", packages: ["engine"] },
  { prefix: "manager-gpui/", packages: [], manager: true },
];

function changedFiles() {
  const result = spawnSync("git", ["status", "--porcelain=v1", "-z"], {
    cwd: cortex,
    encoding: "utf8",
  });
  if (result.status !== 0) {
    console.error(result.stderr || "git status failed");
    process.exit(1);
  }
  return result.stdout
    .split("\0")
    .filter(Boolean)
    .map((entry) => entry.slice(3))
    .filter((p) => p.length > 0);
}

function cratesFor(files) {
  const packages = new Set();
  let manager = false;
  for (const file of files) {
    const rule = CRATE_MAP.find((r) => file.startsWith(r.prefix));
    if (!rule) continue;
    for (const p of rule.packages) packages.add(p);
    if (rule.manager) manager = true;
  }
  return { packages: [...packages], manager };
}

function run(args, cwd = cortex) {
  const result = spawnSync("cargo", args, { cwd, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const argv = process.argv.slice(2);
const checkOnly = argv.includes("--check-only");
const filesFlag = argv.indexOf("--files");
const files =
  filesFlag === -1
    ? changedFiles()
    : argv
        .slice(filesFlag + 1)
        .map((f) => (f.startsWith("/") ? f.slice(cortex.length + 1) : f));
// Filters are positional args before --files.
const filterArgs = [];
for (let i = 0; i < argv.length; i++) {
  if (argv[i] === "--check-only" || argv[i] === "--files") break;
  filterArgs.push(argv[i]);
}

const { packages, manager } = cratesFor(files);
if (!packages.length && !manager) {
  console.log("quick-check: no cargo crates affected by changed files");
  process.exit(0);
}

// Same test-only feature set as the gate (runtime/scripts/test-lib.mjs) so
// `cargo check -p engine --all-targets` compiles the fixture bins and the
// integration-test cfg surface instead of a different flag set.
const ENGINE_FEATURES = [
  "--features",
  "engine/package-worker-fixture,engine/markdown-bridge-fixture",
];

for (const pkg of packages) {
  const features = pkg === "engine" ? ENGINE_FEATURES : [];
  run(["check", "-p", pkg, "--all-targets", ...features]);
}
if (manager) {
  run(["check", "--manifest-path", "manager-gpui/Cargo.toml", "--all-targets"]);
}

if (!checkOnly) {
  for (const pkg of packages) {
    const features = pkg === "engine" ? ENGINE_FEATURES : [];
    run(["nextest", "run", "-p", pkg, ...features, ...filterArgs]);
  }
  if (manager) {
    run(["nextest", "run", "--manifest-path", "manager-gpui/Cargo.toml", ...filterArgs]);
  }
}
