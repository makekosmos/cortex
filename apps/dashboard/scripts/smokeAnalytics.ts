import path from "node:path";
import { fileURLToPath } from "node:url";
import { loadDashboardSnapshot } from "../electron/services/analytics.ts";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");

function argValue(flag: string): string | null {
  const index = process.argv.indexOf(flag);
  if (index === -1 || index === process.argv.length - 1) {
    return null;
  }
  return process.argv[index + 1];
}

const dbPath =
  argValue("--db-path") ??
  path.join(projectRoot, ".tmp", "smoke-dashboard.db");

const snapshot = await loadDashboardSnapshot({
  dbPath,
  rangeDays: 14,
  topAppsLimit: 5,
  recentSessionsLimit: 10,
});

if (!snapshot.status.readable) {
  throw new Error(snapshot.status.message ?? "Dashboard smoke DB is not readable");
}

if (snapshot.summary.sessionCount <= 0) {
  throw new Error("Dashboard smoke DB does not contain usage sessions");
}

if (snapshot.topApps.length <= 0) {
  throw new Error("Dashboard smoke DB did not produce top apps");
}

console.log(
  JSON.stringify(
    {
      dbPath: snapshot.status.path,
      sessions: snapshot.summary.sessionCount,
      topApp: snapshot.topApps[0]?.displayName ?? null,
      recentSessions: snapshot.recentSessions.length,
    },
    null,
    2,
  ),
);
