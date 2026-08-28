import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import { mapDiaryEntry, NUTRITION_ENTRY_TYPE } from "../mapping.mjs";
import { parseFormEncoded, parseOAuthHeader, signRequest } from "../oauth1.mjs";
import { FATSECRET_KEYRING_KEYS, FatSecretError, FatSecretIntegration } from "../provider.mjs";

const fixture = JSON.parse(
  await readFile(new URL("../fixtures/diary.json", import.meta.url), "utf8"),
);

test("OAuth 1.0a RFC 5849 signing fixture is deterministic", () => {
  const signed = signRequest({
    method: "POST",
    url: "http://example.com/request?b5=%3D%253D&a3=a&c%40=&a2=r%20b",
    params: { c2: "", a3: ["2 q"] },
    consumerKey: "9djdj82h48djs9d2",
    consumerSecret: "j49sk3j29djd",
    token: "kkk9d7dh3k39sjv7",
    tokenSecret: "dh893hdasih9",
    oauthNonce: "7d8f3e4a",
    oauthTimestamp: 137131201,
  });
  assert.equal(
    signed.baseString,
    "POST&http%3A%2F%2Fexample.com%2Frequest&a2%3Dr%2520b%26a3%3D2%2520q%26a3%3Da%26b5%3D%253D%25253D%26c%2540%3D%26c2%3D%26oauth_consumer_key%3D9djdj82h48djs9d2%26oauth_nonce%3D7d8f3e4a%26oauth_signature_method%3DHMAC-SHA1%26oauth_timestamp%3D137131201%26oauth_token%3Dkkk9d7dh3k39sjv7",
  );
  assert.equal(signed.signature, "r6/TJjbCOr97/+UU0NsvSne7s5g=");
  assert.equal(parseOAuthHeader(signed.authorization).oauth_signature, signed.signature);
});

test("OAuth form parser decodes plus and percent escapes", () => {
  assert.deepEqual(parseFormEncoded("oauth_token=a%2Bb&oauth_verifier=hello+world"), {
    oauth_token: "a+b",
    oauth_verifier: "hello world",
  });
});

test("diary fixture maps to nutrition_entry_obj and omits unavailable nutrients", () => {
  const object = mapDiaryEntry(fixture.diary_entries[0]);
  assert.equal(object.id, "fatsecret-food-entry:entry-100");
  assert.equal(object.typeId, NUTRITION_ENTRY_TYPE);
  assert.equal(object.data.provider, "fatsecret");
  assert.equal(object.data.nutrients.calories, 320);
  assert.equal(object.data.nutrients.carbohydrate, 54.5);
  assert.equal("fiber" in object.data.nutrients, false);
});

function makeIntegration({ http, keyringValues = {} } = {}) {
  const values = new Map([
    [FATSECRET_KEYRING_KEYS.consumerSecret, "consumer-secret"],
    [FATSECRET_KEYRING_KEYS.accessToken, "access-token"],
    [FATSECRET_KEYRING_KEYS.accessTokenSecret, "access-secret"],
    ...Object.entries(keyringValues),
  ]);
  const writes = [];
  const syncMetadata = [];
  const keyring = {
    async get(key) {
      return values.get(key);
    },
    async set(key, value) {
      values.set(key, value);
    },
    async delete(key) {
      values.delete(key);
    },
  };
  const writer = {
    async upsertObject(object) {
      writes.push(object);
    },
    async recordSync(metadata) {
      syncMetadata.push(metadata);
    },
  };
  const integration = new FatSecretIntegration({
    config: { provider: "fatsecret", clientId: "client-key", baseUrl: "https://api.example.test" },
    keyring,
    http: http ?? {
      async request() {
        return { status: 200, body: JSON.stringify(fixture) };
      },
    },
    writer,
    clock: () => 1700000000,
    random: () => "fixture-nonce",
  });
  return { integration, keyring, values, writes, syncMetadata };
}

test("connect exchanges OAuth tokens while config contains no secrets", async () => {
  const requests = [];
  const responses = [
    { status: 200, body: "oauth_token=request-token&oauth_token_secret=request-secret" },
    {
      status: 200,
      body: "oauth_token=access-token-new&oauth_token_secret=access-secret-new&user_id=42",
    },
  ];
  const { integration, values } = makeIntegration({
    http: {
      async request(request) {
        requests.push(request);
        return responses.shift();
      },
    },
  });
  const opened = [];
  const status = await integration.connect({
    openBrowser: async (url) => opened.push(url),
    verifier: "callback-verifier",
  });
  assert.equal(status.connected, true);
  assert.equal(values.get(FATSECRET_KEYRING_KEYS.accessToken), "access-token-new");
  assert.match(opened[0], /oauth_token=request-token/);
  assert.equal(requests.length, 2);
  assert.doesNotMatch(
    JSON.stringify(integration.config),
    /consumer-secret|access-token|access-secret/i,
  );
});

test("sync is an idempotent upsert, records metadata, and concurrent calls share one request", async () => {
  let requests = 0;
  const { integration, writes, syncMetadata } = makeIntegration({
    http: {
      async request() {
        requests += 1;
        await new Promise((resolve) => setTimeout(resolve, 20));
        return {
          status: 200,
          body: JSON.stringify({ diary_entries: { food_entry: fixture.diary_entries } }),
        };
      },
    },
  });
  const [first, second] = await Promise.all([
    integration.syncNow({ from: "2026-08-27", to: "2026-08-27" }),
    integration.syncNow({ from: "2026-08-27", to: "2026-08-27" }),
  ]);
  assert.deepEqual(first, second);
  assert.equal(requests, 1);
  assert.equal(writes.length, 1);
  const again = await integration.syncNow();
  assert.equal(again.imported, 1);
  assert.equal(writes[0].id, "fatsecret-food-entry:entry-100");
  assert.equal(writes[1].id, writes[0].id);
  assert.equal(syncMetadata.length, 2);
  assert.deepEqual(syncMetadata[0], {
    provider: "fatsecret",
    from: "2026-08-27",
    to: "2026-08-27",
    imported: 1,
    completedAt: "2023-11-14T22:13:20.000Z",
  });
});

test("safe API errors classify auth failures without exposing token values", async () => {
  const leaked = "oauth_token=super-secret-token";
  const { integration } = makeIntegration({
    http: {
      async request() {
        return { status: 401, body: leaked };
      },
    },
  });
  await assert.rejects(integration.syncNow(), (error) => {
    assert.ok(error instanceof FatSecretError);
    assert.equal(error.kind, "auth");
    assert.doesNotMatch(error.message, /super-secret-token/);
    return true;
  });
});

test("disconnect clears keyring connection state but not ARK history", async () => {
  const { integration, values, writes } = makeIntegration();
  await integration.syncNow();
  const status = await integration.disconnect();
  assert.equal(status.connected, false);
  assert.equal(values.has(FATSECRET_KEYRING_KEYS.accessToken), false);
  assert.equal(values.has(FATSECRET_KEYRING_KEYS.accessTokenSecret), false);
  assert.equal(writes.length, 1);
});

test("configuration rejects secret-bearing fields and requires keyring adapter", () => {
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
