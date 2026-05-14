/// SQL statements for creating the ARK core database schema.
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

CREATE TABLE IF NOT EXISTS object_types (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    schema_json TEXT NOT NULL,
    ui_schema_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    system_locked INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS objects (
    id TEXT PRIMARY KEY,
    type_id TEXT NOT NULL,
    title TEXT NOT NULL,
    content_json TEXT NOT NULL DEFAULT '{}',
    props_json TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    deleted_at TEXT,
    FOREIGN KEY(type_id) REFERENCES object_types(id)
);

CREATE TABLE IF NOT EXISTS object_links (
    id TEXT PRIMARY KEY,
    source_object_id TEXT NOT NULL,
    target_object_id TEXT NOT NULL,
    link_type TEXT NOT NULL,
    created_at TEXT NOT NULL,
    FOREIGN KEY(source_object_id) REFERENCES objects(id) ON DELETE CASCADE,
    FOREIGN KEY(target_object_id) REFERENCES objects(id) ON DELETE CASCADE,
    UNIQUE(source_object_id, target_object_id, link_type)
);

CREATE TABLE IF NOT EXISTS sync_kv (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS sync_tombstones (
    id TEXT PRIMARY KEY,
    entity_type TEXT NOT NULL,
    hlc TEXT NOT NULL,
    deleted_at TEXT NOT NULL
);

-- Phase 2: hold-and-replay для schema drift. При приёме SyncEntity типа \"object\"
-- с неизвестным type_id, payload сохраняется здесь; при появлении object_type
-- (через upsert_object_type) запускается replay → upsert_object + DELETE из pending.
CREATE TABLE IF NOT EXISTS sync_pending_objects (
    id TEXT PRIMARY KEY,
    payload TEXT NOT NULL,
    awaited_type_id TEXT NOT NULL,
    received_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sync_pending_awaited_type ON sync_pending_objects(awaited_type_id);
CREATE INDEX IF NOT EXISTS idx_objects_type_id ON objects(type_id);
CREATE INDEX IF NOT EXISTS idx_objects_updated_at ON objects(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_objects_deleted_at ON objects(deleted_at);
CREATE INDEX IF NOT EXISTS idx_object_links_source ON object_links(source_object_id);
CREATE INDEX IF NOT EXISTS idx_object_links_target ON object_links(target_object_id);
CREATE INDEX IF NOT EXISTS idx_sync_tombstones_hlc ON sync_tombstones(hlc);
";

pub const CREATE_OBJECT_SEARCH_FTS: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS object_search_fts USING fts5(
    object_id UNINDEXED,
    title,
    body,
    props,
    tokenize = 'unicode61'
);
";
