import { strict as assert } from 'node:assert'

import { ArkClient } from '../../../packages/arksync-node/src/ark-client.ts'

type Request = Record<string, unknown>
type TestClient = ArkClient & Record<string, unknown>

function pushStdout(client: TestClient, frame: Request): void {
  const line = `${JSON.stringify(frame)}\n`
  client.stdoutChunks = [Buffer.from(line)]
  client.stdoutLength = Buffer.byteLength(line)
  ;(client.flushStdout as () => void)()
}

const client = new ArkClient({
  spaceId: 'space',
  deviceId: 'device',
  dbPath: 'unused.db',
  sidecarPath: 'unused',
}) as TestClient

const writes: Request[] = []
client.ensureChild = () => ({
  stdin: {
    write(line: string) {
      writes.push(JSON.parse(line) as Request)
      return true
    },
  },
})

let firstResolved = false
const first = (client.requestViaChild as (req: Request) => Promise<unknown>)({
  operation: 'get_connected_peers',
}).then((value) => {
  firstResolved = true
  return value
})
const second = (client.requestViaChild as (req: Request) => Promise<unknown>)({
  operation: 'get_host_device_name',
})

assert.equal(writes.length, 2)
assert.equal(typeof writes[0].id, 'string')
assert.equal(typeof writes[1].id, 'string')
assert.notEqual(writes[0].id, writes[1].id)

pushStdout(client, { event: 'peer_connected', device_id: 'peer-a' })
assert.equal(firstResolved, false, 'event frames must not resolve pending requests')

pushStdout(client, { id: writes[1].id, ok: true, data: 'host-name' })
assert.equal(await second, 'host-name')
assert.equal(firstResolved, false, 'out-of-order response must resolve only matching id')

pushStdout(client, { id: writes[0].id, ok: true, data: [] })
assert.deepEqual(await first, [])
assert.equal(firstResolved, true)

const injectedCalls: Request[] = []
const injected = new ArkClient({
  spaceId: 'space',
  deviceId: 'device',
  requestFn: async <T>(req: Request): Promise<T> => {
    injectedCalls.push(req)
    return true as T
  },
})
await injected.start()
assert.deepEqual(
  injectedCalls.map((req) => req.operation),
  ['start_sync'],
)
assert.equal('id' in injectedCalls[0], false, 'injected requestFn mode must remain legacy-shaped')

console.log('verify-arksync-node-request-id PASS')
