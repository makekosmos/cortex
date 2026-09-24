import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { existsSync, writeFileSync } from "node:fs";
import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const root = fileURLToPath(new URL("../", import.meta.url));

test("executor commands never require Bun or a shell-resolved shim", async () => {
  const { createPlan, executePlan } = await import("./check-plan.mjs");
  const seen = [];
  for (const plan of [
    createPlan({ mode: "worktree", full: true }),
    createPlan({ mode: "pre-commit", files: ["desktop/src/App.vue"] }),
    createPlan({ mode: "worktree", files: ["host/electron/main.ts"] }),
    createPlan({ mode: "worktree", files: ["runtime/src/lib.rs"] }),
    createPlan({ mode: "worktree", files: ["native-services/src/main.rs"] }),
  ])
    executePlan(plan, (command) => (seen.push(command), 0));
  assert.ok(seen.length > 0);
  for (const command of seen) {
    assert.doesNotMatch(command.command, /bunx?/i, command.name);
    assert.doesNotMatch(command.command, /\.(?:cmd|bat)$/i, command.name);
    assert.ok(
      ["pnpm", "cargo", process.execPath].includes(command.command),
      `${command.name} uses unexpected runner ${command.command}`,
    );
  }
});

test("selective checks emit pnpm run commands and Rust stays on cargo", async () => {
  const { createPlan, executePlan } = await import("./check-plan.mjs");
  const run = (files) => {
    const seen = [];
    executePlan(createPlan({ mode: "worktree", files }), (command) =>
      seen.push(`${command.command} ${command.args.join(" ")}`),
    );
    return seen;
  };
  assert.deepEqual(run(["desktop/src/App.vue"])[0], "pnpm run typecheck:desktop");
  assert.deepEqual(run(["host/electron/main.ts"]), [
    "pnpm run typecheck:host",
    "pnpm run test:host-contracts",
  ]);
  assert.deepEqual(run(["runtime/src/lib.rs"]), [
    "pnpm run rustfmt",
    "pnpm run clippy",
    "pnpm run test:rust",
    "pnpm --dir desktop run test:runtime-staging",
  ]);
  assert.deepEqual(run(["native-services/src/main.rs"]), [
    "cargo build --locked -p kepler-watcher -p kepler-focus-helper -p kepler-focus-svc --bins",
  ]);
});

test("pre-commit emits the source-size safeguard through pnpm", async () => {
  const { executePlan } = await import("./check-plan.mjs");
  const seen = [];
  executePlan(
    { mode: "pre-commit", full: false, checks: [], changed: [], reasons: [] },
    (command) => (seen.push(`${command.command} ${command.args.join(" ")}`), 0),
  );
  assert.deepEqual(seen, ["pnpm run check:source-size"]);
});

test("lint and format run Node tool entrypoints with verbatim file arguments", async () => {
  const { createPlan, executePlan } = await import("./check-plan.mjs");
  const seen = [];
  executePlan(
    createPlan({ mode: "worktree", files: ["desktop/src/with space/App.vue"] }),
    (command) => (seen.push(command), 0),
  );
  const lint = seen.find((command) => command.name === "lint");
  const format = seen.find((command) => command.name === "format");
  assert.equal(lint.command, process.execPath);
  assert.match(lint.args[0], /bin[/\\]oxlint$/);
  assert.ok(existsSync(lint.args[0]), lint.args[0]);
  assert.deepEqual(lint.args.slice(1), ["desktop/src/with space/App.vue"]);
  assert.equal(format.command, process.execPath);
  assert.match(format.args[0], /bin[/\\]oxfmt$/);
  assert.ok(existsSync(format.args[0]), format.args[0]);
  assert.deepEqual(format.args.slice(1), ["--check", "desktop/src/with space/App.vue"]);
});

test("spawn failures are written to stderr and return non-zero", async () => {
  const { runCommand } = await import("./check-plan-commands.mjs");
  const writes = [];
  const original = process.stderr.write;
  process.stderr.write = (chunk) => (writes.push(String(chunk)), true);
  let status;
  try {
    status = runCommand({ name: "missing-tool", command: "cortex-missing-tool-kos91", args: [] });
  } finally {
    process.stderr.write = original;
  }
  assert.equal(status, 1);
  assert.match(writes.join(""), /missing-tool:.*ENOENT/i);
});

test("pnpm commands prefer the npm_execpath Node entrypoint when present", async () => {
  const { runCommand } = await import("./check-plan-commands.mjs");
  const dir = await mkdtemp(path.join(os.tmpdir(), "pnpm-entry-"));
  const entrypoint = path.join(dir, "pnpm.cjs");
  writeFileSync(entrypoint, "process.exit(3);\n");
  const previous = process.env.npm_execpath;
  process.env.npm_execpath = entrypoint;
  let status;
  try {
    status = runCommand({ name: "probe", command: "pnpm", args: ["run", "probe"] });
  } finally {
    if (previous === undefined) delete process.env.npm_execpath;
    else process.env.npm_execpath = previous;
  }
  assert.equal(status, 3);
});

test("the executor runs through pnpm and Node with Bun absent from PATH", () => {
  const bunFree = process.env.PATH.split(path.delimiter).filter(
    (dir) =>
      !existsSync(path.join(dir, "bun")) &&
      !existsSync(path.join(dir, "bun.exe")) &&
      !existsSync(path.join(dir, "bunx")) &&
      !existsSync(path.join(dir, "bunx.exe")),
  );
  const env = { ...process.env, PATH: bunFree.join(path.delimiter) };
  delete env.npm_execpath;
  delete env.npm_node_execpath;
  const moduleUrl = new URL("./check-plan-commands.mjs", import.meta.url).href;
  const result = spawnSync(
    process.execPath,
    [
      "--input-type=module",
      "--eval",
      `import { executePlan } from ${JSON.stringify(moduleUrl)};\n` +
        "const plan = { mode: 'pre-commit', full: false, changed: ['scripts/check-plan.mjs'], checks: ['format'], reasons: [] };\n" +
        "process.exit(executePlan(plan));",
    ],
    { cwd: root, env, encoding: "utf8" },
  );
  assert.equal(result.status, 0, result.stderr);
});
