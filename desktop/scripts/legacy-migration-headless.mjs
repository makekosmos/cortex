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
const fixtureDir = process.env.KOSMOS_HEADLESS_CATALOG_FIXTURE;
const replacementArchive = process.env.KOSMOS_HEADLESS_REPLACEMENT_ARCHIVE;

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
    async invokeOperation({ operation, params = {}, ...requestFields }) {
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
        body: JSON.stringify({
          operation,
          _req_id: `${operation}-${Date.now()}`,
          ...params,
          ...requestFields,
        }),
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
  if (fixtureDir && replacementArchive) {
    const fixture = JSON.parse(
      await readFile(path.join(path.resolve(fixtureDir), "catalog-apply-request.json"), "utf8"),
    );
    await running.client.invokeOperation({
      operation: "packages.catalog_apply",
      params: { document: fixture.document, signatures: fixture.signatures },
    });
    await running.client.invokeOperation({
      operation: "packages.install_development",
      params: {
        id: "com.kosmos.arcadia",
        version: "0.1.8",
        archive_path: path.resolve(replacementArchive),
      },
    });
  }
  const runner = createLegacyMigrationRunner(dataDir, running.client, async () => {});
  const first = await runner.run();
  if (first !== undefined) throw new Error("runner unexpectedly returned a value");
  const journal = JSON.parse(
    await readFile(
      path.join(dataDir, "legacy-migrations", "v1", "com.kosmos.arcadia", "journal.json"),
      "utf8",
    ),
  );
  if (journal.phase !== "committed") throw new Error("migration did not commit");
  const packages = await running.client.invokeOperation({
    operation: "packages.list",
    params: { kind: "app" },
  });
  const active = packages.packages?.filter(
    (row) => row.id === "com.kosmos.arcadia" && row.enabled === true,
  );
  if (active?.length !== 1 || active[0].version !== "0.1.8")
    throw new Error("migration did not leave exactly one active replacement");
  const migrated = JSON.parse(
    await readFile(
      path.join(dataDir, "extensions-data", "com.kosmos.arcadia", "state.json"),
      "utf8",
    ),
  );
  if (migrated.fixture !== true) throw new Error("canonical user data was not migrated");
  await stopBackend();
  running = await startBackend();
  const second = await createLegacyMigrationRunner(dataDir, running.client, async () => {}).run();
  if (second !== undefined) throw new Error("restart runner unexpectedly returned a value");
  console.log("headless production migration committed and recovered across restart");
} finally {
  await stopBackend();
  await rm(root, { recursive: true, force: true });
}
