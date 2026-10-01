import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { coveredByCache, diskTree, recordPass } from "./check-plan-cache.mjs";
import { commitPlan } from "./check-plan-commit.mjs";
import { executePlan } from "./check-plan-commands.mjs";
import { gitEnv } from "./git-env.mjs";
import { parseNameStatus, readRevision } from "./check-plan-git.mjs";
import { DOC_CONTRACTS, isScriptsManifest, manifestChecks } from "./check-plan-manifest.mjs";

export { executePlan } from "./check-plan-commands.mjs";
export { parseNameStatus } from "./check-plan-git.mjs";

const CHECK_ORDER = [
  "brand",
  "test-skips",
  "core-pin",
  "package-manager",
  "release-bom",
  "desktop-contracts",
  "rustfmt",
  "clippy",
  "test:rust",
  "runtime-staging",
  "manager-gpui",
  "lint",
  "format",
];
const ASSET_EXTENSIONS = /\.(?:png|jpe?g|gif|svg|webp|ico|avif)$/i;
const ZERO_SHA = /^0{40}$/;
const PROSE_EXTENSIONS = /\.(?:md|mdx|txt)$/i;
// .txt files that are build inputs rather than prose.
const TXT_BUILD_INPUTS = /(?:^|\/)(?:CMakeLists|requirements[^/]*)\.txt$/i;

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
  if (result.error || result.status !== 0) return null;
  const bases = result.stdout.trim().split(/\r?\n/).filter(Boolean);
  return bases.length === 1 ? bases[0] : null;
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
    return files
      ? { files, revisions: { before: "HEAD", after: "index" } }
      : { full: "unable to inspect staged changes" };
  }
  if (mode === "worktree") {
    const files = diffFiles(["HEAD"]);
    const untracked = untrackedFiles();
    return files && untracked
      ? { files: [...files, ...untracked], revisions: { before: "HEAD", after: "worktree" } }
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
    const base = oneMergeBase(push.remoteSha, push.localSha);
    if (!base) return { full: "pre-push history has no unique merge base" };
    const files = diffFiles([`${push.remoteSha}...${push.localSha}`]);
    return files
      ? { files, revisions: { before: base, after: push.localSha } }
      : { full: "unable to inspect pre-push diff" };
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

// Both sides of a modified manifest, from the file record (createPlan callers)
// or from git. Anything but a readable in-place modification fails closed.
function manifestPlan(file, revisions) {
  if (file.status !== "M") return { full: `${file.path} is added, removed, or renamed` };
  const readDisk = (path) => readFileSync(path, "utf8");
  const read = (side) =>
    file[side] ?? (revisions ? readRevision(git, readDisk, revisions[side], file.path) : null);
  const before = read("before");
  const after = read("after");
  if (before === null || after === null) return { full: `${file.path} revisions are unavailable` };
  return manifestChecks(file.path, before, after);
}

function classify(file, revisions) {
  const path = file.path;
  if (Object.hasOwn(DOC_CONTRACTS, path)) return DOC_CONTRACTS[path];
  if (isScriptsManifest(path)) {
    const result = manifestPlan(file, revisions);
    return result.full ? result : result.checks;
  }
  // Prose under component trees is read by no check unless DOC_CONTRACTS
  // lists it; core/ docs keep falling through to the core/ rule below.
  if (/^(?:desktop|runtime|packages|shared)\//i.test(path) && PROSE_EXTENSIONS.test(path))
    return TXT_BUILD_INPUTS.test(path) ? "full" : [];
  if (isFullInfluence(path)) return "full";
  if (isDocumentation(path) || ASSET_EXTENSIONS.test(path)) return [];
  if (/^desktop\//.test(path)) return ["lint", "format"];
  if (/^runtime\//.test(path)) return ["rustfmt", "clippy", "test:rust", "runtime-staging"];
  // core/ is the vendored upstream subtree: crate sources join the workspace
  // gates, everything else (docs, tooling) is not built by Cortex checks.
  // Manifests/lockfiles under core/ already failed
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
    const result = classify(file, discovered.revisions);
    if (result === "full" || result.full) {
      const detail = result.full ? ` (${result.full})` : "";
      reasons.push(`${file.path} requires the full check${detail}`);
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
  // `--full` is an explicit request for the whole gate, even at commit time.
  const plan = options.full ? createPlan(options) : commitPlan(createPlan(options));
  process.stdout.write(`${JSON.stringify(plan)}\n`);
  process.stderr.write(
    `changed → ${plan.changed.length ? plan.changed.join(", ") : "none"}\nchecks → ${plan.checks.join(", ") || "none"}\nreasons → ${plan.reasons.join("; ")}\n`,
  );
  if (options.run) process.exitCode = runPlan(plan, options);
}

// A plan whose checks already passed on the identical on-disk tree is not run
// again. Results are recorded only when the checks left the tree unchanged.
// `--full` and CHECK_PLAN_NO_CACHE=1 always run.
function runPlan(plan, options) {
  const cwd = process.cwd();
  const useCache = !options.full && process.env.CHECK_PLAN_NO_CACHE !== "1";
  const tree = useCache ? diskTree(cwd) : null;
  if (tree && plan.checks.length && coveredByCache(cwd, tree, plan.checks)) {
    process.stderr.write(`cache → ${plan.checks.join(", ")} already passed on tree ${tree}\n`);
    return 0;
  }
  const status = executePlan(plan);
  if (status === 0 && tree && diskTree(cwd) === tree) recordPass(cwd, tree, plan.checks);
  return status;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
