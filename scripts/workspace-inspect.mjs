import { spawn } from "node:child_process";
import { createHash } from "node:crypto";
import { lstat, mkdir, readFile, realpath, rm, stat, symlink, writeFile } from "node:fs/promises";
import path from "node:path";
const STAMP = ".tmp/workspace/prepared.json";
function run(exe, args, cwd) {
  return new Promise((resolve) => {
    const child = spawn(exe, args, { cwd, shell: false, windowsHide: true });
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
async function git(cwd, args) {
  return run("git", args, cwd);
}
async function exists(file) {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
}
export async function bridge(managed, source) {
  const result = { path: managed, target: source, ok: false };
  if (!(await exists(source))) return { ...result, status: "source-missing" };
  let info;
  try {
    info = await lstat(managed);
  } catch {
    return { ...result, status: "missing" };
  }
  if (!info.isSymbolicLink()) return { ...result, status: "blocked-existing" };
  try {
    result.target = await realpath(managed);
    result.ok = result.target === (await realpath(source));
  } catch {
    return { ...result, status: "wrong-target" };
  }
  result.status = result.ok ? "ok" : "wrong-target";
  return result;
}
export async function planLocalBootstrap(mode, names, dryRun) {
  const actions = await Promise.all(
    names.map(async (name) => {
      const managed = mode.paths[name],
        source = mode.sources[name],
        current = await bridge(managed, source);
      if (current.status === "source-missing")
        return { name, path: managed, source, action: "blocked-source" };
      if (current.status === "missing") {
        if (dryRun) return { name, path: managed, source, action: "link" };
        await mkdir(path.dirname(managed), { recursive: true });
        await symlink(source, managed, process.platform === "win32" ? "junction" : "dir");
        return { name, path: managed, source, action: "link", mutated: true };
      }
      if (current.status !== "ok")
        return {
          name,
          path: managed,
          source,
          action: current.status === "wrong-target" ? "blocked-link" : current.status,
        };
      return { name, path: managed, source, action: "noop" };
    }),
  );
  return {
    command: "bootstrap",
    mode: "local",
    dryRun,
    mutated: actions.some((action) => action.mutated),
    actions,
  };
}
export async function readStamp(root) {
  try {
    return JSON.parse(await readFile(path.join(root, STAMP), "utf8"));
  } catch {
    return null;
  }
}
export async function writeStamp(root, name, entry) {
  const file = path.join(root, STAMP),
    stamp = (await readStamp(root)) ?? { schema_version: 1, components: {} };
  stamp.components[name] = entry;
  await mkdir(path.dirname(file), { recursive: true });
  await writeFile(file, `${JSON.stringify(stamp, null, 2)}\n`);
}
export async function prepareCheckout(name, pin, checkout, runner = run) {
  const packageFile = path.join(checkout, "package.json"),
    packageJson = JSON.parse(await readFile(packageFile, "utf8"));
  if (packageJson.name !== pin.package.name || packageJson.version !== pin.package.version)
    throw new Error(`${name} package metadata does not match package.json pin`);
  const [manager, managerVersion] = String(packageJson.packageManager ?? "").split("@"),
    lockName = manager === "pnpm" ? "pnpm-lock.yaml" : manager === "bun" ? "bun.lock" : null,
    lockFile = path.join(checkout, lockName ?? "");
  if (!lockName || !(await exists(lockFile)))
    throw new Error(`${name} has no supported pinned package manager or lockfile`);
  const version = await runner(manager, ["--version"], checkout);
  if (version.error) throw version.error;
  if (
    version.status !== 0 ||
    !/^\d+\.\d+\.\d+$/.test(managerVersion) ||
    version.stdout.trim().replace(/^v/, "") !== managerVersion
  )
    throw new Error(`${name} requires ${manager}@${managerVersion}`);
  for (const args of [
    ["install", "--frozen-lockfile"],
    ["run", "build"],
  ]) {
    const result = await runner(manager, args, checkout);
    if (result.error) throw result.error;
    if (result.status !== 0)
      throw new Error(
        `${manager} ${args.join(" ")} failed for ${name}: ${result.stderr || result.stdout}`,
      );
  }
  const outputs = await outputHashes(checkout, packageJson);
  if (!outputs) throw new Error(`${name} build did not produce required exports`);
  return {
    head: (await git(checkout, ["rev-parse", "HEAD"])).stdout,
    package_sha256: await hash(packageFile),
    lock_sha256: await hash(lockFile),
    outputs,
  };
}
export async function cloneExact(root, name, pin, checkout, runner, stamp, gitRunner = git) {
  let created = false;
  try {
    await mkdir(path.dirname(checkout), { recursive: true });
    created = true;
    const clone = await gitRunner(root, [
      "clone",
      `https://github.com/${pin.repository}.git`,
      checkout,
    ]);
    if (clone.status !== 0)
      throw new Error(`clone ${name} failed: ${clone.stderr || clone.stdout}`);
    const fetched = await gitRunner(checkout, ["fetch", "origin", pin.commit]);
    if (fetched.status !== 0)
      throw new Error(`fetch ${name} failed: ${fetched.stderr || fetched.stdout}`);
    const checked = await gitRunner(checkout, ["checkout", "--detach", pin.commit]);
    if (checked.status !== 0)
      throw new Error(`checkout ${name} failed: ${checked.stderr || checked.stdout}`);
    const entry = await prepareCheckout(name, pin, checkout, runner);
    stamp.components[name] = entry;
    return { name, path: checkout, action: "clone", commit: pin.commit, mutated: true };
  } catch (error) {
    if (created) await rm(checkout, { recursive: true, force: true });
    throw error;
  }
}
export async function hash(file) {
  return createHash("sha256")
    .update(await readFile(file))
    .digest("hex");
}
export async function checkoutDirty(checkout, packageJson) {
  const status = await git(checkout, ["status", "--porcelain", "--untracked-files=all"]);
  if (status.status !== 0) return true;
  const generated = new Set(
    ["node_modules", ...outputPaths(packageJson)].map(path.posix.normalize),
  );
  return status.stdout.split(/\r?\n/).some((line) => {
    if (!line || !line.startsWith("?? ")) return Boolean(line);
    const file = line.slice(3).replaceAll("\\", "/").replace(/^\.\//, "");
    return ![...generated].some((path) => file === path || file.startsWith(`${path}/`));
  });
}
function outputPaths(packageJson) {
  const files = new Set([packageJson.main || "dist/index.js"]);
  if (String(packageJson.types) === packageJson.types) files.add(packageJson.types);
  if (packageJson.name === "@makekosmos/visuals") files.add("dist/index.css");
  return files;
}
export async function outputHashes(checkout, packageJson) {
  const outputs = {};
  for (const relative of outputPaths(packageJson)) {
    const file = path.join(checkout, relative);
    if (!(await exists(file))) return null;
    outputs[relative.replaceAll("\\", "/")] = await hash(file);
  }
  return outputs;
}
export async function inspectDependency(root, name, checkout, pin, stamp) {
  const result = {
    name,
    source: pin.repository,
    path: checkout,
    head: null,
    version: null,
    dirty: false,
    checks: {},
  };
  if (!(await exists(checkout))) {
    result.status = "missing";
    result.checks.exists = false;
    return result;
  }
  result.checks.exists = true;
  if (!(await exists(path.join(checkout, ".git")))) {
    result.status = "not-a-repository";
    return result;
  }
  const head = await git(checkout, ["rev-parse", "HEAD"]);
  result.head = head.status === 0 ? head.stdout : null;
  result.checks.pin = result.head === pin.commit;
  try {
    const packageFile = path.join(checkout, "package.json"),
      packageJson = JSON.parse(await readFile(packageFile, "utf8"));
    result.dirty = await checkoutDirty(checkout, packageJson);
    result.checks.clean = !result.dirty;
    result.version = packageJson.version ?? null;
    result.package = {
      name: packageJson.name,
      version: packageJson.version,
      integrity: `git:${result.head}`,
    };
    result.checks.package =
      packageJson.name === pin.package.name && packageJson.version === pin.package.version;
    const lockName = String(packageJson.packageManager ?? "").startsWith("pnpm")
        ? "pnpm-lock.yaml"
        : "bun.lock",
      lockFile = path.join(checkout, lockName),
      outputs = await outputHashes(checkout, packageJson),
      entry = stamp?.components?.[name];
    result.checks.exports = outputs !== null;
    if (stamp)
      result.checks.stamp = Boolean(
        entry &&
        entry.head === result.head &&
        entry.package_sha256 === (await hash(packageFile)) &&
        (await exists(lockFile)) &&
        entry.lock_sha256 === (await hash(lockFile)) &&
        JSON.stringify(entry.outputs) === JSON.stringify(outputs),
      );
  } catch {
    result.dirty = true;
    result.checks.clean = false;
    result.checks.package = false;
    if (stamp) result.checks.stamp = false;
  }
  result.status = Object.values(result.checks).every(Boolean) ? "ok" : "mismatch";
  return result;
}
export async function inspectCore(root) {
  const cargo = await readFile(path.join(root, "runtime", "Cargo.toml"), "utf8"),
    sidecar = await readFile(path.join(root, "desktop", "scripts", "ark-core-rpc.mjs"), "utf8"),
    lock = await readFile(path.join(root, "Cargo.lock"), "utf8");
  const head = cargo.match(/makekosmos\/core\.git", rev = "([0-9a-f]{40})/)?.[1],
    sidecarHead = sidecar.match(/ARK_CORE_REVISION\s*=\s*"([0-9a-f]{40})"/)?.[1];
  return {
    source: "https://github.com/makekosmos/core.git",
    path: "runtime/Cargo.toml",
    head,
    version: lock.match(/name = "ark-core"\s+version = "([^"]+)"/)?.[1] ?? null,
    checks: {
      cargoPin: Boolean(head),
      sidecarPin: head === sidecarHead,
      lockPin: Boolean(head && lock.includes(head)),
    },
  };
}
export async function inspectTools(root, packageJson) {
  const workflow = await readFile(path.join(root, ".github", "workflows", "core-pin.yml"), "utf8"),
    expected = {
      bun: String(packageJson.packageManager ?? "").match(/^bun@(\d+\.\d+\.\d+)$/)?.[1],
      node: workflow.match(/node-version:\s*(\d+\.\d+\.\d+)/)?.[1],
      rust: workflow.match(/rustup toolchain install\s+(\d+\.\d+\.\d+)/)?.[1],
    },
    tools = {};
  for (const [name, exe] of Object.entries({ bun: "bun", node: "node", rust: "rustc" })) {
    const result = await run(exe, ["--version"], root),
      located = await run(process.platform === "win32" ? "where" : "which", [exe], root),
      version =
        result.status === 0 ? result.stdout.replace(/^v/, "").match(/\d+\.\d+\.\d+/)?.[0] : null;
    tools[name] = {
      source: exe,
      path: located.status === 0 ? located.stdout.split(/\r?\n/)[0] : exe,
      head: null,
      version,
      expected: expected[name],
    };
  }
  return {
    tools,
    checks: Object.fromEntries(
      Object.keys(expected).map((name) => [
        name,
        Boolean(expected[name] && tools[name].version === expected[name]),
      ]),
    ),
  };
}
