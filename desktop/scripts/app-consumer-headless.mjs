import { mkdir, mkdtemp, readFile, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createLegacyMigrationRunner } from "../electron/legacy-migration-runtime.ts";

const catalogFixture = path.resolve(
  process.env.KOSMOS_HEADLESS_CATALOG_FIXTURE ??
    "C:/Users/kirill/Coding/makekosmos/.tmp-cortex-catalog14-fixture",
);
const backend = path.resolve(
  process.env.KOSMOS_HEADLESS_BACKEND ??
    path.join(catalogFixture, "kepler-backend-trust-fixture.exe"),
);
const root = await mkdtemp(path.join(os.tmpdir(), "kosmos-app-consumer-"));

async function exists(file) {
  try {
    await stat(file);
    return true;
  } catch {
    return false;
  }
}

async function waitFor(file, child) {
  for (let attempt = 0; attempt < 600; attempt += 1) {
    if (await exists(file)) return;
    if (child.exitCode !== null)
      throw new Error(
        `backend exited before ${path.basename(file)} (${child.exitCode})${child.stderrText ? `: ${child.stderrText}` : ""}`,
      );
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw new Error(`timed out waiting for ${path.basename(file)}`);
}

async function waitForMissing(file) {
  for (;;) {
    if (!(await exists(file))) return;
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}

function client(lock) {
  return async (operation, params = {}) => {
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
      body: JSON.stringify({ operation, _req_id: `${operation}-${Date.now()}`, ...params }),
    });
    const value = await response.json();
    if (!response.ok) throw new Error(`${operation}: HTTP ${response.status}`);
    return value;
  };
}

function assertEnvelope(value, label) {
  const envelope = value && Object(value);
  if (!envelope || (envelope.ok !== true && envelope.ok !== false))
    throw new Error(`${label}: malformed response`);
  if (envelope.ok && !("data" in envelope)) throw new Error(`${label}: success lacks data`);
  if (!envelope.ok && !("message" in envelope) && !("error" in envelope))
    throw new Error(`${label}: error lacks message/error`);
}

export async function runConsumer(app, running) {
  const request = client(
    JSON.parse(await readFile(path.join(running.dataDir, "engine.lock.json"), "utf8")),
  );
  const fixtureRoot = path.resolve(app.catalogFixture ?? catalogFixture);
  const fixture = JSON.parse(
    await readFile(path.join(fixtureRoot, "catalog-apply-request.json"), "utf8"),
  );
  const catalog = JSON.parse(fixture.document);
  if (catalog.schema_version !== 1 || catalog.sequence !== app.catalogSequence)
    throw new Error(`${app.id}: unexpected ephemeral catalog sequence`);
  const entry = catalog.packages.find((item) => item.manifest.id === app.id);
  if (!entry || entry.manifest.version !== app.version || entry.sha256 !== app.sha256)
    throw new Error(
      `${app.id}: signed catalog mismatch; expected ${app.version}/${app.sha256}, got ${entry?.manifest.version}/${entry?.sha256}`,
    );
  const applied = await request("packages.catalog_apply", {
    document: fixture.document,
    signatures: fixture.signatures,
  });
  assertEnvelope(applied, `${app.id} catalog_apply`);
  if (!applied.ok) throw new Error(`${app.id}: catalog rejected`);
  const installed = await request("packages.install", {
    id: app.id,
    version: app.version,
    archive_path: app.archive,
  });
  assertEnvelope(installed, `${app.id} install`);
  if (!installed.ok) throw new Error(`${app.id}: install rejected ${JSON.stringify(installed)}`);
  const enabled = await request("packages.set_enabled", {
    id: app.id,
    version: app.version,
    enabled: true,
  });
  assertEnvelope(enabled, `${app.id} enable`);
  if (!enabled.ok) throw new Error(`${app.id}: enable rejected`);
  const unknown = await request("packages.not_declared", {});
  assertEnvelope(unknown, `${app.id} unknown operation`);
  if (unknown.ok) throw new Error(`${app.id}: unknown operation was accepted`);
  return request;
}

