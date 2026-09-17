// Process spawning and readiness helpers for dev-run: every spawned process is
// recorded in the run manifest with its identity (pid + start time + command
// line) so `stop`/`reset` only ever kill processes this run spawned.
import { spawn } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { processInfo } from "../../desktop/scripts/dev-run-process.mjs";
import { redactText } from "../../desktop/scripts/redaction.mjs";
import { writeRunManifest } from "../../desktop/scripts/dev-run-manifest.mjs";

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

export function spawnOwned(manifest, role, command, args, env, cwd, shell = false) {
  const child = spawn(command, args, {
    cwd,
    env: { ...process.env, ...env },
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
    shell,
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

export async function serves(url) {
  try {
    const response = await fetch(url, { signal: AbortSignal.timeout(1_500) });
    return response.ok || response.status === 404;
  } catch {
    return false;
  }
}

// The port probe must not depend on Vite's log format: Windows builds wrap the
// port in ANSI styling, so substring matching on stdout silently never fires.
export function waitForViteReady(child, port, timeout = 10_000) {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => finish(new Error(`Vite did not bind to port ${port}`)), timeout);
    const probe = async () => {
      try {
        if ((await fetch(`http://127.0.0.1:${port}/`)).ok) finish();
      } catch {}
    };
    const interval = setInterval(probe, 100);
    const finish = (error) => {
      clearTimeout(timer);
      clearInterval(interval);
      child.off("exit", onExit);
      if (error) reject(error);
      else resolve();
    };
    const onExit = (code) =>
      finish(new Error(`Vite exited before binding (code ${code ?? "unknown"})`));
    child.once("exit", onExit);
  });
}

export function waitForEngineLock(engine, dataDir, timeout = 60_000) {
  const lockFile = path.join(dataDir, "engine.lock.json");
  return new Promise((resolve, reject) => {
    const timer = setTimeout(
      () => finish(new Error(`Engine did not write ${lockFile} within ${timeout}ms`)),
      timeout,
    );
    const finish = (error) => {
      clearTimeout(timer);
      clearInterval(interval);
      engine.off("exit", onExit);
      if (error) reject(error);
      else resolve();
    };
    const interval = setInterval(() => {
      try {
        const lock = JSON.parse(readFileSync(lockFile, "utf8"));
        if (lock?.http_port && lock?.auth_token) finish();
      } catch {}
    }, 200);
    const onExit = (code) =>
      finish(new Error(`Engine exited before writing a lock file (code ${code ?? "unknown"})`));
    engine.once("exit", onExit);
  });
}
