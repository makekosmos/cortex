#!/usr/bin/env node
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

const candidate = path.resolve(process.argv[2] ?? "release/win-unpacked");
const executable = path.join(candidate, "Kosmos.exe");
if (!fs.existsSync(executable)) throw new Error("candidate Kosmos.exe is missing");

const root = fs.mkdtempSync(path.join(os.tmpdir(), "kosmos-launcher-smoke-"));
const dataDir = path.join(root, "data");
const accelerator = "Control+Alt+Shift+F11";
fs.mkdirSync(dataDir, { recursive: true });
fs.writeFileSync(
  path.join(dataDir, "kepler-shell-settings.json"),
  JSON.stringify({ hotkey: accelerator, showTrayIcon: true }),
);

const visibleConsoles = () => {
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "@(Get-Process -Name powershell,pwsh,cmd,conhost -ErrorAction SilentlyContinue | Where-Object {$_.MainWindowHandle -ne 0} | Select-Object -ExpandProperty Id) | ConvertTo-Json -Compress",
    ],
    { encoding: "utf8", windowsHide: true },
  ).trim();
  if (!output || output === "[]") return [];
  const parsed = JSON.parse(output);
  return Array.isArray(parsed) ? parsed : [parsed];
};
const candidateProcesses = () => {
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-Command",
      "@(Get-CimInstance Win32_Process | Where-Object {$_.ExecutablePath -and $_.ExecutablePath.StartsWith($env:KOSMOS_SMOKE_CANDIDATE,[StringComparison]::OrdinalIgnoreCase)} | Select-Object -ExpandProperty ProcessId) | ConvertTo-Json -Compress",
    ],
    {
      encoding: "utf8",
      windowsHide: true,
      env: { ...process.env, KOSMOS_SMOKE_CANDIDATE: candidate },
    },
  ).trim();
  if (!output || output === "[]") return [];
  const parsed = JSON.parse(output);
  return Array.isArray(parsed) ? parsed : [parsed];
};

const before = visibleConsoles();
const env = {
  ...process.env,
  APPDATA: path.join(root, "appdata"),
  LOCALAPPDATA: path.join(root, "localappdata"),
  KOSMOS_DATA_DIR: dataDir,
  KEPLER_INSTANCE: "dev",
  KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
  KEPLER_SKIP_SYNC: "1",
  KEPLER_USAGE_TRACKER: "0",
};
let child;
let summary;
const cleanup = async () => {
  if (child?.exitCode === null) {
    child.kill();
    await Promise.race([
      new Promise((resolve) => child.once("exit", resolve)),
      new Promise((resolve) => setTimeout(resolve, 10_000)),
    ]);
  }
  const runtime = path.join(candidate, "resources", "Kosmos Runtime.exe");
  if (fs.existsSync(runtime)) {
    try {
      execFileSync(runtime, ["--shutdown"], {
        env,
        stdio: "ignore",
        windowsHide: true,
        timeout: 10_000,
      });
    } catch {}
  }
  const deadline = Date.now() + 10_000;
  while (candidateProcesses().length && Date.now() < deadline) {
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  const leftovers = candidateProcesses();
  if (leftovers.length)
    throw new Error(`launcher left candidate processes: ${leftovers.join(",")}`);
  fs.rmSync(root, {
    recursive: true,
    force: true,
    maxRetries: 5,
    retryDelay: 200,
  });
};
let cleanupError;
try {
  child = spawn(executable, [], {
    env,
    stdio: ["ignore", "pipe", "pipe"],
    windowsHide: true,
  });
  let output = "";
  for (const stream of [child.stdout, child.stderr]) {
    stream?.on("data", (chunk) => {
      output = `${output}${String(chunk)}`.slice(-16_384);
    });
  }
  const deadline = Date.now() + 30_000;
  while (
    (!output.includes("[kepler-shell] tray created") ||
      !output.includes(`[kepler-shell] globalShortcut ${accelerator} registered`) ||
      !fs.existsSync(path.join(dataDir, "engine.lock.json"))) &&
    Date.now() < deadline
  ) {
    if (child.exitCode !== null) throw new Error(`launcher exited early: ${output}`);
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  if (!output.includes("[kepler-shell] tray created"))
    throw new Error("tray creation was not confirmed");
  if (!output.includes(`[kepler-shell] globalShortcut ${accelerator} registered`))
    throw new Error(`launcher global hotkey is not registered: ${output}`);
  if (!fs.existsSync(path.join(dataDir, "engine.lock.json")))
    throw new Error("launcher Engine lock was not created");
  const extraVisibleConsoles = visibleConsoles().filter((pid) => !before.includes(pid));
  if (extraVisibleConsoles.length) throw new Error("launcher exposed a console window");
  summary = {
    result: "pass",
    packagedCandidate: true,
    trayCreated: true,
    hotkeyRegistered: accelerator,
    extraVisibleConsoles: [],
    leftoverProcesses: [],
  };
} finally {
  await cleanup().catch((error) => {
    cleanupError = error;
  });
}
if (cleanupError) throw cleanupError;
if (summary) console.log(JSON.stringify(summary));
