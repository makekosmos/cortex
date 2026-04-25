import { execFile } from "node:child_process";
import { readlink, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";
import type { ProcessEntry } from "../contracts";

const execFileAsync = promisify(execFile);

function outputToUtf8(value: string | Buffer) {
  return typeof value === "string" ? value : value.toString("utf8");
}

function toNumber(value: unknown) {
  if (typeof value === "number") {
    return value;
  }
  if (typeof value === "string") {
    const parsed = Number(value);
    return Number.isNaN(parsed) ? 0 : parsed;
  }
  return 0;
}

function toStringValue(value: unknown) {
  return typeof value === "string" ? value : "";
}

function extractExecutableCandidate(args: string) {
  const trimmed = args.trim();
  if (!trimmed) {
    return "";
  }

  if (trimmed.startsWith("\"")) {
    const endQuote = trimmed.indexOf("\"", 1);
    if (endQuote > 1) {
      return trimmed.slice(1, endQuote);
    }
  }

  return trimmed.split(/\s+/)[0]?.replace(/^"|"$/g, "") ?? "";
}

function normalizeJsonPayload<T>(payload: T | T[] | null | undefined): T[] {
  if (!payload) {
    return [];
  }
  return Array.isArray(payload) ? payload : [payload];
}

async function runPowerShellJson<T>(script: string): Promise<T[]> {
  const { stdout } = await execFileAsync(
    "powershell.exe",
    ["-NoProfile", "-ExecutionPolicy", "Bypass", "-Command", script],
    {
      maxBuffer: 16 * 1024 * 1024,
      windowsHide: true,
    },
  );

  const text = outputToUtf8(stdout).trim();
  if (!text) {
    return [];
  }

  return normalizeJsonPayload(JSON.parse(text) as T | T[]);
}

async function listWindowsProcesses(): Promise<ProcessEntry[]> {
  try {
    const script = `
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.UTF8Encoding]::new()
$processes = Get-CimInstance Win32_Process | Select-Object ProcessId, Name, ExecutablePath
$cpu = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process | Select-Object IDProcess, PercentProcessorTime
[PSCustomObject]@{ processes = $processes; cpu = $cpu } | ConvertTo-Json -Depth 4 -Compress
`.trim();

    const [bundle] = await runPowerShellJson<{
      processes?: Array<{
        ProcessId?: number;
        Name?: string;
        ExecutablePath?: string | null;
      }>;
      cpu?: Array<{
        IDProcess?: number;
        PercentProcessorTime?: number;
      }>;
    }>(script);

    const cpuMap = new Map<number, number>();
    for (const row of normalizeJsonPayload(bundle?.cpu)) {
      const pid = toNumber(row.IDProcess);
      if (pid > 0) {
        cpuMap.set(pid, toNumber(row.PercentProcessorTime));
      }
    }

    const logicalCoreFallback = os.cpus().length || 1;
    const logicalCores = Math.max(
      1,
      os.availableParallelism?.() ?? logicalCoreFallback,
    );
    const processes: ProcessEntry[] = [];

    for (const row of normalizeJsonPayload(bundle?.processes)) {
      const pid = toNumber(row.ProcessId);
      const exePath = toStringValue(row.ExecutablePath);
      if (pid <= 0 || !exePath) {
        continue;
      }

      try {
        if (!(await stat(exePath)).isFile()) {
          continue;
        }
      } catch {
        continue;
      }

      processes.push({
        pid,
        name: toStringValue(row.Name) || path.basename(exePath),
        path: exePath,
        cpu_usage: (cpuMap.get(pid) ?? 0) / logicalCores,
        gpu_usage: 0,
      });
    }

    processes.sort((a, b) => b.cpu_usage - a.cpu_usage);
    return processes;
  } catch {
    return [];
  }
}

async function listPosixProcesses(): Promise<ProcessEntry[]> {
  try {
    const { stdout } = await execFileAsync(
      "ps",
      process.platform === "darwin"
        ? ["-axo", "pid=,pcpu=,comm=,args="]
        : ["-eo", "pid=,pcpu=,comm=,args="],
      {
        maxBuffer: 16 * 1024 * 1024,
      },
    );

    const lines = outputToUtf8(stdout)
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean);

    const processes: ProcessEntry[] = [];

    for (const line of lines) {
      const match = /^(\d+)\s+([\d.]+)\s+(\S+)\s+(.*)$/.exec(line);
      if (!match) {
        continue;
      }

      const pid = Number(match[1]);
      const cpuUsage = Number(match[2]);
      const comm = match[3];
      const args = match[4].trim();
      if (pid <= 0) {
        continue;
      }

      let exePath = "";
      if (process.platform === "linux") {
        try {
          exePath = await readlink(`/proc/${pid}/exe`);
        } catch {
          exePath = "";
        }
      } else {
        exePath = extractExecutableCandidate(args);
      }

      if (!exePath) {
        continue;
      }

      try {
        if (!(await stat(exePath)).isFile()) {
          continue;
        }
      } catch {
        continue;
      }

      processes.push({
        pid,
        name: comm || path.basename(exePath),
        path: exePath,
        cpu_usage: cpuUsage,
        gpu_usage: 0,
      });
    }

    processes.sort((a, b) => b.cpu_usage - a.cpu_usage);
    return processes;
  } catch {
    return [];
  }
}

export async function listRunningProcesses(): Promise<ProcessEntry[]> {
  if (process.platform === "win32") {
    return listWindowsProcesses();
  }

  return listPosixProcesses();
}
