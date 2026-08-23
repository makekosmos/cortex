//! Additive ARK-owned state for external mappings and device-local sync policy.
use rusqlite::{params, Connection};

pub fn ensure_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS external_refs (
           connector_id TEXT NOT NULL, account_id TEXT NOT NULL,
           external_type TEXT NOT NULL, external_id TEXT NOT NULL,
           object_id TEXT NOT NULL REFERENCES objects(id) ON DELETE CASCADE,
           external_revision TEXT, external_url TEXT, content_hash TEXT,
           last_pulled_at TEXT, last_pushed_at TEXT,
           conflict_state TEXT NOT NULL DEFAULT 'clean'
             CHECK(conflict_state IN ('clean','local_changed','remote_changed','conflict','missing','invalid','disabled')),
           PRIMARY KEY(connector_id,account_id,external_type,external_id)
         );
         CREATE UNIQUE INDEX IF NOT EXISTS external_refs_object_identity
           ON external_refs(connector_id,account_id,object_id,external_type);
         CREATE INDEX IF NOT EXISTS external_refs_object ON external_refs(object_id);
         CREATE TABLE IF NOT EXISTS sync_profiles (
           profile_id TEXT PRIMARY KEY, device_id TEXT NOT NULL, revision INTEGER NOT NULL,
           name TEXT NOT NULL, active INTEGER NOT NULL DEFAULT 0,
           UNIQUE(device_id,name)
         );
         CREATE TABLE IF NOT EXISTS sync_profile_rules (
           profile_id TEXT NOT NULL REFERENCES sync_profiles(profile_id) ON DELETE CASCADE,
           resource_kind TEXT NOT NULL CHECK(resource_kind IN ('type','dataset','blob')),
           resource_id TEXT NOT NULL, mode TEXT NOT NULL CHECK(mode IN ('full','metadata','none')),
           filter_json TEXT NOT NULL DEFAULT '{}', PRIMARY KEY(profile_id,resource_kind,resource_id)
         );"
    ).map_err(|e| e.to_string())
}

pub fn upsert_external_ref(
    conn: &Connection,
    connector: &str,
    account: &str,
    external_type: &str,
    external_id: &str,
    object_id: &str,
    revision: Option<&str>,
    hash: Option<&str>,
    state: &str,
) -> Result<(), String> {
    if connector.is_empty()
        || account.is_empty()
        || external_type.is_empty()
        || external_id.is_empty()
        || object_id.is_empty()
    {
        return Err("invalid external ref".into());
    }
    conn.execute(
        "INSERT INTO external_refs(connector_id,account_id,external_type,external_id,object_id,external_revision,content_hash,conflict_state)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(connector_id,account_id,external_type,external_id) DO UPDATE SET object_id=excluded.object_id,external_revision=excluded.external_revision,content_hash=excluded.content_hash,conflict_state=excluded.conflict_state",
        params![connector,account,external_type,external_id,object_id,revision,hash,state],
    ).map_err(|e| e.to_string()).map(|_| ())
}
