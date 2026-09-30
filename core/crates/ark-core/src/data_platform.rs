//! Additive ARK-owned state for external mappings and device-local sync policy.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

const MAX_FILTER_DEPTH: usize = 16;
const MAX_PREDICATES: usize = 128;
include!("data_platform_selective.rs");

include!("data_platform_filters.rs");

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
           profile_id TEXT PRIMARY KEY CHECK(length(CAST(profile_id AS BLOB)) BETWEEN 1 AND 128),
           device_id TEXT NOT NULL CHECK(length(CAST(device_id AS BLOB)) BETWEEN 1 AND 128),
           name TEXT NOT NULL CHECK(length(CAST(name AS BLOB)) BETWEEN 1 AND 128),
           active INTEGER NOT NULL DEFAULT 0 CHECK(active IN (0,1)),
           revision INTEGER NOT NULL CHECK(revision >= 1),
           UNIQUE(device_id,name), UNIQUE(device_id,revision)
         );
         CREATE UNIQUE INDEX IF NOT EXISTS sync_profiles_one_default ON sync_profiles(device_id) WHERE active = 1;
         CREATE TABLE IF NOT EXISTS sync_profile_rules (
           profile_id TEXT NOT NULL REFERENCES sync_profiles(profile_id) ON DELETE CASCADE,
           resource_kind TEXT NOT NULL CHECK(resource_kind IN ('type','dataset','blob')),
           resource_id TEXT NOT NULL CHECK(length(CAST(resource_id AS BLOB)) BETWEEN 1 AND 128),
           mode TEXT NOT NULL CHECK(mode IN ('full','metadata','none')),
           filter_json TEXT NOT NULL DEFAULT '{}' CHECK(length(CAST(filter_json AS BLOB)) <= 131072),
           PRIMARY KEY(profile_id,resource_kind,resource_id)
         );"
    ).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncProfileRule {
    pub resource_kind: String,
    pub resource_id: String,
    pub mode: SyncMode,
    pub filter_json: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncProfile {
    pub profile_id: String,
    pub device_id: String,
    pub revision: i64,
    pub name: String,
    pub active: bool,
    pub rules: Vec<SyncProfileRule>,
}

impl SyncProfile {
    pub fn selective_profile(&self) -> SelectiveSyncProfile {
        let mut profile = SelectiveSyncProfile::default();
        for rule in &self.rules {
            profile.set_rule(&rule.resource_kind, &rule.resource_id, rule.mode);
        }
        profile
    }
}

pub fn upsert_sync_profile(
    conn: &Connection,
    local_device_id: &str,
    profile_id: &str,
    name: &str,
    active: bool,
    expected_revision: Option<i64>,
    rules: &[SyncProfileRule],
) -> Result<SyncProfile, String> {
    if local_device_id.is_empty() || profile_id.is_empty() || name.is_empty() {
        return Err("invalid sync profile identity".into());
    }
    if rules.len() > 256 {
        return Err("too many sync profile rules".into());
    }
    let current: Option<(String, i64)> = conn
        .query_row(
            "SELECT device_id, revision FROM sync_profiles WHERE profile_id=?1",
            [profile_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let revision = match current {
        Some((owner, _revision)) if owner != local_device_id => {
            return Err("PROFILE_OWNER_DENIED".into())
        }
        Some((_, revision)) => {
            if expected_revision != Some(revision) {
                return Err("PROFILE_REVISION_STALE".into());
            }
            revision
                .checked_add(1)
                .ok_or_else(|| "profile revision overflow".to_string())?
        }
        None => {
            if expected_revision.is_some() {
                return Err("PROFILE_REVISION_STALE".into());
            }
            1
        }
    };
    for rule in rules {
        if !matches!(rule.resource_kind.as_str(), "type" | "dataset" | "blob")
            || rule.resource_id.is_empty()
        {
            return Err("invalid sync profile rule".into());
        }
        if !matches!(
            rule.mode,
            SyncMode::Full | SyncMode::Metadata | SyncMode::None
        ) {
            return Err("invalid sync profile mode".into());
        }
        validate_filter_json(&rule.filter_json)?;
    }
    conn.execute_batch("SAVEPOINT sync_profile_upsert")
        .map_err(|e| e.to_string())?;
    let result = (|| {
        conn.execute(
            "INSERT INTO sync_profiles(profile_id,device_id,revision,name,active) VALUES(?1,?2,?3,?4,?5)
             ON CONFLICT(profile_id) DO UPDATE SET revision=excluded.revision,name=excluded.name,active=excluded.active",
            params![profile_id, local_device_id, revision, name, active as i64],
        ).map_err(|e| e.to_string())?;
        conn.execute(
            "DELETE FROM sync_profile_rules WHERE profile_id=?1",
            [profile_id],
        )
        .map_err(|e| e.to_string())?;
        for rule in rules {
            conn.execute(
                "INSERT INTO sync_profile_rules(profile_id,resource_kind,resource_id,mode,filter_json) VALUES(?1,?2,?3,?4,?5)",
                params![profile_id, rule.resource_kind, rule.resource_id, sync_mode_name(rule.mode), validate_filter_json(&rule.filter_json)?],
            ).map_err(|e| e.to_string())?;
        }
        load_sync_profile(conn, local_device_id, profile_id)
    })();
    match result {
        Ok(profile) => {
            conn.execute_batch("RELEASE sync_profile_upsert")
                .map_err(|e| e.to_string())?;
            Ok(profile)
        }
        Err(error) => {
            let _ =
                conn.execute_batch("ROLLBACK TO sync_profile_upsert; RELEASE sync_profile_upsert");
            Err(error)
        }
    }
}

pub fn load_sync_profile(
    conn: &Connection,
    local_device_id: &str,
    profile_id: &str,
) -> Result<SyncProfile, String> {
    let (device_id, revision, name, active): (String, i64, String, i64) = conn
        .query_row(
            "SELECT device_id,revision,name,active FROM sync_profiles WHERE profile_id=?1",
            [profile_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .map_err(|e| e.to_string())?;
    if device_id != local_device_id {
        return Err("PROFILE_OWNER_DENIED".into());
    }
    let mut statement = conn.prepare("SELECT resource_kind,resource_id,mode,filter_json FROM sync_profile_rules WHERE profile_id=?1 ORDER BY resource_kind,resource_id").map_err(|e| e.to_string())?;
    let rules = statement
        .query_map([profile_id], |row| {
            Ok(SyncProfileRule {
                resource_kind: row.get(0)?,
                resource_id: row.get(1)?,
                mode: parse_sync_mode(&row.get::<_, String>(2)?)?,
                filter_json: row.get(3)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, rusqlite::Error>>()
        .map_err(|e| e.to_string())?;
    Ok(SyncProfile {
        profile_id: profile_id.into(),
        device_id,
        revision,
        name,
        active: active != 0,
        rules,
    })
}

fn sync_mode_name(mode: SyncMode) -> &'static str {
    match mode {
        SyncMode::Full => "full",
        SyncMode::Metadata => "metadata",
        SyncMode::None => "none",
    }
}

fn parse_sync_mode(mode: &str) -> rusqlite::Result<SyncMode> {
    match mode {
        "full" => Ok(SyncMode::Full),
        "metadata" => Ok(SyncMode::Metadata),
        "none" => Ok(SyncMode::None),
        _ => Err(rusqlite::Error::InvalidQuery),
    }
}

/// Params of `external_refs.upsert` — the `Request` newtype variant wraps this
/// struct so the wire fields and the db call share one shape.
#[derive(Debug, serde::Deserialize)]
pub struct ExternalRefUpsert {
    #[serde(rename = "connectorId")]
    pub connector_id: String,
    #[serde(rename = "accountId")]
    pub account_id: String,
    #[serde(rename = "externalType")]
    pub external_type: String,
    #[serde(rename = "externalId")]
    pub external_id: String,
    #[serde(rename = "objectId")]
    pub object_id: String,
    #[serde(default)]
    pub revision: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
    pub state: String,
}

pub fn upsert_external_ref(conn: &Connection, r: &ExternalRefUpsert) -> Result<(), String> {
    if r.connector_id.is_empty()
        || r.account_id.is_empty()
        || r.external_type.is_empty()
        || r.external_id.is_empty()
        || r.object_id.is_empty()
    {
        return Err("invalid external ref".into());
    }
    conn.execute(
        "INSERT INTO external_refs(connector_id,account_id,external_type,external_id,object_id,external_revision,content_hash,conflict_state)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8)
         ON CONFLICT(connector_id,account_id,external_type,external_id) DO UPDATE SET object_id=excluded.object_id,external_revision=excluded.external_revision,content_hash=excluded.content_hash,conflict_state=excluded.conflict_state",
        params![r.connector_id,r.account_id,r.external_type,r.external_id,r.object_id,r.revision,r.hash,r.state],
    ).map_err(|e| e.to_string()).map(|_| ())
}
