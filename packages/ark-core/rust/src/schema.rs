/// SQL statements for creating the database schema.
/// Identical to the current delphi-db sidecar schema.
pub const CREATE_TABLES: &str = "
CREATE TABLE IF NOT EXISTS todos (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    notes TEXT,
    priority INTEGER NOT NULL DEFAULT 0,
    scheduled_date TEXT,
    deadline TEXT,
    reminder_date TEXT,
    is_today INTEGER NOT NULL DEFAULT 0,
    is_evening INTEGER NOT NULL DEFAULT 0,
    is_someday INTEGER NOT NULL DEFAULT 0,
    is_completed INTEGER NOT NULL DEFAULT 0,
    completed_at TEXT,
    is_cancelled INTEGER NOT NULL DEFAULT 0,
    cancelled_at TEXT,
    is_trashed INTEGER NOT NULL DEFAULT 0,
    sort_order INTEGER NOT NULL DEFAULT 0,
    heading_id TEXT,
    project_id TEXT,
    area_id TEXT,
    tag_ids TEXT NOT NULL DEFAULT '[]',
    checklist_items TEXT NOT NULL DEFAULT '[]',
    recurrence_rule TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS projects (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    notes TEXT,
    status TEXT NOT NULL DEFAULT 'active',
    scheduled_date TEXT,
    deadline TEXT,
    sort_order INTEGER NOT NULL DEFAULT 0,
    color_tag TEXT,
    area_id TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS areas (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS tags (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    color TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS headings (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    sort_order INTEGER NOT NULL DEFAULT 0,
    project_id TEXT NOT NULL
);

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

CREATE TABLE IF NOT EXISTS sync_kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
";
