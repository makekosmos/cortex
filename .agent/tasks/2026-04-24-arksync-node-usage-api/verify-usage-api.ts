import { strict as assert } from 'node:assert'

import {
  ArkClient,
  type ArkTrackedAppRecord,
  type ArkUsageEventRecord,
  type ArkUsageSessionRecord,
} from '../../../packages/arksync-node/src/index.ts'

const timestamp = '2026-04-24T00:00:00.000Z'

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
  deviceId: 'device',
  deviceName: 'Device',
  platform: 'windows',
  startedAt: timestamp,
  endedAt: null,
  foregroundMs: 1000,
  idleMs: 0,
  windowTitle: 'Demo',
  processName: trackedApp.processName,
  exePath: trackedApp.exePath,
  pidStart: 10,
  pidEnd: null,
  metaJson: { source: 'test' },
}

const event: ArkUsageEventRecord = {
  id: 'event-1',
  trackedAppId: trackedApp.id,
  usageSessionId: session.id,
  deviceId: 'device',
  deviceName: 'Device',
  platform: 'windows',
  occurredAt: timestamp,
  kind: 'foreground',
  windowTitle: 'Demo',
  processName: trackedApp.processName,
  exePath: trackedApp.exePath,
  pid: 10,
  isForeground: true,
  isIdle: false,
  metaJson: { source: 'test' },
}

const calls: Array<Record<string, unknown>> = []
const injected = new ArkClient({
  spaceId: 'space',
  deviceId: 'device',
  requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
    calls.push(req)
    if (req.operation === 'load_all') {
      return {
        trackedApps: [trackedApp],
        usageSessions: [session],
        usageEvents: [event],
        objects: [{ id: 'ignored' }],
      } as T
    }
    return true as T
  },
})

const snapshot = await injected.usage.loadAll()
await injected.usage.trackedApps.upsert(trackedApp)
await injected.usage.trackedApps.delete(trackedApp.id)
await injected.usage.sessions.upsert(session)
await injected.usage.sessions.delete(session.id)
await injected.usage.events.upsert(event)
await injected.usage.events.delete(event.id)

assert.deepEqual(snapshot, {
  trackedApps: [trackedApp],
  usageSessions: [session],
  usageEvents: [event],
})
assert.deepEqual(
  calls.map((call) => call.operation),
  [
    'load_all',
    'upsert_tracked_app',
    'delete_tracked_app',
    'upsert_usage_session',
    'delete_usage_session',
    'upsert_usage_event',
    'delete_usage_event',
  ],
)
assert.equal('id' in calls[0], false, 'injected usage API must not force request ids')
assert.deepEqual(calls[1].tracked_app, trackedApp)
assert.deepEqual(calls[3].usage_session, session)
assert.deepEqual(calls[5].usage_event, event)

const selfManagedCalls: Array<Record<string, unknown>> = []
const selfManaged = new ArkClient({
  spaceId: 'space',
  deviceId: 'device',
  dbPath: 'D:/Personal/Hobby/Coding/kosmos/.agent/tasks/2026-04-24-arksync-node-usage-api/self-managed/ark.db',
  sidecarPath: 'unused',
}) as ArkClient & { requestViaChild: (req: Record<string, unknown>) => Promise<unknown> }
selfManaged.requestViaChild = async (req: Record<string, unknown>) => {
  selfManagedCalls.push(req)
  if (req.operation === 'load_all') {
    return { trackedApps: [], usageSessions: [], usageEvents: [] }
  }
  return true
}

await selfManaged.usage.loadAll()
assert.deepEqual(
  selfManagedCalls.map((call) => call.operation),
  ['init', 'load_all'],
  'self-managed usage API should initialize before first usage request',
)

console.log('verify-usage-api PASS')
