use crate::application::objects::{
    NewObjectReceipt, ObjectWriteSession, ReceiptOutcome, StoredObjectReceipt,
};
use crate::db;
use crate::types::{ArkObject, ObjectLink};
use ark_domain::object_write::ObjectWriteSnapshot;
use rusqlite::{params, Connection, OptionalExtension};

pub(crate) struct SqliteObjectWriteSession<'a> {
    conn: &'a Connection,
}

impl<'a> SqliteObjectWriteSession<'a> {
    pub(crate) fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }
}

impl ObjectWriteSession for SqliteObjectWriteSession<'_> {
    fn resolve_type_identity(
        &self,
        type_id: &str,
        requested_version: Option<&str>,
    ) -> Result<(String, String), String> {
        crate::type_registry::resolve_object_type_identity(self.conn, type_id, requested_version)
    }

    fn object_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        let identity = self
            .conn
            .query_row(
                "SELECT type_id, type_version FROM objects WHERE id = ?1",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()
            .map_err(|error| error.to_string())?;
        let object_revision = db::get_object_revision(self.conn, id)?;
        let task_revision = db::task_merge::object_snapshot_token(self.conn, id)
            .map_err(|error| error.to_string())?;
        Ok(match identity {
            Some((type_id, type_version)) => {
                let is_task = type_id == "core.task";
                ObjectWriteSnapshot {
                    exists: true,
                    type_id: Some(type_id),
                    type_version: Some(type_version),
                    revision: if is_task {
                        task_revision.or(object_revision)
                    } else {
                        object_revision
                    },
                }
            }
            None => ObjectWriteSnapshot {
                exists: false,
                type_id: None,
                type_version: None,
                revision: task_revision.or(object_revision),
            },
        })
    }

    fn current_revision(&self, id: &str) -> Result<Option<String>, String> {
        db::get_object_revision(self.conn, id)
    }

    fn is_peer_removed(&self, peer_id: &str) -> Result<bool, String> {
        let removed: Vec<String> = db::get_sync_kv(self.conn, "sync.removed_peers")?
            .map(|raw| serde_json::from_str(&raw).map_err(|error| error.to_string()))
            .transpose()?
            .unwrap_or_default();
        Ok(removed.iter().any(|removed_id| removed_id == peer_id))
    }

    fn persist_object(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String> {
        db::upsert_object(self.conn, object)?;
        for link in links {
            db::upsert_object_link(self.conn, link)?;
        }
        Ok(())
    }

    fn record_local_revision(&self, id: &str, device_id: Option<&str>) -> Result<String, String> {
        let device_id = device_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or("ark-core-rpc-local");
        let hlc = db::bump_sync_version_vector(self.conn, "object", id, device_id, false)?;
        self.conn
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 0
                 WHERE excluded.hlc > object_sync_versions.hlc",
                params![id, hlc],
            )
            .map_err(|error| error.to_string())?;
        db::delete_sync_tombstone(self.conn, id)?;
        Ok(hlc)
    }

    fn record_remote_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 0
                 WHERE excluded.hlc > object_sync_versions.hlc",
                params![id, source_hlc],
            )
            .map_err(|error| error.to_string())?;
        db::record_remote_sync_revision(self.conn, id, source_hlc)?;
        db::delete_sync_tombstone(self.conn, id)
    }

    fn load_receipt(
        &self,
        entity_id: &str,
        representation_kind: &str,
        source_hlc: &str,
    ) -> Result<Option<StoredObjectReceipt>, String> {
        self.conn
            .query_row(
                "SELECT change_seq, payload_sha256, payload_json, outcome, resulting_hlc
                 FROM entity_write_receipts
                 WHERE entity_kind = 'object' AND entity_id = ?1
                   AND representation_kind = ?2 AND source_hlc = ?3",
                params![entity_id, representation_kind, source_hlc],
                |row| {
                    let outcome: String = row.get(3)?;
                    Ok(StoredObjectReceipt {
                        change_seq: row.get(0)?,
                        payload_sha256: row.get(1)?,
                        payload_json: row.get(2)?,
                        outcome: match outcome.as_str() {
                            "applied" => ReceiptOutcome::Applied,
                            "stale" => ReceiptOutcome::Stale,
                            _ => return Err(rusqlite::Error::InvalidQuery),
                        },
                        resulting_hlc: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    fn insert_receipt(&self, receipt: NewObjectReceipt) -> Result<StoredObjectReceipt, String> {
        let outcome_text = match receipt.outcome {
            ReceiptOutcome::Applied => "applied",
            ReceiptOutcome::Stale => "stale",
        };
        self.conn
            .execute(
                "INSERT INTO entity_write_receipts(
                    entity_kind, entity_id, representation_kind, source_hlc,
                    source_origin_device_id, type_id, payload_sha256, payload_json,
                    outcome, resulting_hlc, authenticated_peer_id, committed_at
                 ) VALUES ('object', ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                           strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
                params![
                    receipt.entity_id,
                    receipt.representation_kind,
                    receipt.source_hlc,
                    receipt.source_origin_device_id,
                    receipt.type_id,
                    receipt.payload_sha256,
                    receipt.payload_json,
                    outcome_text,
                    receipt.resulting_hlc,
                    receipt.authenticated_peer_id,
                ],
            )
            .map_err(|error| error.to_string())?;
        self.load_receipt(
            &receipt.entity_id,
            &receipt.representation_kind,
            &receipt.source_hlc,
        )?
        .ok_or_else(|| "object_write_receipt_missing_after_insert".into())
    }
}

impl crate::application::task_mutation::TaskMutationSession for SqliteObjectWriteSession<'_> {
    fn ensure_legacy_task_write_allowed(&self, id: &str) -> Result<(), String> {
        let operation_enabled = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM task_merge_operations WHERE object_id = ?1)",
                [id],
                |row| row.get::<_, bool>(0),
            )
            .map_err(|error| error.to_string())?;
        if operation_enabled {
            return Err("task_merge_operation_mode_requires_upgrade".into());
        }
        Ok(())
    }

    fn persist_task(&self, object: &ArkObject, links: &[ObjectLink]) -> Result<(), String> {
        self.persist_object(object, links)
    }

    fn record_task_upsert(&self, id: &str, device_id: &str) -> Result<String, String> {
        self.record_local_revision(id, Some(device_id))
    }

    fn record_task_link_upsert(&self, id: &str, device_id: &str) -> Result<String, String> {
        db::bump_sync_version_vector(self.conn, "object_link", id, device_id, false)
    }

    fn task_snapshot(&self, id: &str) -> Result<ObjectWriteSnapshot, String> {
        self.object_snapshot(id)
    }

    fn task_type_id(&self, id: &str) -> Result<Option<String>, String> {
        db::get_object(self.conn, id).map(|object| object.map(|object| object.type_id))
    }

    fn legacy_task_revision(&self, id: &str) -> Result<Option<String>, String> {
        self.conn
            .query_row(
                "SELECT hlc FROM object_sync_versions WHERE object_id = ?1",
                [id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|error| error.to_string())
    }

    fn record_legacy_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 0
                 WHERE excluded.hlc > object_sync_versions.hlc",
                params![id, source_hlc],
            )
            .map_err(|error| error.to_string())?;
        db::delete_sync_tombstone(self.conn, id)
    }

    fn record_canonical_task_revision(&self, id: &str, source_hlc: &str) -> Result<(), String> {
        self.conn
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 0)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 0
                 WHERE excluded.hlc > object_sync_versions.hlc",
                params![id, source_hlc],
            )
            .map_err(|error| error.to_string())?;
        db::delete_sync_tombstone(self.conn, id)
    }

    fn hard_delete_remote_task(&self, entity: &crate::types::SyncEntity) -> Result<(), String> {
        db::delete_object(self.conn, &entity.id)?;
        self.conn
            .execute(
                "INSERT INTO object_sync_versions(object_id, hlc, deleted) VALUES (?1, ?2, 1)
                 ON CONFLICT(object_id) DO UPDATE SET hlc = excluded.hlc, deleted = 1
                 WHERE excluded.hlc > object_sync_versions.hlc",
                params![entity.id, entity.hlc],
            )
            .map_err(|error| error.to_string())?;
        db::upsert_sync_tombstone(self.conn, entity)
    }

    fn commit_local_task_upsert(
        &self,
        object: &ArkObject,
        links: &[ObjectLink],
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
    ) -> Result<Vec<crate::types::SyncEntity>, String> {
        crate::db::task_merge::commit_local_task_upsert(
            self.conn,
            object,
            links,
            device_id,
            expected_snapshot,
        )
    }

    fn commit_local_task_delete(
        &self,
        id: &str,
        device_id: &str,
        expected_snapshot: Option<&ObjectWriteSnapshot>,
        hard_delete: bool,
    ) -> Result<String, String> {
        crate::db::task_merge::commit_local_task_delete(
            self.conn,
            id,
            device_id,
            expected_snapshot,
            hard_delete,
        )
    }
}
