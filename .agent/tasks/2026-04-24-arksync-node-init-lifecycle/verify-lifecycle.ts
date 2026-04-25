import { strict as assert } from 'node:assert'
import os from 'node:os'
import path from 'node:path'

import { ArkClient } from '../../../packages/arksync-node/src/ark-client.ts'

type Request = Record<string, unknown>
type TestableClient = ArkClient & {
  requestViaChild: (req: Request) => Promise<boolean>
}

const selfManagedCalls: Request[] = []
const selfManaged = new ArkClient({
  spaceId: 'space-test',
  deviceId: 'device-test',
  dbPath: path.join(os.tmpdir(), 'arksync-node-init-lifecycle', 'ark.db'),
  sidecarPath: 'unused-for-monkey-patched-test',
}) as TestableClient
selfManaged.requestViaChild = async (req: Request) => {
  selfManagedCalls.push(req)
  return true
}

await selfManaged.start()
await selfManaged.start()
assert.deepEqual(
  selfManagedCalls.map((req) => req.operation),
  ['init', 'start_sync', 'start_sync'],
  'self-managed start should initialize exactly once before start_sync',
)
assert.equal(
  selfManagedCalls[0]?.dbPath,
  path.join(os.tmpdir(), 'arksync-node-init-lifecycle', 'ark.db'),
  'init request should include the configured dbPath',
)

const injectedCalls: Request[] = []
const injected = new ArkClient({
  spaceId: 'space-test',
  deviceId: 'device-test',
  requestFn: async <T>(req: Request): Promise<T> => {
    injectedCalls.push(req)
    return true as T
  },
})

await injected.start()
assert.deepEqual(
  injectedCalls.map((req) => req.operation),
  ['start_sync'],
  'injected requestFn mode should not send init',
)

let attemptedStartSyncWithoutDbPath = false
const missingDbPath = new ArkClient({
  spaceId: 'space-test',
  deviceId: 'device-test',
  sidecarPath: 'unused-for-monkey-patched-test',
}) as TestableClient
missingDbPath.requestViaChild = async () => {
  attemptedStartSyncWithoutDbPath = true
  return true
}

await assert.rejects(
  () => missingDbPath.start(),
  /@arksync\/node: dbPath is required when requestFn is not provided/,
)
assert.equal(
  attemptedStartSyncWithoutDbPath,
  false,
  'missing dbPath should fail before any sidecar request is attempted',
)

console.log('verify-lifecycle PASS')
