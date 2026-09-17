import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { ElectronApplication } from "playwright";
import { hostE2eEnvironment } from "./host-environment";

export { hostE2eEnvironment };
export type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };
type EngineBinaries = { engine: string; ark: string };
export type Lock = { pid: number; http_port: number; auth_token: string };
type CleanupPid = { pid: number; createdAt?: string };
const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const trackedPidCreatedAt = new Map<number, string>();
const isString = (value: JsonValue | undefined): value is string => typeof value === "string";
export const waitFor = async <T>(read: () => T | undefined, label: string): Promise<T> => {
  const until = Date.now() + 30_000;
  while (Date.now() < until) {
    const value = read();
    if (value !== undefined) return value;
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`timed out waiting for ${label}`);
};

export const cargoTarget = (): string =>
  // SAFETY: cargo metadata --format-version=1 guarantees target_directory.
  JSON.parse(
    execFileSync("cargo", ["metadata", "--no-deps", "--format-version=1"], {
      cwd: repositoryRoot,
      encoding: "utf8",
      env: hostE2eEnvironment(),
    }),
  ).target_directory;

export const executableName = (base: string): string =>
  process.platform === "win32" ? `${base}.exe` : base;

export const buildEngine = (
  trust: { root: string; releases: string; storeKeyId?: string; storePublicKey?: string },
  force = false,
): EngineBinaries => {
  const env = hostE2eEnvironment({
    KOSMOS_PACKAGE_ROOT_KEY_JSON: trust.root,
    KOSMOS_PACKAGE_RELEASE_KEYS_JSON: trust.releases,
  });
  if (trust.storeKeyId) env.KOSMOS_STORE_CATALOG_KEY_ID = trust.storeKeyId;
  if (trust.storePublicKey) env.KOSMOS_STORE_CATALOG_PUBLIC_KEY_B64 = trust.storePublicKey;
  const target = cargoTarget();
  const binaries: EngineBinaries = {
    engine: path.join(target, "debug", executableName("kepler-backend")),
    ark: path.join(target, "debug", executableName("ark-core-rpc")),
  };
  if (!force && fs.existsSync(binaries.engine) && fs.existsSync(binaries.ark)) return binaries;
  execFileSync(
    "node",
    [
      path.join(repositoryRoot, "desktop", "scripts", "ark-core-rpc.mjs"),
      "--debug",
      "--target-dir",
      path.join(target, "debug"),
    ],
    {
      cwd: repositoryRoot,
      env,
      stdio: "inherit",
    },
  );
  execFileSync("cargo", ["build", "-p", "kepler-backend"], {
    cwd: repositoryRoot,
    env,
    stdio: "inherit",
  });
  return binaries;
};

export const startEngine = async (
  engine: string,
  ark: string,
  dataDir: string,
  environment: NodeJS.ProcessEnv = {},
): Promise<{ child: ChildProcess; lock: Lock }> => {
  const lockPath = path.join(dataDir, "engine.lock.json");
  try {
    // SAFETY: only an Engine-created lock can exist in this test-owned data directory.
    const stale = JSON.parse(fs.readFileSync(lockPath, "utf8")) as Lock;
    if (Number.isInteger(stale.pid) && isPidAlive(stale.pid))
      throw new Error(`Engine PID ${stale.pid} is still running`);
    fs.rmSync(lockPath, { force: true });
  } catch (error) {
    if (error instanceof Error && error.message.startsWith("Engine PID ")) throw error;
    fs.rmSync(lockPath, { force: true });
  }
  const child = spawn(engine, [], {
    env: hostE2eEnvironment({
      KOSMOS_DATA_DIR: dataDir,
      ARK_CORE_RPC_PATH: ark,
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
      KEPLER_USAGE_TRACKER: "0",
      ...environment,
    }),
    stdio: "ignore",
    windowsHide: true,
  });
  if (child.pid) {
    const createdAt = processCreatedAt(child.pid);
    if (!createdAt) throw new Error(`Engine PID ${child.pid} identity is unavailable`);
    trackedPidCreatedAt.set(child.pid, createdAt);
  }
  try {
    const lock = await waitFor(() => {
      try {
        // SAFETY: this test-owned path is written only by the Engine lock serializer.
        const candidate = JSON.parse(fs.readFileSync(lockPath, "utf8")) as Lock;
        return Number.isInteger(candidate.pid) && isPidAlive(candidate.pid) ? candidate : undefined;
      } catch {
        return undefined;
      }
    }, "Engine lock");
    return { child, lock };
  } catch (error) {
    if (child.pid)
      await forceStop(child.pid, "failed Engine startup", trackedPidCreatedAt.get(child.pid));
    throw error;
  }
};

