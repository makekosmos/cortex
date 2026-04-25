import { strict as assert } from "node:assert";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import {
  loadDashboardSnapshot,
  resolveArkCoreRpcBinaryPath,
} from "../../../apps/dashboard/electron/services/analytics.ts";

const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "dashboard-ark-"));
const dbPath = path.join(tmp, "ark.db");
fs.writeFileSync(dbPath, "");

const calls: Array<Record<string, unknown>> = [];
const snapshot = await loadDashboardSnapshot(
  {
    dbPath,
    rangeDays: 7,
    topAppsLimit: 2,
    recentSessionsLimit: 3,
  },
  null,
  {
    analyticsProvider: {
      snapshot: async (options) => {
        calls.push(options ?? {});
        return {
          generatedAt: "2026-04-25T00:00:00.000Z",
          summary: {
            trackedAppCount: 1,
            sessionCount: 1,
            eventCount: 1,
            totalForegroundMs: 1000,
            totalIdleMs: 0,
            firstRecordedAt: "2026-04-25T00:00:00.000Z",
            lastRecordedAt: "2026-04-25T00:01:00.000Z",
          },
          dailyTrend: [],
          hourlyHeatmap: [],
          topApps: [
            {
              id: "app-1",
              displayName: "Demo",
              processName: "demo.exe",
              normalizedPath: "c:/demo.exe",
              foregroundMs: 1000,
              idleMs: 0,
              sessions: 1,
              lastSeenAt: "2026-04-25T00:01:00.000Z",
            },
          ],
          recentSessions: [],
        };
      },
    },
  },
);

assert.equal(snapshot.status.readable, true);
assert.equal(snapshot.status.path, dbPath);
assert.equal(snapshot.summary.sessionCount, 1);
assert.deepEqual(calls, [{ rangeDays: 7, topAppsLimit: 2, recentSessionsLimit: 3 }]);

const missing = await loadDashboardSnapshot({ dbPath: path.join(tmp, "missing.db") });
assert.equal(missing.status.exists, false);
assert.equal(fs.existsSync(path.join(tmp, "missing.db")), false);

assert.equal(
  resolveArkCoreRpcBinaryPath({
    appRoot: "D:\\repo\\apps\\dashboard",
    resourcesPath: path.join(tmp, "resources"),
  }),
  path.join(
    "D:\\repo",
    "packages",
    "ark-core",
    "rust",
    "target",
    "debug",
    process.platform === "win32" ? "ark-core-rpc.exe" : "ark-core-rpc",
  ),
);

console.log("verify-dashboard-analytics PASS");
