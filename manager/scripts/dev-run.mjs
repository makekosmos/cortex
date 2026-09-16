#!/usr/bin/env node
// Isolated Manager dev runs: every `run` gets its own Engine data directory,
// Electron user-data directory, Vite port and process manifest, so any number
// of instances can coexist. Owned PIDs are recorded with their process
// identity; `stop`/`reset` only ever kill processes this run spawned.
import { existsSync, mkdirSync, readdirSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { processInfo, stopProcessTree } from "../../desktop/scripts/dev-run-process.mjs";
import { acquirePortLease, releasePortLease } from "../../desktop/scripts/dev-run-port.mjs";
import {
  createRunManifest,
  readRunManifest,
  resetRun,
  writeRunManifest,
} from "../../desktop/scripts/dev-run-manifest.mjs";
import { serves, spawnOwned, waitForEngineLock, waitForViteReady } from "./dev-run-spawn.mjs";
export { createRunManifest, readRunManifest, resetRun, writeRunManifest };

const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
export const managerRoot = path.resolve(scriptRoot, "..");
export const cortexRoot = path.resolve(managerRoot, "..");
export const testRoot = path.join(managerRoot, ".dev", "runs");
const viteCli = path.join(managerRoot, "node_modules", "vite", "bin", "vite.js");
const electronMain = path.join(managerRoot, "dist-electron", "main.js");
const hostMain = path.join(cortexRoot, "host", "dist-electron", "main.js");
const devPackagesConfig = path.join(cortexRoot, "dev-packages.json");

function backendPath() {
  const name = process.platform === "win32" ? "kepler-backend.exe" : "kepler-backend";
  return process.env.KEPLER_BACKEND_EXE ?? path.join(cortexRoot, "target", "debug", name);
}

// A package's own package manager is authoritative: sibling package repos use
// bun or pnpm interchangeably, and spawning the wrong one fails before the dev
// server even starts.
export function packageManagerCommand(root) {
  try {
    const pkg = JSON.parse(readFileSync(path.join(root, "package.json"), "utf8"));
    const spec = String(pkg.packageManager ?? "");
    const binary = spec.startsWith("bun") ? "bun" : "pnpm";
    return { binary, scripts: pkg.scripts ?? {} };
  } catch {
    return { binary: "pnpm", scripts: {} };
  }
}

// Dev servers listed in dev-packages.json are shared across runs: their ports
// are fixed by config, so the first instance to start owns the process and
// later instances reuse the already-serving URL.
export function devPackageServers() {
  try {
    const config = JSON.parse(readFileSync(devPackagesConfig, "utf8"));
    const entries = Array.isArray(config?.packages) ? config.packages : [];
    return entries.flatMap((entry) => {
      const relativePath = String(entry?.path ?? "").trim();
      const url = String(entry?.url ?? "").trim();
      if (!relativePath || !url) return [];
      const root = path.resolve(cortexRoot, relativePath);
      const { binary, scripts } = packageManagerCommand(root);
      if (!scripts.dev) return [];
      return [{ name: path.basename(root), url, root, binary }];
    });
  } catch {
    return [];
  }
}

export async function startRun(root = testRoot, attempt = 0) {
  if (!existsSync(viteCli)) throw new Error(`Vite CLI is missing: ${viteCli}`);
  if (!existsSync(electronMain))
    throw new Error(`manager build is missing: ${electronMain}; run pnpm run build first`);
  const backend = backendPath();
  if (!existsSync(backend))
    throw new Error(`debug engine is missing: ${backend}; run pnpm run dev to build it`);
  const { default: electron } = await import("electron");
  const manifest = createRunManifest("manager", root);
  const lease = await acquirePortLease(path.resolve(root));
  manifest.ports.shell = lease.port;
  manifest.portLease = lease.file;
  mkdirSync(manifest.dataDir, { recursive: true });
  mkdirSync(manifest.userDataDir, { recursive: true });
  mkdirSync(manifest.outputDir, { recursive: true });
  const manifestFile = path.join(manifest.runRoot, "manifest.json");
  writeRunManifest(manifestFile, manifest);
  const runEnv = {
    KOSMOS_DATA_DIR: manifest.dataDir,
    KOSMOS_RUN_ID: manifest.runId,
    KOSMOS_DEV_RUN_MANAGED: "1",
    KOSMOS_DEV_PACKAGES: "1",
    KEPLER_SKIP_SYNC: "1",
    VITE_DEV_SERVER_URL: `http://127.0.0.1:${manifest.ports.shell}`,
    KOSMOS_HOST_MAIN: hostMain,
    KOSMOS_MANAGER_EXTERNAL_ELECTRON: "1",
  };
  try {
    // --core-worker runs the engine core directly. The default supervisor
    // wraps it in a respawning parent that outlives the recorded PID, which
    // leaked backend processes when a dev run was stopped.
    const engine = spawnOwned(manifest, "engine", backend, ["--core-worker"], runEnv, managerRoot);
    await waitForEngineLock(engine, manifest.dataDir);
    for (const server of devPackageServers()) {
      if (await serves(server.url)) continue;
      spawnOwned(
        manifest,
        `pkg-${server.name}`,
        server.binary,
        ["run", "dev"],
        {},
        server.root,
        server.binary === "pnpm" && process.platform === "win32",
      );
    }
    const vite = spawnOwned(
      manifest,
      "renderer",
      process.execPath,
      [
        viteCli,
        "--host",
        "127.0.0.1",
        "--port",
        String(manifest.ports.shell),
        "--strictPort",
        "--configLoader",
        "native",
      ],
      runEnv,
      managerRoot,
    );
    await waitForViteReady(vite, manifest.ports.shell);
    spawnOwned(
      manifest,
      "shell",
      electron,
      [
        electronMain,
        `--user-data-dir=${manifest.userDataDir}`,
        `--kosmos-run-id=${manifest.runId}`,
      ],
      runEnv,
      managerRoot,
    );
    return { manifest, manifestFile };
  } catch (error) {
    stopRun(manifestFile, root);
    releasePortLease(manifest.portLease, root);
    if (attempt < 2) {
      resetRun(manifestFile, root);
      return startRun(root, attempt + 1);
    }
    console.error(`[dev] startup artifacts preserved at ${manifest.outputDir}`);
    throw error;
  }
}

export function stopRun(file, expectedRoot = testRoot) {
  const manifest = readRunManifest(file, expectedRoot);
  const stopped = [
    ...new Set(
      manifest.ownedPids.flatMap((owned) =>
        stopProcessTree(owned.pid, owned.startTime, owned.commandLine),
      ),
    ),
  ];
  const running = manifest.ownedPids.some((owned) => {
    const info = processInfo(owned.pid);
    return info && info.startTime === owned.startTime && info.commandLine === owned.commandLine;
  });
  if (!running) releasePortLease(manifest.portLease, expectedRoot);
  return stopped;
}

export function listRuns(root = testRoot) {
  if (!existsSync(root)) return [];
  return readdirSync(root, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .flatMap((entry) => {
      const file = path.join(root, entry.name, "manifest.json");
      if (!existsSync(file)) return [];
      try {
        const manifest = readRunManifest(file, root);
        const running = manifest.ownedPids.filter((owned) => {
          const info = processInfo(owned.pid);
          return (
            info && info.startTime === owned.startTime && info.commandLine === owned.commandLine
          );
        });
        return [
          {
            runId: manifest.runId,
            file,
            dataDir: manifest.dataDir,
            port: manifest.ports.shell,
            createdAt: manifest.createdAt,
            running: running.map((owned) => owned.role),
          },
        ];
      } catch {
        return [{ runId: entry.name, file, error: "unreadable manifest" }];
      }
    })
    .sort((left, right) => String(left.runId).localeCompare(String(right.runId)));
}

function latestManifest() {
  return listRuns()
    .map((run) => run.file)
    .at(-1);
}

export async function main(argv = process.argv.slice(2)) {
  const command = argv[0] ?? "run";
  const index = argv.indexOf("--manifest");
  const positional = argv[1];
  const file =
    index >= 0
      ? argv[index + 1]
      : positional
        ? path.join(testRoot, path.basename(positional), "manifest.json")
        : latestManifest();
  if (command === "run") {
    const running = await startRun();
    console.log(
      JSON.stringify({
        runId: running.manifest.runId,
        url: `http://127.0.0.1:${running.manifest.ports.shell}`,
        dataDir: running.manifest.dataDir,
        logs: running.manifest.outputDir,
      }),
    );
    const close = () => {
      stopRun(running.manifestFile);
      process.exit(0);
    };
    process.once("SIGINT", close);
    process.once("SIGTERM", close);
    await new Promise(() => {});
  } else if (command === "list") {
    console.log(JSON.stringify(listRuns(), null, 2));
  } else if (command === "stop") {
    if (!file) throw new Error("no dev run manifest found");
    console.log(JSON.stringify({ stopped: stopRun(file) }));
  } else if (command === "reset") {
    if (!file) throw new Error("no dev run manifest found");
    stopRun(file);
    console.log(JSON.stringify({ reset: resetRun(file, testRoot) }));
  } else if (command === "diagnostics") {
    if (!file) throw new Error("no dev run manifest found");
    console.log(JSON.stringify(readRunManifest(file, testRoot), null, 2));
  } else {
    throw new Error(`unknown dev command ${command}; use run, list, stop, reset, or diagnostics`);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  main().catch((error) => {
    console.error(`[dev] ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  });
