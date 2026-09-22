import { readFile } from "node:fs/promises";

import { FATSECRET_KEYRING_KEYS, FatSecretIntegration } from "../provider.mjs";

export const fixture = JSON.parse(
  await readFile(new URL("../fixtures/diary.json", import.meta.url), "utf8"),
);

export function makeIntegration({ http, keyring: keyringOverride, keyringValues = {} } = {}) {
  const values = new Map([
    [FATSECRET_KEYRING_KEYS.consumerSecret, "consumer-secret"],
    [FATSECRET_KEYRING_KEYS.accessToken, "access-token"],
    [FATSECRET_KEYRING_KEYS.accessTokenSecret, "access-secret"],
    ...Object.entries(keyringValues),
  ]);
  const writes = new Map();
  const syncMetadata = [];
  const keyring = keyringOverride ?? {
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
      writes.set(object.id, object);
    },
    async recordSync(metadata) {
      syncMetadata.push(metadata);
    },
  };
  const integration = new FatSecretIntegration({
    config: {
      provider: "fatsecret",
      clientId: "client-key",
      baseUrl: "https://platform.fatsecret.com",
    },
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
