import { strict as assert } from "node:assert";

import { ArkClient } from "../../../packages/arksync-node/src/index.ts";

const calls: Array<Record<string, unknown>> = [];

const client = new ArkClient({
  spaceId: "relay-space",
  deviceId: "relay-device-sdk",
  deviceName: "Relay SDK",
  port: 21992,
  relayUrl: "ws://127.0.0.1:8765",
  relayApiKey: "relay-key",
  authSecret: "shared-secret",
  requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
    calls.push(req);
    return true as T;
  },
});

await client.start();

assert.deepEqual(calls, [
  {
    operation: "start_sync",
    space_id: "relay-space",
    device_id: "relay-device-sdk",
    device_name: "Relay SDK",
    port: 21992,
    seed_addresses: null,
    relay_url: "ws://127.0.0.1:8765",
    relay_api_key: "relay-key",
    auth_secret: "shared-secret",
  },
]);

console.log("verify-arksync-node-relay-options PASS");
