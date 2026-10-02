import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { coveredByCache, diskTree, recordPass } from "./check-plan-cache.mjs";
import { commitPlan } from "./check-plan-commit.mjs";
import { executePlan } from "./check-plan-commands.mjs";
import { filesForMode, git } from "./check-plan-files.mjs";
import { readRevision } from "./check-plan-git.mjs";
import { DOC_CONTRACTS, isScriptsManifest, manifestChecks } from "./check-plan-manifest.mjs";

export { executePlan } from "./check-plan-commands.mjs";
export { parseNameStatus } from "./check-plan-git.mjs";
export { parsePushInput } from "./check-plan-files.mjs";

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
const PROSE_EXTENSIONS = /\.(?:md|mdx|txt)$/i;
// .txt files that are build inputs rather than prose.
const TXT_BUILD_INPUTS = /(?:^|\/)(?:CMakeLists|requirements[^/]*)\.txt$/i;

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
  // A push union can give each file its own merge base and tip.
  const revs = file.revisions ?? revisions;
  const read = (side) =>
    file[side] ?? (revs ? readRevision(git, readDisk, revs[side], file.path) : null);
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
      trees: discovered.trees ?? [],
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
    // The trees whose checks a pass would certify. Only pre-push sets them —
    // one per pushed commit — and the disk-keyed cache may only hit when every
    // pushed tree is the tree the checks ran on.
    trees: discovered.trees ?? [],
  };
}

function parseArgs(argv) {
  const options = { mode: "worktree", files: null, run: false, full: false, noCache: false };
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index];
    if (arg === "--full") options.full = true;
    else if (arg === "--run") options.run = true;
    else if (arg === "--no-cache") options.noCache = true;
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
// again; a pre-push plan certifies the pushed commits' trees, so the disk
// cache applies only when every pushed tree is the disk tree. Results are
// recorded only when the checks left the tree unchanged — a file that appears
// mid-run (including during the 10+ minute `pnpm run check`, which is this
// same path via `--full --run`) means the entry certifies nothing and is not
// written. `--no-cache` and CHECK_PLAN_NO_CACHE=1 skip consulting the cache;
// recording still happens, because a verified pass is a fact either way.
export function runPlan(plan, options = {}, runner) {
  const cwd = process.cwd();
  const tree = diskTree(cwd);
  const consult =
    !options.noCache &&
    process.env.CHECK_PLAN_NO_CACHE !== "1" &&
    tree &&
    plan.checks.length &&
    (plan.trees ?? []).every((pushed) => pushed === tree);
  if (consult && coveredByCache(cwd, tree, plan.checks)) {
    process.stderr.write(`cache → ${plan.checks.join(", ")} already passed on tree ${tree}\n`);
    return 0;
  }
  const status = executePlan(plan, runner);
  if (status === 0 && tree && diskTree(cwd) === tree) recordPass(cwd, tree, plan.checks);
  return status;
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) main();
