import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { executePlan } from "./check-plan-commands.mjs";
import { parseNameStatus } from "./check-plan-git.mjs";

export { executePlan } from "./check-plan-commands.mjs";
export { parseNameStatus } from "./check-plan-git.mjs";

const CHECK_ORDER = [
  "desktop-typecheck",
  "manager-typecheck",
  "host-typecheck",
  "host-contracts",
  "desktop-contracts",
  "first-party-contracts",
  "rustfmt",
  "clippy",
  "test:rust",
  "runtime-staging",
  "native-services",
  "manager-gpui",
  "lint",
  "format",
];
const ASSET_EXTENSIONS = /\.(?:png|jpe?g|gif|svg|webp|ico|avif)$/i;
const ZERO_SHA = /^0{40}$/;

function gitEnv() {
  const env = { ...process.env };
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
    delete env[key];
  return env;
}

function git(args) {
  return spawnSync("git", args, { cwd: process.cwd(), encoding: "utf8", env: gitEnv() });
}

function diffFiles(rangeArgs) {
  const result = git(["diff", "--name-status", "-z", ...rangeArgs]);
  if (result.error || result.status !== 0) return null;
  return parseNameStatus(result.stdout);
}

function untrackedFiles() {
  const result = git(["ls-files", "--others", "--exclude-standard", "-z"]);
  if (result.error || result.status !== 0) return null;
  return result.stdout
    .split("\0")
    .filter(Boolean)
    .map((path) => ({ path: path.replaceAll("\\", "/"), status: "A" }));
}

function isShallow() {
  const result = git(["rev-parse", "--is-shallow-repository"]);
  return result.error || result.status !== 0 || result.stdout.trim() === "true";
}

function validCommit(value) {
  const result = git(["rev-parse", "--verify", `${value}^{commit}`]);
  return !result.error && result.status === 0;
}

function oneMergeBase(base, head) {
  const result = git(["merge-base", "--all", base, head]);
  if (result.error || result.status !== 0) return false;
  return result.stdout.trim().split(/\r?\n/).filter(Boolean).length === 1;
}

export function parsePushInput(input) {
  const records = input
    .split(/\r?\n/)
    .map((line) => line.trim().split(/\s+/))
    .filter((parts) => parts.length > 1 && parts.some(Boolean));
  if (records.length !== 1 || records[0].length !== 4) return null;
  const [localRef, localSha, remoteRef, remoteSha] = records[0];
  if (ZERO_SHA.test(localSha) || ZERO_SHA.test(remoteSha)) return null;
  return { localRef, localSha, remoteRef, remoteSha };
}

function failClosed(mode, reason, changed = []) {
  return {
    schemaVersion: 1,
    mode,
    full: true,
    changed: changed.map((file) => file.path),
    checks: ["full"],
    reasons: [reason],
  };
}

function filesForMode(mode) {
  if (mode === "pre-commit") {
    const files = diffFiles(["--cached", "HEAD"]);
    return files ? { files } : { full: "unable to inspect staged changes" };
  }
  if (mode === "worktree") {
    const files = diffFiles(["HEAD"]);
    const untracked = untrackedFiles();
    return files && untracked
      ? { files: [...files, ...untracked] }
      : { full: "unable to inspect worktree changes" };
  }
  if (mode === "pre-push") {
    let input;
    try {
      input = readFileSync(0, "utf8");
    } catch {
      return { full: "pre-push input is unavailable" };
    }
    const push = parsePushInput(input);
    if (!push) return { full: "pre-push input is missing, new, or ambiguous" };
    if (![push.localSha, push.remoteSha].every((sha) => /^[0-9a-f]{40}$/i.test(sha)))
      return { full: "pre-push contains an invalid object id" };
    if (isShallow() || !validCommit(push.localSha) || !validCommit(push.remoteSha))
      return { full: "pre-push history is shallow or an object is missing" };
    if (!oneMergeBase(push.remoteSha, push.localSha))
      return { full: "pre-push history has no unique merge base" };
    const files = diffFiles([`${push.remoteSha}...${push.localSha}`]);
    return files ? { files } : { full: "unable to inspect pre-push diff" };
  }
  return { full: `unknown planner mode: ${mode}` };
}

function isDocumentation(path) {
  return (
    /^docs\//i.test(path) ||
    /^(?:README|CHANGELOG|CONTRIBUTING|CODE_OF_CONDUCT|SECURITY|LICENSE|NOTICE|AUTHORS|HISTORY|ROADMAP)(?:\.[^/]*)?$/i.test(
      path,
    )
  );
}

