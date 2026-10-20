use std::path::Path;

use ark_domain::types::{ArkObject, ArkObjectWrite};
use rusqlite::{Connection, OptionalExtension};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LegacyRevisionSeed {
    VersionVector {
        hlc: String,
    },
    Tombstone {
        hlc: String,
        type_id: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TombstoneState {
    pub hlc: String,
    pub type_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectRevisionState {
    pub object: Option<ArkObject>,
    pub tombstone: Option<TombstoneState>,
    pub receipt_count: i64,
    pub link_count: i64,
    pub task_operation_count: i64,
    pub committed_feed_count: i64,
}

/// Owns SQLite setup and persistence inspection for Core revision-compatibility tests.
pub struct ObjectRevisionFixture {
    connection: Connection,
}

impl ObjectRevisionFixture {
    pub fn open(path: &Path) -> Result<Self, String> {
        let path = path
            .to_str()
            .ok_or_else(|| "object revision database path is not UTF-8".to_string())?;
        Ok(Self {
            connection: crate::open::open_db(path)?,
        })
    }

    pub fn seed_legacy_object(
        &self,
        object: ArkObjectWrite,
        revision: LegacyRevisionSeed,
    ) -> Result<ArkObject, String> {
        let prepared = crate::object_write::prepare_object(&self.connection, object)
            .map_err(|error| error.to_string())?;
        crate::objects::upsert_object_compat(&self.connection, &prepared)?;
        match revision {
            LegacyRevisionSeed::VersionVector { hlc } => {
                crate::sync_kv::set_sync_kv(
                    &self.connection,
                    crate::VERSION_VECTOR_KEY,
                    &serde_json::json!({ (prepared.id.clone()): hlc }).to_string(),
                )?;
            }
            LegacyRevisionSeed::Tombstone { hlc, type_id } => {
                crate::objects::delete_object(&self.connection, &prepared.id)?;
                crate::sync_tombstones::record(
                    &self.connection,
                    "object",
                    &prepared.id,
                    type_id.as_deref(),
                    &hlc,
                )?;
            }
        }
        Ok(prepared)
    }

    pub fn seed_object(&self, object: ArkObjectWrite) -> Result<ArkObject, String> {
        let prepared = crate::object_write::prepare_object(&self.connection, object)
            .map_err(|error| error.to_string())?;
        crate::objects::upsert_object_compat(&self.connection, &prepared)?;
        Ok(prepared)
    }

    pub fn prepare_object(&self, object: ArkObjectWrite) -> Result<ArkObject, String> {
        crate::object_write::prepare_object(&self.connection, object)
            .map_err(|error| error.to_string())
    }

    pub fn set_revision_hlc(&self, object_id: &str, hlc: &str) -> Result<(), String> {
        crate::sync_versions::set_object_sync_version_state(&self.connection, object_id, hlc, false)
    }

    pub fn inspect(&self, object_id: &str) -> Result<ObjectRevisionState, String> {
        let object = crate::objects::get_object(&self.connection, object_id)?;
        let tombstone = self
            .connection
            .query_row(
                "SELECT hlc, type_id FROM sync_tombstones
                 WHERE id = ?1 AND entity_type = 'object'",
                [object_id],
                |row| {
                    Ok(TombstoneState {
                        hlc: row.get(0)?,
                        type_id: row.get(1)?,
                    })
                },
            )
            .optional()
            .map_err(|error| error.to_string())?;
        Ok(ObjectRevisionState {
            object,
            tombstone,
            receipt_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM entity_write_receipts WHERE entity_id = ?1",
                object_id,
            )?,
            link_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM object_links WHERE source_object_id = ?1",
                object_id,
            )?,
            task_operation_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM task_merge_operations WHERE object_id = ?1",
                object_id,
            )?,
            committed_feed_count: count(
                &self.connection,
                "SELECT COUNT(*) FROM ark_committed_feed WHERE entity_id = ?1",
                object_id,
            )?,
        })
    }
}

fn count(connection: &Connection, sql: &str, object_id: &str) -> Result<i64, String> {
    connection
        .query_row(sql, [object_id], |row| row.get(0))
        .map_err(|error| error.to_string())
}
