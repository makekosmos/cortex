-- ============================================================================
-- ARK CORE: Minimal Event Storage Schema
-- Version: 2.0.0
--
-- A single-file SQLite database for storing all personal life events.
-- This is the CORE schema - no predefined categories or event types.
-- All semantics come from plugins.
--
-- Requirements:
--   - SQLite 3.35+ (for JSON functions)
--   - FTS5 extension (built-in)
-- ============================================================================

-- ============================================================================
-- PRAGMAS: Database Configuration
-- ============================================================================

PRAGMA journal_mode = WAL;
PRAGMA synchronous = FULL;
PRAGMA foreign_keys = ON;
PRAGMA auto_vacuum = INCREMENTAL;
PRAGMA temp_store = MEMORY;
PRAGMA mmap_size = 268435456;
PRAGMA page_size = 4096;

-- ============================================================================
-- TABLE: metadata
-- Database version, settings, ownership
-- ============================================================================

CREATE TABLE IF NOT EXISTS metadata (
    key         TEXT PRIMARY KEY,
    value       TEXT NOT NULL,
    updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

INSERT INTO metadata (key, value) VALUES
    ('schema_version', '2.0.0')
    ON CONFLICT(key) DO UPDATE SET
        value = excluded.value,
        updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now');

INSERT INTO metadata (key, value) VALUES
    ('created_at', strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
    ON CONFLICT(key) DO NOTHING;

INSERT INTO metadata (key, value) VALUES
    ('owner', 'anonymous')
    ON CONFLICT(key) DO NOTHING;

INSERT INTO metadata (key, value) VALUES
    ('description', 'Ark - Personal Life Database')
    ON CONFLICT(key) DO NOTHING;

-- ============================================================================
-- TABLE: events
-- The main table storing ALL life events
-- No foreign keys to categories/event_types - plugins define semantics
-- ============================================================================

CREATE TABLE IF NOT EXISTS events (
    -- Primary identification
    id              TEXT PRIMARY KEY,       -- UUID v4

    -- Classification (plugin-defined, no FK)
    event_type      TEXT NOT NULL,          -- e.g., 'heart_rate', 'video_watched'
    category        TEXT,                   -- e.g., 'health', 'media' (optional)

    -- Temporal
    occurred_at     TEXT NOT NULL,          -- ISO 8601, UTC
    duration_seconds INTEGER,
    timezone        TEXT,                   -- Original timezone

    -- Data
    data            TEXT NOT NULL DEFAULT '{}',  -- JSON payload
    summary         TEXT,                        -- Human-readable summary

    -- Source tracking
    source          TEXT,                   -- e.g., 'garmin', 'youtube', 'manual'
    source_id       TEXT,                   -- Original ID in source system
    device          TEXT,

    -- Metadata
    tags            TEXT DEFAULT '[]',      -- JSON array
    is_deleted      INTEGER NOT NULL DEFAULT 0,

    -- Sync
    hlc             TEXT,                   -- hybrid logical clock of last write

    -- Audit
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    -- Prevent duplicate imports
    UNIQUE(source, source_id)
);

-- Indexes
CREATE INDEX idx_events_occurred_at ON events(occurred_at);
CREATE INDEX idx_events_category ON events(category) WHERE category IS NOT NULL;
CREATE INDEX idx_events_event_type ON events(event_type);
CREATE INDEX idx_events_category_occurred ON events(category, occurred_at);
CREATE INDEX idx_events_type_occurred ON events(event_type, occurred_at);
CREATE INDEX idx_events_source ON events(source) WHERE source IS NOT NULL;
CREATE INDEX idx_events_created_at ON events(created_at);
CREATE INDEX idx_events_not_deleted ON events(is_deleted) WHERE is_deleted = 0;
CREATE INDEX idx_events_date ON events(substr(occurred_at, 1, 10));

-- Auto-update updated_at
CREATE TRIGGER trg_events_updated_at
AFTER UPDATE ON events
FOR EACH ROW
BEGIN
    UPDATE events SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    WHERE id = NEW.id;
END;

-- Validate ISO 8601 timestamp
CREATE TRIGGER trg_events_validate_timestamp
BEFORE INSERT ON events
FOR EACH ROW
WHEN NEW.occurred_at NOT GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9]*'
BEGIN
    SELECT RAISE(ABORT, 'occurred_at must be in ISO 8601 format (YYYY-MM-DDTHH:MM:SS)');
END;

-- ============================================================================
-- TABLE: entities
-- Reference entities (people, places, projects, etc.)
-- ============================================================================

CREATE TABLE IF NOT EXISTS entities (
    id              TEXT PRIMARY KEY,
    entity_type     TEXT NOT NULL,          -- 'person', 'place', 'project', etc.
    name            TEXT NOT NULL,
    aliases         TEXT DEFAULT '[]',      -- JSON array
    data            TEXT NOT NULL DEFAULT '{}',
    is_active       INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX idx_entities_type ON entities(entity_type);
CREATE INDEX idx_entities_name ON entities(name);
CREATE INDEX idx_entities_type_name ON entities(entity_type, name);

CREATE TRIGGER trg_entities_updated_at
AFTER UPDATE ON entities
FOR EACH ROW
BEGIN
    UPDATE entities SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
    WHERE id = NEW.id;
END;

-- ============================================================================
-- TABLE: event_entity_links
-- Many-to-many relationships between events and entities
-- ============================================================================

CREATE TABLE IF NOT EXISTS event_entity_links (
    id              TEXT PRIMARY KEY,
    event_id        TEXT NOT NULL REFERENCES events(id) ON DELETE CASCADE,
    entity_id       TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    role            TEXT,                   -- 'participant', 'location', 'subject', etc.
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    UNIQUE(event_id, entity_id, role)
);

CREATE INDEX idx_links_event ON event_entity_links(event_id);
CREATE INDEX idx_links_entity ON event_entity_links(entity_id);

-- ============================================================================
-- TABLE: embeddings
-- Vector embeddings for semantic search
-- ============================================================================

CREATE TABLE IF NOT EXISTS embeddings (
    id              TEXT PRIMARY KEY,
    event_id        TEXT REFERENCES events(id) ON DELETE CASCADE,
    entity_id       TEXT REFERENCES entities(id) ON DELETE CASCADE,
    content_hash    TEXT NOT NULL,
    model           TEXT NOT NULL,
    dimensions      INTEGER NOT NULL,
    embedding       BLOB NOT NULL,
    created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),

    CHECK ((event_id IS NOT NULL AND entity_id IS NULL) OR
           (event_id IS NULL AND entity_id IS NOT NULL))
);

CREATE INDEX idx_embeddings_event ON embeddings(event_id) WHERE event_id IS NOT NULL;
CREATE INDEX idx_embeddings_entity ON embeddings(entity_id) WHERE entity_id IS NOT NULL;
CREATE UNIQUE INDEX idx_embeddings_content ON embeddings(content_hash, model);

-- ============================================================================
-- TABLE: imports
-- Track import operations
-- ============================================================================

CREATE TABLE IF NOT EXISTS imports (
    id              TEXT PRIMARY KEY,
    source          TEXT NOT NULL,
    file_name       TEXT,
    file_hash       TEXT,
    records_total   INTEGER NOT NULL DEFAULT 0,
    records_created INTEGER NOT NULL DEFAULT 0,
    records_updated INTEGER NOT NULL DEFAULT 0,
    records_skipped INTEGER NOT NULL DEFAULT 0,
    date_range_start TEXT,
    date_range_end  TEXT,
    status          TEXT NOT NULL DEFAULT 'pending',
    error_message   TEXT,
    started_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    completed_at    TEXT
);

CREATE INDEX idx_imports_source ON imports(source);
CREATE INDEX idx_imports_status ON imports(status);
CREATE INDEX idx_imports_file_hash ON imports(file_hash);

-- ============================================================================
-- VIRTUAL TABLE: events_fts
-- Full-text search
-- ============================================================================

CREATE VIRTUAL TABLE IF NOT EXISTS events_fts USING fts5(
    id UNINDEXED,
    event_type,
    category,
    summary,
    data,
    tags,
    content='events',
    content_rowid='rowid',
    tokenize='unicode61 remove_diacritics 2'
);

-- FTS sync triggers
CREATE TRIGGER trg_events_fts_insert
AFTER INSERT ON events
BEGIN
    INSERT INTO events_fts(rowid, id, event_type, category, summary, data, tags)
    VALUES (NEW.rowid, NEW.id, NEW.event_type, NEW.category, NEW.summary, NEW.data, NEW.tags);
END;

CREATE TRIGGER trg_events_fts_update
AFTER UPDATE ON events
BEGIN
    INSERT INTO events_fts(events_fts, rowid, id, event_type, category, summary, data, tags)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.event_type, OLD.category, OLD.summary, OLD.data, OLD.tags);
    INSERT INTO events_fts(rowid, id, event_type, category, summary, data, tags)
    VALUES (NEW.rowid, NEW.id, NEW.event_type, NEW.category, NEW.summary, NEW.data, NEW.tags);
