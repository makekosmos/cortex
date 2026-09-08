import { mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
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

function runCoordinator(dataDir, barrier, barrierPhase, recoveryOnly = false, skipSetup = false) {
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
  if (allowKilled ? code !== null : code !== 0)
    throw new Error(`coordinator exited with code ${code}`);
  return code;
}

async function stopBackend(running) {
  spawnSync(backend, ["--shutdown"], { env: running.env, windowsHide: true });
  running.child.kill();
  await waitForExit(running.child).catch(() => {});
  await waitForMissing(path.join(running.env.KOSMOS_DATA_DIR, "engine.lock.json"));
}

async function runCase(dataDir, phase, recoveryOnly, skipSetup) {
  const barrier = path.join(dataDir, `crash-barrier-${phase}-${randomUUID()}`);
  const coordinator = runCoordinator(dataDir, barrier, phase, false, skipSetup);
  await waitFor(coordinator.barrier);
  const transactionBeforeRecovery =
    phase === "prepared" ? await legacyGrantTransactions(dataDir) : undefined;
  const journalBeforeRecovery =
    phase === "prepared"
      ? JSON.parse(
          await readFile(
            path.join(dataDir, "legacy-migrations", "v1", "com.kosmos.arcadia", "journal.json"),
            "utf8",
          ),
        )
      : undefined;
  coordinator.child.kill();
  await waitForExit(coordinator.child, true);
  if (phase === "prepared" && !isLegacyLaunchBlocked(dataDir, "arcadia"))
    throw new Error("prepared crash did not block legacy launch before recovery");
  const recovery = runCoordinator(dataDir, `${barrier}.unused`, "", recoveryOnly, true);
  await waitForExit(recovery.child);
  return { dataDir, transactionBeforeRecovery, journalBeforeRecovery };
}

async function packageState(dataDir) {
  const value = JSON.parse(await readFile(path.join(dataDir, "packages", "state.json"), "utf8"));
  return value.packages.filter((row) => row.id === "com.kosmos.arcadia");
}

async function packageStatePending(dataDir) {
  return exists(
    path.join(
      dataDir,
      "legacy-migrations",
      "v1",
      "com.kosmos.arcadia",
      "before",
      "package-state.pending",
    ),
  );
}

async function grantState(dataDir) {
  return JSON.stringify(
    JSON.parse(await readFile(path.join(dataDir, "grant-authority.json"), "utf8")),
  );
}

async function legacyGrantTransactions(dataDir) {
  return JSON.parse(await readFile(path.join(dataDir, "legacy-grant-transactions.json"), "utf8"));
}

async function packageStateSnapshot(dataDir) {
  return JSON.parse(
    await readFile(
      path.join(
        dataDir,
        "legacy-migrations",
        "v1",
        "com.kosmos.arcadia",
        "before",
        "package-state.json",
      ),
      "utf8",
    ),
  );
}

async function removeOwnedRoot() {
  for (let attempt = 0; attempt < 40; attempt++) {
    try {
      await rm(root, { recursive: true, force: true });
      return;
    } catch (error) {
      if (!(error instanceof Error) || error.code !== "EBUSY") throw error;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
  }
  return false;
}

try {
  const dataDir = path.join(root, "data");
  await mkdir(dataDir, { recursive: true });
  const grantFixture = [
    {
      version: 1,
      persistent_grant_id: "arcadia-crash-grant",
      extension_id: "arcadia",
      provenance: "NativeDialog",
      exact_file: false,
      selected_path: dataDir,
      root_identity: { primary: 1, secondary: 2 },
      exact_file_identity: null,
      revoked: false,
      unknown_scope: { read: ["preserve"], write: ["deny"] },
    },
  ];
  await writeFile(path.join(dataDir, "grant-authority.json"), `${JSON.stringify(grantFixture)}\n`);
  const running = await startBackend(dataDir);
  const grantsBefore = await grantState(dataDir);
  const preparedRun = await runCase(dataDir, "prepared", true, false);
  const preparedDir = preparedRun.dataDir;
  const transactionsBeforeRecovery = preparedRun.transactionBeforeRecovery;
  if (
    !transactionsBeforeRecovery ||
    transactionsBeforeRecovery.length !== 1 ||
    transactionsBeforeRecovery[0].state !== "active"
  )
    throw new Error("prepared crash did not persist its exact grant transaction");
  if (
    preparedRun.journalBeforeRecovery.grant_transaction_token !==
    transactionsBeforeRecovery[0].token
  )
    throw new Error("prepared crash journal lost the exact grant transaction token");
  const transactionsAfterRecovery = await legacyGrantTransactions(preparedDir);
  if (
    transactionsAfterRecovery.length !== 1 ||
    transactionsAfterRecovery[0].state !== "restored" ||
    transactionsAfterRecovery[0].token !== transactionsBeforeRecovery[0].token
  )
    throw new Error("prepared crash did not roll back its exact grant transaction");
  const restoredToken = transactionsAfterRecovery[0].token;
  if (
    !restoredToken ||
    JSON.stringify(transactionsAfterRecovery[0].records) !==
      JSON.stringify(transactionsBeforeRecovery[0].records)
  )
    throw new Error("prepared crash lost the exact grant transaction token");
  if (
    await exists(
      path.join(preparedDir, "legacy-migrations", "v1", "com.kosmos.arcadia", "journal.json"),
    )
  )
    throw new Error("prepared crash recovery left a journal");
  if (!(await exists(path.join(preparedDir, "extensions-data", "arcadia", "state.json"))))
    throw new Error("prepared crash recovery lost legacy data");
  const legacyState = JSON.parse(
    await readFile(path.join(preparedDir, "extensions-data", "arcadia", "state.json"), "utf8"),
  );
  if (legacyState.unknownSettingsField?.preserve !== "yes")
    throw new Error("prepared crash recovery lost unknown settings");
  if ((await grantState(preparedDir)) !== grantsBefore)
    throw new Error("prepared crash recovery did not restore grants");
  const packageRowsAfterRecovery = await packageState(preparedDir);
  const packageSnapshot = await packageStateSnapshot(preparedDir);
  if (
    JSON.stringify(
      packageRowsAfterRecovery.map(({ version, enabled }) => ({ version, enabled })),
    ) !== JSON.stringify(packageSnapshot)
  )
    throw new Error("prepared crash recovery did not restore package state");
  if (await packageStatePending(preparedDir))
    throw new Error("prepared crash recovery left package-state.pending");
  const preparedActive = (await packageState(preparedDir)).filter((row) => row.enabled === true);
  if (preparedActive.length !== 1 || preparedActive[0].version !== "0.1.8")
    throw new Error("prepared crash recovery did not restore worker state");

  const recoveryAgain = runCoordinator(dataDir, `${randomUUID()}.unused`, "", true, true);
  await waitForExit(recoveryAgain.child);
  const transactionsAfterRetry = await legacyGrantTransactions(preparedDir);
  if (JSON.stringify(transactionsAfterRetry) !== JSON.stringify(transactionsAfterRecovery))
    throw new Error(`idempotent recovery changed grant transaction ${restoredToken}`);
  if ((await grantState(preparedDir)) !== grantsBefore)
    throw new Error("idempotent recovery changed restored grants");
  if (await packageStatePending(preparedDir))
    throw new Error("idempotent recovery recreated package-state.pending");
  if (
    await exists(
      path.join(preparedDir, "legacy-migrations", "v1", "com.kosmos.arcadia", "journal.json"),
    )
  )
    throw new Error("idempotent recovery recreated the migration journal");

  const committedDir = (await runCase(dataDir, "committed", false, true)).dataDir;
  const journal = JSON.parse(
    await readFile(
      path.join(committedDir, "legacy-migrations", "v1", "com.kosmos.arcadia", "journal.json"),
      "utf8",
    ),
  );
  if (journal.phase !== "committed") throw new Error("committed crash lost committed journal");
  const active = (await packageState(committedDir)).filter(
    (row) => row.enabled === true && row.version === "0.1.8",
  );
  if (active.length !== 1) throw new Error("committed recovery did not leave one active worker");
  if (await packageStatePending(committedDir))
    throw new Error("committed recovery left package-state.pending");
  console.log("headless migration crash cases passed: prepared rollback and committed restart");
  await stopBackend(running);
} finally {
  await removeOwnedRoot()
    .then((removed) => {
      if (!removed) console.warn(`temporary crash-harness data retained while locked: ${root}`);
    })
    .catch(() => console.warn(`temporary crash-harness data retained while locked: ${root}`));
}
