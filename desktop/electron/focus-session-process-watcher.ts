import path from "node:path";
import os from "node:os";
import { spawn } from "node:child_process";
import { writeFileSync, unlinkSync } from "node:fs";

import type { FocusBlockedApp } from "./focus-session-types";

let watcherProcess: ReturnType<typeof spawn> | null = null;
let backupSweepTimer: ReturnType<typeof setInterval> | null = null;
const snoozedBlockedApps = new Map<string, number>();
const lastNotifiedAt = new Map<string, number>();
const SNOOZE_MS = 5 * 60 * 1000;
const NOTIFY_DEBOUNCE_MS = 8000;
const SNOOZE_FILE = path.join(os.tmpdir(), "kosmos-focus-snooze.txt");

function exeNameOf(app: FocusBlockedApp): string | null {
  if (!app.exec_path) return null;
  const exe = path.basename(app.exec_path).toLowerCase();
  return exe.length >= 4 ? exe : null;
}

function isBlockedFocusAppSnoozed(appId: string): boolean {
  const until = snoozedBlockedApps.get(appId) ?? 0;
  if (until <= Date.now()) {
    snoozedBlockedApps.delete(appId);
    return false;
  }
  return true;
}

function writeSnoozeFile(blockedApps: FocusBlockedApp[]): void {
  const names = new Set<string>();
  for (const app of blockedApps) {
    if (isBlockedFocusAppSnoozed(app.id)) {
      const exe = exeNameOf(app);
      if (exe) names.add(exe);
    }
  }
  try {
    writeFileSync(SNOOZE_FILE, Array.from(names).join("\n"), "utf8");
  } catch (e) {
    console.warn("[focus-session] writeSnoozeFile failed:", e);
  }
}

function notifyBlocked(
  app: FocusBlockedApp,
  notifier: ((app: { id: string; title: string; icon?: string | null }) => void) | null,
): void {
  const now = Date.now();
  const last = lastNotifiedAt.get(app.id) ?? 0;
  if (now - last < NOTIFY_DEBOUNCE_MS) return;
  lastNotifiedAt.set(app.id, now);
  if (notifier) {
    notifier({ id: app.id, title: app.name, icon: app.icon ?? null });
  }
}

function appByExe(exeName: string, blockedApps: FocusBlockedApp[]): FocusBlockedApp | null {
  const target = exeName.toLowerCase();
  for (const app of blockedApps) {
    if (exeNameOf(app) === target) return app;
  }
  return null;
}

function isProcessRunning(exeName: string): Promise<boolean> {
  return new Promise((resolve) => {
    const proc = spawn("tasklist", ["/FI", `IMAGENAME eq ${exeName}`, "/FO", "CSV", "/NH"], {
      windowsHide: true,
    });
    let out = "";
    proc.stdout?.on("data", (d: Buffer) => (out += d.toString()));
    proc.on("close", () => resolve(out.toLowerCase().includes(exeName.toLowerCase())));
    proc.on("error", () => resolve(false));
  });
}

function killExe(exeName: string): Promise<void> {
  return new Promise((resolve) => {
    const proc = spawn("taskkill", ["/F", "/IM", exeName], { windowsHide: true });
    proc.on("close", () => resolve());
    proc.on("error", () => resolve());
  });
}

async function backupSweep(
  blockedApps: FocusBlockedApp[],
  notifier: ((app: { id: string; title: string; icon?: string | null }) => void) | null,
): Promise<void> {
  for (const app of blockedApps) {
    const exe = exeNameOf(app);
    if (!exe || isBlockedFocusAppSnoozed(app.id)) continue;
    if (!(await isProcessRunning(exe))) continue;
    await killExe(exe);
    console.log(`[focus-watcher] backup-killed: ${exe}`);
    notifyBlocked(app, notifier);
  }
}

export function snoozeBlockedFocusApp(appId: string, blockedApps: FocusBlockedApp[]): void {
  snoozedBlockedApps.set(appId, Date.now() + SNOOZE_MS);
  writeSnoozeFile(blockedApps);
  setTimeout(() => writeSnoozeFile(blockedApps), SNOOZE_MS + 200);
}

