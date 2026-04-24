import { execFile as execFileCb, spawn } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { promisify } from "node:util";

import type { RunningProcessInfo } from "./types";

const execFile = promisify(execFileCb);
const isWindows = process.platform === "win32";
const isLinux = process.platform === "linux";

export type ProcessMatch =
  | { matchType: "exe_path"; value: string }
  | { matchType: "process_name"; value: string };

function escapePowerShellSingleQuoted(value: string): string {
  return value.replace(/'/g, "''");
}

function normalizeForComparison(input: string): string {
  const resolved = fs.existsSync(input) ? fs.realpathSync.native(input) : path.resolve(input);
  const normalized = path.normalize(resolved);
  return isWindows ? normalized.toLowerCase() : normalized;
}

function normalizeProcessName(input: string): string {
  return input.trim().toLowerCase();
}

function firstCommandToken(command: string): string {
  const trimmed = command.trim();
  if (!trimmed) {
    return "";
  }

  if (trimmed.startsWith('"')) {
    const end = trimmed.indexOf('"', 1);
    return end > 1 ? trimmed.slice(1, end) : trimmed.slice(1);
  }

  const spaceIndex = trimmed.indexOf(" ");
  return spaceIndex === -1 ? trimmed : trimmed.slice(0, spaceIndex);
}

async function listWindowsProcesses(): Promise<RunningProcessInfo[]> {
  const script = `
$ErrorActionPreference = 'Stop'
Get-CimInstance Win32_Process |
  Select-Object ProcessId, Name, ExecutablePath |
  ConvertTo-Json -Compress
`.trim();
  const { stdout } = await execFile("powershell.exe", [
    "-NoProfile",
    "-NonInteractive",
    "-Command",
    script,
  ]);
  if (!stdout.trim()) {
    return [];
  }

  const payload = JSON.parse(stdout) as
    | { ProcessId: number; Name?: string; ExecutablePath?: string | null }
    | Array<{ ProcessId: number; Name?: string; ExecutablePath?: string | null }>;
  const items = Array.isArray(payload) ? payload : [payload];

  return items.map((item) => ({
    pid: Number(item.ProcessId),
    name: item.Name ?? "",
    path: item.ExecutablePath ?? "",
  }));
}

async function listUnixProcesses(): Promise<RunningProcessInfo[]> {
  if (isLinux) {
    const entries = await fs.promises.readdir("/proc", { withFileTypes: true });
    const result: RunningProcessInfo[] = [];

    for (const entry of entries) {
      if (!entry.isDirectory() || !/^\d+$/.test(entry.name)) {
        continue;
      }

      const pid = Number(entry.name);
      const procDir = path.join("/proc", entry.name);
      const exePath = path.join(procDir, "exe");
      const commPath = path.join(procDir, "comm");
      try {
        const resolved = await fs.promises.readlink(exePath);
        const name = (await fs.promises.readFile(commPath, "utf8")).trim();
        result.push({
          pid,
          name,
          path: resolved,
        });
      } catch {
        const cmdlinePath = path.join(procDir, "cmdline");
        try {
          const raw = await fs.promises.readFile(cmdlinePath, "utf8");
          const command = raw.replace(/\0/g, " ").trim();
          const token = firstCommandToken(command);
          result.push({
            pid,
            name: path.basename(token || command),
            path: token,
          });
        } catch {
          // Ignore processes we cannot inspect.
        }
      }
    }

    return result;
  }

  const { stdout } = await execFile("ps", ["-axo", "pid=,command="]);
  return stdout
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const match = line.match(/^(\d+)\s+(.*)$/);
      if (!match) {
        return null;
      }

      const pid = Number(match[1]);
      const command = match[2];
      const token = firstCommandToken(command);
      return {
        pid,
        name: path.basename(token || command),
        path: token,
      } satisfies RunningProcessInfo;
    })
    .filter((value): value is RunningProcessInfo => value !== null);
}

export async function listRunningProcesses(): Promise<RunningProcessInfo[]> {
  if (isWindows) {
    return listWindowsProcesses();
  }
  return listUnixProcesses();
}

export async function countRunningInstances(exePath: string): Promise<number> {
  const target = normalizeForComparison(exePath);
  const processes = await listRunningProcesses();
  return processes.filter((process) => {
    if (!process.path) {
      return false;
    }
    return normalizeForComparison(process.path) === target;
  }).length;
}

export async function killMatchingProcesses(exePath: string): Promise<number> {
  const target = normalizeForComparison(exePath);
  const processes = await listRunningProcesses();
  let killed = 0;

  for (const processInfo of processes) {
    if (!processInfo.path) {
      continue;
    }
    if (normalizeForComparison(processInfo.path) !== target) {
      continue;
    }

    try {
      process.kill(processInfo.pid);
      killed += 1;
    } catch {
      // Ignore best-effort failures.
    }
  }

  return killed;
}

function matchesProcess(processInfo: RunningProcessInfo, matches: readonly ProcessMatch[]) {
  for (const match of matches) {
    if (match.matchType === "exe_path") {
      if (!processInfo.path) {
        continue;
      }
      if (normalizeForComparison(processInfo.path) === normalizeForComparison(match.value)) {
        return true;
      }
      continue;
    }

    if (normalizeProcessName(processInfo.name) === normalizeProcessName(match.value)) {
      return true;
    }
  }

  return false;
}

export async function countRunningInstancesByMatches(matches: readonly ProcessMatch[]) {
  if (matches.length === 0) {
    return 0;
  }

  const processes = await listRunningProcesses();
  return processes.filter((processInfo) => matchesProcess(processInfo, matches)).length;
}

export async function killMatchingProcessesByMatches(matches: readonly ProcessMatch[]) {
  if (matches.length === 0) {
    return 0;
  }

  const processes = await listRunningProcesses();
  let killed = 0;

  for (const processInfo of processes) {
    if (!matchesProcess(processInfo, matches)) {
      continue;
    }

    try {
      process.kill(processInfo.pid);
      killed += 1;
    } catch {
      // Ignore best-effort failures.
    }
  }

  return killed;
}

export async function resolveShortcutTarget(pathname: string): Promise<string> {
  if (!isWindows || !pathname.toLowerCase().endsWith(".lnk")) {
    return pathname;
  }

  const script = `
$ErrorActionPreference = 'Stop'
$shell = New-Object -ComObject WScript.Shell
$shortcut = $shell.CreateShortcut('${escapePowerShellSingleQuoted(pathname)}')
[Console]::Out.Write($shortcut.TargetPath)
`.trim();

  try {
    const { stdout } = await execFile("powershell.exe", [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      script,
    ]);
    const resolved = stdout.trim();
    return resolved || pathname;
  } catch {
    return pathname;
  }
}

export async function spawnGameProcess(exePath: string): Promise<void> {
  const cwd = path.dirname(exePath);

  await new Promise<void>((resolve, reject) => {
    const child = spawn(exePath, {
      cwd,
      detached: true,
      stdio: "ignore",
      windowsHide: true,
    });

    child.once("error", (error) => reject(error));
    child.once("spawn", () => {
      child.unref();
      resolve();
    });
  });
}
