import fs from "node:fs";
import path from "node:path";
import { randomUUID } from "node:crypto";
import { fileURLToPath } from "node:url";
import { _electron as electron, type ElectronApplication } from "playwright";
import electronBinary from "electron";

export const root = path.resolve(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
);
export const runRoot = path.join(
  root,
  ".e2e",
  "focus",
  `${process.pid}-${Date.now()}`,
);
export const dataDir = path.join(runRoot, "data");
const lockPath = path.join(dataDir, "engine.lock.json");
export const engineBinary = path.resolve(
  root,
  "..",
  "..",
  "target",
  "debug",
  "kepler-backend.exe",
);
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

function readLock() {
  try {
    // SAFETY: the fixture lock file is written by the Engine launcher with this schema.
    const value = JSON.parse(fs.readFileSync(lockPath, "utf8")) as {
      pid: number;
      http_port: number;
      auth_token: string;
    };
    return value.pid > 0 && value.http_port > 0 ? value : null;
  } catch {
    return null;
  }
}

export async function engineRpc(
  lockValue: { http_port: number; auth_token: string },
  operation: string,
  input: { [key: string]: string | string[] | undefined } = {},
) {
  const response = await fetch(
    `http://127.0.0.1:${lockValue.http_port}/v1/rpc`,
    {
      method: "POST",
      headers: {
        Authorization: `Bearer ${lockValue.auth_token}`,
        "Content-Type": "application/json",
        "X-Kosmos-Api-Version": "1.0.0",
        "X-Kosmos-Client-Class": "focus-fixture",
        "X-Kosmos-Client-Version": "test",
        "X-Kosmos-Client-Pid": String(process.pid),
      },
      body: JSON.stringify({ operation, _req_id: randomUUID(), ...input }),
    },
  );
  // SAFETY: the fixture endpoint returns the tested RPC envelope.
  return (await response.json()) as { ok: boolean; data?: unknown };
}

export async function waitLock() {
  for (let i = 0; i < 150; i++) {
    const value = readLock();
    if (value) return value;
    await wait(200);
  }
  throw new Error("Engine lock timeout");
}

export async function launch(
  slot: string,
  serviceState: "absent" | "installed",
  action = "",
): Promise<ElectronApplication> {
  const userData = path.join(runRoot, slot, "userdata");
  fs.mkdirSync(userData, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: root,
    args: [
      `--user-data-dir=${userData}`,
      path.join(root, "dist-electron", "main.js"),
    ],
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
      KOSMOS_TEST_FOCUS_SERVICE: serviceState,
      KOSMOS_TEST_FOCUS_SERVICE_ACTION: action,
      KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
      NODE_ENV: "test",
    },
  });
}

export async function launchMissing(
  slot: string,
): Promise<ElectronApplication> {
  const userData = path.join(runRoot, slot, "userdata");
  fs.mkdirSync(userData, { recursive: true });
  return electron.launch({
    executablePath: electronBinary,
    cwd: root,
    args: [
      `--user-data-dir=${userData}`,
      path.join(root, "dist-electron", "missing-main.js"),
    ],
    env: {
      ...process.env,
      KOSMOS_DATA_DIR: dataDir,
      KOSMOS_HEADLESS: "1",
      KOSMOS_TEST_MODE: "1",
    },
    timeout: 30_000,
  });
}