export const rpc = async (
  lock: Lock,
  operation: string,
  params: Record<string, JsonValue> = {},
) => {
  const requestId = randomUUID();
  console.log(`[host-e2e] rpc start request_id=${requestId} operation=${operation}`);
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      Connection: "close",
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "desktop-host",
      "X-Kosmos-Client-Version": "1.0.0",
      "X-Kosmos-Client-Pid": String(process.pid),
    },
    body: JSON.stringify({ operation, _req_id: requestId, ...params }),
    signal: AbortSignal.timeout(20_000),
  });
  // SAFETY: the test Engine endpoint returns the documented JSON RPC envelope.
  const result = (await response.json()) as { ok: boolean; data?: JsonValue; error?: JsonValue };
  console.log(
    `[host-e2e] rpc result request_id=${requestId} operation=${operation} ok=${result.ok}`,
  );
  return result;
};

export const rpcError = (result: { error?: JsonValue }): string =>
  isString(result.error) ? result.error.slice(0, 256) : "unknown error";

const isPidAlive = (pid: number): boolean => {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
};

type ProcRow = { pid: number; ppid: number; start: string };

// Linux: /proc/<pid>/stat field 22 (starttime, jiffies since boot) identifies
// the process incarnation; comm may contain spaces/parens so parse after the
// last ")". Windows: WMI CreationDate (ISO timestamp).
const readProcStat = (pid: number): ProcRow | undefined => {
  try {
    const stat = fs.readFileSync(`/proc/${pid}/stat`, "utf8");
    const close = stat.lastIndexOf(")");
    if (close < 0) return undefined;
    const fields = stat.slice(close + 2).split(" ");
    const ppid = Number(fields[1]);
    const start = fields[19];
    return Number.isInteger(ppid) && start !== undefined && start.length > 0
      ? { pid, ppid, start }
      : undefined;
  } catch {
    return undefined;
  }
};

const listProcStats = (): ProcRow[] => {
  try {
    return fs
      .readdirSync("/proc")
      .filter((name) => /^\d+$/.test(name))
      .flatMap((name) => {
        const row = readProcStat(Number(name));
        return row ? [row] : [];
      });
  } catch {
    return [];
  }
};

const processCreatedAt = (pid: number): string | undefined => {
  if (process.platform !== "win32") return readProcStat(pid)?.start;
  try {
    const createdAt = execFileSync(
      "powershell.exe",
      [
        "-NoProfile",
        "-NonInteractive",
        "-Command",
        `Get-CimInstance Win32_Process -Filter 'ProcessId = ${pid}' | Select-Object -ExpandProperty CreationDate`,
      ],
      { encoding: "utf8", windowsHide: true },
    ).trim();
    return createdAt || undefined;
  } catch {
    return undefined;
  }
};

const isSameProcess = (pid: number, createdAt?: string): boolean =>
  createdAt !== undefined && isPidAlive(pid) && processCreatedAt(pid) === createdAt;

const processTreePidsPosix = (rootPid: number): Set<number> => {
  const rows = listProcStats();
  const rootRow = readProcStat(rootPid);
  const rootCreatedAt = trackedPidCreatedAt.get(rootPid);
  if (rootCreatedAt && rootRow?.start !== rootCreatedAt) return new Set([rootPid]);
  const result = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const row of rows) {
      if (result.has(row.ppid) && !result.has(row.pid)) {
        result.add(row.pid);
        changed = true;
      }
    }
  }
  for (const row of rows) if (result.has(row.pid)) trackedPidCreatedAt.set(row.pid, row.start);
  return result;
};

