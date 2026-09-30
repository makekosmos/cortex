import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

const SOURCE_EXTENSIONS = /\.(?:[cm]?[jt]sx?|vue)$/i;
const FORMAT_EXTENSIONS = /\.(?:[cm]?[jt]sx?|vue|json|ya?ml)$/i;
const requireFromRoot = createRequire(new URL("../package.json", import.meta.url));

const COMMANDS_BY_CHECK = {
  fast: ["pnpm", ["run", "check:fast"]],
  "core-pin": ["pnpm", ["run", "check:core-pin"]],
  "package-manager": ["pnpm", ["run", "test:package-manager"]],
  "release-bom": ["pnpm", ["run", "test:release-bom"]],
  "desktop-contracts": ["pnpm", ["run", "test:desktop-contracts"]],
  rustfmt: ["pnpm", ["run", "rustfmt"]],
  clippy: ["pnpm", ["run", "clippy"]],
  "test:rust": ["pnpm", ["run", "test:rust"]],
  "runtime-staging": ["pnpm", ["--dir", "desktop", "run", "test:runtime-staging"]],
  "manager-gpui": ["pnpm", ["run", "check:manager-gpui"]],
};

const FULL_CONTRACT_CHECKS = [];

function commandFor(check) {
  const [command, args] = COMMANDS_BY_CHECK[check];
  return { name: check, command, args };
}

function toolBin(name) {
  const manifest = requireFromRoot.resolve(`${name}/package.json`);
  const bin = JSON.parse(readFileSync(manifest, "utf8")).bin;
  const relative = bin[name] ?? bin;
  return path.join(path.dirname(manifest), relative);
}

function commandsFor(plan) {
  if (plan.full)
    return [
      { name: "full", command: "pnpm", args: ["run", "check"] },
      ...FULL_CONTRACT_CHECKS.map(commandFor),
    ];
  // The fast gate already includes the brand and source-size scans.
  if (plan.checks.includes("fast")) return [commandFor("fast")];
  const commands = [];
  // The brand-rename gate is a fast git-grep scan; a stray legacy name can
  // appear in any file, so it runs on every hook invocation.
  commands.push({ name: "brand", command: "pnpm", args: ["run", "check:brand"] });
  if (plan.mode === "pre-commit")
    commands.push({ name: "source-size", command: "pnpm", args: ["run", "check:source-size"] });
  const files = plan.changed.filter((path) => SOURCE_EXTENSIONS.test(path));
  const formatFiles = plan.changed.filter((path) => FORMAT_EXTENSIONS.test(path));
  for (const check of plan.checks) {
    if (check === "lint" && files.length)
      commands.push({
        name: check,
        command: process.execPath,
        // Changed files can all sit under oxlint ignore patterns (e.g.
        // desktop/build); that is "nothing to lint", not a failure.
        args: [toolBin("oxlint"), "--no-error-on-unmatched-pattern", ...files],
      });
    else if (check === "format" && formatFiles.length)
      commands.push({
        name: check,
        command: process.execPath,
        args: [toolBin("oxfmt"), "--check", ...formatFiles],
      });
    else if (COMMANDS_BY_CHECK[check]) commands.push(commandFor(check));
  }
  return commands;
}

// pnpm is spawned without a shell so argument vectors (including file paths)
// reach it verbatim. When check-plan itself runs under `pnpm run`, pnpm
// exports npm_execpath pointing at its own entrypoint: .cjs/.mjs/.js files go
// through the current Node, native binaries and .exe spawn directly. Without
// it we fall back to PATH — on Windows pnpm resolves to pnpm.cmd, which Node
// refuses to spawn without a shell (EINVAL, CVE-2024-27980), so that one
// literal-argument-only fallback goes through cmd.
function pnpmSpawn() {
  const entrypoint = process.env.npm_execpath;
  if (entrypoint && /pnpm/i.test(path.basename(entrypoint))) {
    if (/\.[cm]?js$/i.test(entrypoint)) return { file: process.execPath, prefix: [entrypoint] };
    if (process.platform !== "win32" || /\.exe$/i.test(entrypoint))
      return { file: entrypoint, prefix: [] };
  }
  return process.platform === "win32"
    ? { file: "pnpm.cmd", prefix: [], shell: true }
    : { file: "pnpm", prefix: [] };
}

// Git exports GIT_DIR/GIT_WORK_TREE and friends into hook processes. Inside a
// linked worktree that leaks the current worktree's gitdir into every check we
// spawn — e.g. host e2e fixtures shell out to `git` in sibling app checkouts
// and would silently operate on the wrong repository. Scrub git's hook env so
// each command re-discovers the repository from its own cwd.
const GIT_HOOK_ENV_KEYS = [
  "GIT_DIR",
  "GIT_WORK_TREE",
  "GIT_COMMON_DIR",
  "GIT_INDEX_FILE",
  "GIT_OBJECT_DIRECTORY",
  "GIT_ALTERNATE_OBJECT_DIRECTORIES",
  "GIT_CEILING_DIRECTORIES",
  "GIT_PREFIX",
];

export function checkEnv() {
  const env = { ...process.env };
  for (const key of GIT_HOOK_ENV_KEYS) delete env[key];
  return env;
}

export function runCommand(command) {
  const spawn = command.command === "pnpm" ? pnpmSpawn() : { file: command.command, prefix: [] };
  const result = spawnSync(spawn.file, [...spawn.prefix, ...command.args], {
    cwd: process.cwd(),
    env: checkEnv(),
    encoding: "utf8",
    stdio: ["inherit", "pipe", "pipe"],
    windowsHide: true,
    shell: spawn.shell === true,
  });
  if (result.error) process.stderr.write(`${command.name}: ${result.error.message}\n`);
  if (result.stdout) process.stderr.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  return result.error || result.status === null ? 1 : result.status;
}

export function executePlan(plan, runner = runCommand) {
  let failed = false;
  for (const command of commandsFor(plan)) if (runner(command) !== 0) failed = true;
  return failed ? 1 : 0;
}
