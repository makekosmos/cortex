import { strict as assert } from 'node:assert'

import { ArkClient } from '../../../packages/arksync-node/src/index.ts'

const calls: Array<Record<string, unknown>> = []

const client = new ArkClient({
  spaceId: 'space-auth',
  deviceId: 'device-sdk-auth',
  deviceName: 'SDK Auth',
  port: 21991,
  authSecret: 'shared-secret',
  requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
    calls.push(req)
    return true as T
  },
})

await client.start()

assert.deepEqual(calls, [
  {
    operation: 'start_sync',
    space_id: 'space-auth',
    device_id: 'device-sdk-auth',
    device_name: 'SDK Auth',
    port: 21991,
    seed_addresses: null,
    relay_url: null,
    relay_api_key: null,
    auth_secret: 'shared-secret',
  },
])

console.log('verify-arksync-node-auth-secret PASS')
