import fs from "node:fs";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import { fileURLToPath } from "node:url";

type SmokeStatement = ReturnType<DatabaseSync["prepare"]>;
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

fs.mkdirSync(path.dirname(dbPath), { recursive: true });

const db = new DatabaseSync(dbPath);

db.exec(`
CREATE TABLE IF NOT EXISTS tracked_apps (
  id TEXT PRIMARY KEY,
  platform TEXT NOT NULL,
  exe_path TEXT NOT NULL,
  normalized_exe_path TEXT NOT NULL,
  process_name TEXT NOT NULL,
  display_name TEXT,
  publisher TEXT,
  icon_ref TEXT,
  first_seen_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS usage_sessions (
  id TEXT PRIMARY KEY,
  tracked_app_id TEXT NOT NULL,
  device_id TEXT NOT NULL,
  device_name TEXT NOT NULL,
  platform TEXT NOT NULL,
  started_at TEXT NOT NULL,
  ended_at TEXT,
  foreground_ms INTEGER NOT NULL DEFAULT 0,
  idle_ms INTEGER NOT NULL DEFAULT 0,
  window_title TEXT,
  process_name TEXT NOT NULL,
  exe_path TEXT NOT NULL,
  pid_start INTEGER,
  pid_end INTEGER,
  meta_json TEXT NOT NULL DEFAULT '{}'
);
CREATE TABLE IF NOT EXISTS usage_events (
  id TEXT PRIMARY KEY,
  tracked_app_id TEXT NOT NULL,
  usage_session_id TEXT,
  device_id TEXT NOT NULL,
  device_name TEXT NOT NULL,
  platform TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  kind TEXT NOT NULL,
  window_title TEXT,
  process_name TEXT NOT NULL,
  exe_path TEXT NOT NULL,
  pid INTEGER,
  is_foreground INTEGER NOT NULL DEFAULT 0,
  is_idle INTEGER NOT NULL DEFAULT 0,
  meta_json TEXT NOT NULL DEFAULT '{}'
);
`);

const insertApp: SmokeStatement = db.prepare(`
INSERT OR REPLACE INTO tracked_apps (
  id, platform, exe_path, normalized_exe_path, process_name, display_name,
  publisher, icon_ref, first_seen_at, last_seen_at
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
`);

const insertSession: SmokeStatement = db.prepare(`
INSERT OR REPLACE INTO usage_sessions (
  id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
  foreground_ms, idle_ms, window_title, process_name, exe_path, pid_start, pid_end, meta_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
`);

const insertEvent: SmokeStatement = db.prepare(`
INSERT OR REPLACE INTO usage_events (
  id, tracked_app_id, usage_session_id, device_id, device_name, platform, occurred_at,
  kind, window_title, process_name, exe_path, pid, is_foreground, is_idle, meta_json
) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
`);

const apps = [
  {
    id: "smoke-atlas",
    name: "Atlas",
    process: "atlas.exe",
    path: "c:\\games\\atlas\\atlas.exe",
  },
  {
    id: "smoke-helios",
    name: "Helios IDE",
    process: "helios.exe",
    path: "c:\\tools\\helios\\helios.exe",
  },
  {
    id: "smoke-odyssey",
    name: "Odyssey Browser",
    process: "odyssey.exe",
    path: "c:\\program files\\odyssey\\odyssey.exe",
  },
];

for (const app of apps) {
  insertApp.run(
    app.id,
    "windows",
    app.path,
    app.path,
    app.process,
    app.name,
    "Kepler",
    null,
    "2026-04-01T07:00:00.000Z",
    "2026-04-16T18:00:00.000Z",
  );
}

for (let day = 0; day < 10; day += 1) {
  const date = new Date(Date.UTC(2026, 3, 16 - day, 8 + (day % 4), 0, 0));
  for (const [index, app] of apps.entries()) {
    const startedAt = new Date(date.getTime() + index * 90 * 60 * 1000);
    const foregroundMs = (45 + (index * 22 + day * 7) % 120) * 60 * 1000;
    const idleMs = (5 + (day * 3 + index * 4) % 18) * 60 * 1000;
    const endedAt = new Date(startedAt.getTime() + foregroundMs + idleMs);
    const sessionId = `${app.id}-session-${day}`;
    insertSession.run(
      sessionId,
      app.id,
      "smoke-device",
      "Smoke Workstation",
      "windows",
      startedAt.toISOString(),
      endedAt.toISOString(),
      foregroundMs,
      idleMs,
      `${app.name} Window ${day + 1}`,
      app.process,
      app.path,
      1000 + day,
      1000 + day,
      JSON.stringify({ source: "smoke-seed", day }),
    );
    insertEvent.run(
      `${sessionId}-event`,
      app.id,
      sessionId,
      "smoke-device",
      "Smoke Workstation",
      "windows",
      new Date(startedAt.getTime() + 12 * 60 * 1000).toISOString(),
      "window_changed",
      `${app.name} Focus ${day + 1}`,
      app.process,
      app.path,
      1000 + day,
      1,
      0,
      JSON.stringify({ source: "smoke-seed", highlight: true }),
    );
  }
}

db.close();
console.log(`Smoke dashboard DB seeded at ${dbPath}`);
