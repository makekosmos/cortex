import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";

const validPid = (pid) => Number.isSafeInteger(pid) && pid > 0;
const sameCommand = (actual, expected) => actual.trim() === expected.trim();
const normalizedStartTime = (value) => String(value).trim();
let unixClockTicks;

function unixCreatedAtMs(startTicks) {
  try {
    const boot = Number(readFileSync("/proc/stat", "utf8").match(/^btime\s+(\d+)/m)?.[1]);
    unixClockTicks ??= Number(execFileSync("getconf", ["CLK_TCK"], { encoding: "utf8" }).trim());
    return Number.isFinite(boot) && Number.isFinite(unixClockTicks)
      ? boot * 1000 + (Number(startTicks) * 1000) / unixClockTicks
      : Number.NaN;
  } catch {
    return Number.NaN;
  }
}

export function processInfo(pid) {
  if (!validPid(pid)) return null;
  if (process.platform !== "win32") {
    try {
      const stat = readFileSync(`/proc/${pid}/stat`, "utf8");
      const commandLine = readFileSync(`/proc/${pid}/cmdline`, "utf8").replaceAll("\0", " ").trim();
      const fields = stat.slice(stat.lastIndexOf(")") + 2).split(" ");
      if (fields[0] === "Z") return null;
      if (fields[19] && commandLine)
        return {
          pid,
          startTime: `proc:${fields[19]}`,
          createdAtMs: unixCreatedAtMs(fields[19]),
          commandLine,
        };
    } catch {}
    try {
      const value = execFileSync("ps", ["-o", "lstart=,args=", "-p", String(pid)], {
        encoding: "utf8",
      }).trim();
      return value
        ? {
            pid,
            startTime: normalizedStartTime(value.slice(0, 24)),
            createdAtMs: Date.parse(value.slice(0, 24)),
            commandLine: value.slice(25).trim(),
          }
        : null;
    } catch {
      return null;
    }
  }
  try {
    const value = execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        `Get-CimInstance Win32_Process -Filter 'ProcessId = ${Number(pid)}' | Select-Object ProcessId,ParentProcessId,CreationDate,CommandLine | ConvertTo-Json -Compress`,
      ],
      { encoding: "utf8", windowsHide: true },
    ).trim();
    if (!value) return null;
    const item = JSON.parse(value);
    const creationDate = normalizedStartTime(item.CreationDate ?? "");
    return {
      pid: Number(item.ProcessId),
      parentPid: Number(item.ParentProcessId),
      startTime: creationDate,
      createdAtMs: creationDate.startsWith("/Date(")
        ? Number(creationDate.match(/^\/Date\((-?\d+)\)\/$/)?.[1])
        : Date.parse(creationDate),
      commandLine: String(item.CommandLine ?? "").trim(),
    };
  } catch {
    return null;
  }
}

export function sameProcessStartTime(actual, recorded) {
  return (
    Object.prototype.toString.call(actual) === "[object String]" &&
    Object.prototype.toString.call(recorded) === "[object String]" &&
    actual.trim() === recorded.trim()
  );
}

export function sameProcessIdentity(actual, expected) {
  return Boolean(
    actual &&
    expected &&
    actual.pid === expected.pid &&
    sameProcessStartTime(actual.startTime, expected.startTime) &&
    sameCommand(actual.commandLine, expected.commandLine),
  );
}

export function processIdentityFromLock(
  lockPath,
  launchStartedAt,
  expectedExecutable,
  now = Date.now(),
) {
  try {
    const lock = JSON.parse(readFileSync(lockPath, "utf8"));
    const startedAt = lock?.started_at;
    const lockStartedAt = Date.parse(startedAt);
    const executable = path.resolve(String(expectedExecutable ?? "")).toLowerCase();
    if (
      !validPid(lock?.pid) ||
      !Number.isFinite(lockStartedAt) ||
      lockStartedAt < launchStartedAt ||
      lockStartedAt > now ||
      !executable ||
      Object.prototype.toString.call(startedAt) !== "[object String]" ||
      !/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?(?:Z|[+-]\d{2}:\d{2})$/.test(startedAt)
    )
      return null;
    const info = processInfo(lock.pid);
    return info &&
      Number.isFinite(info.createdAtMs) &&
      info.createdAtMs <= lockStartedAt &&
      info.commandLine.toLowerCase().includes(executable) &&
      /(?:^|\s)--start(?:\s|$)/i.test(info.commandLine)
      ? info
      : null;
  } catch {
    return null;
  }
}

function waitForExit(pids, timeout = 2_000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline && pids.some((pid) => processInfo(pid))) {
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 25);
  }
}

function inventory() {
  try {
    if (process.platform !== "win32") {
      const value = execFileSync("ps", ["-eo", "pid=,ppid=,args="], { encoding: "utf8" }).trim();
      return value
        ? value.split(/\r?\n/).flatMap((line) => {
            const match = line.trim().match(/^(\d+)\s+(\d+)\s+(.*)$/);
            return match
              ? [{ pid: Number(match[1]), parentPid: Number(match[2]), commandLine: match[3] }]
              : [];
          })
        : [];
    }
    const value = execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CreationDate,CommandLine | ConvertTo-Json -Compress",
      ],
      { encoding: "utf8", windowsHide: true },
    ).trim();
    const parsed = value ? JSON.parse(value) : [];
    return (Array.isArray(parsed) ? parsed : [parsed]).map((item) => ({
      pid: Number(item.ProcessId),
      parentPid: Number(item.ParentProcessId),
      startTime: String(item.CreationDate ?? ""),
      commandLine: String(item.CommandLine ?? ""),
    }));
  } catch {
    return [];
  }
}

export function processTree(rootPid) {
  const items = inventory();
  const result = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const item of items) {
      if (result.has(item.parentPid) && !result.has(item.pid)) {
        result.add(item.pid);
        changed = true;
      }
    }
  }
  return result;
}

export function stopProcessTree(pid, expectedStartTime, expectedCommandLine, kind = "run") {
  if (
    !validPid(pid) ||
    Object.prototype.toString.call(expectedCommandLine) !== "[object String]" ||
    !expectedCommandLine.trim()
  )
    return [];
  const root = processInfo(pid);
  if (
    !root ||
    !sameProcessStartTime(root.startTime, expectedStartTime) ||
    !sameCommand(root.commandLine, expectedCommandLine)
  )
    return [];
  if (kind === "backend" && !/kepler-backend|kosmos runtime/i.test(root.commandLine)) return [];
  const owned = [...processTree(pid)];
  if (process.platform === "win32") {
    try {
      execFileSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
        stdio: "ignore",
        windowsHide: true,
      });
    } catch {}
  } else {
    for (const ownedPid of owned.sort((left, right) => right - left)) {
      try {
        process.kill(ownedPid, "SIGTERM");
      } catch {}
    }
    waitForExit(owned);
    const survivors = owned.filter((ownedPid) => processInfo(ownedPid));
    for (const ownedPid of survivors) {
      try {
        process.kill(ownedPid, "SIGKILL");
      } catch {}
    }
    waitForExit(survivors, 500);
  }
  return owned;
}
