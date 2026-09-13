import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";
import { cleanup, runConsumer, startBackend, stopBackend } from "./app-consumer-headless.mjs";

process.env.ARK_CORE_RPC_PATH ??=
  "C:/Users/kirill/AppData/Local/Temp/kosmos-cortex-core-78f1-20e4e9efe1f1448994d7ce2dd8108219/bin/ark-core-rpc.exe";

const app = {
  id: "com.kosmos.arcadia",
  version: "0.1.11",
  sha256: "c2b820511c97caf26696cbeb5db15f31c98617fac7cadc976162b5357f0f9f5c",
  archive:
    process.env.KOSMOS_ARCADIA_011_ARCHIVE ??
    "C:/Users/kirill/Coding/makekosmos-verify/arcadia-published-v011/arcadia-0.1.11.kspkg",
  catalogFixture:
    process.env.KOSMOS_ARCADIA_CATALOG_FIXTURE ??
    "C:/Users/kirill/Coding/makekosmos-verify/arcadia-catalog-fixture",
  catalogSequence: Number(process.env.KOSMOS_ARCADIA_CATALOG_SEQUENCE ?? 0),
  backend: process.env.KOSMOS_ARCADIA_BACKEND,
};

async function launch(app, running) {
  const lock = JSON.parse(await readFile(`${running.dataDir}/engine.lock.json`, "utf8"));
  const base = `http://127.0.0.1:${lock.http_port}`;
  const headers = {
    Authorization: `Bearer ${lock.auth_token}`,
    "Content-Type": "application/json",
    "X-Kosmos-Api-Version": "1.0.0",
    "X-Kosmos-Client-Class": "app-consumer-headless",
    "X-Kosmos-Client-Version": "1.0.0",
    "X-Kosmos-Client-Pid": String(lock.pid),
  };
  const response = await fetch(`${base}/v1/apps/launch`, {
    method: "POST",
    headers,
    body: JSON.stringify({ id: app.id, version: app.version }),
  });
  assert.equal(response.status, 200);
  const value = await response.json();
  assert.equal(value.ok, true, JSON.stringify(value));
  return { base, headers, data: value.data };
}

async function invoke(lease, operation, params = {}) {
  return fetch(lease.data.data_api, {
    method: "POST",
    headers: { ...lease.headers, "x-kosmos-launch-token": lease.data.broker_token },
    body: JSON.stringify({ _req_id: `${operation}-${Date.now()}`, operation, params }),
  });
}

if (!app.backend || !process.env.KOSMOS_ARCADIA_CATALOG_FIXTURE) {
  test(
    "installed Arcadia principal contract",
    { skip: "signed fixture and delivered backend not supplied" },
    () => {},
  );
} else {
  test("installed Arcadia principal contract", { timeout: 30000 }, async () => {
    const running = await startBackend(app);
    try {
      await runConsumer(app, running);
      const lease = await launch(app, running);
      const allowed = await invoke(lease, "games.list");
      assert.equal(allowed.status, 200);
      const allowedValue = await allowed.json();
      assert.equal(allowedValue.ok, true, JSON.stringify(allowedValue));

      const denied = await invoke(lease, "delete_object", { id: "arcadia-denied" });
      assert.equal(denied.status, 403);
      assert.deepEqual(await denied.json(), { ok: false, error: "forbidden" });
    } finally {
      await stopBackend(running);
      await cleanup();
    }
  });
}
