#!/usr/bin/env node
// Reproducible Linux Engine smoke: ordered gates under isolated XDG roots;
// each gate reports PASS / FAIL / NOT_RUN and the run exits 0 only when all
// passed.
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";
import { flag } from "./argv.mjs";
import { gitEnv } from "./git-env.mjs";
import { loadWorkspace } from "./workspace-config.mjs";
import { planBootstrap } from "./workspace.mjs";

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const WORKSPACE_OUTPUTS = {
  imago: ["index.ts", "theme/css-variables.css", "dist/index.css"],
  "arca-sdk": ["dist/index.js"],
};

const arg = (name) => flag(process.argv, name);
const reportPath = path.resolve(repoRoot, arg("--report") ?? ".tmp/linux-smoke-report.md");

const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "mundus-linux-smoke-"));
const childEnv = {
  ...process.env,
  MUNDUS_HEADLESS: "1",
  XDG_CONFIG_HOME: path.join(runRoot, "xdg-config"),
  XDG_CACHE_HOME: path.join(runRoot, "xdg-cache"),
  XDG_DATA_HOME: path.join(runRoot, "xdg-data"),
};
const results = [],
  tools = {};
const git = (cwd, args) =>
  spawnSync("git", ["-C", cwd, ...args], { encoding: "utf8", env: gitEnv() });