END;

CREATE TRIGGER trg_events_fts_delete
AFTER DELETE ON events
BEGIN
    INSERT INTO events_fts(events_fts, rowid, id, event_type, category, summary, data, tags)
    VALUES ('delete', OLD.rowid, OLD.id, OLD.event_type, OLD.category, OLD.summary, OLD.data, OLD.tags);
END;

-- ============================================================================
-- VIEW: v_recent_events
-- ============================================================================

CREATE VIEW IF NOT EXISTS v_recent_events AS
SELECT * FROM events
WHERE is_deleted = 0
ORDER BY occurred_at DESC
LIMIT 100;

-- ============================================================================
-- VIEW: v_events_by_day
-- ============================================================================

CREATE VIEW IF NOT EXISTS v_events_by_day AS
SELECT
    substr(occurred_at, 1, 10) as date,
    category,
    event_type,
    COUNT(*) as count
FROM events
WHERE is_deleted = 0
GROUP BY date, category, event_type
ORDER BY date DESC;

-- ============================================================================
-- SYNC TABLES: Multi-device synchronization
-- See SYNC.md for protocol details
-- ============================================================================

-- Registered devices
CREATE TABLE IF NOT EXISTS sync_devices (
    device_id   TEXT PRIMARY KEY,
    name        TEXT NOT NULL,              -- "MacBook", "Pixel 8", "Windows PC"
    platform    TEXT NOT NULL,              -- "macos", "android", "windows"
    last_seen_at TEXT,
    created_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Version vector: what each device has seen from others
CREATE TABLE IF NOT EXISTS sync_vectors (
    device_id   TEXT NOT NULL,              -- whose vector
    peer_id     TEXT NOT NULL,              -- seen from whom
    last_seq    INTEGER NOT NULL,           -- up to which seq
    PRIMARY KEY (device_id, peer_id)
);

-- Offline outbox queue
CREATE TABLE IF NOT EXISTS sync_outbox (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    event_id    TEXT NOT NULL,
    change_type TEXT NOT NULL,              -- 'create', 'update', 'delete'
    data        TEXT NOT NULL,              -- JSON of full event
    device_id   TEXT NOT NULL,
    device_seq  INTEGER NOT NULL,
    created_at  TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    synced      INTEGER NOT NULL DEFAULT 0, -- 0 = pending, 1 = synced
    origin_device TEXT,                     -- device that originally created the change
    origin_seq  INTEGER,                    -- original device_seq from origin
    hlc         TEXT,                       -- hybrid logical clock value
    hop_path    TEXT                        -- JSON array of device_ids that relayed this change
);

CREATE INDEX IF NOT EXISTS idx_sync_outbox_synced ON sync_outbox(synced) WHERE synced = 0;
CREATE INDEX IF NOT EXISTS idx_sync_outbox_device ON sync_outbox(device_id, device_seq);

-- Unresolved conflicts
CREATE TABLE IF NOT EXISTS sync_conflicts (
    id                  TEXT PRIMARY KEY,
    event_id            TEXT NOT NULL,
    local_data          TEXT NOT NULL,      -- JSON of local version
    remote_data         TEXT NOT NULL,      -- JSON of remote version
    local_device        TEXT NOT NULL,
    remote_device       TEXT NOT NULL,
    local_updated_at    TEXT NOT NULL,
    remote_updated_at   TEXT NOT NULL,
    resolved            INTEGER DEFAULT 0,  -- 0 = pending, 1 = resolved
    resolution          TEXT,               -- 'local', 'remote', 'manual'
    created_at          TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_sync_conflicts_unresolved ON sync_conflicts(resolved) WHERE resolved = 0;

-- ============================================================================
-- MIGRATION: Add sync columns to events table
-- SQLite doesn't support ADD COLUMN in schema scripts cleanly,
-- so we use a conditional approach via metadata tracking.
-- ============================================================================

-- Add device_id column (which device created/modified this event)
-- ALTER TABLE ADD COLUMN is safe in SQLite — it's a no-op if column exists (will error, caught by IF NOT EXISTS pattern)
-- We wrap in a trigger-safe way: just run ALTER and ignore if already present.

-- Note: SQLite ALTER TABLE ADD COLUMN doesn't support IF NOT EXISTS,
-- so we track migration state in metadata.
INSERT INTO metadata (key, value) VALUES ('migration_sync_columns', '1')
    ON CONFLICT(key) DO NOTHING;

-- These must be run manually or by the application on first connect.
-- The SyncManager handles adding these columns if missing.

-- ============================================================================
-- P2P SYNC TABLES: Mesh networking and peer discovery
-- ============================================================================

-- Mesh networks this device belongs to
CREATE TABLE IF NOT EXISTS sync_mesh (
    mesh_id         TEXT PRIMARY KEY,
    mesh_secret_hash TEXT NOT NULL,
    relay_url       TEXT,
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Known peers in the mesh
CREATE TABLE IF NOT EXISTS sync_peers (
    peer_id         TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    platform        TEXT NOT NULL,
    mesh_id         TEXT NOT NULL,
    last_connected  TEXT,
    connection_type TEXT,                    -- 'lan', 'relay', 'bluetooth'
    address         TEXT,                    -- last known address
    created_at      TEXT DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

-- Initialize statistics
ANALYZE;
