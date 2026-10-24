#![allow(clippy::unwrap_used)]

use rusqlite::Connection;

fn setup_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA foreign_keys = ON;
         PRAGMA busy_timeout = 5000;",
    )
    .unwrap();
    crate::open::init_schema(&conn).unwrap();
    conn
}

#[test]
fn test_schema_creation_excludes_retired_entity_tables() {
    let conn = setup_db();
    let current_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                'tracked_apps','usage_sessions','usage_events','sync_kv','sync_tombstones',
                'object_types','objects','object_links'
            )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(current_count, 8);

    let retired_count: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN (
                'todos','projects','tags','areas','headings'
            )",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retired_count, 0);
}

#[test]
fn test_init_schema_adds_usage_runtime_ms_to_existing_sessions() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
            CREATE TABLE tracked_apps (
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
            CREATE TABLE usage_sessions (
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
            INSERT INTO tracked_apps
                (id, platform, exe_path, normalized_exe_path, process_name, display_name,
                first_seen_at, last_seen_at)
            VALUES
                ('app-legacy', 'windows', 'C:\\Games\\Legacy\\legacy.exe',
                'c:\\games\\legacy\\legacy.exe', 'legacy.exe', 'Legacy',
                '2026-01-01T00:00:00.000Z', '2026-01-01T01:00:00.000Z');
            INSERT INTO usage_sessions
                (id, tracked_app_id, device_id, device_name, platform, started_at, ended_at,
                foreground_ms, idle_ms, window_title, process_name, exe_path)
            VALUES
                ('session-legacy', 'app-legacy', 'device-1', 'Device', 'windows',
                '2026-01-01T00:00:00.000Z', '2026-01-01T01:00:00.000Z',
                2400000, 300000, 'Legacy', 'legacy.exe', 'C:\\Games\\Legacy\\legacy.exe');
            ",
    )
    .unwrap();

    crate::open::init_schema(&conn).unwrap();

    let runtime_ms: i64 = conn
        .query_row(
            "SELECT runtime_ms FROM usage_sessions WHERE id = 'session-legacy'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(runtime_ms, 2_700_000);
    let data = crate::load_all::load_all(&conn).unwrap();
    assert_eq!(data.usage_sessions[0].runtime_ms, 2_700_000);
}
