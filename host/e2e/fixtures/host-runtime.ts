import { execFileSync, spawn, type ChildProcess } from "node:child_process";
import { randomUUID } from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";
import type { ElectronApplication } from "playwright";

export type JsonValue =
  | string
  | number
  | boolean
  | null
  | readonly JsonValue[]
  | { readonly [key: string]: JsonValue };
type EngineBinaries = { engine: string; ark: string };
export type Lock = { pid: number; http_port: number; auth_token: string };

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..", "..", "..");

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
  JSON.parse(
    execFileSync("cargo", ["metadata", "--no-deps", "--format-version=1"], {
      cwd: repositoryRoot,
      encoding: "utf8",
    }),
  ).target_directory;

export const buildEngine = (trust: { root: string; releases: string }): EngineBinaries => {
  const env = {
    ...process.env,
    KOSMOS_PACKAGE_ROOT_KEY_JSON: trust.root,
    KOSMOS_PACKAGE_RELEASE_KEYS_JSON: trust.releases,
  };
  const target = cargoTarget();
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
  const binaries: EngineBinaries = {
    engine: path.join(target, "debug", "kepler-backend.exe"),
    ark: path.join(target, "debug", "ark-core-rpc.exe"),
  };
  return binaries;
};

export const startEngine = async (
  engine: string,
  ark: string,
  dataDir: string,
): Promise<{ child: ChildProcess; lock: Lock }> => {
  const child = spawn(engine, [], {
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      ARK_CORE_RPC_PATH: ark,
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      KEPLER_SKIP_SYNC: "1",
      KEPLER_USAGE_TRACKER: "0",
    },
    stdio: "ignore",
    windowsHide: true,
  });
  const lock = await waitFor(() => {
    try {
      // SAFETY: waitFor only resolves after the Engine lock file is written.
      return JSON.parse(fs.readFileSync(path.join(dataDir, "engine.lock.json"), "utf8")) as Lock;
    } catch {
      return undefined;
    }
  }, "Engine lock");
  return { child, lock };
};

export const rpc = async (
  lock: Lock,
  operation: string,
  params: Record<string, JsonValue> = {},
) => {
  console.log(`[host-e2e] rpc start operation=${operation}`);
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
    body: JSON.stringify({ operation, _req_id: randomUUID(), ...params }),
    signal: AbortSignal.timeout(20_000),
  });
  // SAFETY: the test Engine endpoint returns the documented JSON RPC envelope.
  const result = (await response.json()) as { ok: boolean; data?: JsonValue; error?: JsonValue };
  console.log(
    `[host-e2e] rpc result operation=${operation} ok=${result.ok} error=${rpcError(result)}`,
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

export const waitForPidGone = async (pid: number, label: string): Promise<void> => {
  const deadline = Date.now() + 10_000;
  while (isPidAlive(pid) && Date.now() < deadline)
    await new Promise((resolve) => setTimeout(resolve, 100));
  if (isPidAlive(pid)) throw new Error(`${label} PID ${pid} is still alive`);
};

const forceStop = async (pid: number, label: string): Promise<void> => {
  try {
    execFileSync("taskkill.exe", ["/PID", String(pid), "/T", "/F"], {
      windowsHide: true,
      stdio: "ignore",
      timeout: 5_000,
    });
  } catch {}
  await waitForPidGone(pid, label);
};

export const terminate = async (
  child: ChildProcess | undefined,
  engine: string,
  dataDir: string,
  label: string,
): Promise<void> => {
  const pid = child?.pid;
  if (!pid || !isPidAlive(pid)) return;
  console.log(
    `[host-e2e] teardown ${label}: pid=${pid} lock=${path.join(dataDir, "engine.lock.json")}`,
  );
  try {
    execFileSync(engine, ["--shutdown"], {
      env: { ...process.env, KOSMOS_DATA_DIR: dataDir, KOSMOS_LOCK_PERMISSIONS_DISABLED: "1" },
      windowsHide: true,
      stdio: "ignore",
      timeout: 10_000,
    });
  } catch {}
  if (isPidAlive(pid)) await forceStop(pid, label);
  await waitForPidGone(pid, label);
};

export const closeHost = async (host: ElectronApplication | undefined): Promise<void> => {
  if (!host) return;
  const pid = host.process().pid;
  console.log(`[host-e2e] teardown Host: pid=${pid}`);
  await Promise.race([
    host.close().catch(() => undefined),
    new Promise((resolve) => setTimeout(resolve, 5_000)),
  ]);
  if (isPidAlive(pid)) await forceStop(pid, "Host");
  await waitForPidGone(pid, "Host");
};

export const recordCleanup = (manifestPath: string, root: string, pids: Set<number>): void => {
  // SAFETY: the cleanup manifest is written by this test with the expected arrays.
  const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8")) as {
    roots?: unknown;
    pids?: unknown;
  };
  if (!Array.isArray(manifest.roots) || !Array.isArray(manifest.pids))
    throw new Error("invalid Host E2E cleanup manifest");
  manifest.roots.push(root);
  manifest.pids.push(...pids);
  fs.writeFileSync(manifestPath, JSON.stringify(manifest));
};
