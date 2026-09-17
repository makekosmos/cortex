import { spawnSync } from "node:child_process";

const SOURCE_EXTENSIONS = /\.(?:[cm]?[jt]sx?|vue)$/i;
const FORMAT_EXTENSIONS = /\.(?:[cm]?[jt]sx?|vue|json|ya?ml)$/i;

const COMMANDS_BY_CHECK = {
  "desktop-typecheck": ["bun", ["run", "typecheck:desktop"]],
  "manager-typecheck": ["bun", ["run", "typecheck:manager"]],
  "host-typecheck": ["bun", ["run", "typecheck:host"]],
  "desktop-contracts": ["bun", ["run", "test:desktop-contracts"]],
  "host-contracts": ["bun", ["run", "test:host-contracts"]],
  "first-party-contracts": ["bun", ["run", "test:first-party-contracts"]],
  rustfmt: ["bun", ["run", "rustfmt"]],
  clippy: ["bun", ["run", "clippy"]],
  "test:rust": ["bun", ["run", "test:rust"]],
  "runtime-staging": ["bun", ["run", "--cwd", "desktop", "test:runtime-staging"]],
  "native-services": [
    "cargo",
    [
      "build",
      "--locked",
      "-p",
      "kepler-watcher",
      "-p",
      "kepler-focus-helper",
      "-p",
      "kepler-focus-svc",
      "--bins",
    ],
  ],
};

const FULL_CONTRACT_CHECKS = ["host-contracts", "first-party-contracts"];

function commandFor(check) {
  const [command, args] = COMMANDS_BY_CHECK[check];
  return { name: check, command, args };
}

function commandsFor(plan) {
  if (plan.full)
    return [
      { name: "full", command: "bun", args: ["run", "check"] },
      ...FULL_CONTRACT_CHECKS.map(commandFor),
    ];
  const commands = [];
  if (plan.mode === "pre-commit")
    commands.push({ name: "source-size", command: "bun", args: ["run", "check:source-size"] });
  const files = plan.changed.filter((path) => SOURCE_EXTENSIONS.test(path));
  const formatFiles = plan.changed.filter((path) => FORMAT_EXTENSIONS.test(path));
  for (const check of plan.checks) {
    if (check === "lint" && files.length)
      commands.push({ name: check, command: "bunx", args: ["oxlint", ...files] });
    else if (check === "format" && formatFiles.length)
      commands.push({ name: check, command: "bunx", args: ["oxfmt", "--check", ...formatFiles] });
    else if (COMMANDS_BY_CHECK[check]) commands.push(commandFor(check));
  }
  return commands;
}

function runCommand(command) {
  const result = spawnSync(command.command, command.args, {
    cwd: process.cwd(),
    encoding: "utf8",
    stdio: ["inherit", "pipe", "pipe"],
  });
  if (result.stdout) process.stderr.write(result.stdout);
  if (result.stderr) process.stderr.write(result.stderr);
  return result.error || result.status === null ? 1 : result.status;
}

export function executePlan(plan, runner = runCommand) {
  let failed = false;
  for (const command of commandsFor(plan)) if (runner(command) !== 0) failed = true;
  return failed ? 1 : 0;
}