const step = (cmd, args, opts = {}) => {
  console.log(`[linux-smoke] $ ${cmd} ${args.join(" ")}${opts.cwd ? `  (cwd ${opts.cwd})` : ""}`);
  return spawnSync(cmd, args, {
    cwd: repoRoot,
    env: childEnv,
    encoding: "utf8",
    maxBuffer: 64 * 1024 * 1024,
    timeout: 30 * 60 * 1000,
    ...opts,
  });
};
const tail = (text, lines = 12) => text.split(/\r?\n/).filter(Boolean).slice(-lines).join("\n");
const must = (result, label) => {
  if (!(result.error ?? result.status !== 0)) return result.stdout ?? "";
  const out = tail(`${result.stdout}\n${result.stderr}`);
  throw new Error(`${label}: ${result.error?.message ?? `exit ${result.status} ${out}`}`);
};
const runGate = async (gate, blockedBy, body) => {
  const started = Date.now(),
    blocker = blockedBy.find((name) => results.find((r) => r.gate === name)?.status !== "PASS");
  let status = blocker ? "NOT_RUN" : "PASS",
    detail = blocker ? `blocked by ${blocker}` : "";
  if (!blocker)
    try {
      detail = await body();
    } catch (error) {
      status = "FAIL";
      detail = error instanceof Error ? error.message : String(error);
    }
  results.push({ gate, status, detail, ms: Date.now() - started });
  console.log(`\n[linux-smoke] ${gate}: ${status}${detail ? ` — ${detail}` : ""}`);
};
await runGate("preflight", [], () => {
  if (process.platform !== "linux") throw new Error("Linux only");
  for (const tool of ["node", "pnpm", "cargo", "git", "bun"]) {
    if (step("which", [tool]).status !== 0) throw new Error(`missing tool: ${tool}`);
    tools[tool] = must(step(tool, ["--version"]), tool).split(/\r?\n/)[0];
  }
  return `node ${tools.node}, pnpm ${tools.pnpm}, cargo ${tools.cargo}, bun ${tools.bun}`;
});
await runGate("workspace-deps", ["preflight"], async () => {
  const { pins } = await loadWorkspace(repoRoot);
  // planBootstrap prepares only on an exact package-manager version match
  // (arca-sdk pins bun@1.3.14); fall back to manual clone + build otherwise.
  const boot = await planBootstrap(repoRoot)
    .then((r) => r.actions.map((a) => `${a.action}:${a.name}`).join(",") || "noop")
    .catch((error) => `partial: ${error.message}`);
  for (const name of Object.keys(WORKSPACE_OUTPUTS)) {
    const checkout = path.join(repoRoot, ".tmp", "workspace", name);
    if (!fs.existsSync(path.join(checkout, ".git"))) {
      const repo = pins[name].repository;
      must(
        git(path.dirname(checkout), ["clone", `https://github.com/${repo}.git`, name]),
        `${name} clone`,
      );
      must(git(checkout, ["fetch", "origin", pins[name].commit]), `${name} fetch`);
      must(git(checkout, ["checkout", "--detach", pins[name].commit]), `${name} checkout`);
    }
    const head = must(git(checkout, ["rev-parse", "HEAD"]), `${name} HEAD`).trim();
    if (head !== pins[name].commit) throw new Error(`${name} HEAD ${head} != pin`);
    const absent = WORKSPACE_OUTPUTS[name].filter(
      (file) => !fs.existsSync(path.join(checkout, file)),
    );
    if (!absent.length) continue;
    const manager = name === "imago" ? "pnpm" : "bun";
    if (step(manager, ["install", "--frozen-lockfile"], { cwd: checkout }).status !== 0)
      must(step(manager, ["install"], { cwd: checkout }), `${name} install`);
    must(step(manager, ["run", "build"], { cwd: checkout }), `${name} build`);
    const still = absent.filter((file) => !fs.existsSync(path.join(checkout, file)));
    if (still.length) throw new Error(`${name} outputs missing: ${still.join(",")}`);
  }
  return `${boot}; imago@${pins.imago.commit.slice(0, 7)} arca-sdk@${pins["arca-sdk"].commit.slice(0, 7)}`;
});
await runGate("engine-bootstrap", ["preflight"], async () => {
  const target = JSON.parse(
    must(step("cargo", ["metadata", "--no-deps", "--format-version=1"]), "cargo metadata"),
  ).target_directory;
  const debug = path.join(target, "debug");
  const engine = path.join(debug, "mundus-engine");
  must(step("cargo", ["build", "-p", "engine"]), "cargo build engine");
  const dataDir = path.join(runRoot, "engine-data");
  const engineEnv = {
    ...childEnv,
    MUNDUS_DATA_DIR: dataDir,
    MUNDUS_LOCK_PERMISSIONS_DISABLED: "1",
    MUNDUS_SKIP_SYNC: "1",
    MUNDUS_USAGE_TRACKER: "0",
  };
  const child = spawn(engine, [], { env: engineEnv, stdio: "ignore" });
  const readLock = () => {
    try {
      const lock = JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8"));
      return Number.isInteger(lock.pid) ? lock : undefined;
    } catch {
      return undefined;
    }
  };
  try {
    let lock,
      until = Date.now() + 30_000;
    while (Date.now() < until && !(lock = readLock()))
      await new Promise((resolve) => setTimeout(resolve, 200));
    if (!lock) throw new Error("timed out waiting for engine.lock.json");
    const health = await fetch(`http://127.0.0.1:${lock.http_port}/v1/health`, {
      headers: {
        Authorization: `Bearer ${lock.auth_token}`,
        "X-Kosmos-Client-Pid": String(process.pid),
        "X-Kosmos-Api-Version": "1.0.0",
      },
    });
    if (health.status !== 200) throw new Error(`/v1/health -> ${health.status}`);
  } finally {
    spawnSync(engine, ["--shutdown"], { env: engineEnv, timeout: 10_000 });
    for (let i = 0; i < 50 && child.exitCode === null; i++) spawnSync("sleep", ["0.1"]);
    if (child.exitCode === null) child.kill("SIGKILL");
  }
  return `engine.lock.json + GET /v1/health 200 (pid ${child.pid})`;
});
const head = git(repoRoot, ["rev-parse", "HEAD"]).stdout?.trim() ?? "?",
  cell = (text) => text.replaceAll("\n", " ").replaceAll("|", "\\|").slice(0, 400);
const rows = results
  .map((r) => `| ${r.gate} | ${r.status} | ${(r.ms / 1000).toFixed(1)}s | ${cell(r.detail)} |`)
  .join("\n");
fs.mkdirSync(path.dirname(reportPath), { recursive: true });
fs.writeFileSync(
  reportPath,
  `# Linux Engine smoke\n\n- repo: ${repoRoot}\n- commit: ${head}\n- date: ${new Date().toISOString()}\n- node ${tools.node ?? "?"} / pnpm ${tools.pnpm ?? "?"} / ${tools.cargo ?? "?"}\n- XDG isolated under ${runRoot}\n\n| gate | status | time | detail |\n|------|--------|------|--------|\n${rows}\n`,
);
console.log(`[linux-smoke] report: ${reportPath}`);
fs.rmSync(runRoot, { recursive: true, force: true });
const failed = results.filter((row) => row.status !== "PASS");
console.log(
  failed.length
    ? `[linux-smoke] ${failed.map((row) => `${row.gate}=${row.status}`).join(", ")}`
    : "[linux-smoke] all gates PASS",
);
if (failed.length) process.exit(1);
