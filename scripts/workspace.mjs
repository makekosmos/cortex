#!/usr/bin/env node
import { spawn } from "node:child_process";
import { lstat, mkdir, readFile, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import {
  bridge,
  checkoutDirty,
  cloneExact,
  hash,
  inspectCore,
  inspectDependency,
  inspectTools,
  outputHashes,
  planLocalBootstrap,
  prepareCheckout,
  readStamp,
  writeStamp,
} from "./workspace-inspect.mjs";
import { NAMES, loadWorkspace, resolveMode, resolveWorkspacePaths } from "./workspace-config.mjs";
import { gitEnv } from "./git-env.mjs";
import { flag } from "./argv.mjs";
export {
  cloneExact,
  checkoutDirty,
  hash,
  inspectCore,
  inspectDependency,
  inspectTools,
  loadWorkspace,
  outputHashes,
  prepareCheckout,
  readStamp,
  resolveMode,
  resolveWorkspacePaths,
  writeStamp,
};
const fail = (message) => {
  throw new Error(message);
};
function run(exe, args, cwd, env = process.env) {
  const command = process.platform === "win32" && exe === "pnpm" ? "pnpm.cmd" : exe;
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd,
      env,
      shell: command.endsWith(".cmd"),
      windowsHide: true,
    });
    let stdout = "",
      stderr = "";
    child.stdout?.on("data", (data) => (stdout += data));
    child.stderr?.on("data", (data) => (stderr += data));
    child.on("error", (error) => resolve({ status: null, stdout, stderr, error }));
    child.on("close", (status) =>
      resolve({ status, stdout: stdout.trim(), stderr: stderr.trim() }),
    );
  });
}
async function git(cwd, args, allowFailure = false) {
  const result = await run("git", args, cwd, gitEnv());
  if (result.error) throw result.error;
  if (result.status !== 0 && !allowFailure)
    fail(`git ${args.join(" ")} failed: ${result.stderr || result.stdout}`);
  return result;
}
async function exists(file) {
  try {
    await lstat(file);
    return true;
  } catch {
    return false;
  }
}
export async function doctor(root, { env = process.env } = {}) {
  const started = performance.now(),
    workspace = await loadWorkspace(root),
    mode = resolveMode(env, root),
    stamp = mode.name === "pinned" ? ((await readStamp(root)) ?? { components: {} }) : null,
    dependencies = {},
    issues = [];
  for (const name of NAMES)
    dependencies[name] = await inspectDependency(
      root,
      name,
      mode.paths[name],
      workspace.pins[name],
      stamp,
    );
  if (mode.name === "local") {
    for (const name of NAMES) {
      dependencies[name].bridge = await bridge(mode.paths[name], mode.sources[name]);
      if (!dependencies[name].bridge.ok)
        issues.push(`${name}: bridge-${dependencies[name].bridge.status}`);
    }
  }
  const core = await inspectCore(root),
    toolchain = await inspectTools(root, workspace.packageJson);
  for (const dependency of Object.values(dependencies))
    if (dependency.status !== "ok") issues.push(`${dependency.name}: ${dependency.status}`);
  for (const [name, ok] of Object.entries(core.checks)) if (!ok) issues.push(`core: ${name}`);
  for (const [name, ok] of Object.entries(toolchain.checks)) if (!ok) issues.push(`tool: ${name}`);
  return {
    schema_version: 1,
    command: "doctor",
    ok: issues.length === 0,
    mode: mode.name,
    pins: workspace.pins,
    components: { core, ...dependencies, tools: toolchain.tools },
    tool_checks: toolchain.checks,
    issues,
    duration_ms: Math.round(performance.now() - started),
  };
}
export async function planBootstrap(
  root,
  { dryRun = false, env = process.env, prepare = true, runTool = run } = {},
) {
  const workspace = await loadWorkspace(root),
    mode = resolveMode(env, root),
    actions = [],
    loadedStamp = await readStamp(root),
    stamp = loadedStamp?.components ? loadedStamp : { schema_version: 1, components: {} };
  if (mode.name === "local") return planLocalBootstrap(mode, NAMES, dryRun);
  for (const name of NAMES) {
    const checkout = mode.paths[name],
      pin = workspace.pins[name];
    if (!(await exists(checkout))) {
      actions.push(
        dryRun
          ? {
              name,
              path: checkout,
              action: "clone",
              commit: pin.commit,
              steps: prepare ? ["install", "build"] : [],
            }
          : await cloneExact(root, name, pin, checkout, runTool, stamp),
      );
      if (!dryRun && prepare) await writeStamp(root, name, stamp.components[name]);
      continue;
    }
    if (!(await exists(path.join(checkout, ".git")))) {
      actions.push({ name, path: checkout, action: "blocked-existing", commit: pin.commit });
      continue;
    }
    const head = (await git(checkout, ["rev-parse", "HEAD"], true)).stdout,
      dirty = await checkoutDirty(
        checkout,
        JSON.parse(await readFile(path.join(checkout, "package.json"), "utf8")),
      );
    if (dirty && head !== pin.commit) {
      actions.push({ name, path: checkout, action: "blocked-dirty", head, commit: pin.commit });
      continue;
    }
    if (head === pin.commit) {
      const inspected = prepare ? await inspectDependency(root, name, checkout, pin, stamp) : null;
      if (prepare && !dirty && inspected?.status !== "ok") {
        if (dryRun) {
          actions.push({
            name,
            path: checkout,
            action: "prepare",
            commit: pin.commit,
            steps: ["install", "build"],
          });
          continue;
        }
        stamp.components[name] = await prepareCheckout(name, pin, checkout, runTool);
        await writeStamp(root, name, stamp.components[name]);
        actions.push({
          name,
          path: checkout,
          action: "prepare",
          commit: pin.commit,
          mutated: true,
        });
        continue;
      }
      actions.push({
        name,
        path: checkout,
        action: dirty ? "dirty-noop" : "noop",
        commit: pin.commit,
      });
      continue;
    }
    if (dryRun) {
      actions.push({
        name,
        path: checkout,
        action: "checkout",
        head,
        commit: pin.commit,
        steps: prepare ? ["install", "build"] : [],
      });
      continue;
    }
    const fetched = await git(checkout, ["fetch", "origin", pin.commit], true);
    if (fetched.status !== 0) {
      actions.push({ name, path: checkout, action: "failed-fetch", head, commit: pin.commit });
      continue;
    }
    await git(checkout, ["checkout", "--detach", pin.commit]);
    if (prepare) {
      stamp.components[name] = await prepareCheckout(name, pin, checkout, runTool);
      await writeStamp(root, name, stamp.components[name]);
    }
    actions.push({
      name,
      path: checkout,
      action: "checkout",
      head,
      commit: pin.commit,
      mutated: true,
    });
  }
  return {
    command: "bootstrap",
    mode: "pinned",
    dryRun,
    mutated: actions.some((action) => action.mutated),
    actions,
  };
}
export async function writeCiOutput(root, file) {
  const { pins } = await loadWorkspace(root),
    values = {
      imago_commit: pins.imago.commit,
      imago_version: pins.imago.package.version,
      imago_repository: pins.imago.repository,
      imago_path: ".tmp/workspace/imago",
      arca_sdk_commit: pins["arca-sdk"].commit,
      arca_sdk_version: pins["arca-sdk"].package.version,
      arca_sdk_repository: pins["arca-sdk"].repository,
      arca_sdk_path: ".tmp/workspace/arca-sdk",
    };
  await mkdir(path.dirname(path.resolve(file)), { recursive: true });
  await writeFile(
    file,
    `${Object.entries(values)
      .map(([key, value]) => `${key}=${value}`)
      .join(os.EOL)}${os.EOL}`,
  );
  return values;
}
async function main() {
  const args = process.argv.slice(2),
    name = args[0] && !args[0].startsWith("-") ? args.shift() : "doctor",
    root = path.resolve(import.meta.dirname, ".."),
    machine = args.includes("--ci") || args.includes("--json");
  let result;
  if (name === "doctor") result = await doctor(root);
  else if (name === "bootstrap")
    result = await planBootstrap(root, { dryRun: args.includes("--dry-run") });
  else if (name === "dry-run") result = await planBootstrap(root, { dryRun: true });
  else if (name === "ci") {
    const output =
        flag(args, "--github-output") ||
        args.find((arg) => arg.startsWith("--github-output="))?.slice("--github-output=".length),
      pins = output ? await writeCiOutput(root, output) : (await loadWorkspace(root)).pins;
    result = { schema_version: 1, command: "ci", ok: true, pins, duration_ms: 0 };
  } else fail(`unknown workspace command: ${name}`);
  if (machine || name === "ci") console.log(JSON.stringify(result, null, machine ? 0 : 2));
  else if (name === "doctor")
    console.log(
      `${result.ok ? "PASS" : "FAIL"} workspace doctor (${result.mode}) ${result.duration_ms}ms\n${result.issues.map((issue) => `- ${issue}`).join("\n")}`,
    );
  else
    console.log(
      result.actions
        .map(({ name: item, action, path: checkout }) => `${action}: ${item} (${checkout})`)
        .join("\n"),
    );
  if (
    result.ok === false ||
    result.actions?.some(
      ({ action }) => action.startsWith("blocked") || action.startsWith("failed"),
    )
  )
    process.exitCode = 1;
}
if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === path.resolve(fileURLToPath(import.meta.url))
)
  main().catch((error) => {
    console.error(`workspace: ${error.message}`);
    process.exitCode = 1;
  });
