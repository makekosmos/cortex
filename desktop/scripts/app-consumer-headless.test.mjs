import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import {
  catalogFixture,
  cleanup,
  runConsumer,
  runExistingUserUpgrade,
  runDictation,
  startBackend,
  stopBackend,
} from "./app-consumer-headless.mjs";

const apps = {
  arcadia: {
    id: "com.kosmos.arcadia",
    version: "0.1.11",
    sha256: "a4957008562b3ddc1c2bc59d15c03cfe1cbf2c4f0c402bf9d0160ef7c3bb5d7d",
    archive:
      process.env.KOSMOS_ARCADIA_ARCHIVE ??
      "C:/Users/kirill/Coding/makekosmos/arcadia/release/arcadia-0.1.11.kspkg",
    catalogFixture:
      process.env.KOSMOS_ARCADIA_CATALOG_FIXTURE ??
      "C:/Users/kirill/Coding/makekosmos/.tmp-cortex-arcadia011-fixture",
    catalogSequence: 1,
    backend: process.env.KOSMOS_ARCADIA_BACKEND,
  },
  dictation: {
    id: "com.kosmos.dictation",
    version: "0.2.4",
    sha256: "a7eaf9c84ee63fc01799df531ba0469390a20c4a4df6e37f1c9901af8a4c5fd5",
    archive:
      process.env.KOSMOS_DICTATION_ARCHIVE ??
      "C:/Users/kirill/Coding/makekosmos/package-index-catalog-14/.tmp-artifacts/dictation-0.2.4.kspkg",
    catalogFixture:
      process.env.KOSMOS_DICTATION_CATALOG_FIXTURE ??
      "C:/Users/kirill/Coding/makekosmos/.tmp-cortex-catalog14-fixture",
    catalogSequence: 14,
    backend: process.env.KOSMOS_DICTATION_BACKEND,
  },
};

const upgradeApps = {
  catalogFixture:
    process.env.KOSMOS_UPGRADE_CATALOG_FIXTURE ??
    "C:/Users/kirill/Coding/makekosmos/.tmp-cortex-catalog14-fixture",
  catalogSequence: 14,
  backend:
    process.env.KOSMOS_UPGRADE_BACKEND ??
    "C:/Users/kirill/Coding/makekosmos/.tmp-cortex-catalog14-fixture/kepler-backend-trust-fixture.exe",
  items: [
    {
      id: "com.kosmos.arcadia",
      legacyId: "arcadia",
      version: "0.1.8",
      sha256: "9f774db4228ee13cf7ce205f011abc52bcdbdb3b236271ad8dae6eabb4233b64",
      archive:
        process.env.KOSMOS_UPGRADE_ARCADIA_ARCHIVE ??
        "C:/Users/kirill/Coding/makekosmos/package-index-catalog-14/.tmp-artifacts/com.kosmos.arcadia-0.1.8.kspkg",
    },
    {
      id: "com.kosmos.agenda",
      legacyId: "delphi",
      version: "0.2.5",
      sha256: "cdca5e3654fb9f4c8e8300be8b59d8c8fa88b80c83b42921f3fd123d08318ef3",
      archive:
        process.env.KOSMOS_UPGRADE_AGENDA_ARCHIVE ??
        "C:/Users/kirill/Coding/makekosmos/package-index-catalog-14/.tmp-artifacts/agenda-0.2.5.kspkg",
    },
    {
      id: "com.kosmos.memoria",
      legacyId: "eden",
      version: "0.6.5",
      sha256: "2fe248dcf5d627d80bcc25f022cdb1cef01704a6194c735d789035f72112dd3b",
      archive:
        process.env.KOSMOS_UPGRADE_MEMORIA_ARCHIVE ??
        "C:/Users/kirill/Coding/makekosmos/package-index-catalog-14/.tmp-artifacts/memoria-0.6.5.kspkg",
    },
  ],
};

let running;
if (process.env.KOSMOS_SKIP_FRESH_CONSUMER !== "1") {
  try {
    running = await startBackend(apps.dictation);
    const dictation = await runConsumer(apps.dictation, running);
    await runDictation(dictation);
    await stopBackend(running);
    running = undefined;
    running = await startBackend(apps.arcadia);
    await runConsumer(apps.arcadia, running);
    console.log("arcadia consumer production install and boundary cases passed");
    console.log(
      `app consumer headless evidence passed using ${running.dataDir} and ${catalogFixture}`,
    );
  } finally {
    if (running) await stopBackend(running);
    await cleanup();
  }
}

