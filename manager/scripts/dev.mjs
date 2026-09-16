#!/usr/bin/env node
// Full local dev bring-up for Manager: builds the debug Engine and its
// ark-core-rpc sidecar from this checkout, builds Host and Manager, builds the
// configured dev packages with each package's own package manager, then starts
// an isolated run (engine + vite + electron) via dev-run.mjs.
//
//   pnpm run dev          build everything, then run in the foreground
//   pnpm run dev:run      skip the builds, just start a run
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { packageManagerCommand } from "./dev-run.mjs";

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
const manager = path.resolve(scriptRoot, "..");
const cortex = path.resolve(manager, "..");
const node = process.execPath;

const step = (label) => console.log(`\x1b[36m[dev] ${label}\x1b[0m`);
const attempt = (command, args, cwd, env = process.env, stdio = "ignore") =>
  spawnSync(command, args, {
    cwd,
    env,
    stdio,
    windowsHide: true,
    shell: command === "pnpm" && process.platform === "win32",
  }).status ?? 1;
const run = (command, args, cwd, env = process.env) => {
  const result = spawnSync(command, args, {
    cwd,
    env,
    stdio: "inherit",
    windowsHide: true,
    shell: command === "pnpm" && process.platform === "win32",
  });
  if ((result.status ?? 1) !== 0) {
    console.error(`[dev] ${command} ${args.join(" ")} failed in ${cwd}`);
    process.exit(result.status ?? 1);
  }
};

const workspaceScript = path.join(cortex, "scripts", "workspace.mjs");
if (attempt(node, [workspaceScript, "doctor"], cortex) !== 0) {
  step("preparing workspace dependencies (imago, arca-sdk)");
  const workspaceEnv = { ...process.env };
  if (!workspaceEnv.KOSMOS_WORKSPACE_MODE) {
    const imago = path.resolve(cortex, "..", "imago");
    const arcaSdk = path.resolve(cortex, "..", "arca-sdk");
    if (
      existsSync(path.join(imago, "package.json")) &&
      existsSync(path.join(arcaSdk, "package.json"))
    ) {
      workspaceEnv.KOSMOS_WORKSPACE_MODE = "local";
      workspaceEnv.KOSMOS_IMAGO_PATH = imago;
      workspaceEnv.KOSMOS_ARCA_SDK_PATH = arcaSdk;
      step(`linking sibling checkouts (${imago}, ${arcaSdk})`);
    }
  }
  run(node, [workspaceScript, "bootstrap"], cortex, workspaceEnv);
}

step("building debug engine (cargo)");
run(node, [path.join(cortex, "desktop", "scripts", "build-backend-dev.mjs")], cortex);

step("building package host");
run("pnpm", ["run", "build"], path.join(cortex, "host"));

step("building manager");
run("pnpm", ["run", "build"], manager);

const configPath = path.join(cortex, "dev-packages.json");
const config = JSON.parse(readFileSync(configPath, "utf8"));
const failedPackages = [];
for (const entry of Array.isArray(config.packages) ? config.packages : []) {
  const relativePath = String(entry?.path ?? "").trim();
  if (!relativePath) continue;
  const root = path.resolve(cortex, relativePath);
  const manifestFile = path.join(root, "package.json");
  if (!existsSync(manifestFile)) {
    step(`skipping ${relativePath}: package.json missing`);
    continue;
  }
  const { binary, scripts } = packageManagerCommand(root);
  if (scripts["package:kspkg"]) {
    step(`packaging ${relativePath} (${binary} run package:kspkg)`);
    if (attempt(binary, ["run", "package:kspkg"], root, process.env, "inherit") !== 0) {
      console.error(`[dev] ${relativePath} failed to package; skipping`);
      failedPackages.push(relativePath);
    }
  }
}
if (failedPackages.length) console.error(`[dev] packages not built: ${failedPackages.join(", ")}`);

step("starting isolated run (Ctrl+C to stop; `pnpm run dev:list` shows all)");
run(node, [path.join(scriptRoot, "dev-run.mjs"), "run"], manager);
