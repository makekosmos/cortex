import { mkdir, mkdtemp, readFile, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { isLegacyLaunchBlocked } from "../electron/legacy-migration-journal.ts";

const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-migration-crash-"));
const backend = path.resolve(
  process.env.KOSMOS_HEADLESS_BACKEND ?? "target/debug/kepler-backend.exe",
);
const fixtureDir = path.resolve(
  process.env.KOSMOS_HEADLESS_CATALOG_FIXTURE ?? "../../.tmp-cortex-catalog14-fixture",
);
const replacementArchive = path.resolve(
  process.env.KOSMOS_HEADLESS_REPLACEMENT_ARCHIVE ??
    "../../package-index-catalog-14/.tmp-artifacts/com.kosmos.arcadia-0.1.8.kspkg",
);
const runner = path.resolve(import.meta.dirname, "legacy-migration-headless.mjs");
const runtime = process.env.BUN_EXECUTABLE ?? process.execPath;

async function exists(file) {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
}

async function waitFor(file) {
  for (;;) {
    if (await exists(file)) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
}

async function waitForMissing(file) {
  for (;;) {
    if (!(await exists(file))) return;
    await new Promise((resolve) => setTimeout(resolve, 25));
  }
}

async function startBackend(dataDir) {
  const env = { ...process.env, KOSMOS_DATA_DIR: dataDir, KOSMOS_HEADLESS: "1" };
  const child = spawn(backend, [], { env, windowsHide: true, stdio: "ignore" });
  const lockPath = path.join(dataDir, "engine.lock.json");
  await waitFor(lockPath);
  return { child, env };
}

function runCoordinator(dataDir, barrierPhase, recoveryOnly = false, skipSetup = false) {
  const barrier = path.join(dataDir, "crash-barrier");
  const env = {
    ...process.env,
    KOSMOS_HEADLESS: "1",
    KOSMOS_HEADLESS_EXTERNAL_BACKEND: "1",
    KOSMOS_HEADLESS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS_CATALOG_FIXTURE: fixtureDir,
    KOSMOS_HEADLESS_REPLACEMENT_ARCHIVE: replacementArchive,
    KOSMOS_TEST_MIGRATION_BARRIER: barrier,
    KOSMOS_TEST_MIGRATION_BARRIER_PHASE: barrierPhase,
  };
  if (skipSetup) env.KOSMOS_HEADLESS_SKIP_SETUP = "1";
  if (recoveryOnly) env.KOSMOS_HEADLESS_RECOVERY_ONLY = "1";
  const child = spawn(runtime, [runner], { env, windowsHide: true, stdio: "inherit" });
  return { child, barrier };
}

async function waitForExit(child, allowKilled = false) {
  const code = await new Promise((resolve, reject) => {
    child.once("error", reject);
    child.once("exit", resolve);
  });
  if (code !== 0 && !(allowKilled && code === null))
    throw new Error(`coordinator exited with code ${code}`);
}

async function stopBackend(running) {
  spawnSync(backend, ["--shutdown"], { env: running.env, windowsHide: true });
  running.child.kill();
  await waitForExit(running.child).catch(() => {});
  await waitForMissing(path.join(running.env.KOSMOS_DATA_DIR, "engine.lock.json"));
}

async function runCase(dataDir, phase, recoveryOnly, skipSetup) {
  const coordinator = runCoordinator(dataDir, phase, false, skipSetup);
  await waitFor(coordinator.barrier);
  coordinator.child.kill();
  await waitForExit(coordinator.child, true);
  if (phase === "prepared" && !isLegacyLaunchBlocked(dataDir, "arcadia"))
    throw new Error("prepared crash did not block legacy launch before recovery");
  const recovery = runCoordinator(dataDir, "", recoveryOnly, true);
  await waitForExit(recovery.child);
  return dataDir;
}

try {
  const dataDir = path.join(root, "data");
  await mkdir(dataDir, { recursive: true });
  const running = await startBackend(dataDir);
  const preparedDir = await runCase(dataDir, "prepared", true, false);
  if (
    await exists(
      path.join(preparedDir, "legacy-migrations", "v1", "com.kosmos.arcadia", "journal.json"),
    )
  )
    throw new Error("prepared crash recovery left a journal");
  if (!(await exists(path.join(preparedDir, "extensions-data", "arcadia", "state.json"))))
    throw new Error("prepared crash recovery lost legacy data");

  const committedDir = await runCase(dataDir, "committed", false, true);
  const journal = JSON.parse(
    await readFile(
      path.join(committedDir, "legacy-migrations", "v1", "com.kosmos.arcadia", "journal.json"),
      "utf8",
    ),
  );
  if (journal.phase !== "committed") throw new Error("committed crash lost committed journal");
  console.log("headless migration crash cases passed: prepared rollback and committed restart");
  await stopBackend(running);
} finally {
  try {
    await rm(root, { recursive: true, force: true });
  } catch (error) {
    if (!(error instanceof Error) || !(error.code === "EBUSY")) throw error;
    console.warn(`temporary crash-harness data retained while locked: ${root}`);
  }
}
