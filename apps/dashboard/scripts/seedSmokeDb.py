from __future__ import annotations

import argparse
import json
import sqlite3
from datetime import datetime, timedelta, timezone
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--db-path", required=True)
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    db_path = Path(args.db_path)
    db_path.parent.mkdir(parents=True, exist_ok=True)

    connection = sqlite3.connect(db_path)
    cursor = connection.cursor()

    cursor.executescript(
        """
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
        DELETE FROM usage_events;
        DELETE FROM usage_sessions;
        DELETE FROM tracked_apps;
        """
    )

    apps = [
      {
        "id": "smoke-atlas",
        "name": "Atlas",
        "process": "atlas.exe",
        "path": "c:\\games\\atlas\\atlas.exe",
      },
      {
        "id": "smoke-helios",
        "name": "Helios IDE",
        "process": "helios.exe",
        "path": "c:\\tools\\helios\\helios.exe",
      },
      {
        "id": "smoke-odyssey",
        "name": "Odyssey Browser",
        "process": "odyssey.exe",
        "path": "c:\\program files\\odyssey\\odyssey.exe",
      },
    ]

    cursor.executemany(
        """
        INSERT OR REPLACE INTO tracked_apps (
          id, platform, exe_path, normalized_exe_path, process_name, display_name,
          publisher, icon_ref, first_seen_at, last_seen_at
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        [
            (
                app["id"],
                "windows",
                app["path"],
                app["path"],
                app["process"],
                app["name"],
                "Kepler",
                None,
                "2026-04-01T07:00:00.000Z",
                "2026-04-16T18:00:00.000Z",
            )
            for app in apps
        ],
    )

    anchor = datetime(2026, 4, 16, 8, 0, 0, tzinfo=timezone.utc)
    sessions = []
    events = []

    for day in range(10):
        base = anchor - timedelta(days=day)
        base = base.replace(hour=8 + (day % 4))
        for index, app in enumerate(apps):
            started_at = base + timedelta(minutes=index * 90)
            foreground_ms = (45 + ((index * 22 + day * 7) % 120)) * 60 * 1000
            idle_ms = (5 + ((day * 3 + index * 4) % 18)) * 60 * 1000
            ended_at = started_at + timedelta(milliseconds=foreground_ms + idle_ms)
            session_id = f"{app['id']}-session-{day}"
            sessions.append(
                (
                    session_id,
                    app["id"],
                    "smoke-device",
                    "Smoke Workstation",
                    "windows",
                    started_at.isoformat().replace("+00:00", "Z"),
                    ended_at.isoformat().replace("+00:00", "Z"),
                    foreground_ms,
                    idle_ms,
                    f"{app['name']} Window {day + 1}",
                    app["process"],
                    app["path"],
                    1000 + day,
                    1000 + day,
                    json.dumps({"source": "smoke-seed", "day": day}),
                )
            )
            events.append(
                (
                    f"{session_id}-event",
                    app["id"],
                    session_id,
                    "smoke-device",
                    "Smoke Workstation",
                    "windows",
                    (started_at + timedelta(minutes=12)).isoformat().replace("+00:00", "Z"),
                    "window_changed",
                    f"{app['name']} Focus {day + 1}",
                    app["process"],
                    app["path"],
                    1000 + day,
                    1,
                    0,
                    json.dumps({"source": "smoke-seed", "highlight": True}),
                )
            )

    cursor.executemany(
        """
        INSERT OR REPLACE INTO usage_sessions (
          id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
          foreground_ms, idle_ms, window_title, process_name, exe_path, pid_start, pid_end, meta_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        sessions,
    )

    cursor.executemany(
        """
        INSERT OR REPLACE INTO usage_events (
          id, tracked_app_id, usage_session_id, device_id, device_name, platform, occurred_at,
          kind, window_title, process_name, exe_path, pid, is_foreground, is_idle, meta_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        """,
        events,
    )

    connection.commit()
    connection.close()
    print(f"Smoke dashboard DB seeded at {db_path}")


if __name__ == "__main__":
    main()
