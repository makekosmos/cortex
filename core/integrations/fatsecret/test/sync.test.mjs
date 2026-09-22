import assert from "node:assert/strict";
import test from "node:test";

import { FATSECRET_KEYRING_KEYS, FatSecretError, FatSecretIntegration } from "../provider.mjs";
import { fixture, makeIntegration } from "./support.mjs";

test("sync is an idempotent upsert over official entries and concurrent calls share one request", async () => {
  const requests = [];
  let calories = 320;
  const { integration, writes, syncMetadata } = makeIntegration({
    http: {
      async request(request) {
        requests.push(request);
        await new Promise((resolve) => setTimeout(resolve, 10));
        return {
          status: 200,
          body: JSON.stringify({
            food_entries: {
              food_entry: [{ ...fixture.food_entries.food_entry, calories: String(calories) }],
            },
          }),
        };
      },
    },
  });
  const [first, second] = await Promise.all([
    integration.syncNow({ from: "2026-08-27", to: "2026-08-27" }),
    integration.syncNow({ from: "2026-08-27", to: "2026-08-27" }),
  ]);
  assert.deepEqual(first, second);
  assert.equal(requests.length, 1);
  assert.equal(writes.size, 1);
  const url = new URL(requests[0].url);
  assert.equal(url.origin, "https://platform.fatsecret.com");
  assert.equal(url.pathname, "/rest/food-entries/v2");
  assert.equal(url.searchParams.get("date"), "20692");
  assert.equal(url.searchParams.get("format"), "json");
  calories = 321;
  assert.equal((await integration.syncNow()).imported, 1);
  assert.equal(writes.get("fatsecret-food-entry:entry-100").data.nutrients.calories, 321);
  assert.equal(syncMetadata.length, 2);
});

test("sync requests every inclusive UTC day and rejects invalid or oversized ranges", async () => {
  const requests = [];
  const { integration } = makeIntegration({
    http: {
      async request(request) {
        requests.push(request);
        return { status: 200, body: JSON.stringify({ food_entries: { food_entry: [] } }) };
      },
    },
  });
  await integration.syncNow({ from: "2026-08-27", to: "2026-08-29" });
  assert.deepEqual(
    requests.map(({ url }) => new URL(url).searchParams.get("date")),
    ["20692", "20693", "20694"],
  );
  await assert.rejects(integration.syncNow({ from: "2026-08-29", to: "2026-08-27" }), {
    kind: "config",
  });
  await assert.rejects(integration.syncNow({ from: "2026-01-01", to: "2026-02-01" }), {
    kind: "config",
  });
});

test("disconnect wins deterministically over an in-flight sync", async () => {
  let started;
  let release;
  const requestStarted = new Promise((resolve) => {
    started = resolve;
  });
  const response = new Promise((resolve) => {
    release = resolve;
  });
  const { integration } = makeIntegration({
    http: {
      async request() {
        started();
        return response;
      },
    },
  });
  const sync = integration.syncNow();
  await requestStarted;
  const disconnect = integration.disconnect();
  release({ status: 200, body: JSON.stringify(fixture) });
  await disconnect;
  await sync;
  assert.equal(integration.status().connected, false);
});

test("startup keeps a safe error and schedules a retry after failure", async () => {
  const timers = [];
  const { integration } = makeIntegration({
    http: {
      async request() {
        throw new Error("offline oauth_token=secret");
      },
    },
  });
  const originalSetInterval = globalThis.setInterval;
  globalThis.setInterval = (callback, interval) => {
    timers.push({ callback, interval });
    return 1;
  };
  try {
    const status = await integration.start({
      from: "2026-08-27",
      to: "2026-08-27",
      intervalMs: 1234,
    });
    assert.equal(timers[0].interval, 1234);
    assert.equal(status.lastError.kind, "transport");
    assert.doesNotMatch(status.lastError.message, /secret/);
  } finally {
    globalThis.setInterval = originalSetInterval;
    integration.stop();
  }
});

test("safe errors distinguish auth, rate limit, invalid response, and transport", async () => {
  for (const [response, kind] of [
    [{ status: 401, body: "oauth_token=super-secret-token" }, "auth"],
    [{ status: 429, body: "retry later" }, "rate_limit"],
    [{ status: 200, body: '{"oauth_token":"super-secret-token"' }, "invalid_response"],
  ]) {
    const { integration } = makeIntegration({
      http: {
        async request() {
          return response;
        },
      },
    });
    await assert.rejects(integration.syncNow(), (error) => {
      assert.ok(error instanceof FatSecretError);
      assert.equal(error.kind, kind);
      assert.equal("cause" in error, false);
      assert.doesNotMatch(error.message, /super-secret-token/);
      return true;
    });
  }
  const { integration } = makeIntegration({
    http: {
      async request() {
        throw new Error("oauth_token=super-secret-token");
      },
    },
  });
  await assert.rejects(integration.syncNow(), (error) => {
    assert.equal(error.kind, "transport");
    assert.equal("cause" in error, false);
    assert.doesNotMatch(error.message, /super-secret-token/);
    return true;
  });
});

test("disconnect clears keyring connection state but not ARK history", async () => {
  const { integration, values, writes } = makeIntegration();
  await integration.syncNow();
  assert.equal((await integration.disconnect()).connected, false);
  assert.equal(values.has(FATSECRET_KEYRING_KEYS.accessToken), false);
  assert.equal(values.has(FATSECRET_KEYRING_KEYS.accessTokenSecret), false);
  assert.equal(writes.size, 1);
});

test("configuration rejects secret-bearing fields and non-official origins", () => {
  assert.throws(
    () =>
      new FatSecretIntegration({
        config: {
          provider: "fatsecret",
          clientId: "client",
          baseUrl: "https://example.test",
          clientSecret: "bad",
        },
        keyring: {},
        http: {},
        writer: {},
      }),
    /secrets must stay in the OS keyring/,
  );
});
