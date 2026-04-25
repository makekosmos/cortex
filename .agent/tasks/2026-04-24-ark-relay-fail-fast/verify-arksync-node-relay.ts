import { strict as assert } from 'node:assert'

import { ArkClient } from '../../../packages/arksync-node/src/ark-client.ts'

let requestAttempted = false
const client = new ArkClient({
  spaceId: 'space',
  deviceId: 'device',
  relayUrl: 'wss://relay.example.test',
  requestFn: async <T>(): Promise<T> => {
    requestAttempted = true
    return true as T
  },
})

await assert.rejects(
  () => client.start(),
  /@arksync\/node: relay sync is not supported yet/,
)
assert.equal(requestAttempted, false, 'relay validation should fail before requestFn is called')

const legacyCalls: Array<Record<string, unknown>> = []
const legacy = new ArkClient({
  spaceId: 'space',
  deviceId: 'device',
  requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
    legacyCalls.push(req)
    return true as T
  },
})
await legacy.start()
assert.equal(legacyCalls.length, 1)
assert.equal(legacyCalls[0].operation, 'start_sync')
assert.equal(legacyCalls[0].relay_url, null)
assert.equal(legacyCalls[0].relay_api_key, null)

console.log('verify-arksync-node-relay PASS')