function isFullInfluence(path) {
  return (
    /(?:^|\/)(?:package\.json|(?:bun|pnpm|yarn)\.lock|package-lock\.json|Cargo\.(?:lock|toml)|dev-packages\.json|rust-toolchain(?:\.toml)?|\.tool-versions)$/.test(
      path,
    ) ||
    /^(?:\.github\/workflows\/|\.github\/actions\/)/.test(path) ||
    /^(?:lefthook\.ya?ml|\.ox(?:lintrc|fmtrc)\.|scripts\/check-plan\.)/.test(path) ||
    /(?:^|\/)(?:tsconfig(?:\.[^/]+)?\.json|vite\.config\.|webpack\.config\.|rollup\.config\.)/.test(
      path,
    ) ||
    /(?:^|\/)(?:build|release|toolchain|manifest|package|protocol|ipc|sdk|compat|alias)[^/]*\.(?:[cm]?[jt]sx?|vue|json|ya?ml|toml|rs)$/i.test(
      path,
    ) ||
    /^(?:packages|shared)\//.test(path) ||
    /(?:^|\/)(?:packages|shared|ipc|protocol|sdk|compat|aliases?)\//i.test(path)
  );
}

function classify(file) {
  const path = file.path;
  if (
    /^(?:desktop|host|manager|runtime|packages|shared)\//i.test(path) &&
    /\.(?:md|mdx|txt)$/i.test(path)
  )
    return "full";
  if (isFullInfluence(path)) return "full";
  if (isDocumentation(path) || ASSET_EXTENSIONS.test(path)) return [];
  if (/^desktop\/src\//.test(path)) return ["desktop-typecheck", "lint", "format"];
  if (/^desktop\/electron\//.test(path))
    return ["desktop-typecheck", "desktop-contracts", "lint", "format"];
  if (/^manager\/src\//.test(path)) return ["manager-typecheck", "lint", "format"];
  if (/^host\/electron\//.test(path)) return ["host-typecheck", "host-contracts"];
  if (/^host\/e2e\//.test(path)) return "full";
  if (/^runtime\//.test(path)) return ["rustfmt", "clippy", "test:rust", "runtime-staging"];
  if (/^native-services\//.test(path)) return ["native-services"];
  // core/ is the vendored upstream subtree: crate sources join the workspace
  // gates, everything else (docs, the generated TS package, tooling) is not
  // built by Cortex checks. Manifests/lockfiles under core/ already failed
  // closed via isFullInfluence above.
  if (/^core\/crates\//.test(path)) return ["rustfmt", "clippy", "test:rust"];
  if (/^core\//.test(path)) return [];
  // manager-gpui is a standalone Cargo workspace (own [workspace] table), so
  // it stays out of the cortex `cargo fmt/clippy/test --workspace` sweep and
  // gets its own gate. Its Cargo.toml/Cargo.lock still fail closed via
  // isFullInfluence above.
  if (/^manager-gpui\//.test(path)) return ["manager-gpui"];
  return "full";
}

function orderedChecks(checks) {
  return CHECK_ORDER.filter((check) => checks.has(check));
}

export function createPlan({ mode = "worktree", full = false, files } = {}) {
  if (full) return failClosed(mode, "explicit full override");
  const discovered = files
    ? {
        files: files.map((file) => (file?.path ? file : { path: file, status: "A" })),
      }
    : filesForMode(mode);
  if (discovered.full) return failClosed(mode, discovered.full);
  const changed = discovered.files;
  if (changed.length === 0)
    return {
      schemaVersion: 1,
      mode,
      full: false,
      changed: [],
      checks: [],
      reasons: ["no changes"],
    };

  const checks = new Set();
  const reasons = [];
  for (const file of changed) {
    const result = classify(file);
    if (result === "full") {
      reasons.push(`${file.path} requires the full check`);
      return failClosed(mode, reasons.at(-1), changed);
    }
    for (const check of result) checks.add(check);
  }
  if (checks.size) reasons.push("selected checks cover each changed component");
  else reasons.push("documentation and isolated assets only");
  const ordered = orderedChecks(checks);
  return {
    schemaVersion: 1,
    mode,
    full: false,
    changed: changed.map((file) => file.path),
    checks: ordered,
    reasons,
  };
}

function parseArgs(argv) {
  const options = { mode: "worktree", files: null, run: false, full: false };
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index];
    if (arg === "--full") options.full = true;
    else if (arg === "--run") options.run = true;
    else if (arg === "--mode") options.mode = argv[++index];
    else if (arg.startsWith("--mode=")) options.mode = arg.slice(7);
    else if (arg === "--files") {
      options.files = [];
      while (argv[index + 1] && !argv[index + 1].startsWith("--"))
        options.files.push(argv[++index]);
    }
  }
  return options;
}

function main() {
  const options = parseArgs(process.argv.slice(2));
  const plan = createPlan(options);
  process.stdout.write(`${JSON.stringify(plan)}\n`);
  process.stderr.write(
    `changed → ${plan.changed.length ? plan.changed.join(", ") : "none"}\nchecks → ${plan.checks.join(", ") || "none"}\nreasons → ${plan.reasons.join("; ")}\n`,
  );
  if (options.run) process.exitCode = executePlan(plan);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
