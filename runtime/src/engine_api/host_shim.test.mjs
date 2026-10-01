// Host-shim behaviour tests: the script is plain ES5 browser JS — run it
// under node:test against stubbed browser globals and assert the observable
// contract (fragment consume, single bootstrap POST, synchronous API
// install, renew loop, release beacon, session restore).
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import vm from "node:vm";

const SHIM = readFileSync(join(import.meta.dirname, "host_shim.js"), "utf8");

function makePage(hash) {
  const calls = { fetch: [], beacons: [], listeners: {} };
  const storage = new Map();
  const sandbox = {
    location: { hash, pathname: "/v1/apps/assets/tok/index.html", search: "" },
    history: {
      replaced: null,
      replaceState(_s, _t, url) {
        this.replaced = url;
      },
    },
    sessionStorage: {
      getItem: (k) => (storage.has(k) ? storage.get(k) : null),
      setItem: (k, v) => storage.set(k, String(v)),
      removeItem: (k) => storage.delete(k),
    },
    navigator: {
      sendBeacon: (url, body) => calls.beacons.push({ url, body }),
    },
    addEventListener: (name, fn) => {
      (calls.listeners[name] ??= []).push(fn);
    },
    window: {},
    fetch: (url, init) => {
      calls.fetch.push({ url, init });
      return sandbox.__respond(url, init);
    },
    setTimeout: () => 0,
    clearTimeout: () => {},
    AbortController,
    TextDecoder,
    Promise,
    JSON,
    Date,
    console,
  };
  sandbox.__storage = storage;
  sandbox.__calls = calls;
  sandbox.__respond = () =>
    Promise.resolve({ ok: true, json: () => Promise.resolve({ ok: false }) });
  sandbox.__run = () => vm.runInContext(SHIM, sandbox);
  vm.createContext(sandbox);
  return sandbox;
}

function flush() {
  // Let the shim's bootstrap promise chain settle: bootstrap → then×2 → wired.
  return new Promise((resolve) => setTimeout(resolve, 0));
}

function header(headers, name) {
  const key = Object.keys(headers).find((k) => k.toLowerCase() === name);
  return key ? headers[key] : undefined;
}

const BOOTSTRAP_OK = {
  ok: true,
  data: {
    launch_id: "7e3d-42",
    id: "com.kosmos.demo",
    version: "1.0.0",
    broker_token: "tok",
    data_api: "/v1/apps/launch/7e3d-42/ark",
    expires_at: new Date(Date.now() + 900_000).toISOString(),
  },
};

const STORED = {
  launch_id: "7e3d-42",
  id: "com.kosmos.demo",
  version: "1.0.0",
  broker_token: "tok",
  data_api: "/v1/apps/launch/7e3d-42/ark",
  expires_at: new Date(Date.now() + 900_000).toISOString(),
};

test("no fragment and no session leaves no bridge", () => {
  const page = makePage("");
  page.__run();
  assert.equal(page.window.kosmosApp, undefined);
  assert.equal(page.__calls.fetch.length, 0);
});

test("kosmosApp installs synchronously with identity before bootstrap resolves", async () => {
  const page = makePage("#launch=7e3d-42&code=deadbeef&pkg=com.kosmos.demo&v=1.0.0");
  page.__respond = (url) => {
    if (url.endsWith("/bootstrap")) {
      return Promise.resolve({ ok: true, json: () => Promise.resolve(BOOTSTRAP_OK) });
    }
    return Promise.resolve({ ok: true, json: () => Promise.resolve({ ok: true, data: [] }) });
  };
  page.__run();
  // Synchronous: app scripts run right after the shim parses.
  const api = page.window.kosmosApp;
  assert.ok(api, "kosmosApp must exist before the bootstrap reply");
  assert.equal(api, page.window.kepler);
  assert.equal(api.identity.id, "com.kosmos.demo");
  assert.equal(api.identity.version, "1.0.0");
  // And the first queued request still resolves after bootstrap.
  const reply = api.ark.request("list_object_types", {});
  await flush();
  assert.equal((await reply).ok, true);
});

test("fragment code is exchanged once, then stripped from the URL", async () => {
  const page = makePage("#launch=7e3d-42&code=deadbeef");
  page.__respond = (url) => {
    if (url.endsWith("/bootstrap")) {
      return Promise.resolve({ ok: true, json: () => Promise.resolve(BOOTSTRAP_OK) });
    }
    return Promise.resolve({ ok: true, json: () => Promise.resolve({ ok: true, data: [] }) });
  };
  page.__run();
  assert.equal(page.history.replaced, "/v1/apps/assets/tok/index.html");
  assert.equal(page.__calls.fetch.length, 1);
  assert.ok(page.__calls.fetch[0].url.endsWith("/bootstrap"));
  assert.deepEqual(JSON.parse(page.__calls.fetch[0].init.body), { code: "deadbeef" });
  await flush();
  const reply = await page.window.kosmosApp.ark.request("list_object_types", {});
  assert.equal(reply.ok, true);
  const arkCall = page.__calls.fetch.find((c) => c.url.endsWith("/ark"));
  assert.equal(header(arkCall.init.headers, "x-kosmos-launch-token"), "tok");
  // The code never appears in any request URL.
  assert.ok(!page.__calls.fetch.some((c) => c.url.includes("deadbeef")));
});

test("pagehide releases the lease; a bfcache-persisted hide does not", async () => {
  const page = makePage("#launch=7e3d-42&code=deadbeef");
  page.__respond = (url) =>
    Promise.resolve({
      ok: true,
      json: () =>
        Promise.resolve(url.endsWith("/bootstrap") ? BOOTSTRAP_OK : { ok: true, data: null }),
    });
  page.__run();
  await flush();
  // persisted = bfcache handoff: the page keeps running, no release.
  for (const handler of page.__calls.listeners["pagehide"] ?? []) handler({ persisted: true });
  assert.equal(page.__calls.beacons.length, 0);
  // A real close releases: the lease dies at the end of the grace window.
  for (const handler of page.__calls.listeners["pagehide"] ?? []) handler({ persisted: false });
  assert.equal(page.__calls.beacons.length, 1);
  assert.ok(page.__calls.beacons[0].url.endsWith("/release"));
  assert.equal(JSON.parse(page.__calls.beacons[0].body).token, "tok");
  // sessionStorage stays: a non-persisted navigation (F5) still restores.
  assert.ok(page.__storage.has("mundus.launch"));
});

test("a reloaded page restores the session and renews to cancel a release", async () => {
  const page = makePage("");
  page.__storage.set("mundus.launch", JSON.stringify(STORED));
  page.__respond = () =>
    Promise.resolve({ ok: true, json: () => Promise.resolve({ ok: true, data: [] }) });
  page.__run();
  assert.ok(page.window.kosmosApp, "restored session must expose the bridge");
  assert.equal(page.window.kosmosApp.identity.id, "com.kosmos.demo");
  assert.ok(
    !page.__calls.fetch.some((c) => c.url.endsWith("/bootstrap")),
    "restore must not re-bootstrap",
  );
  await flush();
  // The restore path renews immediately — that is what cancels a release
  // the outgoing page's pagehide beacon just sent.
  assert.ok(page.__calls.fetch.some((c) => c.url.endsWith("/renew")));
});

test("an expired stored session is ignored", () => {
  const page = makePage("");
  page.__storage.set(
    "mundus.launch",
    JSON.stringify({
      launch_id: "x",
      broker_token: "tok",
      expires_at: new Date(Date.now() - 1000).toISOString(),
    }),
  );
  page.__run();
  assert.equal(page.window.kosmosApp, undefined);
});
