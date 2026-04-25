import { strict as assert } from 'node:assert'

import {
  ArkClient,
  type ArkObjectLinkRecord,
  type ArkObjectRecord,
  type ArkObjectTypeRecord,
  type ArkTrackedAppRecord,
  type ArkUsageEventRecord,
  type ArkUsageSessionRecord,
} from '../../../packages/arksync-node/src/index.ts'

const calls: Array<Record<string, unknown>> = []
const client = new ArkClient({
  spaceId: 'space',
  deviceId: 'device-sdk',
  requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
    calls.push(req)
    return true as T
  },
})

const timestamp = '2026-04-24T00:00:00.000Z'
const object: ArkObjectRecord = {
  id: 'obj-1',
  typeId: 'game_obj',
  title: 'Game',
  contentJson: {},
  propsJson: {},
  createdAt: timestamp,
  updatedAt: timestamp,
  deletedAt: null,
}
const objectType: ArkObjectTypeRecord = {
  id: 'game_obj',
  name: 'Game',
  schemaJson: '{}',
  uiSchemaJson: '{}',
  createdAt: timestamp,
  updatedAt: timestamp,
  systemLocked: false,
}
const link: ArkObjectLinkRecord = {
  id: 'link-1',
  sourceObjectId: object.id,
  targetObjectId: object.id,
  linkType: 'related',
  createdAt: timestamp,
}
const trackedApp: ArkTrackedAppRecord = {
  id: 'app-1',
  platform: 'windows',
  exePath: 'C:/Games/Demo/demo.exe',
  normalizedExePath: 'c:/games/demo/demo.exe',
  processName: 'demo.exe',
  displayName: 'Demo',
  publisher: null,
  iconRef: null,
  firstSeenAt: timestamp,
  lastSeenAt: timestamp,
}
const session: ArkUsageSessionRecord = {
  id: 'session-1',
  trackedAppId: trackedApp.id,
  deviceId: 'device-sdk',
  deviceName: 'Device',
  platform: 'windows',
  startedAt: timestamp,
  endedAt: null,
  foregroundMs: 1,
  idleMs: 0,
  windowTitle: null,
  processName: trackedApp.processName,
  exePath: trackedApp.exePath,
  pidStart: null,
  pidEnd: null,
  metaJson: {},
}
const event: ArkUsageEventRecord = {
  id: 'event-1',
  trackedAppId: trackedApp.id,
  usageSessionId: session.id,
  deviceId: 'device-sdk',
  deviceName: 'Device',
  platform: 'windows',
  occurredAt: timestamp,
  kind: 'foreground',
  windowTitle: null,
  processName: trackedApp.processName,
  exePath: trackedApp.exePath,
  pid: null,
  isForeground: true,
  isIdle: false,
  metaJson: {},
}

await client.objects.upsert(object)
await client.objects.delete(object.id)
await client.objectTypes.upsert(objectType)
await client.objectTypes.delete(objectType.id)
await client.links.upsert(link)
await client.links.delete(link.id)
await client.usage.trackedApps.upsert(trackedApp)
await client.usage.trackedApps.delete(trackedApp.id)
await client.usage.sessions.upsert(session)
await client.usage.sessions.delete(session.id)
await client.usage.events.upsert(event)
await client.usage.events.delete(event.id)

assert.deepEqual(
  calls.map((call) => call.operation),
  [
    'upsert_object',
    'delete_object',
    'upsert_object_type',
    'delete_object_type',
    'upsert_object_link',
    'delete_object_link',
    'upsert_tracked_app',
    'delete_tracked_app',
    'upsert_usage_session',
    'delete_usage_session',
    'upsert_usage_event',
    'delete_usage_event',
  ],
)
for (const call of calls) {
  assert.equal(call.device_id, 'device-sdk', `${String(call.operation)} should include device_id`)
}
console.log('verify-arksync-node-device-id PASS')
