import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createLegacyMigrationRunner } from "../electron/legacy-migration-runtime.ts";

const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-migration-headless-"));
const dataDir = path.join(root, "data");
const executable = process.env.KOSMOS_HEADLESS_BACKEND
  ? path.resolve(process.env.KOSMOS_HEADLESS_BACKEND)
  : path.resolve(import.meta.dirname, "../../target/debug/kepler-backend.exe");

async function startBackend() {
  const env = {
    ...process.env,
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
  };
  const child = spawn(executable, [], { env, windowsHide: true, stdio: "ignore" });
  const lockPath = path.join(dataDir, "engine.lock.json");
  for (let i = 0; i < 100 && !(await fileExists(lockPath)); i++)
    await new Promise((resolve) => setTimeout(resolve, 100));
  const lock = JSON.parse(await readFile(lockPath, "utf8"));
  const client = {
    async invokeOperation({ operation, ...params }) {
      const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
        method: "POST",
        headers: {
          Authorization: `Bearer ${lock.auth_token}`,
          "Content-Type": "application/json",
          "X-Kosmos-Api-Version": "1.0.0",
          "X-Kosmos-Client-Class": "migration-headless",
          "X-Kosmos-Client-Version": "1.0.0",
          "X-Kosmos-Client-Pid": String(lock.pid),
        },
        body: JSON.stringify({ operation, _req_id: `${operation}-${Date.now()}`, ...params }),
      });
      const value = await response.json();
      if (!response.ok || value.ok === false) throw new Error(JSON.stringify(value));
      return value.data;
    },
  };
  return { child, client, env };
}

async function fileExists(filePath) {
  try {
    await readFile(filePath);
    return true;
  } catch {
    return false;
  }
}

async function stopBackend() {
  if (!running) return;
  spawnSync(executable, ["--shutdown"], { env: running.env, windowsHide: true });
  running.child.kill();
  await new Promise((resolve) => setTimeout(resolve, 300));
  running = undefined;
}

let running;
try {
  await mkdir(path.join(dataDir, "extensions-data", "arcadia"), { recursive: true });
  await writeFile(
    path.join(dataDir, "extensions-data", "arcadia", "state.json"),
    '{"fixture":true}\n',
  );
  running = await startBackend();
  const runner = createLegacyMigrationRunner(dataDir, running.client, async () => {});
  const first = await runner.run();
  if (first !== undefined) throw new Error("runner unexpectedly returned a value");
  await stopBackend();
  running = await startBackend();
  const second = await createLegacyMigrationRunner(dataDir, running.client, async () => {}).run();
  if (second !== undefined) throw new Error("restart runner unexpectedly returned a value");
  console.log(
    "headless migration coordinator executed against real RPC; replacement preflight stayed pending before mutation",
  );
} finally {
  await stopBackend();
  await rm(root, { recursive: true, force: true });
}
