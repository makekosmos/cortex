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
    runtime_ms INTEGER NOT NULL DEFAULT 0,
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

CREATE TABLE IF NOT EXISTS usage_days (
    id TEXT PRIMARY KEY,
    device_id TEXT NOT NULL,
    day TEXT NOT NULL,
    payload_json TEXT NOT NULL DEFAULT '{\"a\":[],\"t\":[],\"s\":[]}',
    updated_at TEXT NOT NULL,
    UNIQUE(device_id, day)
);

CREATE INDEX IF NOT EXISTS idx_usage_days_day ON usage_days(day DESC);

CREATE TABLE IF NOT EXISTS usage_sync_versions (
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    hlc TEXT NOT NULL,
    PRIMARY KEY(entity_type, entity_id)
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS usage_sync_heads (
    device_id TEXT PRIMARY KEY,
    max_seq INTEGER NOT NULL
) WITHOUT ROWID;

CREATE TABLE IF NOT EXISTS usage_sync_log (
    device_id TEXT NOT NULL,
    seq INTEGER NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    hlc TEXT NOT NULL,
    deleted INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY(device_id, seq)
) WITHOUT ROWID;

CREATE INDEX IF NOT EXISTS idx_usage_sync_log_entity
ON usage_sync_log(entity_type, entity_id);

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
    type_version TEXT NOT NULL DEFAULT '0.0.0-legacy',
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
    deleted_at TEXT NOT NULL,
    type_id TEXT
);
CREATE INDEX IF NOT EXISTS idx_objects_type_id ON objects(type_id);
CREATE INDEX IF NOT EXISTS idx_objects_updated_at ON objects(updated_at DESC);
CREATE INDEX IF NOT EXISTS idx_objects_deleted_at ON objects(deleted_at);
CREATE INDEX IF NOT EXISTS idx_object_links_source ON object_links(source_object_id);
CREATE INDEX IF NOT EXISTS idx_object_links_target ON object_links(target_object_id);
CREATE INDEX IF NOT EXISTS idx_sync_tombstones_hlc ON sync_tombstones(hlc);

CREATE TABLE IF NOT EXISTS integration_configurations (
    integration_id TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    account_subject TEXT NOT NULL,
    public_scopes_json TEXT NOT NULL DEFAULT '[]',
    public_settings_json TEXT NOT NULL DEFAULT '{}',
    enabled INTEGER NOT NULL DEFAULT 1,
    sync_cursor TEXT,
    revision INTEGER NOT NULL DEFAULT 0,
    hlc TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_integration_configurations_provider
ON integration_configurations(provider, account_subject);

CREATE TABLE IF NOT EXISTS authorized_nodes (
    node_id TEXT PRIMARY KEY,
    key_fingerprint TEXT NOT NULL,
    signing_public_key TEXT NOT NULL,
    encryption_public_key TEXT NOT NULL,
    transport_public_key TEXT,
    grant_epoch INTEGER NOT NULL,
    status TEXT NOT NULL,
    authorized_at TEXT NOT NULL,
    revoked_at TEXT,
    revocation_epoch INTEGER,
    revision INTEGER NOT NULL DEFAULT 0,
    hlc TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_authorized_nodes_status
ON authorized_nodes(status, grant_epoch);

CREATE TABLE IF NOT EXISTS integration_node_grants (
    integration_id TEXT NOT NULL,
    node_id TEXT NOT NULL,
    node_encryption_key TEXT NOT NULL,
    grant_epoch INTEGER NOT NULL,
    status TEXT NOT NULL,
    authorized_at TEXT NOT NULL,
    revoked_at TEXT,
    revision INTEGER NOT NULL DEFAULT 0,
    hlc TEXT NOT NULL,
    PRIMARY KEY(integration_id, node_id)
);
CREATE INDEX IF NOT EXISTS idx_integration_node_grants_node
ON integration_node_grants(node_id, status, grant_epoch);

CREATE TABLE IF NOT EXISTS integration_credential_envelopes (
    envelope_id TEXT PRIMARY KEY,
    integration_id TEXT NOT NULL,
    recipient_node_id TEXT NOT NULL,
    grant_epoch INTEGER NOT NULL,
    credential_generation INTEGER NOT NULL,
    refresh_fencing_token INTEGER NOT NULL,
    key_id TEXT NOT NULL,
    algorithm TEXT NOT NULL,
    nonce TEXT NOT NULL,
    ciphertext TEXT NOT NULL,
    authenticated_metadata_json TEXT,
    issuer_node_id TEXT NOT NULL,
    issued_at TEXT NOT NULL,
    revision INTEGER NOT NULL DEFAULT 0,
    hlc TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_integration_envelopes_recipient
ON integration_credential_envelopes(integration_id, recipient_node_id, credential_generation);

CREATE TABLE IF NOT EXISTS integration_refresh_leases (
    integration_id TEXT PRIMARY KEY,
    credential_generation INTEGER NOT NULL,
    holder_node_id TEXT NOT NULL,
    fencing_token INTEGER NOT NULL,
    issued_at_ms INTEGER NOT NULL,
    expires_at_ms INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_integration_refresh_leases_holder
ON integration_refresh_leases(holder_node_id, expires_at_ms);

CREATE TABLE IF NOT EXISTS sync_signed_replay_reservations (
    space_id TEXT NOT NULL,
    origin_node_id TEXT NOT NULL,
    key_epoch INTEGER NOT NULL,
    message_id TEXT NOT NULL,
    reserved_at_ms INTEGER NOT NULL,
    PRIMARY KEY(space_id, origin_node_id, key_epoch, message_id)
);

CREATE INDEX IF NOT EXISTS idx_sync_signed_replay_origin
ON sync_signed_replay_reservations(origin_node_id, key_epoch);
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
