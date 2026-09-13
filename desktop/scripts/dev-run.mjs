#!/usr/bin/env node
import { spawn } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { processInfo, stopProcessTree } from "./dev-run-process.mjs";
import { acquirePortLease, releasePortLease } from "./dev-run-port.mjs";
import { redactText } from "./redaction.mjs";
import {
  createRunManifest,
  readRunManifest,
  resetRun,
  writeRunManifest,
} from "./dev-run-manifest.mjs";
export { createRunManifest, readRunManifest, resetRun, writeRunManifest };
const scriptRoot = path.dirname(fileURLToPath(import.meta.url));
export const shellRoot = path.resolve(scriptRoot, "..");
export const testRoot = path.join(shellRoot, ".e2e", "runs");
const viteCli = path.join(shellRoot, "node_modules", "vite", "bin", "vite.js");
const electronMain = path.join(shellRoot, "dist-electron", "main.js");
function attachLog(child, file) {
  writeFileSync(file, "", "utf8");
  let output = "";
  const append = (chunk) => {
    output = redactText(output + String(chunk));
    writeFileSync(file, output, "utf8");
  };
  child.stdout?.on("data", append);
  child.stderr?.on("data", append);
}
function spawnOwned(manifest, role, command, args, env) {
  const child = spawn(command, args, {
    cwd: shellRoot,
    env: { ...process.env, ...env },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  if (!child.pid) throw new Error(`${role} did not provide a PID`);
  const info = processInfo(child.pid);
  if (!info?.startTime || !info.commandLine) throw new Error(`${role} identity is unavailable`);
  manifest.ownedPids.push({
    pid: child.pid,
    role,
    startTime: info.startTime,
    commandLine: info.commandLine,
  });
  writeRunManifest(path.join(manifest.runRoot, "manifest.json"), manifest);
  attachLog(child, path.join(manifest.outputDir, `${role}.log`));
  return child;
}

function waitForViteReady(child, port, timeout = 10_000) {
  return new Promise((resolve, reject) => {
    let output = "";
    const timer = setTimeout(() => finish(new Error(`Vite did not bind to port ${port}`)), timeout);
    const probe = async () => {
      if (!output.includes(`127.0.0.1:${port}`) && !output.includes(`localhost:${port}`)) return;
      try {
        if ((await fetch(`http://127.0.0.1:${port}/`)).ok) finish();
      } catch {}
    };
    const interval = setInterval(probe, 100);
    const finish = (error) => {
      clearTimeout(timer);
      clearInterval(interval);
      child.stdout?.off("data", onData);
      child.stderr?.off("data", onData);
      child.off("exit", onExit);
      if (error) reject(error);
      else resolve();
    };
    const onData = (chunk) => {
      output += String(chunk);
      void probe();
    };
    const onExit = (code) =>
      finish(new Error(`Vite exited before binding (code ${code ?? "unknown"})`));
    child.stdout?.on("data", onData);
    child.stderr?.on("data", onData);
    child.once("exit", onExit);
  });
}

export async function startRun(root = testRoot, attempt = 0, launchShell = true) {
  if (!existsSync(viteCli)) throw new Error(`Vite CLI is missing: ${viteCli}`);
  if (!existsSync(electronMain)) throw new Error(`desktop build is missing: ${electronMain}`);
  const backendName = process.platform === "win32" ? "kepler-backend.exe" : "kepler-backend";
  const backend =
    process.env.KEPLER_BACKEND_EXE ?? path.resolve(shellRoot, "..", "target", "debug", backendName);
  if (!existsSync(backend))
    throw new Error(`debug runtime is missing: ${backend}; run build:backend:dev first`);
  const { default: electron } = await import("electron");
  const manifest = createRunManifest("desktop", root);
  const lease = await acquirePortLease(path.resolve(root));
  manifest.ports.shell = lease.port;
  manifest.portLease = lease.file;
  mkdirSync(manifest.dataDir, { recursive: true });
  mkdirSync(manifest.userDataDir, { recursive: true });
  mkdirSync(manifest.outputDir, { recursive: true });
  const manifestFile = path.join(manifest.runRoot, "manifest.json");
  writeRunManifest(manifestFile, manifest);
  const runEnv = {
    KEPLER_DEV: "1",
    KEPLER_INSTANCE: `test-${manifest.runId}`,
    KOSMOS_DATA_DIR: manifest.dataDir,
    KOSMOS_TEST_MODE: "1",
    KOSMOS_HEADLESS: "1",
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
    KEPLER_SKIP_SYNC: "1",
    KOSMOS_SHELL_DEV_PORT: String(manifest.ports.shell),
    KOSMOS_RUN_ID: manifest.runId,
    VITE_DEV_SERVER_URL: `http://127.0.0.1:${manifest.ports.shell}`,
  };
  try {
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
    );
    await waitForViteReady(vite, manifest.ports.shell);
    const children = [vite];
    if (launchShell)
      children.push(
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
        ),
      );
    return { manifest, manifestFile, children };
  } catch (error) {
    stopRun(manifestFile, root);
    releasePortLease(manifest.portLease, root);
    if (attempt < 2) {
      resetRun(manifestFile, root);
      return startRun(root, attempt + 1, launchShell);
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

function latestManifest() {
  if (!existsSync(testRoot)) return undefined;
  return readdirSync(testRoot, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => path.join(testRoot, entry.name, "manifest.json"))
    .filter(existsSync)
    .sort()
    .at(-1);
}

function packagedNotRunReason() {
  const root = process.env.KOSMOS_PACKAGED_ROOT?.trim();
  if (!root) return "KOSMOS_PACKAGED_ROOT is not set";
  if (!existsSync(path.join(root, "Kosmos.exe"))) return `Kosmos.exe is missing under ${root}`;
  if (!existsSync(path.join(root, "resources", "Kosmos Runtime.exe")))
    return `bundled Kosmos Runtime.exe is missing under ${root}`;
  return null;
}

async function runSmoke() {
  const packaged = Boolean(process.env.KOSMOS_PACKAGED_ROOT);
  if (packaged) {
    const reason = packagedNotRunReason();
    if (reason) {
      console.log(JSON.stringify({ result: "not_run", reason }));
      process.exitCode = 2;
      return;
    }
  }
  const running = packaged
    ? (() => {
        const manifest = createRunManifest("desktop-packaged", testRoot);
        const manifestFile = path.join(manifest.runRoot, "manifest.json");
        writeRunManifest(manifestFile, manifest);
        return { manifest, manifestFile };
      })()
    : await startRun(testRoot, 0, false);
  try {
    const cli = path.join(shellRoot, "node_modules", "playwright", "cli.js");
    const smokeEnv = { ...process.env, KOSMOS_DEV_RUN_MANIFEST: running.manifestFile };
    if (!packaged)
      smokeEnv.VITE_DEV_SERVER_URL = `http://127.0.0.1:${running.manifest.ports.shell}`;
    const child = spawn(
      process.execPath,
      [cli, "test", "e2e/smoke.spec.ts", "--config", "playwright.config.ts"],
      {
        cwd: shellRoot,
        env: smokeEnv,
        stdio: "inherit",
        windowsHide: true,
      },
    );
    process.exitCode = await new Promise((resolve) =>
      child.once("exit", (code) => resolve(code ?? 1)),
    );
  } finally {
    if (!packaged) stopRun(running.manifestFile);
    if (process.exitCode === 0) resetRun(running.manifestFile, testRoot);
    else console.error(`[dev] smoke artifacts preserved at ${running.manifest.outputDir}`);
  }
}

export async function main(argv = process.argv.slice(2)) {
  const command = argv[0] ?? "run";
  const index = argv.indexOf("--manifest");
  const file = index >= 0 ? argv[index + 1] : latestManifest();
  if (command === "run") {
    const running = await startRun();
    console.log(JSON.stringify(running.manifest));
    const close = () => {
      stopRun(running.manifestFile);
      process.exit(0);
    };
    process.once("SIGINT", close);
    process.once("SIGTERM", close);
    await new Promise(() => {});
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
  } else if (command === "smoke") {
    await runSmoke();
  } else {
    throw new Error(`unknown dev command ${command}; use run, reset, stop, diagnostics, or smoke`);
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url))
  main().catch((error) => {
    console.error(`[dev] ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 1;
  });
