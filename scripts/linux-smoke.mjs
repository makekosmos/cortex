#!/usr/bin/env node
// Reproducible Linux full-contour smoke (docs/linux-dev.md): ordered gates
// under Xvfb with ELECTRON_DISABLE_SANDBOX=1 + isolated XDG roots; each gate
// reports PASS / FAIL / NOT_RUN and the run exits 0 only when all passed.
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
const workspaceRoot = path.resolve(repoRoot, "..");
const HOST_SPECS = [
  "first-party-agenda-contract.spec.ts",
  "first-party-agenda-smoke.spec.ts",
  "first-party-memoria-contract.spec.ts",
  "first-party-memoria-smoke.spec.ts",
  "first-party-memoria-import-crash.spec.ts",
  "first-party-ordo-contract.spec.ts",
  "first-party-arcadia-contract.spec.ts",
  "first-party-dictation-contract.spec.ts",
];
// Reviewed pins from host/e2e/fixtures/*-archive.ts; "object" needs the
// <commit>:<file> blob, "release" needs HEAD == commit + the built file.
const APP_GATES = `app kind commit file
agenda object 04425784fda4864e5f66fc575d3d042864a4b8dd release/agenda-0.2.7.kspkg
memoria object 9ae7892bb6d66615506f2109c12f8438f6f1aa36 release/memoria-0.6.8.kspkg
ordo release 452b7be298f7f080fc8f2f80618e8e70651f1b99 release/ordo-0.1.3.kspkg
arcadia release ae10decb1aa91e0704302a26bb19df031396bd79 release/arcadia-0.1.11.kspkg
dictation release b37e8cdb1ffd60762138606cd9dd3491815ee4f1 release/dictation-0.2.5.kspkg`
  .split("\n")
  .slice(1)
  .map((line) => {
    const [app, kind, commit, file] = line.split(" ");
    return { app, kind, commit, file };
  });
const WORKSPACE_OUTPUTS = {
  imago: ["index.ts", "theme/css-variables.css", "dist/index.css"],
  "arca-sdk": ["dist/index.js"],
};

const arg = (name) => flag(process.argv, name);
const has = (name) => process.argv.includes(name);
const reportPath = path.resolve(repoRoot, arg("--report") ?? ".tmp/linux-smoke-report.md"),
  appsRoot = arg("--apps-root") ?? process.env.KOSMOS_SMOKE_APPS_ROOT;

// Re-exec under xvfb-run once so every Electron launch has a display.
if (!has("--no-xvfb") && !process.env.DISPLAY && !process.env.KOSMOS_SMOKE_XVFB) {
  const args = ["-a", process.execPath, fileURLToPath(import.meta.url), ...process.argv.slice(2)];
  const env = { ...process.env, KOSMOS_SMOKE_XVFB: "1" };
  process.exit(spawnSync("xvfb-run", args, { cwd: repoRoot, env, stdio: "inherit" }).status ?? 1);
}