export async function runExistingUserUpgrade(apps, running, { install = true } = {}) {
  const request = client(
    JSON.parse(await readFile(path.join(running.dataDir, "engine.lock.json"), "utf8")),
  );
  const fixtureRoot = path.resolve(apps.catalogFixture);
  const fixture = JSON.parse(
    await readFile(path.join(fixtureRoot, "catalog-apply-request.json"), "utf8"),
  );
  const catalog = JSON.parse(fixture.document);
  if (catalog.schema_version !== 1 || catalog.sequence !== apps.catalogSequence)
    throw new Error(`upgrade: unexpected signed catalog sequence`);
  if (install) {
    const applied = await request("packages.catalog_apply", {
      document: fixture.document,
      signatures: fixture.signatures,
    });
    assertEnvelope(applied, "upgrade catalog_apply");
    if (!applied.ok && !String(applied.error ?? "").includes("replay-rejected"))
      throw new Error(`upgrade: catalog rejected ${JSON.stringify(applied)}`);

    for (const app of apps.items) {
      const entry = catalog.packages.find((item) => item.manifest.id === app.id);
      if (!entry || entry.manifest.version !== app.version || entry.sha256 !== app.sha256)
        throw new Error(`${app.id}: signed catalog mismatch`);
      const installed = await request("packages.install", {
        id: app.id,
        version: app.version,
        archive_path: app.archive,
      });
      assertEnvelope(installed, `${app.id} replacement install`);
      if (!installed.ok) throw new Error(`${app.id}: replacement rejected`);
    }
  }

  const runner = createLegacyMigrationRunner(
    running.dataDir,
    {
      invokeOperation: async ({ operation, params = {} }) => {
        const response = await request(operation, params);
        if (!response.ok) throw new Error(`${operation}: ${response.message ?? response.error}`);
        return response.data;
      },
    },
    async () => {},
  );
  await runner.run();
  return request;
}

export async function runDictation(request) {
  const config = {
    hotkey: "Ctrl+Shift:;",
    language: "ru",
    injectMode: "clipboard_only",
    provider: "local",
    model: "whisper-large-v3-turbo",
    localModelId: "fixture-model",
    providerEnabled: true,
  };
  for (const operation of [
    "dictation.get_config",
    "dictation.get_state",
    "dictation.list_local_models",
  ]) {
    const response = await request(operation);
    assertEnvelope(response, operation);
  }
  const updated = await request("dictation.update_config", { ...config, apiKey: "secret" });
  assertEnvelope(updated, "dictation.update_config");
  const cancelled = await request("dictation.cancel");
  assertEnvelope(cancelled, "dictation.cancel");
  const unknown = await request("dictation.not_declared", {});
  assertEnvelope(unknown, "dictation.not_declared");
  if (unknown.ok) throw new Error("dictation unknown operation was accepted");
  console.log("dictation consumer boundary cases passed");
}

export async function startBackend(app, requestedDataDir) {
  const dataDir = requestedDataDir ?? path.join(root, app.id);
  await mkdir(dataDir, { recursive: true });
  const env = {
    ...process.env,
    KOSMOS_DATA_DIR: dataDir,
    KOSMOS_HEADLESS: "1",
    KOSMOS_LOCK_PERMISSIONS_DISABLED: "1",
  };
  const executable = path.resolve(app.backend ?? backend);
  const child = spawn(executable, [], {
    env,
    windowsHide: true,
    stdio: ["ignore", "ignore", "pipe"],
  });
  child.stderrText = "";
  child.stderr.on("data", (chunk) => {
    child.stderrText += chunk.toString().trim();
  });
  await waitFor(path.join(dataDir, "engine.lock.json"), child);
  return { child, env, dataDir, executable };
}

export async function stopBackend(running) {
  const exited = new Promise((resolve) => running.child.once("exit", resolve));
  spawnSync(running.executable, ["--shutdown"], { env: running.env, windowsHide: true });
  running.child.kill();
  await exited;
  await waitForMissing(path.join(running.dataDir, "engine.lock.json"));
}

export async function cleanup() {
  try {
    await rm(root, { recursive: true, force: true });
  } catch (error) {
    if (!(error instanceof Error) || error.code !== "EBUSY") throw error;
    console.warn(`consumer test data retained while locked: ${root}`);
  }
}

export { catalogFixture };