export const processTreePids = (rootPid: number): Set<number> => {
  if (process.platform !== "win32") return processTreePidsPosix(rootPid);
  const output = execFileSync(
    "powershell.exe",
    [
      "-NoProfile",
      "-NonInteractive",
      "-Command",
      "Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId,CreationDate | ConvertTo-Json -Compress",
    ],
    { encoding: "utf8", windowsHide: true },
  ).trim();
  // SAFETY: the PowerShell projection emits process IDs, parent IDs, and creation timestamps.
  const parsed = output
    ? (JSON.parse(output) as
        | { ProcessId: number; ParentProcessId: number; CreationDate?: string }
        | Array<{ ProcessId: number; ParentProcessId: number; CreationDate?: string }>)
    : [];
  const processes = Array.isArray(parsed) ? parsed : [parsed];
  const rootCreatedAt = trackedPidCreatedAt.get(rootPid);
  if (
    rootCreatedAt &&
    !processes.some(
      (process) => process.ProcessId === rootPid && process.CreationDate === rootCreatedAt,
    )
  )
    return new Set([rootPid]);
  const result = new Set([rootPid]);
  let changed = true;
  while (changed) {
    changed = false;
    for (const process of processes) {
      if (result.has(process.ParentProcessId) && !result.has(process.ProcessId)) {
        result.add(process.ProcessId);
        changed = true;
      }
    }
  }
  for (const process of processes)
    if (result.has(process.ProcessId) && process.CreationDate)
      trackedPidCreatedAt.set(process.ProcessId, process.CreationDate);
  return result;
};

export const waitForPidGone = async (
  pid: number,
  label: string,
  createdAt?: string,
): Promise<void> => {
  const deadline = Date.now() + 10_000;
  while (isSameProcess(pid, createdAt) && Date.now() < deadline)
    await new Promise((resolve) => setTimeout(resolve, 100));
  if (isSameProcess(pid, createdAt)) throw new Error(`${label} PID ${pid} is still alive`);
};

const forceStop = async (pid: number, label: string, createdAt?: string): Promise<void> => {
  if (!isSameProcess(pid, createdAt)) return;
  if (process.platform === "win32") {
    try {
      execFileSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
        windowsHide: true,
        stdio: "ignore",
        timeout: 5_000,
      });
    } catch {}
  } else {
    try {
      process.kill(pid, "SIGKILL");
    } catch {}
  }
  await waitForPidGone(pid, label, createdAt);
};

export const crashProcessTree = async (
  child: ChildProcess | undefined,
  label: string,
): Promise<Set<number>> => {
  const pid = child?.pid;
  const createdAt = pid ? trackedPidCreatedAt.get(pid) : undefined;
  if (!pid || !createdAt || !isSameProcess(pid, createdAt))
    throw new Error(`${label} is not running`);
  const pids = processTreePids(pid);
  const alivePids = new Set([...pids].filter(isPidAlive));
  await forceStop(pid, label, createdAt);
  for (const processId of processTreePids(pid)) pids.add(processId);
  for (const processId of pids) {
    const descendantCreatedAt = trackedPidCreatedAt.get(processId);
    if (isSameProcess(processId, descendantCreatedAt))
      await forceStop(processId, `${label} descendant`, descendantCreatedAt);
  }
  for (const processId of pids)
    await waitForPidGone(processId, `${label} descendant`, trackedPidCreatedAt.get(processId));
  return alivePids;
};