const runRoot = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-linux-smoke-"));
const childEnv = {
  ...process.env,
  ELECTRON_DISABLE_SANDBOX: "1",
  KOSMOS_HEADLESS: "1",
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
      ((status = "FAIL"), (detail = error instanceof Error ? error.message : String(error)));
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
  if (step("which", ["xvfb-run"]).status !== 0) throw new Error("missing tool: xvfb-run");
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
await runGate("app-checkouts", ["preflight"], () => {
  const problems = [];
  for (const gate of APP_GATES) {
    const dir = path.join(workspaceRoot, gate.app);
    const source = appsRoot && path.join(path.resolve(appsRoot), gate.app);
    if (!fs.existsSync(dir) && source && fs.existsSync(source)) fs.symlinkSync(source, dir, "dir");
    if (!fs.existsSync(dir)) {
      problems.push(`${gate.app} missing (set --apps-root)`);
      continue;
    }
    if (gate.kind === "object") {
      if (git(dir, ["cat-file", "-e", `${gate.commit}:${gate.file}`]).status !== 0)
        problems.push(`${gate.app} lacks ${gate.file}@${gate.commit.slice(0, 7)}`);
      continue;
    }
    const head = must(git(dir, ["rev-parse", "HEAD"]), `${gate.app} HEAD`).trim();
    if (head !== gate.commit)
      problems.push(`${gate.app} HEAD ${head.slice(0, 7)} != ${gate.commit.slice(0, 7)}`);
    else if (!fs.existsSync(path.join(dir, gate.file)))
      problems.push(`${gate.app} ${gate.file} not built`);
  }
  if (problems.length) throw new Error(problems.join("; "));
  return APP_GATES.map((gate) => gate.app).join(", ");
});
await runGate("engine-bootstrap", ["preflight"], async () => {
  const target = JSON.parse(
    must(step("cargo", ["metadata", "--no-deps", "--format-version=1"]), "cargo metadata"),
  ).target_directory;
  const debug = path.join(target, "debug");
  const engine = path.join(debug, "kepler-backend");
  const sidecar = ["desktop/scripts/ark-core-rpc.mjs", "--debug", "--target-dir", debug];
  must(step("node", sidecar), "ark-core-rpc");
  must(step("cargo", ["build", "-p", "kepler-backend"]), "cargo build kepler-backend");
  const dataDir = path.join(runRoot, "engine-data");
  const engineEnv = {
    ...childEnv,
    KOSMOS_DATA_DIR: dataDir,
    ARK_CORE_RPC_PATH: path.join(debug, "ark-core-rpc"),
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
    KEPLER_SKIP_SYNC: "1",
    KEPLER_USAGE_TRACKER: "0",
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
const hostPkg = path.join(repoRoot, "host", "package.json"),
  hostLock = path.join(repoRoot, "host", "pnpm-lock.yaml");
const ensureElectron = (dir) => {
  const electronPkg = path.join(repoRoot, dir, "node_modules", "electron");
  if (!fs.existsSync(path.join(electronPkg, "dist", "electron")))
    must(step("node", ["install.js"], { cwd: electronPkg }), `${dir} electron postinstall`);
};
const buildGate = (dir) => () => {
  must(step("pnpm", ["--dir", dir, "run", "build"]), `${dir} vite build`);
  if (!fs.existsSync(path.join(repoRoot, dir, "dist-electron", "main.js")))
    throw new Error(`${dir}/dist-electron/main.js missing`);
  return "dist + dist-electron";
};
const e2eGate = (dir, specs, label) => () => {
  const run = step("node", ["scripts/run-e2e.mjs", ...specs], { cwd: path.join(repoRoot, dir) });
  const summary =
    tail(run.stdout ?? "").match(/\d+ (?:passed|failed|skipped|did not run).*/)?.[0] ?? "";
  if (run.status !== 0) throw new Error(`${dir} e2e exit ${run.status}: ${tail(run.stdout ?? "")}`);
  return `${label} ${summary}`.trim();
};
let savedHost;
try {
  await runGate("host-deps", ["workspace-deps"], () => {
    const install = step("pnpm", ["--dir", "host", "install", "--frozen-lockfile"]);
    if (install.status === 0) {
      ensureElectron("host");
      return "frozen lockfile";
    }
    // GitHub Packages 403 fallback; the link: edit is restored, never committed.
    savedHost = [hostPkg, hostLock].map((f) => [f, fs.readFileSync(f, "utf8")]);
    const link = '"@makekosmos/ark": "link:../.tmp/workspace/arca-sdk"',
      args = [
        "--dir",
        "host",
        "install",
        "--no-frozen-lockfile",
        "--trust-lockfile",
        "--ignore-scripts",
      ];
    fs.writeFileSync(hostPkg, savedHost[0][1].replace('"@makekosmos/ark": "0.1.1"', link));
    must(step("pnpm", args), "host install (link:)");
    ensureElectron("host");
    return "link:../.tmp/workspace/arca-sdk fallback";
  });
  await runGate("host-build", ["host-deps"], buildGate("host"));
  await runGate(
    "host-e2e",
    ["host-build", "app-checkouts", "engine-bootstrap"],
    e2eGate("host", HOST_SPECS, `${HOST_SPECS.length} first-party specs`),
  );
  await runGate("manager-deps", ["workspace-deps"], () => {
    must(step("pnpm", ["--dir", "manager", "install", "--frozen-lockfile"]), "manager install");
    ensureElectron("manager");
    return "frozen lockfile";
  });
  await runGate("manager-build", ["manager-deps"], buildGate("manager"));
  await runGate(
    "manager-e2e",
    ["manager-build", "engine-bootstrap"],
    e2eGate("manager", [], "all specs incl. store-catalog"),
  );
} finally {
  if (savedHost) for (const [f, data] of savedHost) fs.writeFileSync(f, data);
}
const head = git(repoRoot, ["rev-parse", "HEAD"]).stdout?.trim() ?? "?",
  cell = (text) => text.replaceAll("\n", " ").replaceAll("|", "\\|").slice(0, 400);
const rows = results
  .map((r) => `| ${r.gate} | ${r.status} | ${(r.ms / 1000).toFixed(1)}s | ${cell(r.detail)} |`)
  .join("\n");
fs.mkdirSync(path.dirname(reportPath), { recursive: true });
fs.writeFileSync(
  reportPath,
  `# Linux full-contour smoke\n\n- repo: ${repoRoot}\n- commit: ${head}\n- date: ${new Date().toISOString()}\n- node ${tools.node ?? "?"} / pnpm ${tools.pnpm ?? "?"} / ${tools.cargo ?? "?"}\n- display: ${process.env.DISPLAY ?? "none"}; ELECTRON_DISABLE_SANDBOX=1; XDG isolated under ${runRoot}\n\n| gate | status | time | detail |\n|------|--------|------|--------|\n${rows}\n`,
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