const upgradeRoot = await mkdtemp(path.join(os.tmpdir(), "kosmos-app-upgrade-"));
const upgradeDataDir = path.join(upgradeRoot, "data");
try {
  await mkdir(path.join(upgradeDataDir, "extensions-data"), { recursive: true });
  for (const app of upgradeApps.items) {
    await mkdir(path.join(upgradeDataDir, "extensions-data", app.legacyId), {
      recursive: true,
    });
    await writeFile(
      path.join(upgradeDataDir, "extensions-data", app.legacyId, "state.json"),
      JSON.stringify({
        legacy_id: app.legacyId,
        settings: { theme: "dark", preserve: app.legacyId },
        unknownSettingsField: { preserve: app.legacyId },
      }) + "\n",
    );
  }
  await writeFile(
    path.join(upgradeDataDir, "grant-authority.json"),
    JSON.stringify(
      upgradeApps.items.map((app) => ({
        version: 1,
        persistent_grant_id: `${app.legacyId}-upgrade-grant`,
        extension_id: app.legacyId,
        provenance: "NativeDialog",
        exact_file: false,
        selected_path: upgradeDataDir,
        root_identity: { primary: 1, secondary: 2 },
        exact_file_identity: null,
        revoked: false,
        unknown_scope: { read: ["preserve"], write: ["deny"] },
      })),
    ) + "\n",
  );

  running = await startBackend(
    { id: "existing-user-upgrade", backend: upgradeApps.backend },
    upgradeDataDir,
  );
  await runExistingUserUpgrade(upgradeApps, running);

  const packages = await requestPackages(running);
  const installedBeforeRestart = await requestStoreInstalled(running);
  for (const app of upgradeApps.items) {
    const active = packages.filter((row) => row.id === app.id && row.enabled === true);
    if (active.length !== 1 || active[0].version !== app.version)
      throw new Error(`${app.id}: upgrade did not leave exactly one canonical package`);
    const migrated = JSON.parse(
      await readFile(path.join(upgradeDataDir, "extensions-data", app.id, "state.json"), "utf8"),
    );
    if (migrated.settings?.preserve !== app.legacyId)
      throw new Error(`${app.id}: user settings were not preserved`);
    if (migrated.unknownSettingsField?.preserve !== app.legacyId)
      throw new Error(`${app.id}: unknown settings were not preserved`);
    const journal = JSON.parse(
      await readFile(
        path.join(upgradeDataDir, "legacy-migrations", "v1", app.id, "journal.json"),
        "utf8",
      ),
    );
    if (journal.phase !== "committed" || journal.source_ids.length !== 1)
      throw new Error(`${app.id}: migration journal is not committed`);
  }
  const grants = JSON.parse(
    await readFile(path.join(upgradeDataDir, "grant-authority.json"), "utf8"),
  );
  if (grants.length !== upgradeApps.items.length || grants.some((grant) => !grant.revoked))
    throw new Error("legacy grant records were not durably revoked");

  await stopBackend(running);
  running = undefined;
  running = await startBackend(
    { id: "existing-user-upgrade", backend: upgradeApps.backend },
    upgradeDataDir,
  );
  await runExistingUserUpgrade(upgradeApps, running, { install: false });
  const restartedPackages = await requestPackages(running);
  const installedAfterRestart = await requestStoreInstalled(running);
  for (const app of upgradeApps.items) {
    const active = restartedPackages.filter((row) => row.id === app.id && row.enabled === true);
    if (active.length !== 1 || active[0].version !== app.version)
      throw new Error(`${app.id}: restart duplicated or resurrected a package`);
  }
  if (JSON.stringify(installedAfterRestart) !== JSON.stringify(installedBeforeRestart))
    throw new Error("restart changed canonical package grants or registry records");
  console.log("existing-user Agenda/Memoria/Arcadia upgrade passed across restart");
} finally {
  if (running) await stopBackend(running);
  for (let attempt = 0; attempt < 40; attempt += 1) {
    try {
      await rm(upgradeRoot, { recursive: true, force: true });
      break;
    } catch (error) {
      if (!(error instanceof Error) || error.code !== "EBUSY" || attempt === 39) throw error;
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
  }
}

async function requestPackages(running) {
  const lock = JSON.parse(await readFile(path.join(running.dataDir, "engine.lock.json"), "utf8"));
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "app-consumer-headless",
      "X-Kosmos-Client-Version": "1.0.0",
      "X-Kosmos-Client-Pid": String(lock.pid),
    },
    body: JSON.stringify({ operation: "packages.list", kind: "app" }),
  });
  const value = await response.json();
  if (!response.ok || value.ok !== true) throw new Error("packages.list failed during upgrade");
  return value.data.packages ?? [];
}

async function requestStoreInstalled(running) {
  const lock = JSON.parse(await readFile(path.join(running.dataDir, "engine.lock.json"), "utf8"));
  const response = await fetch(`http://127.0.0.1:${lock.http_port}/v1/rpc`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${lock.auth_token}`,
      "Content-Type": "application/json",
      "X-Kosmos-Api-Version": "1.0.0",
      "X-Kosmos-Client-Class": "app-consumer-headless",
      "X-Kosmos-Client-Version": "1.0.0",
      "X-Kosmos-Client-Pid": String(lock.pid),
    },
    body: JSON.stringify({ operation: "store.catalog" }),
  });
  const value = await response.json();
  if (!response.ok || value.ok !== true || !Array.isArray(value.data?.installed))
    throw new Error("store.catalog failed during upgrade");
  return value.data.installed
    .filter((row) => upgradeApps.items.some((app) => app.id === row.id))
    .map((row) => ({
      id: row.id,
      version: row.version,
      enabled: row.enabled,
      revoked: row.revoked,
      effective_grants: row.effective_grants,
    }))
    .sort((left, right) =>
      `${left.id}:${left.version}`.localeCompare(`${right.id}:${right.version}`),
    );
}
