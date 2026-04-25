import { strict as assert } from 'node:assert'

import { ArkClient, type ArkUsageAnalyticsSnapshot } from '../../../packages/arksync-node/src/index.ts'

const calls: Array<Record<string, unknown>> = []
const expected: ArkUsageAnalyticsSnapshot = {
  generatedAt: '2026-04-25T00:00:00.000Z',
  summary: {
    trackedAppCount: 1,
    sessionCount: 1,
    eventCount: 1,
    totalForegroundMs: 1000,
    totalIdleMs: 0,
    firstRecordedAt: '2026-04-25T00:00:00.000Z',
    lastRecordedAt: '2026-04-25T00:01:00.000Z',
  },
  dailyTrend: [{ date: '2026-04-25', foregroundMs: 1000, idleMs: 0, sessions: 1 }],
  hourlyHeatmap: [{ weekday: 6, hour: 0, foregroundMs: 1000 }],
  topApps: [
    {
      id: 'app-1',
      displayName: 'Demo',
      processName: 'demo.exe',
      normalizedPath: 'c:/demo.exe',
      foregroundMs: 1000,
      idleMs: 0,
      sessions: 1,
      lastSeenAt: '2026-04-25T00:01:00.000Z',
    },
  ],
  recentSessions: [
    {
      id: 'session-1',
      trackedAppId: 'app-1',
      displayName: 'Demo',
      processName: 'demo.exe',
      platform: 'windows',
      deviceName: 'Device',
      startedAt: '2026-04-25T00:00:00.000Z',
      endedAt: '2026-04-25T00:01:00.000Z',
      foregroundMs: 1000,
      idleMs: 0,
      windowTitle: null,
    },
  ],
}

const client = new ArkClient({
  spaceId: 'space',
  deviceId: 'device-sdk',
  requestFn: async <T>(req: Record<string, unknown>): Promise<T> => {
    calls.push(req)
    return expected as T
  },
})

const actual = await client.usage.analytics.snapshot({
  rangeDays: 7,
  topAppsLimit: 3,
  recentSessionsLimit: 5,
})

assert.deepEqual(actual, expected)
assert.deepEqual(calls, [
  {
    operation: 'get_usage_analytics',
    range_days: 7,
    top_apps_limit: 3,
    recent_sessions_limit: 5,
  },
])
console.log('verify-arksync-node-usage-analytics PASS')
