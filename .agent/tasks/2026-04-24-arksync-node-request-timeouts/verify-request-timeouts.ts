import { strict as assert } from 'node:assert'

import { ArkClient } from '../../../packages/arksync-node/src/index.ts'

type InternalClient = ArkClient & {
  ensureChild: () => { stdin: { write: (line: string) => void } }
  sendRequest: <T>(req: Record<string, unknown>) => Promise<T>
  request: <T>(req: Record<string, unknown>) => Promise<T>
  takePendingRequest: (id: string | undefined) => {
    resolve: (value: unknown) => void
    reject: (error: Error) => void
  } | null
  failAll: (error: Error) => void
  pendingRequests: Map<string, unknown>
}

function makeSelfManaged(timeoutMs: number, write: (line: string) => void = () => undefined): InternalClient {
  const client = new ArkClient({
    spaceId: 'space',
    deviceId: 'device',
    dbPath: 'D:/Personal/Hobby/Coding/kepler/.agent/tasks/2026-04-24-arksync-node-request-timeouts/self-managed/ark.db',
    sidecarPath: 'unused',
    requestTimeoutMs: timeoutMs,
  }) as unknown as InternalClient
  client.ensureChild = () => ({ stdin: { write } })
  return client
}

const timeoutClient = makeSelfManaged(10)
await assert.rejects(
  timeoutClient.sendRequest({ operation: 'never_returns' }),
  /request timed out after 10ms: never_returns/,
)
assert.equal(timeoutClient.pendingRequests.size, 0, 'timeout should remove pending request')

const responseClient = makeSelfManaged(50)
const responsePromise = responseClient.sendRequest<string>({ operation: 'fast_response' })
const responseId = [...responseClient.pendingRequests.keys()][0]
const pending = responseClient.takePendingRequest(responseId)
assert.ok(pending, 'response should find pending request')
pending.resolve('ok')
assert.equal(await responsePromise, 'ok')
await new Promise((resolve) => setTimeout(resolve, 70))
assert.equal(responseClient.pendingRequests.size, 0, 'matched response should clear timeout')

const writeFailureClient = makeSelfManaged(100, () => {
  throw new Error('stdin closed')
})
await assert.rejects(
  writeFailureClient.sendRequest({ operation: 'write_failure' }),
  /stdin closed/,
)
assert.equal(writeFailureClient.pendingRequests.size, 0, 'write failure should clear pending request')

const failAllClient = makeSelfManaged(100)
const first = failAllClient.sendRequest({ operation: 'first' })
const second = failAllClient.sendRequest({ operation: 'second' })
failAllClient.failAll(new Error('sidecar exited'))
await assert.rejects(first, /sidecar exited/)
await assert.rejects(second, /sidecar exited/)
assert.equal(failAllClient.pendingRequests.size, 0, 'failAll should clear pending requests')

let injectedCalls = 0
const injected = new ArkClient({
  spaceId: 'space',
  deviceId: 'device',
  requestTimeoutMs: 1,
  requestFn: async <T>() => {
    injectedCalls += 1
    await new Promise((resolve) => setTimeout(resolve, 20))
    return [] as T
  },
}) as unknown as InternalClient
assert.deepEqual(await injected.request<unknown[]>({ operation: 'slow_injected' }), [])
assert.equal(injectedCalls, 1, 'injected requestFn should not be wrapped by SDK timeout')

console.log('verify-request-timeouts PASS')