export const terminate = async (
  child: ChildProcess | undefined,
  engine: string,
  dataDir: string,
  label: string,
): Promise<void> => {
  const pid = child?.pid;
  if (!pid) return;
  const createdAt = trackedPidCreatedAt.get(pid);
  if (!createdAt) throw new Error(`${label} PID ${pid} identity is unavailable`);
  const pids = processTreePids(pid);
  if (isSameProcess(pid, createdAt)) {
    console.log(
      `[host-e2e] teardown ${label}: pid=${pid} lock=${path.join(dataDir, "engine.lock.json")}`,
    );
    try {
      execFileSync(engine, ["--shutdown"], {
        env: hostE2eEnvironment({
          KOSMOS_DATA_DIR: dataDir,
          KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
        }),
        windowsHide: true,
        stdio: "ignore",
        timeout: 10_000,
      });
    } catch {}
    for (const processId of processTreePids(pid)) pids.add(processId);
    if (isSameProcess(pid, createdAt)) await forceStop(pid, label, createdAt);
  }
  for (const processId of pids) {
    const descendantCreatedAt = trackedPidCreatedAt.get(processId);
    if (isSameProcess(processId, descendantCreatedAt))
      await forceStop(processId, `${label} descendant`, descendantCreatedAt);
  }
  for (const processId of pids)
    await waitForPidGone(processId, `${label} descendant`, trackedPidCreatedAt.get(processId));
};

export const closeHost = async (
  host: ElectronApplication | undefined,
  trackedPids: Set<number>,
): Promise<void> => {
  if (!host) return;
  const child = host.process();
  if (child.exitCode !== null || child.signalCode !== null) return;
  const pid = child.pid;
  const createdAt = processCreatedAt(pid);
  if (!createdAt) throw new Error(`Host PID ${pid} identity is unavailable`);
  const pids = processTreePids(pid);
  for (const processId of pids) trackedPids.add(processId);
  console.log(`[host-e2e] teardown Host: pid=${pid}`);
  const closePromise = host.close().catch(() => undefined);
  const closeTimedOut = !(await Promise.race([
    closePromise.then(() => true),
    new Promise<boolean>((resolve) => setTimeout(() => resolve(false), 5_000)),
  ]));
  if (isSameProcess(pid, createdAt))
    for (const processId of processTreePids(pid)) {
      pids.add(processId);
      trackedPids.add(processId);
    }
  try {
    if (isSameProcess(pid, createdAt)) await forceStop(pid, "Host", createdAt);
    for (const processId of pids) {
      if (processId === pid || !isPidAlive(processId)) continue;
      const descendantCreatedAt = trackedPidCreatedAt.get(processId);
      if (!descendantCreatedAt)
        throw new Error(`Host descendant PID ${processId} identity is unavailable`);
      await forceStop(processId, "Host descendant", descendantCreatedAt);
    }
    await waitForPidGone(pid, "Host", createdAt);
    for (const processId of pids) {
      if (processId === pid) continue;
      const descendantCreatedAt = trackedPidCreatedAt.get(processId);
      if (descendantCreatedAt)
        await waitForPidGone(processId, "Host descendant", descendantCreatedAt);
    }
    for (const processId of pids) trackedPids.delete(processId);
  } finally {
    if (closeTimedOut)
      await Promise.race([
        closePromise,
        new Promise<void>((resolve) => setTimeout(resolve, 5_000)),
      ]);
  }
};

export const recordCleanup = (manifestPath: string, root: string, pids: Set<number>): void => {
  // SAFETY: the cleanup manifest is written by this test with the expected arrays.
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as {
    roots?: unknown;
    pids?: unknown;
  };
  if (!Array.isArray(manifest.roots) || !Array.isArray(manifest.pids))
    throw new Error("invalid Host E2E cleanup manifest");
  if (!manifest.roots.includes(root)) manifest.roots.push(root);
  for (const pid of pids) {
    const createdAt = trackedPidCreatedAt.get(pid);
    if (createdAt) manifest.pids.push({ pid, createdAt } satisfies CleanupPid);
    else if (isPidAlive(pid)) throw new Error(`cleanup PID ${pid} identity is unavailable`);
  }
  fs.writeFileSync(manifestPath, JSON.stringify(manifest));
};
