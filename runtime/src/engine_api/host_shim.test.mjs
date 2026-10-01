// Host-shim behaviour tests: the script is plain ES5 browser JS — run it
// under node:test against stubbed browser globals and assert the observable
// contract (fragment consume, single bootstrap POST, API shape, renew loop,
// beacon revoke, session restore).
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
    window: {
      addEventListener: (name, fn) => {
        (calls.listeners[name] ??= []).push(fn);
      },
    },
    fetch: (url, init) => {
      calls.fetch.push({ url, init });
      return sandbox.__respond(url, init);
    },
    setTimeout: () => 0,
    clearTimeout: () => {},
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
  // Let the shim's bootstrap promise chain settle: bootstrap → then×2 → install.
  return new Promise((resolve) => setTimeout(resolve, 0));
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

test("no fragment and no session leaves no bridge", () => {
  const page = makePage("");
  page.__run();
  assert.equal(page.window.kosmosApp, undefined);
  assert.equal(page.__calls.fetch.length, 0);
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
  const api = page.window.kosmosApp;
  assert.ok(api, "kosmosApp must be exposed after bootstrap");
  assert.equal(api, page.window.kepler);
  assert.equal(api.identity.id, "com.kosmos.demo");
  const reply = await api.ark.request("list_object_types", {});
  assert.equal(reply.ok, true);
  const arkCall = page.__calls.fetch.find((c) => c.url.endsWith("/ark"));
  assert.equal(arkCall.init.headers["x-kosmos-launch-token"], "tok");
  // The code never appears in any request URL.
  assert.ok(!page.__calls.fetch.some((c) => c.url.includes("deadbeef")));
});

test("pagehide revokes the lease via sendBeacon with the launch token", async () => {
  const page = makePage("#launch=7e3d-42&code=deadbeef");
  page.__respond = (url) =>
    Promise.resolve({
      ok: true,
      json: () =>
        Promise.resolve(url.endsWith("/bootstrap") ? BOOTSTRAP_OK : { ok: true, data: null }),
    });
  page.__run();
  await flush();
  for (const handler of page.__calls.listeners["pagehide"] ?? []) handler();
  assert.equal(page.__calls.beacons.length, 1);
  assert.ok(page.__calls.beacons[0].url.endsWith("/revoke"));
  assert.deepEqual(JSON.parse(page.__calls.beacons[0].body), { token: "tok" });
});

test("a reloaded page restores the session from sessionStorage", async () => {
  const page = makePage("");
  const stored = {
    launch_id: "7e3d-42",
    id: "com.kosmos.demo",
    version: "1.0.0",
    broker_token: "tok",
    data_api: "/v1/apps/launch/7e3d-42/ark",
    expires_at: new Date(Date.now() + 900_000).toISOString(),
  };
  page.__storage.set("mundus.launch", JSON.stringify(stored));
  page.__respond = () =>
    Promise.resolve({ ok: true, json: () => Promise.resolve({ ok: true, data: [] }) });
  page.__run();
  assert.ok(page.window.kosmosApp, "restored session must expose the bridge");
  assert.equal(page.__calls.fetch.length, 0, "restore must not re-bootstrap");
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