export function startProcessWatcher(options: {
  blockedApps: FocusBlockedApp[];
  notifier: ((app: { id: string; title: string; icon?: string | null }) => void) | null;
}): void {
  stopProcessWatcher();
  writeSnoozeFile(options.blockedApps);

  const exeNames = new Set<string>();
  for (const app of options.blockedApps) {
    const exe = exeNameOf(app);
    if (exe) exeNames.add(exe);
  }
  if (exeNames.size === 0) return;

  const allowedLiteral = Array.from(exeNames)
    .map((n) => `'${n}'`)
    .join(",");
  const snoozePathLiteral = SNOOZE_FILE.replace(/'/g, "''");

  const psScript = [
    `$ErrorActionPreference = 'SilentlyContinue'`,
    `$snoozeFile = '${snoozePathLiteral}'`,
    `$targets = @{}`,
    `@(${allowedLiteral}) | ForEach-Object { $targets[$_] = $true }`,
    `function Get-Snoozed {`,
    `  $s = @{}`,
    `  if (Test-Path $snoozeFile) {`,
    `    foreach ($l in (Get-Content $snoozeFile)) { $t = $l.Trim().ToLower(); if ($t) { $s[$t] = $true } }`,
    `  }`,
    `  return $s`,
    `}`,
    `$query = "SELECT * FROM __InstanceCreationEvent WITHIN 0.1 WHERE TargetInstance ISA 'Win32_Process'"`,
    `$w = New-Object System.Management.ManagementEventWatcher`,
    `$w.Query = New-Object System.Management.WqlEventQuery($query)`,
    `$w.Start()`,
    `while ($true) {`,
    `  try { $evt = $w.WaitForNextEvent() } catch { Start-Sleep -Milliseconds 500; continue }`,
    `  if (-not $evt) { continue }`,
    `  $ti = $evt.TargetInstance`,
    `  $name = ([string]$ti.Name).ToLower()`,
    `  if ($targets.ContainsKey($name)) {`,
    `    $snoozed = Get-Snoozed`,
    `    if (-not $snoozed.ContainsKey($name)) {`,
    `      try { Stop-Process -Id ([int]$ti.ProcessId) -Force } catch {}`,
    `      [Console]::Out.WriteLine('KILLED ' + $name)`,
    `      [Console]::Out.Flush()`,
    `    }`,
    `  }`,
    `}`,
  ].join("\n");

  watcherProcess = spawn(
    "powershell.exe",
    ["-NonInteractive", "-NoProfile", "-Command", psScript],
    { windowsHide: true },
  );

  let buf = "";
  watcherProcess.stdout?.on("data", (d: Buffer) => {
    buf += d.toString();
    const lines = buf.split(/\r?\n/);
    buf = lines.pop() ?? "";
    for (const line of lines) {
      const m = line.trim().match(/^KILLED\s+(.+)$/i);
      if (!m) continue;
      const app = appByExe(m[1]!.trim(), options.blockedApps);
      if (app) {
        console.log(`[focus-watcher] killed on launch: ${m[1]}`);
        notifyBlocked(app, options.notifier);
      }
    }
  });
  watcherProcess.stderr?.on("data", (d: Buffer) =>
    console.warn("[focus-watcher] ps stderr:", d.toString().slice(0, 200)),
  );
  watcherProcess.on("error", (e) => console.warn("[focus-watcher] spawn error:", e));
  watcherProcess.on("exit", () => {
    watcherProcess = null;
  });

  backupSweepTimer = setInterval(() => {
    void backupSweep(options.blockedApps, options.notifier);
  }, 1500);
}

export function stopProcessWatcher(): void {
  if (watcherProcess) {
    try {
      watcherProcess.kill();
    } catch {
      /* ignore */
    }
    watcherProcess = null;
  }
  if (backupSweepTimer) {
    clearInterval(backupSweepTimer);
    backupSweepTimer = null;
  }
  try {
    unlinkSync(SNOOZE_FILE);
  } catch {
    /* ignore */
  }
}

export async function killRunningBlockedApps(
  blockedApps: FocusBlockedApp[],
  notifier: ((app: { id: string; title: string; icon?: string | null }) => void) | null,
): Promise<void> {
  for (const app of blockedApps) {
    const exe = exeNameOf(app);
    if (!exe || isBlockedFocusAppSnoozed(app.id)) continue;
    if (!(await isProcessRunning(exe))) continue;
    await killExe(exe);
    notifyBlocked(app, notifier);
  }
}
