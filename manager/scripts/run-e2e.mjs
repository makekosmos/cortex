import { execFileSync, spawn } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import process from "node:process";
import { createRequire } from "node:module";

const managerRoot = path.resolve(import.meta.dirname, "..");
const repositoryRoot = path.resolve(managerRoot, "..");
const tempRoot = path.resolve(os.tmpdir());
const manifestPath = path.join(repositoryRoot, ".tmp", `manager-e2e-cleanup-${randomUUID()}.json`);
const playwrightCli = createRequire(import.meta.url).resolve("@playwright/test/cli");
fs.mkdirSync(path.dirname(manifestPath), { recursive: true });
fs.writeFileSync(manifestPath, JSON.stringify({ roots: [], pids: [] }));

let exited = 1;
try {
  exited = await new Promise((resolve) => {
    const child = spawn(
      process.execPath,
      [playwrightCli, "test", "--config", "playwright.config.ts", ...process.argv.slice(2)],
      {
        cwd: managerRoot,
        env: { ...process.env, KOSMOS_MANAGER_E2E_CLEANUP_MANIFEST: manifestPath },
        stdio: "inherit",
        windowsHide: true,
      },
    );
    child.once("error", (error) => {
      console.error(`[manager-e2e] runner failed: ${error.message}`);
      resolve(1);
    });
    child.once("exit", (code) => resolve(code ?? 1));
  });
} catch (error) {
  console.error(
    `[manager-e2e] runner failed: ${error instanceof Error ? error.message : String(error)}`,
  );
}

let cleanupFailed = false;
const isString = (value) => value?.constructor === String;
const isWindows = process.platform === "win32";
const readProcStart = (pid) => {
  try {
    const stat = fs.readFileSync(`/proc/${pid}/stat`, "utf8");
    const close = stat.lastIndexOf(")");
    if (close < 0) return undefined;
    return stat.slice(close + 2).split(" ")[19] || undefined;
  } catch {
    return undefined;
  }
};
const ownedPidsWindows = (root) => {
  const literal = root.replaceAll("'", "''");
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and $_.CommandLine.Contains('${literal}') } | Select-Object ProcessId,CreationDate | ConvertTo-Json -Compress`,
    ],
    { encoding: "utf8", windowsHide: true },
  ).trim();
  if (!output) return [];
  const parsed = JSON.parse(output);
  return (Array.isArray(parsed) ? parsed : [parsed]).filter(
    (entry) =>
      Number.isInteger(entry.ProcessId) &&
      entry.ProcessId > 0 &&
      entry.ProcessId !== process.pid &&
      isString(entry.CreationDate),
  );
};
const ownedPidsPosix = (root) => {
  const out = [];
  for (const name of fs.readdirSync("/proc")) {
    if (!/^\d+$/.test(name)) continue;
    const pid = Number(name);
    if (pid === process.pid) continue;
    try {
      const cmdline = fs.readFileSync(`/proc/${name}/cmdline`, "utf8").replaceAll("\0", " ");
      if (!cmdline.includes(root)) continue;
      const createdAt = readProcStart(pid);
      if (createdAt) out.push({ ProcessId: pid, CreationDate: createdAt });
    } catch {}
  }
  return out;
};
const ownedPids = (root) => (isWindows ? ownedPidsWindows(root) : ownedPidsPosix(root));
const processCreatedAt = (pid) => {
  if (!isWindows) return readProcStart(pid);
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      `Get-CimInstance Win32_Process -Filter 'ProcessId = ${pid}' | Select-Object -ExpandProperty CreationDate`,
    ],
    { encoding: "utf8", windowsHide: true },
  ).trim();
  return output || undefined;
};
const forceKill = (pid) => {
  if (isWindows) {
    execFileSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
      stdio: "ignore",
      windowsHide: true,
    });
  } else {
    process.kill(pid, "SIGKILL");
  }
};
try {
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (!Array.isArray(manifest.roots) || !Array.isArray(manifest.pids))
    throw new Error("invalid cleanup manifest");
  for (const root of manifest.roots) {
    const resolved = isString(root) ? path.resolve(root) : "";
    if (
      path.dirname(resolved).toLowerCase() !== tempRoot.toLowerCase() ||
      !path.basename(resolved).startsWith("kosmos-manager-e2e-")
    )
      throw new Error(`unsafe cleanup root: ${root}`);
    for (const { ProcessId: pid, CreationDate: createdAt } of ownedPids(resolved)) {
      if (processCreatedAt(pid) !== createdAt) continue;
      try {
        forceKill(pid);
      } catch {}
    }
    fs.rmSync(resolved, { recursive: true, force: true, maxRetries: 50, retryDelay: 100 });
    if (fs.existsSync(resolved)) throw new Error(`cleanup root remains: ${resolved}`);
    console.log(`[manager-e2e] cleaned ${resolved}`);
  }
  for (const entry of manifest.pids) {
    const pid = Number.isInteger(entry) ? entry : entry?.pid;
    const createdAt = Number.isInteger(entry) ? undefined : entry?.createdAt;
    if (!Number.isInteger(pid) || pid <= 0) throw new Error(`invalid cleanup PID: ${pid}`);
    if (createdAt && processCreatedAt(pid) !== createdAt) continue;
    try {
      process.kill(pid, 0);
      throw new Error(`cleanup PID remains alive: ${pid}`);
    } catch (error) {
      if (error instanceof Error && error.message.startsWith("cleanup PID")) throw error;
    }
  }
} catch (error) {
  cleanupFailed = true;
  console.error(
    `[manager-e2e] cleanup failed: ${error instanceof Error ? error.message : String(error)}`,
  );
} finally {
  fs.rmSync(manifestPath, { force: true });
}

process.exitCode = exited === 0 && !cleanupFailed ? 0 : 1;
